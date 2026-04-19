//! Render chapters and the site index from `content/` into `site/`.
//!
//! The entry points are [`build_chapter`] (render one chapter's pages
//! and index) and [`generate_site_index`] (render the top-level landing
//! page from `site.toml`).
//!
//! Weave lessons are rendered by wrapping their HTML fragment in the
//! `weave.html` Tera template; an `inject_drills` callback hooks the
//! drill-audio pipeline on the per-span Italian text (see the root
//! binary's `drills` module). `fragment` pages skip the drill-audio
//! step and render the fragment verbatim. `static` pages are hand-
//! authored and left untouched.

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use tera::{Context, Tera};

use crate::config::{ChapterConfig, PageConfig, SiteConfig};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("TOML parse error in {path}: {source}")]
    Toml {
        path: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("Tera template error: {0}")]
    Tera(#[from] tera::Error),
    #[error("unknown page type {page_type:?} for page {slug}")]
    UnknownPageType { slug: String, page_type: String },
}

impl BuildError {
    fn io(path: &Path, source: std::io::Error) -> Self {
        Self::Io { path: path.display().to_string(), source }
    }
    fn toml(path: &Path, source: toml::de::Error) -> Self {
        Self::Toml { path: path.display().to_string(), source }
    }
}

/// Callback that transforms a raw weave fragment's HTML to inject
/// `data-audio` attributes on Italian spans. The root binary wires this
/// to its hash-and-synthesize pipeline; pure builds (no audio) can pass
/// `|html| html.to_string()` as a no-op.
pub type InjectDrills<'a> = &'a dyn Fn(&str) -> String;

/// Data passed to the chapter index template for each page entry.
#[derive(Debug, Serialize)]
struct IndexPageData {
    slug: String,
    title: String,
    description: String,
    flag: Option<String>,
}

#[derive(Debug, Serialize)]
struct IndexSectionData {
    heading: String,
    pages: Vec<IndexPageData>,
}

/// Build all pages for a single chapter, plus its `index.html`.
pub fn build_chapter(
    content_dir: &Path,
    output_dir: &Path,
    templates_dir: &Path,
    base_url: Option<&str>,
    inject_drills: InjectDrills<'_>,
) -> Result<(), BuildError> {
    let tera = load_templates(templates_dir)?;

    let config_path = content_dir.join("chapter.toml");
    let config_str = std::fs::read_to_string(&config_path)
        .map_err(|e| BuildError::io(&config_path, e))?;
    let config: ChapterConfig =
        toml::from_str(&config_str).map_err(|e| BuildError::toml(&config_path, e))?;

    std::fs::create_dir_all(output_dir)
        .map_err(|e| BuildError::io(output_dir, e))?;

    for section in &config.sections {
        for page in &section.pages {
            match page.page_type.as_str() {
                "weave" => build_weave_page(
                    &tera, &config, page, content_dir, output_dir, base_url, inject_drills,
                )?,
                "fragment" => build_fragment_page(
                    &tera, &config, page, content_dir, output_dir, base_url,
                )?,
                "static" => {
                    println!("  skip (static): {}.html", page.slug);
                }
                other => {
                    return Err(BuildError::UnknownPageType {
                        slug: page.slug.clone(),
                        page_type: other.to_string(),
                    });
                }
            }
        }
    }

    build_chapter_index(&tera, &config, output_dir, base_url)?;
    Ok(())
}

fn load_templates(templates_dir: &Path) -> Result<Tera, BuildError> {
    let glob = format!("{}/**/*.html", templates_dir.display());
    Ok(Tera::new(&glob)?)
}

