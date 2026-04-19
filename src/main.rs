//! Intreccio — CLI dispatcher.
//!
//! Invocation shape mirrors francais-rouille: `intreccio <subcommand>
//! [args]`. Commands that don't need TTS run synchronously; `drills`
//! and `file` use the async TTS client.

mod drills;
mod italian;
mod serve;
mod tts;

use std::collections::HashSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::drills::{drill_url, generate_drills};
use crate::tts::GoogleTts;

type CmdResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> CmdResult {
    let args: Vec<String> = std::env::args().collect();
    let prog = args.first().cloned().unwrap_or_else(|| "intreccio".into());

    let sub = match args.get(1).map(String::as_str) {
        None => {
            print_usage(&prog);
            std::process::exit(1);
        }
        Some(s) => s,
    };

    match sub {
        "--help" | "-h" | "help" => {
            print_help(&prog);
            Ok(())
        }
        "--version" | "-V" | "version" => {
            println!("intreccio {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "build" => cmd_build(&args),
        "drills" => cmd_drills(&args).await,
        "serve" => cmd_serve(&args),
        "audit-shares" => cmd_audit_shares(&args),
        "extract-vocab" => cmd_extract_vocab(&args),
        "prune-vocab" => cmd_prune_vocab(&args),
        "vocab-page" => cmd_vocab_page(&args),
        "verify-language" => cmd_verify_language(&args),
        "check-csp" => cmd_check_csp(&args),
        "strip-metadata" => cmd_strip_metadata(&args),
        "file" => cmd_file(&args).await,
        other => {
            eprintln!("unknown command: {other}\n");
            print_usage(&prog);
            std::process::exit(1);
        }
    }
}

fn print_usage(prog: &str) {
    eprintln!("Usage:");
    eprintln!("  {prog} build          [<chapter>] [--output DIR] [--site-url URL]");
    eprintln!("  {prog} drills         <chapter>");
    eprintln!("  {prog} serve          [--port N] [--site DIR]");
    eprintln!("  {prog} audit-shares   [<chapter>] [--tolerance N] [--quick-audit <file.html>]");
    eprintln!("  {prog} extract-vocab  [<chapter>]");
    eprintln!("  {prog} prune-vocab    <chapter>");
    eprintln!("  {prog} vocab-page     <chapter>");
    eprintln!("  {prog} verify-language [<chapter>] [--fix]");
    eprintln!("  {prog} check-csp      [--site DIR]");
    eprintln!("  {prog} strip-metadata <path> [--output DIR] [--keep-icc]");
    eprintln!("  {prog} file           <input.txt> <output.mp3> [--voice NAME]");
    eprintln!();
    eprintln!("Run `{prog} --help` for details.");
}

fn print_help(prog: &str) {
    println!("intreccio {} — triglot Italian build tool", env!("CARGO_PKG_VERSION"));
    println!();
    println!("COMMANDS:");
    println!("  build          Render HTML from content/ into site/. Injects");
    println!("                 data-audio attributes on Italian spans pointing");
    println!("                 at audio/drills/<hash>.mp3 files. Does NOT");
    println!("                 synthesize audio — run `{prog} drills` for that.");
    println!();
    println!("  drills         Synthesize missing drill MP3s for a chapter.");
    println!("                 Requires GOOGLE_TTS_API_KEY in the environment.");
    println!("                 Idempotent: existing files are reused.");
    println!();
    println!("  serve          Serve site/ on http://127.0.0.1:<port> (default");
    println!("                 port 8000). A minimal static-file server for");
    println!("                 local preview; the production CSP lives in");
    println!("                 site/_headers and is enforced by Cloudflare Pages.");
    println!();
    println!("  audit-shares   Count words per language in each weave lesson");
    println!("                 and compare against the level's share range");
    println!("                 (DESIGN.md §4) and any declared shares in");
    println!("                 <slug>.meta.json.");
    println!();
    println!("                 --tolerance N    Widen each range by N percentage");
    println!("                                  points on each side before the");
    println!("                                  pass/fail check. Warnings still");
    println!("                                  show the tight §4 range.");
    println!("                 --quick-audit F  Audit a single HTML file without");
    println!("                                  walking a chapter. Reads a sibling");
    println!("                                  <stem>.meta.json for level if present.");
    println!("                                  Fast iteration while authoring.");
    println!();
    println!("  extract-vocab  Emit a skeleton <slug>.vocab.json for each weave");
    println!("                 lesson from its <span lang=\"it\"> items. If the");
    println!("                 vocab file already exists, new Italian items are");
    println!("                 appended with empty gloss fields; existing items");
    println!("                 are preserved verbatim.");
    println!();
    println!("  vocab-page     Aggregate a chapter's vocab.json files into a");
    println!("                 single vocabolario.html, grouped by lesson slug,");
    println!("                 with drill audio wired on each Italian term using");
    println!("                 the existing <hash>.mp3 files. Deduplicates terms");
    println!("                 across lessons, and collapses bare-vs-article-");
    println!("                 bearing pairs of the same headword (e.g. `pasta`");
    println!("                 and `la pasta`) into a single article-bearing");
    println!("                 entry so each noun appears once.");
    println!();
    println!("  prune-vocab    Remove entries from each lesson's vocab.json whose");
    println!("                 Italian text no longer appears as a <span lang=\"it\">");
    println!("                 in any current lesson HTML for that chapter. Run");
    println!("                 after revising lesson content to clean out stale");
    println!("                 entries; glosses on surviving items are preserved.");
    println!();
    println!("  verify-language Check or fix Italian typographic rules");
    println!("                 (apostrophes, ellipsis) in text content files.");
    println!("                 Pass --fix to auto-correct.");
    println!();
    println!("  check-csp      Scan built HTML for Content Security Policy");
    println!("                 violations (inline scripts/styles/handlers,");
    println!("                 external resources, form elements).");
    println!();
    println!("  strip-metadata Strip EXIF/XMP/IPTC/comment metadata from JPEG");
    println!("                 and PNG images, preserving orientation and");
    println!("                 optionally ICC profiles.");
    println!();
    println!("  file           Synthesize a single text file to an MP3 using");
    println!("                 one Italian voice (default: Chirp3-HD-Aoede).");
    println!();
    println!("ENVIRONMENT:");
    println!("  GOOGLE_TTS_API_KEY  Required by `drills` and `file`.");
    println!();
    println!("See DESIGN.md §9 for the pedagogical and structural context.");
}

// ── build ───────────────────────────────────────────────────────────────

fn cmd_build(args: &[String]) -> CmdResult {
    let mut site_url: Option<String> = None;
    let mut chapter_filter: Option<String> = None;
    let mut output_override: Option<String> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--site-url" => {
                i += 1;
                site_url = Some(args.get(i).ok_or("--site-url requires a value")?.clone());
            }
            "--output" | "-o" => {
                i += 1;
                output_override = Some(
                    args.get(i).ok_or("--output requires a value")?.clone(),
                );
            }
            other if !other.starts_with('-') && chapter_filter.is_none() => {
                chapter_filter = Some(other.to_string());
            }
            other => return Err(format!("unknown flag: {other}").into()),
        }
        i += 1;
    }

    let content_root = PathBuf::from("content");
    let site_dir = output_override
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("site"));
    let output_root = site_dir.join("chapters");
    let templates_dir = PathBuf::from("templates");

    let chapters = drills::select_chapters(&content_root, chapter_filter.as_deref())?;
    if chapters.is_empty() {
        eprintln!("no chapters found in {}", content_root.display());
        std::process::exit(1);
    }

    // The drill-audio injector: for each <span lang="it">, set data-audio
    // to its content-addressed MP3 URL. Build is idempotent and API-free;
    // missing MP3s are synthesized separately by `drills`. Spans shorter
    // than weave::MIN_DRILL_LENGTH don't get data-audio — function words
    // like "e" / "il" / "da" don't warrant drills.
    let inject_drills = |html: &str| -> String {
        weave::transform::inject_drill_audio(html, |span_text| {
            let canonical = weave::normalize_for_hash(span_text);
            if !weave::is_drillable(&canonical) {
                return None;
            }
            Some(drill_url(&canonical))
        })
    };

    for chapter in &chapters {
        let content_dir = content_root.join(chapter);
        let output_dir = output_root.join(chapter);
        let base_url = site_url
            .as_deref()
            .map(|u| format!("{}/chapters/{}", u.trim_end_matches('/'), chapter));
        println!("Building chapter: {chapter}");
        site_gen::build::build_chapter(
            &content_dir,
            &output_dir,
            &templates_dir,
            base_url.as_deref(),
            &inject_drills,
        )?;
    }

    // Render the site index only for a full build.
    if chapter_filter.is_none() {
        let site_config = content_root.join("site.toml");
        if site_config.exists() {
            site_gen::build::generate_site_index(&site_config, &templates_dir, &site_dir)?;
        }
    }

    if let Some(url) = &site_url {
        site_gen::build::generate_sitemap(&site_dir, url)?;
    }

    println!("\nDone.");
    Ok(())
}

