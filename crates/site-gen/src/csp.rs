//! Scan built HTML for Content Security Policy violations.
//!
//! The deployed CSP (`site/_headers`) is strict:
//!
//! ```text
//! default-src 'none';
//! script-src  'self';
//! style-src   'self';
//! connect-src 'self';
//! media-src   'self';
//! img-src     'self';
//! font-src    'self';
//! base-uri    'self';
//! form-action 'none';
//! frame-ancestors 'none';
//! ```
//!
//! This module catches the violations a static-site build can produce:
//! inline scripts and styles, inline event handlers, `javascript:` URIs,
//! `<form>` elements, and external resource URLs.

use std::path::{Path, PathBuf};

/// A CSP violation found during checking.
#[derive(Debug, Clone)]
pub struct CspViolation {
    pub source: String,
    pub line: usize,
    pub reason: String,
}

impl std::fmt::Display for CspViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.source, self.line, self.reason)
    }
}

/// Inline event handler attributes that violate `script-src 'self'`.
const EVENT_HANDLERS: &[&str] = &[
    "onclick", "ondblclick", "onmousedown", "onmouseup", "onmouseover",
    "onmouseout", "onmousemove", "onkeydown", "onkeyup", "onkeypress",
    "onfocus", "onblur", "onchange", "onsubmit", "onreset", "onload",
    "onunload", "onerror", "onresize", "onscroll", "oninput", "onselect",
    "ontouchstart", "ontouchmove", "ontouchend",
];

/// Scan all HTML files under `site_dir` for CSP violations.
pub fn check_site(site_dir: &Path) -> Result<Vec<CspViolation>, std::io::Error> {
    let mut violations = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    collect_html_files(site_dir, &mut files)?;

    for path in &files {
        let content = std::fs::read_to_string(path)?;
        let source = path
            .strip_prefix(site_dir)
            .unwrap_or(path)
            .display()
            .to_string();
        check_html(&source, &content, &mut violations);
    }

    Ok(violations)
}

/// Check a single HTML document and append any violations to `out`.
pub fn check_html(source: &str, html: &str, out: &mut Vec<CspViolation>) {
    for (idx, line) in html.lines().enumerate() {
        let line_num = idx + 1;
        let lower = line.to_lowercase();

        if lower.contains("<style") {
            out.push(CspViolation {
                source: source.into(),
                line: line_num,
                reason: "inline <style> block (use an external CSS file)".into(),
            });
        }

        if has_style_attribute(&lower) {
            out.push(CspViolation {
                source: source.into(),
                line: line_num,
                reason: "inline style=\"…\" attribute (use CSS classes)".into(),
            });
        }

        if lower.contains("<script") && !lower.contains("src=") {
            out.push(CspViolation {
                source: source.into(),
                line: line_num,
                reason: "inline <script> block (use an external JS file)".into(),
            });
        }

        for handler in EVENT_HANDLERS {
            let sp = format!(" {handler}=");
            let tab = format!("\t{handler}=");
            if lower.contains(&sp) || lower.contains(&tab) {
                out.push(CspViolation {
                    source: source.into(),
                    line: line_num,
                    reason: format!("inline event handler {handler}= (move to external JS)"),
                });
                break; // one handler-violation per line is enough.
            }
        }

        if lower.contains("href=\"javascript:") {
            out.push(CspViolation {
                source: source.into(),
                line: line_num,
                reason: "javascript: URI in href (move logic to external JS)".into(),
            });
        }

        if lower.contains("<form") {
            out.push(CspViolation {
                source: source.into(),
                line: line_num,
                reason: "<form> element (form-action 'none' in CSP)".into(),
            });
        }

        check_external_resources(&lower, source, line_num, out);
    }
}

