//! Drill-audio orchestration.
//!
//! Walks every weave lesson in a chapter, extracts its Italian spans,
//! and synthesizes an MP3 per unique normalized text. Output lives at
//! `site/chapters/<chapter>/audio/drills/<hash>.mp3`.
//!
//! The pipeline is deliberately idempotent:
//!
//! 1. Each span's text is canonicalized via [`weave::normalize_for_hash`]
//!    (strips surrounding punctuation, collapses whitespace).
//! 2. The canonical text is BLAKE3-hashed; the first 16 hex chars become
//!    the filename.
//! 3. If `<hash>.mp3` already exists on disk, we skip it.
//! 4. Otherwise, we pass the canonical text through Italian pre-processing
//!    (Roman-numeral expansion, parenthetical stripping) before sending
//!    to the Google Cloud TTS API, and write the returned bytes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use site_gen::config::ChapterConfig;

use crate::italian;
use crate::tts::{GoogleTts, TtsError};

/// Summary of a drill-generation run.
#[derive(Debug, Default)]
pub struct DrillReport {
    /// Count of spans encountered (before dedup).
    pub spans_seen: usize,
    /// Count of unique drills (after dedup by hash).
    pub unique_drills: usize,
    /// How many MP3s we actually synthesized this run.
    pub synthesized: usize,
    /// How many MP3s were already present on disk and reused.
    pub reused: usize,
}

/// One drill that still needs to be synthesized.
#[derive(Debug, Clone)]
pub struct DrillJob {
    /// Lesson slug that first introduced this drill (first occurrence wins).
    pub slug: String,
    /// Canonical Italian text (post-`normalize_for_hash`), used for voice
    /// selection and as the input to `normalize_for_tts`.
    pub canonical: String,
    /// BLAKE3-prefix filename stem (see [`drill_hash`]).
    pub hash: String,
    /// Absolute path where the MP3 will be written.
    pub out_path: PathBuf,
}

/// The result of walking a chapter's lessons without any TTS I/O: everything
/// needed to decide whether synthesis is worth doing, or to preview it.
///
/// This split lets callers run `drills --dry-run` without requiring a Google
/// Cloud key — handy in CI, for cost estimation, and for sanity-checking a
/// new lesson before committing to spend on TTS.
#[derive(Debug, Default)]
pub struct DrillPlan {
    /// Total Italian spans encountered across all weave lessons.
    pub spans_seen: usize,
    /// Distinct drill-eligible spans after dedup by hash.
    pub unique_drills: usize,
    /// Drills whose MP3 is already on disk and will be reused as-is.
    pub reused: usize,
    /// Drills that need synthesis — the missing MP3s.
    pub to_synthesize: Vec<DrillJob>,
}

impl DrillPlan {
    /// How many MP3s are missing and would be synthesized by [`execute_plan`].
    pub fn missing(&self) -> usize {
        self.to_synthesize.len()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DrillError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("chapter config parse error in {path}: {source}")]
    Config {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("TTS error: {0}")]
    Tts(#[from] TtsError),
}

impl DrillError {
    fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io { path: path.display().to_string(), source }
    }
    fn config(path: &Path, source: toml::de::Error) -> Self {
        Self::Config { path: path.display().to_string(), source }
    }
}