// ── drills ──────────────────────────────────────────────────────────────

async fn cmd_drills(args: &[String]) -> CmdResult {
    let chapter = args
        .get(2)
        .cloned()
        .ok_or("drills requires a chapter name")?;

    let content_root = PathBuf::from("content");
    let content_dir = content_root.join(&chapter);
    if !content_dir.is_dir() {
        return Err(format!("no such chapter: {}", content_dir.display()).into());
    }
    let output_dir = PathBuf::from("site").join("chapters").join(&chapter);

    let tts = GoogleTts::from_env()?;

    println!("Generating drills for chapter: {chapter}");
    let report = generate_drills(&tts, &content_dir, &output_dir).await?;

    println!();
    println!("  spans scanned:   {}", report.spans_seen);
    println!("  unique drills:   {}", report.unique_drills);
    println!("  synthesized:     {}", report.synthesized);
    println!("  reused existing: {}", report.reused);

    Ok(())
}

// ── serve ───────────────────────────────────────────────────────────────

fn cmd_serve(args: &[String]) -> CmdResult {
    let mut port: u16 = 8000;
    let mut site_dir = PathBuf::from("site");

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--port" => {
                i += 1;
                port = args
                    .get(i)
                    .ok_or("--port requires a value")?
                    .parse()
                    .map_err(|e| format!("invalid port: {e}"))?;
            }
            "--site" => {
                i += 1;
                site_dir = PathBuf::from(args.get(i).ok_or("--site requires a value")?);
            }
            other => return Err(format!("unknown flag: {other}").into()),
        }
        i += 1;
    }

    if !site_dir.is_dir() {
        return Err(format!("site directory not found: {}", site_dir.display()).into());
    }

    serve::serve(&site_dir, port)?;
    Ok(())
}

// ── audit-shares ────────────────────────────────────────────────────────

/// Declared shares from a lesson's `<slug>.meta.json` file.
#[derive(Debug, Deserialize)]
struct LessonMeta {
    #[serde(default)]
    level: Option<String>,
    #[serde(default)]
    language_shares: Option<LanguageShares>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
struct LanguageShares {
    #[serde(default)]
    en: f64,
    #[serde(default)]
    fr: f64,
    #[serde(default)]
    it: f64,
}

/// Expected share range for a CEFR level, from DESIGN.md §4.
#[derive(Debug, Clone, Copy)]
struct LevelRange {
    it: (f64, f64),
    fr: (f64, f64),
    en: (f64, f64),
}

fn range_for_level(level: &str) -> Option<LevelRange> {
    match level {
        "A2" => Some(LevelRange {
            it: (0.30, 0.40),
            fr: (0.30, 0.40),
            en: (0.25, 0.35),
        }),
        "A2-B1" => Some(LevelRange {
            it: (0.45, 0.55),
            fr: (0.25, 0.35),
            en: (0.15, 0.25),
        }),
        "B1" => Some(LevelRange {
            it: (0.60, 0.70),
            fr: (0.15, 0.25),
            en: (0.10, 0.20),
        }),
        _ => None,
    }
}

/// Widen each (lo, hi) range by `tolerance_pp` percentage points on each
/// side, clamping to [0.0, 1.0].
fn widen_range(range: LevelRange, tolerance_pp: f64) -> LevelRange {
    let t = (tolerance_pp / 100.0).max(0.0);
    let widen = |(lo, hi): (f64, f64)| ((lo - t).max(0.0), (hi + t).min(1.0));
    LevelRange {
        it: widen(range.it),
        fr: widen(range.fr),
        en: widen(range.en),
    }
}

fn cmd_audit_shares(args: &[String]) -> CmdResult {
    let mut chapter_filter: Option<String> = None;
    let mut quick_audit_file: Option<String> = None;
    let mut tolerance_pp: f64 = 0.0;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--quick-audit" => {
                i += 1;
                quick_audit_file = Some(
                    args.get(i)
                        .ok_or("--quick-audit requires a path to an HTML file")?
                        .clone(),
                );
            }
            "--tolerance" => {
                i += 1;
                let raw = args
                    .get(i)
                    .ok_or("--tolerance requires a percentage-point value")?;
                tolerance_pp = raw
                    .parse::<f64>()
                    .map_err(|e| format!("invalid tolerance '{raw}': {e}"))?;
                if tolerance_pp < 0.0 {
                    return Err(format!("--tolerance must be non-negative, got {tolerance_pp}").into());
                }
            }
            other if !other.starts_with('-') && chapter_filter.is_none() => {
                chapter_filter = Some(other.to_string());
            }
            other => return Err(format!("unknown flag: {other}").into()),
        }
        i += 1;
    }

    if let Some(path) = quick_audit_file {
        if chapter_filter.is_some() {
            return Err("--quick-audit cannot be combined with a chapter argument".into());
        }
        return run_quick_audit(Path::new(&path), tolerance_pp);
    }

    let content_root = PathBuf::from("content");
    let chapters = drills::select_chapters(&content_root, chapter_filter.as_deref())?;
    if chapters.is_empty() {
        eprintln!("no chapters found in {}", content_root.display());
        std::process::exit(1);
    }

    if tolerance_pp > 0.0 {
        println!("(tolerance: ±{tolerance_pp}% on §4 ranges)");
    }

    let mut any_out_of_range = false;
    for chapter in &chapters {
        audit_chapter(
            &content_root.join(chapter),
            tolerance_pp,
            &mut any_out_of_range,
        )?;
    }

    if any_out_of_range {
        std::process::exit(1);
    }
    Ok(())
}