fn check_external_resources(
    lower_line: &str,
    source: &str,
    line_num: usize,
    out: &mut Vec<CspViolation>,
) {
    // <link rel="canonical"> is metadata, not a resource load.
    if lower_line.contains("rel=\"canonical\"") {
        return;
    }
    // <meta> tags declare metadata; their content/href aren't resource fetches.
    if lower_line.trim_start().starts_with("<meta ") {
        return;
    }

    for attr in ["src=\"", "href=\""] {
        let mut rest = lower_line;
        while let Some(pos) = rest.find(attr) {
            let start = pos + attr.len();
            rest = &rest[start..];
            if let Some(end) = rest.find('"') {
                let value = &rest[..end];
                rest = &rest[end + 1..];

                if value.starts_with("http://") || value.starts_with("https://") {
                    out.push(CspViolation {
                        source: source.into(),
                        line: line_num,
                        reason: format!(
                            "external resource URL {value} (all resources must be same-origin)"
                        ),
                    });
                } else if value.starts_with("data:") && attr == "src=\"" {
                    out.push(CspViolation {
                        source: source.into(),
                        line: line_num,
                        reason: format!("data: URI in src ({value}) — not allowed by CSP"),
                    });
                }
            } else {
                break;
            }
        }
    }
}

fn has_style_attribute(lower_line: &str) -> bool {
    let trimmed = lower_line.trim_start();
    if trimmed.starts_with("style=\"") || trimmed.starts_with("style =") {
        return true;
    }
    for pat in [" style=\"", "\tstyle=\"", " style =", "\tstyle ="] {
        if lower_line.contains(pat) {
            return true;
        }
    }
    false
}

fn collect_html_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_html_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("html") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_one(html: &str) -> Vec<CspViolation> {
        let mut v = Vec::new();
        check_html("test.html", html, &mut v);
        v
    }

    #[test]
    fn flags_inline_style_block() {
        let v = check_one("<html><style>body{color:red}</style></html>");
        assert!(v.iter().any(|v| v.reason.contains("inline <style>")));
    }

    #[test]
    fn flags_inline_style_attribute() {
        let v = check_one(r#"<p style="color:red">x</p>"#);
        assert!(v.iter().any(|v| v.reason.contains("inline style=")));
    }

    #[test]
    fn does_not_flag_compound_style_attributes() {
        // font-style and list-style are legitimate attributes that contain "style="
        let v = check_one(r#"<svg><text font-style="italic">x</text></svg>"#);
        assert!(v.iter().all(|v| !v.reason.contains("inline style=")));
    }

    #[test]
    fn flags_inline_script_without_src() {
        let v = check_one("<script>alert(1)</script>");
        assert!(v.iter().any(|v| v.reason.contains("inline <script>")));
    }

    #[test]
    fn allows_script_with_src() {
        let v = check_one(r#"<script src="/shared/drill.js"></script>"#);
        assert!(v.iter().all(|v| !v.reason.contains("inline <script>")));
    }

    #[test]
    fn flags_inline_event_handler() {
        let v = check_one(r#"<button onclick="x()">go</button>"#);
        assert!(v.iter().any(|v| v.reason.contains("onclick=")));
    }

    #[test]
    fn flags_javascript_uri() {
        let v = check_one(r#"<a href="javascript:void(0)">x</a>"#);
        assert!(v.iter().any(|v| v.reason.contains("javascript:")));
    }

    #[test]
    fn flags_form_element() {
        let v = check_one("<form><input/></form>");
        assert!(v.iter().any(|v| v.reason.contains("<form>")));
    }

    #[test]
    fn flags_external_http_src() {
        let v = check_one(r#"<img src="https://cdn.example.com/x.png">"#);
        assert!(v.iter().any(|v| v.reason.contains("external resource URL")));
    }

    #[test]
    fn allows_relative_src() {
        let v = check_one(r#"<img src="images/map.webp">"#);
        assert!(v.iter().all(|v| !v.reason.contains("external resource URL")));
    }

    #[test]
    fn allows_canonical_link_to_external_url() {
        let v = check_one(r#"<link rel="canonical" href="https://example.org/x.html">"#);
        assert!(v.iter().all(|v| !v.reason.contains("external resource URL")));
    }

    #[test]
    fn ignores_meta_tags() {
        let v = check_one(r#"<meta property="og:image" content="https://example.org/x.png">"#);
        assert!(v.iter().all(|v| !v.reason.contains("external resource URL")));
    }
}