/// Walk a chapter's weave lessons and enumerate the drills that would be
/// synthesized, without touching the TTS API.
///
/// Used directly by `drills --dry-run` (no API key required) and indirectly
/// by [`generate_drills`], which simply plans and then executes.
///
/// `content_dir` is `content/<chapter>/` and `output_dir` is
/// `site/chapters/<chapter>/`. The `audio/drills/` subdirectory is created
/// eagerly so the same directory layout holds whether we synthesize or not.
pub fn plan_drills(
    content_dir: &Path,
    output_dir: &Path,
) -> Result<DrillPlan, DrillError> {
    let config_path = content_dir.join("chapter.toml");
    let config_str = std::fs::read_to_string(&config_path)
        .map_err(|e| DrillError::io(&config_path, e))?;
    let config: ChapterConfig =
        toml::from_str(&config_str).map_err(|e| DrillError::config(&config_path, e))?;

    let drills_dir = output_dir.join("audio").join("drills");
    std::fs::create_dir_all(&drills_dir)
        .map_err(|e| DrillError::io(&drills_dir, e))?;

    let mut plan = DrillPlan::default();
    let mut seen_hashes: HashSet<String> = HashSet::new();

    for section in &config.sections {
        for page in &section.pages {
            if page.page_type != "weave" {
                continue;
            }
            let lesson_path = content_dir.join(format!("{}.html", page.slug));
            let html = match std::fs::read_to_string(&lesson_path) {
                Ok(s) => s,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    eprintln!("  skip (missing): {}.html", page.slug);
                    continue;
                }
                Err(e) => return Err(DrillError::io(&lesson_path, e)),
            };

            let spans = weave::extract::extract_italian_spans(&html);
            for span in &spans {
                plan.spans_seen += 1;

                let canonical = weave::normalize_for_hash(&span.text);
                if canonical.is_empty() {
                    continue;
                }
                // Skip spans shorter than the drill threshold (function
                // words, single letters). See weave::MIN_DRILL_LENGTH.
                if !weave::is_drillable(&canonical) {
                    continue;
                }

                let hash = drill_hash(&canonical);
                if !seen_hashes.insert(hash.clone()) {
                    // Already accounted for within this chapter run.
                    continue;
                }
                plan.unique_drills += 1;

                let mp3_path = drills_dir.join(format!("{hash}.mp3"));
                if mp3_path.exists() {
                    plan.reused += 1;
                    continue;
                }

                plan.to_synthesize.push(DrillJob {
                    slug: page.slug.clone(),
                    canonical,
                    hash,
                    out_path: mp3_path,
                });
            }
        }
    }

    Ok(plan)
}

/// Synthesize every job in a plan. Returns the number of MP3s written.
pub async fn execute_plan(
    tts: &GoogleTts,
    plan: &DrillPlan,
) -> Result<usize, DrillError> {
    for job in &plan.to_synthesize {
        synthesize_one(tts, &job.canonical, &job.out_path).await?;
        println!(
            "  [{}] {}.mp3 — {}",
            job.slug,
            job.hash,
            preview(&job.canonical, 60),
        );
    }
    Ok(plan.to_synthesize.len())
}

/// Generate all missing drill MP3s for one chapter.
///
/// `content_dir` is `content/<chapter>/` and `output_dir` is
/// `site/chapters/<chapter>/` — the MP3s land in
/// `<output_dir>/audio/drills/`.
pub async fn generate_drills(
    tts: &GoogleTts,
    content_dir: &Path,
    output_dir: &Path,
) -> Result<DrillReport, DrillError> {
    let plan = plan_drills(content_dir, output_dir)?;
    let synthesized = execute_plan(tts, &plan).await?;
    Ok(DrillReport {
        spans_seen: plan.spans_seen,
        unique_drills: plan.unique_drills,
        synthesized,
        reused: plan.reused,
    })
}

/// Compute a drill's MP3 filename stem (BLAKE3 first 16 hex chars).
pub fn drill_hash(canonical_text: &str) -> String {
    let hash = blake3::hash(canonical_text.as_bytes());
    hash.to_hex()[..16].to_string()
}

/// Build the `audio/drills/<hash>.mp3` URL for a span's canonical text.
///
/// The returned path is relative to a chapter's built HTML page.
pub fn drill_url(canonical_text: &str) -> String {
    format!("audio/drills/{}.mp3", drill_hash(canonical_text))
}