/// Audit a single HTML lesson file without walking a chapter config.
///
/// Reads sibling `<slug>.meta.json` if present to pick up the level
/// declaration; falls back to `unknown` if missing (in which case no
/// §4 range check is performed, but measured shares are still printed).
fn run_quick_audit(path: &Path, tolerance_pp: f64) -> CmdResult {
    let html = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let shares = weave::count_words_by_lang(&html);

    // Derive a sibling meta.json path: `<stem>.meta.json` in the same dir.
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("cannot derive slug from file path")?;
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let meta_path = dir.join(format!("{stem}.meta.json"));
    let meta: Option<LessonMeta> = std::fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let level = meta
        .as_ref()
        .and_then(|m| m.level.clone())
        .unwrap_or_else(|| "unknown".into());

    if tolerance_pp > 0.0 {
        println!("(tolerance: ±{tolerance_pp}% on §4 ranges)");
    }

    let mut any_out_of_range = false;
    print_lesson_audit(
        stem,
        &level,
        &shares,
        meta.as_ref(),
        tolerance_pp,
        &mut any_out_of_range,
    );

    if any_out_of_range {
        std::process::exit(1);
    }
    Ok(())
}

fn audit_chapter(
    content_dir: &Path,
    tolerance_pp: f64,
    any_out_of_range: &mut bool,
) -> CmdResult {
    let config_path = content_dir.join("chapter.toml");
    let config_str = std::fs::read_to_string(&config_path)?;
    let config: site_gen::config::ChapterConfig = toml::from_str(&config_str)?;

    let chapter_name = content_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    println!("Auditing chapter: {chapter_name}  (level: {})", config.chapter.level);

    for section in &config.sections {
        for page in &section.pages {
            if page.page_type != "weave" {
                continue;
            }
            let lesson_path = content_dir.join(format!("{}.html", page.slug));
            let Ok(html) = std::fs::read_to_string(&lesson_path) else {
                eprintln!("  skip (missing): {}.html", page.slug);
                continue;
            };
            let shares = weave::count_words_by_lang(&html);

            let meta_path = content_dir.join(format!("{}.meta.json", page.slug));
            let meta: Option<LessonMeta> = std::fs::read_to_string(&meta_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());
            let level = meta
                .as_ref()
                .and_then(|m| m.level.clone())
                .unwrap_or_else(|| config.chapter.level.clone());

            print_lesson_audit(
                &page.slug,
                &level,
                &shares,
                meta.as_ref(),
                tolerance_pp,
                any_out_of_range,
            );
        }
    }
    Ok(())
}

fn print_lesson_audit(
    slug: &str,
    level: &str,
    shares: &weave::LangShares,
    meta: Option<&LessonMeta>,
    tolerance_pp: f64,
    any_out_of_range: &mut bool,
) {
    let total = shares.total();
    let en = shares.ratio("en");
    let fr = shares.ratio("fr");
    let it = shares.ratio("it");

    println!("  [{level}] {slug}.html  ({total} words)");
    println!(
        "    measured: en {:>5.1}%  fr {:>5.1}%  it {:>5.1}%",
        en * 100.0,
        fr * 100.0,
        it * 100.0,
    );

    if let Some(m) = meta {
        if let Some(declared) = m.language_shares {
            let den = (en - declared.en).abs();
            let dfr = (fr - declared.fr).abs();
            let dit = (it - declared.it).abs();
            let max_delta = den.max(dfr).max(dit);
            println!(
                "    declared: en {:>5.1}%  fr {:>5.1}%  it {:>5.1}%  (max Δ {:.1}%)",
                declared.en * 100.0,
                declared.fr * 100.0,
                declared.it * 100.0,
                max_delta * 100.0,
            );
        }
    }

    if let Some(tight) = range_for_level(level) {
        // The effective range used for the pass/fail check incorporates
        // the user-supplied tolerance; the tight §4 range is always what
        // we display in the warning message so the author sees the
        // design target, not just the widened version.
        let effective = if tolerance_pp > 0.0 {
            widen_range(tight, tolerance_pp)
        } else {
            tight
        };
        let tol_suffix = if tolerance_pp > 0.0 {
            format!(" (tolerance ±{tolerance_pp}%)")
        } else {
            String::new()
        };
        let check = |name: &str,
                     value: f64,
                     tight: (f64, f64),
                     effective: (f64, f64)|
         -> bool {
            if value < effective.0 || value > effective.1 {
                println!(
                    "    ⚠ {name} {:.1}% outside expected {:.0}–{:.0}%{}",
                    value * 100.0,
                    tight.0 * 100.0,
                    tight.1 * 100.0,
                    tol_suffix,
                );
                false
            } else {
                true
            }
        };
        let mut ok = true;
        ok &= check("it", it, tight.it, effective.it);
        ok &= check("fr", fr, tight.fr, effective.fr);
        ok &= check("en", en, tight.en, effective.en);
        if ok {
            let within_msg = if tolerance_pp > 0.0 {
                format!("    within §4 range for {level} (±{tolerance_pp}%)")
            } else {
                format!("    within §4 range for {level}")
            };
            println!("{within_msg}");
        } else {
            *any_out_of_range = true;
        }
    } else {
        println!("    (no §4 range defined for level {level}; not checking)");
    }
}

// ── extract-vocab ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct VocabFile {
    items: Vec<VocabItem>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct VocabItem {
    it: String,
    #[serde(default)]
    fr: String,
    #[serde(default)]
    en: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    es: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pattern: String,
}