fn build_weave_page(
    tera: &Tera,
    config: &ChapterConfig,
    page: &PageConfig,
    content_dir: &Path,
    output_dir: &Path,
    base_url: Option<&str>,
    inject_drills: InjectDrills<'_>,
) -> Result<(), BuildError> {
    let fragment_path = content_dir.join(format!("{}.html", page.slug));
    let fragment = std::fs::read_to_string(&fragment_path)
        .map_err(|e| BuildError::io(&fragment_path, e))?;

    let rendered_fragment = inject_drills(&fragment);

    let mut ctx = page_context(config, page, base_url);
    ctx.insert("content", &rendered_fragment);
    ctx.insert("has_drill_audio", &true);

    let html = tera.render("weave.html", &ctx)?;
    let out_path = output_dir.join(format!("{}.html", page.slug));
    std::fs::write(&out_path, html).map_err(|e| BuildError::io(&out_path, e))?;
    println!("  wrote {}.html (weave)", page.slug);
    Ok(())
}

fn build_fragment_page(
    tera: &Tera,
    config: &ChapterConfig,
    page: &PageConfig,
    content_dir: &Path,
    output_dir: &Path,
    base_url: Option<&str>,
) -> Result<(), BuildError> {
    let fragment_path = content_dir.join(format!("{}.html", page.slug));
    let fragment = std::fs::read_to_string(&fragment_path)
        .map_err(|e| BuildError::io(&fragment_path, e))?;

    let mut ctx = page_context(config, page, base_url);
    ctx.insert("content", &fragment);
    ctx.insert("has_drill_audio", &false);

    let html = tera.render("fragment.html", &ctx)?;
    let out_path = output_dir.join(format!("{}.html", page.slug));
    std::fs::write(&out_path, html).map_err(|e| BuildError::io(&out_path, e))?;
    println!("  wrote {}.html (fragment)", page.slug);
    Ok(())
}

fn page_context(
    config: &ChapterConfig,
    page: &PageConfig,
    base_url: Option<&str>,
) -> Context {
    let mut ctx = Context::new();
    ctx.insert("chapter", &config.chapter);
    ctx.insert("title", &page.title);
    ctx.insert("subtitle", &page.subtitle);
    ctx.insert("description", &page.description);
    ctx.insert("slug", &page.slug);
    if let Some(ref flag) = page.flag {
        ctx.insert("flag", flag);
    }
    if let Some(base) = base_url {
        ctx.insert(
            "canonical_url",
            &format!("{}/{}.html", base.trim_end_matches('/'), page.slug),
        );
    }
    ctx
}

fn build_chapter_index(
    tera: &Tera,
    config: &ChapterConfig,
    output_dir: &Path,
    base_url: Option<&str>,
) -> Result<(), BuildError> {
    let sections: Vec<IndexSectionData> = config
        .sections
        .iter()
        .map(|s| IndexSectionData {
            heading: s.heading.clone(),
            pages: s
                .pages
                .iter()
                .map(|p| IndexPageData {
                    slug: p.slug.clone(),
                    title: p.title.clone(),
                    description: p.description.clone(),
                    flag: p.flag.clone(),
                })
                .collect(),
        })
        .collect();

    let mut ctx = Context::new();
    ctx.insert("chapter", &config.chapter);
    ctx.insert("sections", &sections);
    if let Some(base) = base_url {
        ctx.insert(
            "canonical_url",
            &format!("{}/index.html", base.trim_end_matches('/')),
        );
    }

    let html = tera.render("chapter_index.html", &ctx)?;
    let out_path = output_dir.join("index.html");
    std::fs::write(&out_path, html).map_err(|e| BuildError::io(&out_path, e))?;
    println!("  wrote index.html");
    Ok(())
}

/// Render the top-level `site/index.html` from `content/site.toml`.
pub fn generate_site_index(
    site_config_path: &Path,
    templates_dir: &Path,
    output_dir: &Path,
) -> Result<(), BuildError> {
    let config_str = std::fs::read_to_string(site_config_path)
        .map_err(|e| BuildError::io(site_config_path, e))?;
    let config: SiteConfig =
        toml::from_str(&config_str).map_err(|e| BuildError::toml(site_config_path, e))?;

    let tera = load_templates(templates_dir)?;
    let mut ctx = Context::new();
    ctx.insert("site", &config.site);
    ctx.insert("levels", &config.levels);

    let html = tera.render("site_index.html", &ctx)?;
    let out_path = output_dir.join("index.html");
    std::fs::write(&out_path, html).map_err(|e| BuildError::io(&out_path, e))?;
    println!("  wrote site index.html");
    Ok(())
}