async fn synthesize_one(
    tts: &GoogleTts,
    canonical_text: &str,
    out_path: &Path,
) -> Result<(), DrillError> {
    // Canonical form → TTS-ready form. Two stages:
    //   1. Strip parenthetical Roman-numeral reader aids: the author
    //      writes `il quindicesimo (XV) secolo` or `Nel diciannovesimo
    //      (XIX) secolo` so the learner sees both the word form and
    //      the numeral, but the voice should only speak the word form.
    //   2. Expand any remaining standalone Roman numerals
    //      (`il XV secolo` → `il quindicesimo secolo`) to Italian
    //      ordinals.
    let stripped = weave::normalize_for_tts(canonical_text);
    let tts_text = italian::normalize_roman_numerals(&stripped);

    // Pick the voice deterministically from the canonical text (NOT the
    // post-expansion text) so the mapping stays stable if we later
    // change the expansion rules.
    let voice = italian::voice_for_text(canonical_text);

    tts.synthesize_to_file(&tts_text, voice, out_path).await?;
    Ok(())
}

/// Truncate `s` to at most `max` Unicode scalar values, appending a single
/// ellipsis if the string was shortened. Used for log lines that should not
/// wrap the terminal when a drill's canonical text is long.
pub fn preview(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{truncated}…")
    }
}