fn cmd_extract_vocab(args: &[String]) -> CmdResult {
    let chapter_filter = args.get(2).filter(|s| !s.starts_with('-')).cloned();
    let content_root = PathBuf::from("content");
    let chapters = drills::select_chapters(&content_root, chapter_filter.as_deref())?;
    if chapters.is_empty() {
        eprintln!("no chapters found in {}", content_root.display());
        std::process::exit(1);
    }

    for chapter in &chapters {
        extract_vocab_chapter(&content_root.join(chapter))?;
    }
    Ok(())
}

fn extract_vocab_chapter(content_dir: &Path) -> CmdResult {
    let config_path = content_dir.join("chapter.toml");
    let config_str = std::fs::read_to_string(&config_path)?;
    let config: site_gen::config::ChapterConfig = toml::from_str(&config_str)?;

    let chapter_name = content_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    println!("Extracting vocab: {chapter_name}");

    for section in &config.sections {
        for page in &section.pages {
            if page.page_type != "weave" {
                continue;
            }
            let lesson_path = content_dir.join(format!("{}.html", page.slug));
            let Ok(html) = std::fs::read_to_string(&lesson_path) else {
                eprintln!("  skip (missing): {}.html", page.slug);
                continue;
            };

            let mut seen: HashSet<String> = HashSet::new();
            let mut ordered: Vec<String> = Vec::new();
            for span in weave::extract::extract_italian_spans(&html) {
                let canonical = weave::normalize_for_hash(&span.text);
                if canonical.is_empty() {
                    continue;
                }
                if seen.insert(canonical.clone()) {
                    ordered.push(canonical);
                }
            }

            let vocab_path = content_dir.join(format!("{}.vocab.json", page.slug));
            let (items, existing_count, added) =
                merge_vocab(&vocab_path, &ordered)?;
            let out = VocabFile { items };
            std::fs::write(&vocab_path, serde_json::to_string_pretty(&out)? + "\n")?;
            println!(
                "  {}.vocab.json: {} items ({} existing, {} added)",
                page.slug,
                out.items.len(),
                existing_count,
                added,
            );
        }
    }
    Ok(())
}

// ── prune-vocab ─────────────────────────────────────────────────────────

fn cmd_prune_vocab(args: &[String]) -> CmdResult {
    let chapter = args
        .get(2)
        .cloned()
        .ok_or("prune-vocab requires a chapter name")?;

    let content_dir = PathBuf::from("content").join(&chapter);
    if !content_dir.is_dir() {
        return Err(format!("no such chapter: {}", content_dir.display()).into());
    }

    let config_path = content_dir.join("chapter.toml");
    let config_str = std::fs::read_to_string(&config_path)?;
    let config: site_gen::config::ChapterConfig = toml::from_str(&config_str)?;

    // Walk all weave lessons in the chapter and collect the canonical
    // form of every Italian span currently on a page. Canonical form
    // is the same one used by the drill pipeline, so apostrophe
    // variants (ASCII vs U+2019) match automatically.
    let mut current_canonicals: HashSet<String> = HashSet::new();
    for section in &config.sections {
        for page in &section.pages {
            if page.page_type != "weave" {
                continue;
            }
            let lesson_path = content_dir.join(format!("{}.html", page.slug));
            let Ok(html) = std::fs::read_to_string(&lesson_path) else {
                continue;
            };
            for span in weave::extract::extract_italian_spans(&html) {
                let canonical = weave::normalize_for_hash(&span.text);
                if !canonical.is_empty() {
                    current_canonicals.insert(canonical);
                }
            }
        }
    }

    // For each weave lesson's vocab.json, drop items whose canonical
    // form isn't in the current set. Preserve everything else (glosses
    // intact).
    let chapter_name = content_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    println!("Pruning vocab: {chapter_name}");

    let mut total_pruned = 0;
    for section in &config.sections {
        for page in &section.pages {
            if page.page_type != "weave" {
                continue;
            }
            let vocab_path = content_dir.join(format!("{}.vocab.json", page.slug));
            let Ok(raw) = std::fs::read_to_string(&vocab_path) else {
                continue;
            };
            let vocab: VocabFile = serde_json::from_str(&raw)
                .map_err(|e| format!("parsing {}: {e}", vocab_path.display()))?;
            let before = vocab.items.len();
            let retained: Vec<VocabItem> = vocab
                .items
                .into_iter()
                .filter(|item| {
                    let canonical = weave::normalize_for_hash(&item.it);
                    current_canonicals.contains(&canonical)
                })
                .collect();
            let pruned = before - retained.len();
            if pruned > 0 {
                let out = VocabFile { items: retained };
                std::fs::write(&vocab_path, serde_json::to_string_pretty(&out)? + "\n")?;
                println!(
                    "  {}.vocab.json: pruned {pruned}, kept {}",
                    page.slug,
                    out.items.len(),
                );
                total_pruned += pruned;
            } else {
                println!("  {}.vocab.json: no changes", page.slug);
            }
        }
    }

    if total_pruned > 0 {
        println!("\nPruned {total_pruned} stale item(s) total.");
    } else {
        println!("\nAll vocab entries match current lesson content.");
    }
    Ok(())
}

// ── vocab-page ──────────────────────────────────────────────────────────

