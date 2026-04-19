//! Deserializable configuration for sites and chapters.
//!
//! A chapter has a `chapter.toml` at the root of its content directory,
//! listing its lessons in sections. The site has a top-level `site.toml`
//! in `content/` listing each chapter under a CEFR-level heading.

use serde::{Deserialize, Serialize};

// ── Site-level (`content/site.toml`) ────────────────────────────────────

/// Site-level configuration loaded from `content/site.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SiteConfig {
    pub site: SiteMeta,
    pub levels: Vec<LevelConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SiteMeta {
    pub title: String,
    pub subtitle: String,
    pub tagline: String,
    pub description: String,
    pub canonical_url: String,
    pub intro: String,
    pub footer: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LevelConfig {
    pub heading: String,
    pub chapters: Vec<SiteChapterEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SiteChapterEntry {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub meta: String,
    pub flag: Option<String>,
    #[serde(default)]
    pub new: bool,
}

// ── Chapter-level (`content/<chapter>/chapter.toml`) ────────────────────

/// Chapter configuration loaded from `chapter.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChapterConfig {
    pub chapter: ChapterMeta,
    #[serde(default)]
    pub sections: Vec<SectionConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChapterMeta {
    pub title: String,
    #[serde(default)]
    pub subtitle: String,
    /// CEFR level: "A1", "A1-reference", "A2", "A2-B1", "B1".
    #[serde(default = "default_level")]
    pub level: String,
    /// Footer text; typically the site title.
    #[serde(default)]
    pub footer_text: String,
    /// Footer suffix; typically the level label.
    #[serde(default)]
    pub footer_suffix: String,
}

fn default_level() -> String {
    "A2-B1".into()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SectionConfig {
    pub heading: String,
    pub pages: Vec<PageConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageConfig {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// One of "weave", "fragment", "static".
    ///
    /// - `weave`: an HTML fragment containing `<span lang="…">` markers;
    ///   the build injects `data-audio` attributes on Italian spans.
    /// - `fragment`: an HTML fragment rendered verbatim (no drill wiring).
    /// - `static`: a hand-authored complete page; build is a no-op.
    #[serde(rename = "type")]
    pub page_type: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    /// Optional feature-flag name. If set, the page is wrapped with
    /// `class="flag-hidden" data-flag="…"` so `shared/flags.js` can
    /// reveal it to reviewers who opt in.
    #[serde(default)]
    pub flag: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_config_minimal() {
        let s = r#"
            [chapter]
            title = "Roma Antica"

            [[sections]]
            heading = "Lessons"

            [[sections.pages]]
            slug = "001-fondazione"
            title = "The Founding"
            type = "weave"
        "#;
        let cfg: ChapterConfig = toml::from_str(s).unwrap();
        assert_eq!(cfg.chapter.title, "Roma Antica");
        assert_eq!(cfg.chapter.level, "A2-B1"); // default
        assert_eq!(cfg.sections[0].pages[0].page_type, "weave");
    }

    #[test]
    fn chapter_config_with_flag() {
        let s = r#"
            [chapter]
            title = "X"

            [[sections]]
            heading = "Draft"

            [[sections.pages]]
            slug = "999-experiment"
            title = "Experiment"
            type = "weave"
            flag = "new-weave"
        "#;
        let cfg: ChapterConfig = toml::from_str(s).unwrap();
        assert_eq!(cfg.sections[0].pages[0].flag.as_deref(), Some("new-weave"));
    }

    #[test]
    fn site_config_round_trip() {
        let s = r#"
            [site]
            title = "Intreccio"
            subtitle = "Triglot Italian"
            tagline = "Learn Italian via French"
            description = "..."
            canonical_url = "https://example.org"
            intro = "Welcome."
            footer = "Intreccio"

            [[levels]]
            heading = "A2"

            [[levels.chapters]]
            slug = "20-roma-antica"
            title = "Roma Antica"
            description = "Intro to Roman history"
            meta = "2 lessons"
            new = true
        "#;
        let cfg: SiteConfig = toml::from_str(s).unwrap();
        assert_eq!(cfg.levels[0].chapters[0].slug, "20-roma-antica");
        assert!(cfg.levels[0].chapters[0].new);
    }
}