// ── Sitemap ─────────────────────────────────────────────────────────────

/// Generate `sitemap.xml` listing all HTML pages under `site_dir`.
pub fn generate_sitemap(site_dir: &Path, site_url: &str) -> Result<(), BuildError> {
    let base = site_url.trim_end_matches('/');

    let mut urls: Vec<(String, f32)> = Vec::new();
    collect_html_for_sitemap(site_dir, site_dir, base, &mut urls)?;
    urls.sort_by(|a, b| a.0.cmp(&b.0));

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for (loc, priority) in &urls {
        let _ = write!(
            xml,
            "  <url>\n    <loc>{loc}</loc>\n    <priority>{priority:.1}</priority>\n  </url>\n",
        );
    }
    xml.push_str("</urlset>\n");

    let out = site_dir.join("sitemap.xml");
    std::fs::write(&out, &xml).map_err(|e| BuildError::io(&out, e))?;
    println!("Wrote sitemap.xml ({} URLs)", urls.len());
    Ok(())
}

fn collect_html_for_sitemap(
    dir: &Path,
    site_root: &Path,
    base_url: &str,
    out: &mut Vec<(String, f32)>,
) -> Result<(), BuildError> {
    for entry in std::fs::read_dir(dir).map_err(|e| BuildError::io(dir, e))? {
        let entry = entry.map_err(|e| BuildError::io(dir, e))?;
        let path = entry.path();

        if path.is_dir() {
            // Skip audio directories — they're binary assets, not pages.
            if path.file_name().is_some_and(|n| n == "audio") {
                continue;
            }
            collect_html_for_sitemap(&path, site_root, base_url, out)?;
        } else if path.extension().is_some_and(|e| e == "html") {
            let name = path.file_name().unwrap().to_string_lossy();
            if name == "404.html" {
                continue;
            }
            let rel = path
                .strip_prefix(site_root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");

            let priority = if rel == "index.html" {
                1.0
            } else if rel.ends_with("/index.html") {
                0.8
            } else {
                0.5
            };
            out.push((format!("{base_url}/{rel}"), priority));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_index_serializes() {
        // Smoke test: build an index section and confirm it serializes.
        let section = IndexSectionData {
            heading: "A2".into(),
            pages: vec![IndexPageData {
                slug: "001-roma".into(),
                title: "Roma".into(),
                description: "".into(),
                flag: None,
            }],
        };
        let json = serde_json::to_string(&section).unwrap();
        assert!(json.contains("001-roma"));
    }

    #[test]
    fn sitemap_classify() {
        // Priority rules: root index gets 1.0, chapter indexes 0.8, rest 0.5.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<html></html>").unwrap();
        let chap = dir.path().join("chapters").join("20-roma");
        std::fs::create_dir_all(&chap).unwrap();
        std::fs::write(chap.join("index.html"), "<html></html>").unwrap();
        std::fs::write(chap.join("001-roma.html"), "<html></html>").unwrap();

        let mut urls = Vec::new();
        collect_html_for_sitemap(dir.path(), dir.path(), "https://example.org", &mut urls)
            .unwrap();
        assert_eq!(urls.len(), 3);

        let find = |url: &str| {
            urls.iter()
                .find(|(loc, _)| loc == url)
                .map(|(_, p)| *p)
                .unwrap_or_else(|| panic!("no URL {url}; got {urls:?}"))
        };
        assert_eq!(
            find("https://example.org/index.html"),
            1.0,
            "root index.html should be priority 1.0",
        );
        assert_eq!(find("https://example.org/chapters/20-roma/index.html"), 0.8);
        assert_eq!(find("https://example.org/chapters/20-roma/001-roma.html"), 0.5);
    }
}