fn cmd_vocab_page(args: &[String]) -> CmdResult {
    let chapter = args
        .get(2)
        .cloned()
        .ok_or("vocab-page requires a chapter name")?;

    let content_dir = PathBuf::from("content").join(&chapter);
    if !content_dir.is_dir() {
        return Err(format!("no such chapter: {}", content_dir.display()).into());
    }

    // Walk chapter.toml in authoring order so the generated page
    // reflects the sequence the learner encountered the terms in.
    let config_path = content_dir.join("chapter.toml");
    let config_str = std::fs::read_to_string(&config_path)?;
    let config: site_gen::config::ChapterConfig = toml::from_str(&config_str)?;

    let mut lesson_vocabs: Vec<(String, String, Vec<VocabItem>)> = Vec::new();
    let mut total_before_dedup: usize = 0;
    for section in &config.sections {
        for page in &section.pages {
            if page.page_type != "weave" {
                continue;
            }
            let vocab_path = content_dir.join(format!("{}.vocab.json", page.slug));
            let raw = match std::fs::read_to_string(&vocab_path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let vocab: VocabFile = serde_json::from_str(&raw)
                .map_err(|e| format!("parsing {}: {e}", vocab_path.display()))?;
            total_before_dedup += vocab.items.len();
            lesson_vocabs.push((page.slug.clone(), page.title.clone(), vocab.items));
        }
    }

    // Dedup across lessons. Each term lives in the section of the
    // first lesson it appears in; repetitions elsewhere are dropped.
    let mut seen: HashSet<String> = HashSet::new();
    let mut sections: Vec<(String, String, Vec<VocabItem>)> = Vec::new();
    for (slug, title, items) in lesson_vocabs {
        let unique: Vec<VocabItem> = items
            .into_iter()
            .filter(|i| seen.insert(i.it.clone()))
            .collect();
        if !unique.is_empty() {
            sections.push((slug, title, unique));
        }
    }

    // Second-pass dedup: collapse entries that share a bare form but
    // differ on article (`pasta` / `la pasta` / `una pasta`). Keep the
    // variant with the highest article_priority — definite > indefinite
    // > bare — so the vocabolario shows one form per noun, with gender
    // visible.
    let sections = collapse_bare_article_pairs(sections);

    let html = render_vocab_page(&sections);
    let out_path = content_dir.join("vocabolario.html");
    std::fs::write(&out_path, html)?;

    let unique_total: usize = sections.iter().map(|(_, _, v)| v.len()).sum();
    let dropped = total_before_dedup.saturating_sub(unique_total);
    println!(
        "Wrote {} ({} unique terms across {} lesson section(s); {} cross-lesson duplicate(s) dropped)",
        out_path.display(),
        unique_total,
        sections.len(),
        dropped,
    );
    Ok(())
}

/// Collapse vocab entries that share a bare headword (modulo leading
/// article) across the entire aggregated vocabolario. Each resulting
/// section retains at most one entry per bare form; the surviving
/// variant is the one with the highest article priority — definite
/// article > indefinite article > bare — so the final page displays
/// `la pasta` rather than `pasta` and `il Parmigiano Reggiano` rather
/// than `Parmigiano Reggiano`.
fn collapse_bare_article_pairs(
    sections: Vec<(String, String, Vec<VocabItem>)>,
) -> Vec<(String, String, Vec<VocabItem>)> {
    // Pass 1: index every item by its bare form, noting section and
    // position for later removal.
    let mut bare_map: std::collections::HashMap<String, Vec<(usize, usize)>> =
        std::collections::HashMap::new();
    for (si, (_, _, items)) in sections.iter().enumerate() {
        for (ii, item) in items.iter().enumerate() {
            let bare = italian::bare_form(&item.it);
            bare_map.entry(bare).or_default().push((si, ii));
        }
    }

    // Pass 2: for each bare form with multiple entries, pick the
    // highest-priority variant and mark the others for removal.
    let mut to_remove: HashSet<(usize, usize)> = HashSet::new();
    for positions in bare_map.values() {
        if positions.len() < 2 {
            continue;
        }
        let preferred = positions
            .iter()
            .copied()
            .max_by_key(|&(si, ii)| italian::article_priority(&sections[si].2[ii].it))
            .expect("non-empty slice");
        for &pos in positions {
            if pos != preferred {
                to_remove.insert(pos);
            }
        }
    }

    // Pass 3: rebuild sections excluding the marked positions.
    sections
        .into_iter()
        .enumerate()
        .map(|(si, (slug, title, items))| {
            let kept: Vec<VocabItem> = items
                .into_iter()
                .enumerate()
                .filter(|(ii, _)| !to_remove.contains(&(si, *ii)))
                .map(|(_, item)| item)
                .collect();
            (slug, title, kept)
        })
        .collect()
}

fn render_vocab_page(sections: &[(String, String, Vec<VocabItem>)]) -> String {
    let mut html = String::with_capacity(32 * 1024);

    html.push_str("<h2>Vocabolario della cucina</h2>\n\n");
    html.push_str(
        "<p class=\"vocab-intro\">\
         All Italian terms and phrases from the cooking weave lessons, in order \
         of first appearance. Click any Italian term to hear it pronounced — \
         drill audio is shared with the lessons, so the voice for a given term \
         is the same one you heard while reading. French and English translations \
         are blurred by default; click any gloss to reveal it, and click again \
         to hide.\
         </p>\n\n",
    );

    // Table of contents across the sections, for long pages where
    // scrolling to a specific lesson is handy.
    if sections.len() > 1 {
        html.push_str("<nav class=\"vocab-toc\" aria-label=\"lessons\">\n<ul>\n");
        for (slug, title, items) in sections {
            html.push_str(&format!(
                "  <li><a href=\"#vocab-{}\">{}</a> <span class=\"vocab-count\">({} term(s))</span></li>\n",
                escape_attr(slug),
                escape_text(title),
                items.len(),
            ));
        }
        html.push_str("</ul>\n</nav>\n\n");
    }

    for (slug, title, items) in sections {
        html.push_str(&format!(
            "<section class=\"vocab-group\" id=\"vocab-{}\">\n",
            escape_attr(slug),
        ));
        html.push_str(&format!("<h3>{}</h3>\n", escape_text(title)));
        html.push_str("<table class=\"vocab-table\">\n");
        html.push_str(
            "<thead><tr><th>Italiano</th><th>Français</th><th>English</th></tr></thead>\n",
        );
        html.push_str("<tbody>\n");
        for item in items {
            let canonical = weave::normalize_for_hash(&item.it);
            let audio = drill_url(&canonical);
            html.push_str("<tr>\n");
            html.push_str(&format!(
                "  <td class=\"vocab-it\"><span lang=\"it\" data-audio=\"{}\">{}</span></td>\n",
                escape_attr(&audio),
                escape_text(&item.it),
            ));
            html.push_str(&format!(
                "  <td class=\"vocab-fr\">{}</td>\n",
                render_gloss_cell(&item.fr, Some("fr"), "French"),
            ));
            html.push_str(&format!(
                "  <td class=\"vocab-en\">{}</td>\n",
                render_gloss_cell(&item.en, None, "English"),
            ));
            html.push_str("</tr>\n");
        }
        html.push_str("</tbody>\n</table>\n</section>\n\n");
    }

    html
}

/// Render one gloss cell's inner content.
///
/// An empty gloss renders as a plain em-dash span (not clickable,
/// there's nothing to reveal). A non-empty gloss is wrapped in a
/// `.gloss` element that's keyboard-focusable and role="button"; the
/// site-wide `gloss.js` toggles a `.revealed` class on click or
/// Enter/Space, which removes the CSS blur that hides the text by
/// default.
///
/// `lang` is the BCP 47 code to wrap the gloss text in for styling
/// and accessibility (`Some("fr")` for French glosses; `None` for
/// English, which doesn't need a lang wrapper since the page is
/// already `lang="en"`). `language_name` is the human-readable
/// language name used in the aria-label ("Reveal French translation").
fn render_gloss_cell(content: &str, lang: Option<&str>, language_name: &str) -> String {
    if content.is_empty() {
        return "<span class=\"vocab-empty\">—</span>".to_string();
    }
    let inner = match lang {
        Some(l) => format!(
            "<span lang=\"{}\">{}</span>",
            escape_attr(l),
            escape_text(content),
        ),
        None => escape_text(content),
    };
    format!(
        "<span class=\"gloss\" tabindex=\"0\" role=\"button\" \
         aria-expanded=\"false\" aria-label=\"Reveal {lang_name} translation\">\
         {inner}</span>",
        lang_name = escape_attr(language_name),
    )
}

/// Escape text content for embedding between HTML tags.
fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape a value for an HTML attribute in double quotes.
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── extract-vocab helpers (shared with vocab-page) ──────────────────────

/// Merge ordered discovered Italian items into an existing vocab file.
/// Returns the merged items, the count of already-present items, and
/// the count of newly-added items.
fn merge_vocab(
    vocab_path: &Path,
    discovered: &[String],
) -> Result<(Vec<VocabItem>, usize, usize), Box<dyn std::error::Error>> {
    let existing: Vec<VocabItem> = if vocab_path.exists() {
        let s = std::fs::read_to_string(vocab_path)?;
        let file: VocabFile = serde_json::from_str(&s)?;
        file.items
    } else {
        Vec::new()
    };

    let existing_set: HashSet<String> = existing.iter().map(|i| i.it.clone()).collect();

    let mut merged = existing.clone();
    let mut added = 0;
    for it in discovered {
        if !existing_set.contains(it) {
            merged.push(VocabItem {
                it: it.clone(),
                fr: String::new(),
                en: String::new(),
                es: String::new(),
                pattern: String::new(),
            });
            added += 1;
        }
    }

    Ok((merged, existing.len(), added))
}

// ── verify-language ─────────────────────────────────────────────────────

fn cmd_verify_language(args: &[String]) -> CmdResult {
    let mut fix = false;
    let mut chapter_filter: Option<String> = None;

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--fix" => fix = true,
            other if !other.starts_with('-') && chapter_filter.is_none() => {
                chapter_filter = Some(other.to_string());
            }
            other => return Err(format!("unknown flag: {other}").into()),
        }
        i += 1;
    }

    let rules = site_gen::typography::ItalianTypography;
    let content_root = PathBuf::from("content");
    let chapters = drills::select_chapters(&content_root, chapter_filter.as_deref())?;
    if chapters.is_empty() {
        eprintln!("no chapters found in {}", content_root.display());
        std::process::exit(1);
    }

    if fix {
        let mut total = 0;
        for chapter in &chapters {
            let dir = content_root.join(chapter);
            // Two passes: text-file rules (full line-by-line fixing of
            // .md/.txt/.json) and HTML-aware rules (text nodes only
            // inside .html, tags and attributes preserved verbatim).
            let text_fixed = site_gen::typography::fix_files(&dir, &rules)?;
            let html_fixed = site_gen::typography::fix_html_files(&dir, &rules)?;
            let n = text_fixed + html_fixed;
            if n > 0 {
                println!(
                    "{chapter}: fixed {n} file(s) ({text_fixed} text, {html_fixed} html)"
                );
            }
            total += n;
        }
        if total == 0 {
            println!("All files conform to Italian typography rules.");
        } else {
            println!("\nFixed {total} file(s) total.");
        }
    } else {
        let mut total = 0;
        for chapter in &chapters {
            let dir = content_root.join(chapter);
            let violations = site_gen::typography::verify_files(&dir, &rules)?;
            for v in &violations {
                println!("{v}");
            }
            total += violations.len();
        }
        if total > 0 {
            eprintln!("\nFound {total} violation(s). Run with --fix to auto-correct.");
            std::process::exit(1);
        } else {
            println!("No violations found.");
        }
    }
    Ok(())
}