/// Discover all chapter directories under a content root that contain
/// a `chapter.toml`.
pub fn discover_chapters(content_root: &Path) -> Result<Vec<String>, DrillError> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(content_root)
        .map_err(|e| DrillError::io(content_root, e))?
    {
        let entry = entry.map_err(|e| DrillError::io(content_root, e))?;
        if entry.path().join("chapter.toml").exists() {
            if let Some(n) = entry.file_name().to_str() {
                names.push(n.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Resolve a chapter name or list. If `filter` is `Some(name)`, returns
/// just that name; otherwise discovers all chapters under `content_root`.
pub fn select_chapters(
    content_root: &Path,
    filter: Option<&str>,
) -> Result<Vec<String>, DrillError> {
    match filter {
        Some(n) => Ok(vec![n.to_string()]),
        None => discover_chapters(content_root),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drill_hash_is_stable() {
        let a = drill_hash("la storia");
        let b = drill_hash("la storia");
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn drill_hash_differs_across_texts() {
        let a = drill_hash("la storia");
        let b = drill_hash("la lingua");
        assert_ne!(a, b);
    }

    /// Every `data-audio` attribute emitted by the build step derives
    /// from `drill_hash`. If this mapping ever changes — new hash
    /// algorithm, new normalization pass, different prefix length —
    /// every committed lesson's drill URLs silently go stale and the
    /// audio files on disk become orphans. Pin the text→hash mapping
    /// for a representative sample so a regression shows up as a test
    /// failure rather than as a batch of broken clickable spans.
    #[test]
    fn drill_hash_matches_committed_lesson_audio() {
        // These pairs are the hashes baked into
        // site/chapters/00-pronuncia/001-vocali.html for the minimal-pair
        // row, pulled directly from the rendered HTML.
        for (text, expected) in [
            ("mano", "a81f61c2acdbbef4"),
            ("meno", "2750b2a7da516330"),
            ("minore", "d7dc0728ab12bf38"),
            ("mondo", "f112bccda78d253e"),
            ("muto", "af1d0e59853abcff"),
        ] {
            assert_eq!(
                drill_hash(text),
                expected,
                "drill_hash({text:?}) produced a different hash than the one \
                 baked into committed lesson HTML — every data-audio attribute \
                 in the rendered site now points at the wrong MP3",
            );
        }
    }

    #[test]
    fn drill_url_matches_hash() {
        let url = drill_url("la storia");
        let expected = format!("audio/drills/{}.mp3", drill_hash("la storia"));
        assert_eq!(url, expected);
    }

    #[test]
    fn preview_truncates_with_ellipsis() {
        assert_eq!(preview("short", 60), "short");
        assert_eq!(preview(&"x".repeat(100), 10), format!("{}…", "x".repeat(10)));
    }

    /// A fixture that stands up a minimal chapter on disk and returns its
    /// (content_dir, output_dir) paths. Used by the plan_drills tests below
    /// so each test owns an isolated tempdir.
    fn make_fixture_chapter() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("content").join("00-test");
        let output = dir.path().join("site").join("chapters").join("00-test");
        std::fs::create_dir_all(&content).unwrap();

        std::fs::write(
            content.join("chapter.toml"),
            r#"[chapter]
title = "Test chapter"

[[sections]]
heading = "Lessons"

[[sections.pages]]
slug = "001-intro"
title = "Intro"
description = "d"
type = "weave"
"#,
        )
        .unwrap();

        // Two drillable Italian spans ("la storia", "la lingua"),
        // plus a too-short one ("a") that should be filtered by
        // MIN_DRILL_LENGTH, plus a duplicate to exercise dedup.
        std::fs::write(
            content.join("001-intro.html"),
            r#"<p><span lang="it">la storia</span> è una cosa —
               <span lang="it">la lingua</span> è un'altra.
               <span lang="it">a</span> non si deve drillare.
               <span lang="it">la storia</span> di nuovo.</p>"#,
        )
        .unwrap();

        (dir, content, output)
    }

    #[test]
    fn plan_drills_reports_unique_eligible_spans() {
        let (_guard, content, output) = make_fixture_chapter();
        let plan = plan_drills(&content, &output).unwrap();

        // 4 spans total, but "a" is under MIN_DRILL_LENGTH and "la storia"
        // appears twice, so only 2 unique drill jobs survive.
        assert_eq!(plan.spans_seen, 4);
        assert_eq!(plan.unique_drills, 2);
        assert_eq!(plan.reused, 0);
        assert_eq!(plan.missing(), 2);

        let canonicals: Vec<&str> = plan
            .to_synthesize
            .iter()
            .map(|j| j.canonical.as_str())
            .collect();
        assert!(canonicals.contains(&"la storia"));
        assert!(canonicals.contains(&"la lingua"));
    }

    #[test]
    fn plan_drills_counts_existing_mp3s_as_reused() {
        let (_guard, content, output) = make_fixture_chapter();

        // Pre-seed one of the two expected MP3s on disk so plan_drills
        // classifies it as "reused" instead of "to_synthesize".
        let drills_dir = output.join("audio").join("drills");
        std::fs::create_dir_all(&drills_dir).unwrap();
        let hash = drill_hash("la storia");
        std::fs::write(drills_dir.join(format!("{hash}.mp3")), b"fake").unwrap();

        let plan = plan_drills(&content, &output).unwrap();
        assert_eq!(plan.unique_drills, 2);
        assert_eq!(plan.reused, 1);
        assert_eq!(plan.missing(), 1);
        assert_eq!(plan.to_synthesize[0].canonical, "la lingua");
    }

    #[test]
    fn plan_drills_creates_drills_dir_even_with_nothing_to_do() {
        let (_guard, content, output) = make_fixture_chapter();
        let drills_dir = output.join("audio").join("drills");
        assert!(!drills_dir.exists());

        plan_drills(&content, &output).unwrap();
        assert!(drills_dir.is_dir(), "plan_drills should mkdir -p the drills dir");
    }

    #[test]
    fn plan_drills_fails_loudly_on_missing_chapter_toml() {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("nonexistent");
        let output = dir.path().join("out");
        let err = plan_drills(&content, &output).unwrap_err();
        assert!(matches!(err, DrillError::Io { .. }));
    }

    #[test]
    fn discover_chapters_finds_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("00-pronuncia")).unwrap();
        std::fs::write(
            dir.path().join("00-pronuncia").join("chapter.toml"),
            "[chapter]\ntitle=\"x\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("20-roma")).unwrap();
        std::fs::write(
            dir.path().join("20-roma").join("chapter.toml"),
            "[chapter]\ntitle=\"y\"\n",
        )
        .unwrap();
        // A directory without chapter.toml should be skipped.
        std::fs::create_dir_all(dir.path().join("notes")).unwrap();

        let chapters = discover_chapters(dir.path()).unwrap();
        assert_eq!(chapters, vec!["00-pronuncia", "20-roma"]);
    }
}