// ── check-csp ───────────────────────────────────────────────────────────

fn cmd_check_csp(args: &[String]) -> CmdResult {
    let mut site_dir = PathBuf::from("site");
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--site" => {
                i += 1;
                site_dir = PathBuf::from(args.get(i).ok_or("--site requires a value")?);
            }
            other => return Err(format!("unknown flag: {other}").into()),
        }
        i += 1;
    }

    if !site_dir.is_dir() {
        return Err(format!("site directory not found: {}", site_dir.display()).into());
    }

    let violations = site_gen::csp::check_site(&site_dir)?;
    if violations.is_empty() {
        println!("No CSP violations in {}", site_dir.display());
        Ok(())
    } else {
        for v in &violations {
            eprintln!("{v}");
        }
        eprintln!("\nFound {} violation(s) in {}", violations.len(), site_dir.display());
        std::process::exit(1);
    }
}

// ── strip-metadata ──────────────────────────────────────────────────────

fn cmd_strip_metadata(args: &[String]) -> CmdResult {
    let input = args
        .get(2)
        .ok_or("strip-metadata requires an input path")?
        .clone();
    let mut output_dir: Option<PathBuf> = None;
    let mut keep_icc = false;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--output" => {
                i += 1;
                output_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--output requires a value")?,
                ));
            }
            "--keep-icc" => keep_icc = true,
            other => return Err(format!("unknown flag: {other}").into()),
        }
        i += 1;
    }

    let opts = image_strip::StripOptions { keep_icc };
    let input_path = PathBuf::from(&input);

    let targets = if input_path.is_dir() {
        let mut v = Vec::new();
        collect_images(&input_path, &mut v)?;
        v
    } else {
        vec![input_path.clone()]
    };

    for path in &targets {
        let out = match &output_dir {
            Some(dir) => dir.join(path.file_name().unwrap()),
            None => path.clone(),
        };
        let report = image_strip::strip_metadata(path, &out, &opts)?;
        println!("{report}");
    }
    Ok(())
}

fn collect_images(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_images(&path, out)?;
        } else if image_strip::detect_format(&path).is_some() {
            out.push(path);
        }
    }
    Ok(())
}

// ── file ────────────────────────────────────────────────────────────────

async fn cmd_file(args: &[String]) -> CmdResult {
    let input = args.get(2).ok_or("file requires an input path")?;
    let output = args.get(3).ok_or("file requires an output path")?;

    let mut voice_name: Option<String> = None;
    let mut i = 4;
    while i < args.len() {
        match args[i].as_str() {
            "--voice" => {
                i += 1;
                voice_name = Some(args.get(i).ok_or("--voice requires a value")?.clone());
            }
            other => return Err(format!("unknown flag: {other}").into()),
        }
        i += 1;
    }

    let text = std::fs::read_to_string(input)?;
    let processed = italian::normalize_roman_numerals(&text);

    let voice = match voice_name {
        Some(name) => {
            // Leak to obtain a &'static str suitable for the Voice struct.
            // This runs once per CLI invocation; memory pressure is negligible.
            let leaked: &'static str = Box::leak(name.into_boxed_str());
            tts::Voice { language_code: "it-IT", name: leaked }
        }
        None => *italian::voice_for_text(&text),
    };

    let tts = GoogleTts::from_env()?;
    let out_path = PathBuf::from(output);
    tts.synthesize_to_file(&processed, &voice, &out_path).await?;

    let mut stdout = std::io::stdout().lock();
    writeln!(
        stdout,
        "wrote {} using voice {}",
        out_path.display(),
        voice.name,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── range_for_level ─────────────────────────────────────────────

    #[test]
    fn range_for_level_matches_design_md() {
        let a2 = range_for_level("A2").unwrap();
        assert_eq!(a2.it, (0.30, 0.40));
        assert_eq!(a2.fr, (0.30, 0.40));
        assert_eq!(a2.en, (0.25, 0.35));

        let a2b1 = range_for_level("A2-B1").unwrap();
        assert_eq!(a2b1.it, (0.45, 0.55));

        let b1 = range_for_level("B1").unwrap();
        assert_eq!(b1.it, (0.60, 0.70));
    }

    #[test]
    fn range_for_level_returns_none_for_unknown_levels() {
        assert!(range_for_level("A1").is_none());
        assert!(range_for_level("A1-reference").is_none());
        assert!(range_for_level("B2").is_none());
        assert!(range_for_level("unknown").is_none());
    }

    // ── widen_range ─────────────────────────────────────────────────

    #[test]
    fn widen_range_adds_tolerance_on_each_side() {
        let tight = LevelRange {
            it: (0.45, 0.55),
            fr: (0.25, 0.35),
            en: (0.15, 0.25),
        };
        let widened = widen_range(tight, 5.0);
        assert!((widened.it.0 - 0.40).abs() < 1e-9);
        assert!((widened.it.1 - 0.60).abs() < 1e-9);
        assert!((widened.fr.0 - 0.20).abs() < 1e-9);
        assert!((widened.fr.1 - 0.40).abs() < 1e-9);
        assert!((widened.en.0 - 0.10).abs() < 1e-9);
        assert!((widened.en.1 - 0.30).abs() < 1e-9);
    }

    #[test]
    fn widen_range_clamps_to_zero_and_one() {
        // A tolerance large enough to push a range below 0 or above 1
        // must clamp rather than return a negative or >1 bound.
        let tight = LevelRange {
            it: (0.10, 0.90),
            fr: (0.02, 0.98),
            en: (0.00, 1.00),
        };
        let widened = widen_range(tight, 20.0);
        assert_eq!(widened.it, (0.00, 1.00));
        assert_eq!(widened.fr, (0.00, 1.00));
        assert_eq!(widened.en, (0.00, 1.00));
    }

    #[test]
    fn widen_range_zero_tolerance_is_identity() {
        let tight = LevelRange {
            it: (0.45, 0.55),
            fr: (0.25, 0.35),
            en: (0.15, 0.25),
        };
        let widened = widen_range(tight, 0.0);
        assert_eq!(widened.it, tight.it);
        assert_eq!(widened.fr, tight.fr);
        assert_eq!(widened.en, tight.en);
    }

    // ── escape helpers ──────────────────────────────────────────────

    #[test]
    fn escape_text_escapes_the_three_essentials() {
        assert_eq!(escape_text("a & b"), "a &amp; b");
        assert_eq!(escape_text("<span>"), "&lt;span&gt;");
        assert_eq!(escape_text("no special chars"), "no special chars");
    }

    #[test]
    fn escape_text_preserves_unicode_and_quotes() {
        // Curly apostrophes and double quotes inside text content don't
        // need escaping — they have no syntactic meaning there.
        assert_eq!(escape_text("l\u{2019}acqua"), "l\u{2019}acqua");
        assert_eq!(escape_text("\"Roma\""), "\"Roma\"");
    }

    #[test]
    fn escape_attr_escapes_double_quotes() {
        assert_eq!(escape_attr("a\"b"), "a&quot;b");
        assert_eq!(escape_attr("a & b"), "a &amp; b");
        assert_eq!(escape_attr("<x>"), "&lt;x&gt;");
    }

    #[test]
    fn escape_attr_preserves_single_quotes() {
        // Our attributes are always double-quoted, so single quotes
        // (ASCII or curly) don't need escaping inside the value.
        assert_eq!(escape_attr("l'acqua"), "l'acqua");
        assert_eq!(escape_attr("l\u{2019}acqua"), "l\u{2019}acqua");
    }

    // ── render_vocab_page ───────────────────────────────────────────

    #[test]
    fn render_vocab_page_produces_drill_audio_links() {
        let sections = vec![(
            "001-test".to_string(),
            "Test Lesson".to_string(),
            vec![VocabItem {
                it: "la storia".to_string(),
                fr: "l'histoire".to_string(),
                en: "history".to_string(),
                es: String::new(),
                pattern: String::new(),
            }],
        )];
        let html = render_vocab_page(&sections);

        // Italian term has a drill-audio attribute
        assert!(html.contains(r#"<span lang="it" data-audio="audio/drills/"#));
        assert!(html.contains("la storia"));
        // French and English glosses appear inside the rendered page
        assert!(html.contains("l\u{2019}histoire") || html.contains("l'histoire"));
        assert!(html.contains("history"));
        // Section anchor
        assert!(html.contains(r#"id="vocab-001-test""#));
    }

    #[test]
    fn render_vocab_page_renders_em_dash_for_empty_glosses() {
        let sections = vec![(
            "001-test".to_string(),
            "Test".to_string(),
            vec![VocabItem {
                it: "mano".to_string(),
                fr: String::new(),
                en: String::new(),
                es: String::new(),
                pattern: String::new(),
            }],
        )];
        let html = render_vocab_page(&sections);
        // Missing glosses show as plain em-dash spans, not reveal buttons
        assert!(html.contains("<span class=\"vocab-empty\">—</span>"));
        // And the gloss-reveal wrapper must NOT appear when content is empty
        assert!(!html.contains("class=\"gloss\""));
    }

    // ── render_gloss_cell ───────────────────────────────────────────

    #[test]
    fn gloss_cell_empty_renders_em_dash_only() {
        assert_eq!(
            render_gloss_cell("", Some("fr"), "French"),
            "<span class=\"vocab-empty\">—</span>",
        );
        assert_eq!(
            render_gloss_cell("", None, "English"),
            "<span class=\"vocab-empty\">—</span>",
        );
    }

    #[test]
    fn gloss_cell_wraps_french_content_in_reveal_span() {
        let out = render_gloss_cell("l'histoire", Some("fr"), "French");
        // keyboard-focusable, screen-reader-announced as a button
        assert!(out.contains(r#"class="gloss""#));
        assert!(out.contains(r#"tabindex="0""#));
        assert!(out.contains(r#"role="button""#));
        assert!(out.contains(r#"aria-expanded="false""#));
        assert!(out.contains(r#"aria-label="Reveal French translation""#));
        // inner text is wrapped in lang="fr"
        assert!(out.contains(r#"<span lang="fr">l'histoire</span>"#));
    }

    #[test]
    fn gloss_cell_wraps_english_content_without_lang_attribute() {
        // English glosses skip the inner lang wrapper (page is already en).
        let out = render_gloss_cell("history", None, "English");
        assert!(out.contains(r#"class="gloss""#));
        assert!(out.contains(r#"aria-label="Reveal English translation""#));
        // No lang="en" wrapper on the content itself
        assert!(!out.contains(r#"lang="en""#));
        // Text appears directly inside the gloss span
        assert!(out.contains(">history</span>"));
    }

    #[test]
    fn gloss_cell_escapes_html_in_content() {
        let out = render_gloss_cell("<script>alert(1)</script>", None, "English");
        assert!(out.contains("&lt;script&gt;"));
        assert!(!out.contains("<script>"));
    }

    #[test]
    fn gloss_cell_escapes_html_in_french_content() {
        let out = render_gloss_cell("A & B", Some("fr"), "French");
        assert!(out.contains(r#"<span lang="fr">A &amp; B</span>"#));
    }

    // ── collapse_bare_article_pairs ─────────────────────────────────

    fn vocab_item(it: &str) -> VocabItem {
        VocabItem {
            it: it.to_string(),
            fr: String::new(),
            en: String::new(),
            es: String::new(),
            pattern: String::new(),
        }
    }

    fn section_texts(sections: &[(String, String, Vec<VocabItem>)]) -> Vec<Vec<String>> {
        sections
            .iter()
            .map(|(_, _, items)| items.iter().map(|i| i.it.clone()).collect())
            .collect()
    }

    #[test]
    fn collapse_prefers_definite_over_bare() {
        let sections = vec![(
            "001".to_string(),
            "Section".to_string(),
            vec![vocab_item("pasta"), vocab_item("la pasta")],
        )];
        let collapsed = collapse_bare_article_pairs(sections);
        assert_eq!(section_texts(&collapsed), vec![vec!["la pasta".to_string()]]);
    }

    #[test]
    fn collapse_prefers_definite_over_indefinite() {
        let sections = vec![(
            "001".to_string(),
            "Section".to_string(),
            vec![vocab_item("una pasta"), vocab_item("la pasta")],
        )];
        let collapsed = collapse_bare_article_pairs(sections);
        assert_eq!(section_texts(&collapsed), vec![vec!["la pasta".to_string()]]);
    }

    #[test]
    fn collapse_prefers_indefinite_over_bare() {
        let sections = vec![(
            "001".to_string(),
            "Section".to_string(),
            vec![vocab_item("libro"), vocab_item("un libro")],
        )];
        let collapsed = collapse_bare_article_pairs(sections);
        assert_eq!(section_texts(&collapsed), vec![vec!["un libro".to_string()]]);
    }

    #[test]
    fn collapse_handles_case_differences() {
        // `La pasta` (sentence-initial) and `la pasta` (mid-sentence)
        // share a bare form and collapse.
        let sections = vec![(
            "001".to_string(),
            "Section".to_string(),
            vec![vocab_item("La pasta"), vocab_item("la pasta")],
        )];
        let collapsed = collapse_bare_article_pairs(sections);
        // One survives; both are priority 3, so whichever came first.
        let texts = section_texts(&collapsed);
        assert_eq!(texts[0].len(), 1);
        assert!(texts[0][0].eq_ignore_ascii_case("la pasta"));
    }

    #[test]
    fn collapse_spans_multiple_sections() {
        // Two sections, each with a variant — one survives overall.
        let sections = vec![
            (
                "001".to_string(),
                "First".to_string(),
                vec![vocab_item("pasta")],
            ),
            (
                "002".to_string(),
                "Second".to_string(),
                vec![vocab_item("la pasta")],
            ),
        ];
        let collapsed = collapse_bare_article_pairs(sections);
        let texts = section_texts(&collapsed);
        assert_eq!(texts[0], Vec::<String>::new());
        assert_eq!(texts[1], vec!["la pasta".to_string()]);
    }

    #[test]
    fn collapse_leaves_unrelated_forms_alone() {
        // Different bare forms don't collapse.
        let sections = vec![(
            "001".to_string(),
            "Section".to_string(),
            vec![
                vocab_item("la pasta"),
                vocab_item("la pasta secca"),
                vocab_item("il pomodoro"),
            ],
        )];
        let collapsed = collapse_bare_article_pairs(sections);
        assert_eq!(
            section_texts(&collapsed),
            vec![vec![
                "la pasta".to_string(),
                "la pasta secca".to_string(),
                "il pomodoro".to_string(),
            ]],
        );
    }

    #[test]
    fn collapse_works_with_elided_articles() {
        let sections = vec![(
            "001".to_string(),
            "Section".to_string(),
            vec![vocab_item("acqua"), vocab_item("l\u{2019}acqua")],
        )];
        let collapsed = collapse_bare_article_pairs(sections);
        let texts = section_texts(&collapsed);
        assert_eq!(texts[0], vec!["l\u{2019}acqua".to_string()]);
    }

    #[test]
    fn render_vocab_page_emits_toc_when_multiple_sections() {
        let sections = vec![
            (
                "001-a".to_string(),
                "First".to_string(),
                vec![VocabItem {
                    it: "a".into(),
                    fr: "".into(),
                    en: "".into(),
                    es: "".into(),
                    pattern: "".into(),
                }],
            ),
            (
                "002-b".to_string(),
                "Second".to_string(),
                vec![VocabItem {
                    it: "b".into(),
                    fr: "".into(),
                    en: "".into(),
                    es: "".into(),
                    pattern: "".into(),
                }],
            ),
        ];
        let html = render_vocab_page(&sections);
        assert!(html.contains("<nav class=\"vocab-toc\""));
        assert!(html.contains("href=\"#vocab-001-a\""));
        assert!(html.contains("href=\"#vocab-002-b\""));
    }

    #[test]
    fn render_vocab_page_skips_toc_when_single_section() {
        let sections = vec![(
            "only".to_string(),
            "Only".to_string(),
            vec![VocabItem {
                it: "x".into(),
                fr: "".into(),
                en: "".into(),
                es: "".into(),
                pattern: "".into(),
            }],
        )];
        let html = render_vocab_page(&sections);
        assert!(!html.contains("vocab-toc"));
    }

    #[test]
    fn widen_range_negative_tolerance_treated_as_zero() {
        // Negative tolerance would narrow ranges which is nonsensical
        // for this API; clamp to zero (identity behavior).
        let tight = LevelRange {
            it: (0.45, 0.55),
            fr: (0.25, 0.35),
            en: (0.15, 0.25),
        };
        let widened = widen_range(tight, -5.0);
        assert_eq!(widened.it, tight.it);
        assert_eq!(widened.fr, tight.fr);
        assert_eq!(widened.en, tight.en);
    }
}
