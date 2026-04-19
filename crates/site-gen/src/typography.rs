//! Typographic verification and auto-fix for intreccio content files.
//!
//! Italian typography is minimal: ASCII apostrophe between alphanumerics
//! (the elision context: `l'acqua`, `un'altra`, `dell'Impero`,
//! `quest'anno`, `c'è`) must be the Unicode curly apostrophe U+2019,
//! and three-dot ellipses must be U+2026.
//!
//! ## Scope
//!
//! The verifier currently scans text files (`.md`, `.txt`, `.json`) but
//! not HTML fragments. HTML-aware typography — applying rules only to
//! text nodes, not to tag names or attribute values — is a planned
//! follow-up; for now, author `.html` lessons using typographic
//! apostrophes and ellipses directly.

use std::fmt;
use std::path::{Path, PathBuf};

/// A single typographic rule violation in a content file.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub file: PathBuf,
    pub line: usize,
    pub col: usize,
    pub rule: &'static str,
    pub found: String,
    pub expected: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: [{}] found {}, expected {}",
            self.file.display(),
            self.line,
            self.col,
            self.rule,
            self.found,
            self.expected,
        )
    }
}

/// Language-specific typographic rules.
pub trait TypographyRules {
    /// BCP 47 language code this rule set applies to.
    fn language_code(&self) -> &'static str;

    /// Check a single line of text and return any violations found.
    fn check_line(&self, line: &str, line_number: usize) -> Vec<Violation>;

    /// Apply fixes to a single line of text, returning the corrected version.
    fn fix_line(&self, line: &str) -> String;
}

// ── Italian (it-IT) ─────────────────────────────────────────────────────

/// Italian typographic rules.
///
/// Rules:
/// 1. ASCII apostrophe (U+0027) between two alphanumerics → U+2019.
///    This catches Italian elisions like `l'acqua`, `un'altra`,
///    `dell'Impero`, `c'è`, `d'Italia`, plus incidental English
///    contractions, which are also better typography.
/// 2. Three consecutive dots → ellipsis character (U+2026).
pub struct ItalianTypography;

impl TypographyRules for ItalianTypography {
    fn language_code(&self) -> &'static str {
        "it-IT"
    }

    fn check_line(&self, line: &str, line_number: usize) -> Vec<Violation> {
        let mut violations = Vec::new();

        // Rule 1: ASCII apostrophe between alphanumerics.
        let chars: Vec<(usize, char)> = line.char_indices().collect();
        for (i, (byte_pos, ch)) in chars.iter().enumerate() {
            if *ch != '\'' {
                continue;
            }
            if i == 0 || i + 1 >= chars.len() {
                continue;
            }
            let prev = chars[i - 1].1;
            let next = chars[i + 1].1;
            if prev.is_alphanumeric() && next.is_alphanumeric() {
                let col = line[..*byte_pos].chars().count() + 1;
                violations.push(Violation {
                    file: PathBuf::new(),
                    line: line_number,
                    col,
                    rule: "apostrophe",
                    found: "' (U+0027)".into(),
                    expected: "\u{2019} (U+2019)".into(),
                });
            }
        }

        // Rule 2: Three consecutive dots → ellipsis.
        let mut search_from = 0;
        while let Some(rel) = line[search_from..].find("...") {
            let byte_pos = search_from + rel;
            let before_ok = byte_pos == 0 || line.as_bytes()[byte_pos - 1] != b'.';
            let after_ok =
                byte_pos + 3 >= line.len() || line.as_bytes()[byte_pos + 3] != b'.';
            if before_ok && after_ok {
                let col = line[..byte_pos].chars().count() + 1;
                violations.push(Violation {
                    file: PathBuf::new(),
                    line: line_number,
                    col,
                    rule: "ellipsis",
                    found: "... (three dots)".into(),
                    expected: "\u{2026} (U+2026)".into(),
                });
            }
            search_from = byte_pos + 3;
        }

        violations
    }

    fn fix_line(&self, line: &str) -> String {
        let mut result = String::with_capacity(line.len());
        let chars: Vec<(usize, char)> = line.char_indices().collect();

        let mut i = 0;
        while i < chars.len() {
            let (_, ch) = chars[i];

            // Rule 1: ASCII apostrophe between alphanumerics → U+2019.
            if ch == '\''
                && i > 0
                && i + 1 < chars.len()
                && chars[i - 1].1.is_alphanumeric()
                && chars[i + 1].1.is_alphanumeric()
            {
                result.push('\u{2019}');
                i += 1;
                continue;
            }

            // Rule 2: Three dots → ellipsis, but not four-or-more.
            if ch == '.' && i + 2 < chars.len() && chars[i + 1].1 == '.' && chars[i + 2].1 == '.' {
                let before_ok = i == 0 || chars[i - 1].1 != '.';
                let after_ok = i + 3 >= chars.len() || chars[i + 3].1 != '.';
                if before_ok && after_ok {
                    result.push('\u{2026}');
                    i += 3;
                    continue;
                }
            }

            result.push(ch);
            i += 1;
        }

        result
    }
}

/// Return the typography rules for a given language code, if supported.
pub fn rules_for_language(code: &str) -> Option<Box<dyn TypographyRules>> {
    match code {
        "it-IT" | "it" => Some(Box::new(ItalianTypography)),
        _ => None,
    }
}

// ── File scanning ───────────────────────────────────────────────────────

/// Check all text content files under `content_dir` for typography violations.
///
/// Recursively scans `.md`, `.txt`, and `.json` files. Does not currently
/// scan `.html` files (see the module docs).
pub fn verify_files(
    content_dir: &Path,
    rules: &dyn TypographyRules,
) -> Result<Vec<Violation>, std::io::Error> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_content_files(content_dir, &mut files)?;
    files.sort();

    let mut all = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file)?;
        for (i, line) in text.lines().enumerate() {
            let mut vs = rules.check_line(line, i + 1);
            for v in &mut vs {
                v.file.clone_from(file);
            }
            all.extend(vs);
        }
    }
    Ok(all)
}

/// Fix all content files in place, returning the count of modified files.
pub fn fix_files(
    content_dir: &Path,
    rules: &dyn TypographyRules,
) -> Result<usize, std::io::Error> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_content_files(content_dir, &mut files)?;
    files.sort();

    let mut count = 0;
    for file in &files {
        let original = std::fs::read_to_string(file)?;
        let fixed: String = original
            .lines()
            .map(|line| rules.fix_line(line))
            .collect::<Vec<_>>()
            .join("\n");

        let fixed = if original.ends_with('\n') && !fixed.ends_with('\n') {
            fixed + "\n"
        } else {
            fixed
        };

        if fixed != original {
            std::fs::write(file, &fixed)?;
            count += 1;
        }
    }
    Ok(count)
}

fn collect_content_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_content_files(&path, out)?;
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        if name.ends_with(".md") || name.ends_with(".txt") || name.ends_with(".json") {
            out.push(path);
        }
    }
    Ok(())
}

// ── HTML-aware typography ───────────────────────────────────────────────

/// Apply typography fixes to an HTML source string, touching only text
/// nodes — not tag names, attribute names, or attribute values.
///
/// The implementation is a small state machine: start in "text" mode,
/// switch to "tag" mode at the first `<`, pass everything through
/// unchanged until the matching `>`, then switch back. Fix rules apply
/// only inside the text-mode runs.
///
/// This is a lightweight alternative to full HTML parsing. It handles
/// the well-formed HTML our content produces — the edge cases it can't
/// cope with (apostrophes inside CDATA, script or style blocks with
/// inline content) are already forbidden by the CSP.
pub fn fix_html(html: &str, rules: &dyn TypographyRules) -> String {
    let mut out = String::with_capacity(html.len());
    let bytes = html.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        if bytes[pos] == b'<' {
            // Tag mode: copy through to the matching `>`.
            match html[pos..].find('>') {
                Some(rel_end) => {
                    let end = pos + rel_end + 1;
                    out.push_str(&html[pos..end]);
                    pos = end;
                }
                None => {
                    // Malformed: emit the rest verbatim.
                    out.push_str(&html[pos..]);
                    return out;
                }
            }
        } else {
            // Text mode: apply rules up to the next `<` (or end).
            let next = html[pos..]
                .find('<')
                .map(|i| pos + i)
                .unwrap_or(bytes.len());
            let text = &html[pos..next];
            out.push_str(&rules.fix_line(text));
            pos = next;
        }
    }

    out
}

/// Fix typography in all `.html` files under `content_dir`, touching
/// only text nodes. Returns the count of files modified.
pub fn fix_html_files(
    content_dir: &Path,
    rules: &dyn TypographyRules,
) -> Result<usize, std::io::Error> {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_html_files(content_dir, &mut files)?;
    files.sort();

    let mut count = 0;
    for file in &files {
        let original = std::fs::read_to_string(file)?;
        let fixed = fix_html(&original, rules);
        if fixed != original {
            std::fs::write(file, &fixed)?;
            count += 1;
        }
    }
    Ok(count)
}

fn collect_html_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            collect_html_files(&path, out)?;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) == Some("html") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it() -> ItalianTypography {
        ItalianTypography
    }

    // ── Apostrophe rule ─────────────────────────────────────────────

    #[test]
    fn detects_ascii_apostrophe_in_italian_elision() {
        let v = it().check_line("l'acqua", 1);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "apostrophe");
    }

    #[test]
    fn detects_multiple_elisions_on_one_line() {
        let v = it().check_line("l'acqua e un'altra dell'Impero", 1);
        let apos: Vec<_> = v.iter().filter(|v| v.rule == "apostrophe").collect();
        assert_eq!(apos.len(), 3);
    }

    #[test]
    fn accepts_typographic_apostrophe() {
        let v = it().check_line("l\u{2019}acqua", 1);
        assert!(v.iter().all(|v| v.rule != "apostrophe"));
    }

    #[test]
    fn ignores_apostrophe_not_between_alphanumerics() {
        // Quoted speech: 'hello' — the apostrophes are edge, not between letters.
        let v = it().check_line("'basta'", 1);
        assert!(v.iter().all(|v| v.rule != "apostrophe"));
    }

    // ── Ellipsis rule ───────────────────────────────────────────────

    #[test]
    fn detects_three_dot_ellipsis() {
        let v = it().check_line("Basta...", 1);
        assert_eq!(v.iter().filter(|v| v.rule == "ellipsis").count(), 1);
    }

    #[test]
    fn accepts_ellipsis_character() {
        let v = it().check_line("Basta\u{2026}", 1);
        assert!(v.iter().all(|v| v.rule != "ellipsis"));
    }

    #[test]
    fn ignores_four_or_more_dots() {
        let v = it().check_line("Hmm....", 1);
        assert!(v.iter().all(|v| v.rule != "ellipsis"));
    }

    // ── Fix line ────────────────────────────────────────────────────

    #[test]
    fn fix_replaces_ascii_apostrophe() {
        assert_eq!(it().fix_line("l'acqua"), "l\u{2019}acqua");
    }

    #[test]
    fn fix_replaces_three_dots() {
        assert_eq!(it().fix_line("Basta..."), "Basta\u{2026}");
    }

    #[test]
    fn fix_preserves_quote_apostrophes() {
        assert_eq!(it().fix_line("'basta'"), "'basta'");
    }

    #[test]
    fn fix_handles_multiple_apostrophes() {
        assert_eq!(
            it().fix_line("l'acqua e un'altra dell'Impero"),
            "l\u{2019}acqua e un\u{2019}altra dell\u{2019}Impero",
        );
    }

    #[test]
    fn fix_idempotent() {
        let line = "l\u{2019}acqua dell\u{2019}Impero\u{2026}";
        assert_eq!(it().fix_line(line), line);
    }

    #[test]
    fn fix_preserves_four_dots() {
        assert_eq!(it().fix_line("Hmm...."), "Hmm....");
    }

    // ── File scanning ──────────────────────────────────────────────

    #[test]
    fn fix_files_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.md");
        std::fs::write(&path, "l'acqua...\n").unwrap();

        let rules = ItalianTypography;
        assert_eq!(fix_files(dir.path(), &rules).unwrap(), 1);

        let out = std::fs::read_to_string(&path).unwrap();
        assert_eq!(out, "l\u{2019}acqua\u{2026}\n");

        // Re-running is a no-op.
        assert_eq!(fix_files(dir.path(), &rules).unwrap(), 0);
    }

    #[test]
    fn rules_for_language_lookup() {
        assert!(rules_for_language("it").is_some());
        assert!(rules_for_language("it-IT").is_some());
        assert!(rules_for_language("fr-FR").is_none());
    }

    // ── HTML-aware fixing ──────────────────────────────────────────

    #[test]
    fn fix_html_touches_text_outside_tags() {
        let html = r#"<p>l'acqua è pulita</p>"#;
        let fixed = fix_html(html, &ItalianTypography);
        assert_eq!(fixed, "<p>l\u{2019}acqua è pulita</p>");
    }

    #[test]
    fn fix_html_leaves_attribute_values_alone() {
        // Even if an attribute value contained an ASCII apostrophe
        // (say, inside a data-* or aria-* attr), it must stay intact.
        let html = r#"<span data-it="l'acqua">l'acqua</span>"#;
        let fixed = fix_html(html, &ItalianTypography);
        // The attribute value is untouched (still ASCII apostrophe)...
        assert!(fixed.contains("data-it=\"l'acqua\""));
        // ...while the text node between the tags is fixed.
        assert!(fixed.contains(">l\u{2019}acqua<"));
    }

    #[test]
    fn fix_html_leaves_tag_names_and_attribute_names_alone() {
        let html = r#"<p class="vocab-it"><span lang="it">un'altra</span></p>"#;
        let fixed = fix_html(html, &ItalianTypography);
        assert!(fixed.contains(r#"<p class="vocab-it">"#));
        assert!(fixed.contains(r#"<span lang="it">"#));
        assert!(fixed.contains("un\u{2019}altra"));
    }

    #[test]
    fn fix_html_handles_multiple_apostrophes_across_tags() {
        let html = r#"<p>l'acqua, <span lang="it">dell'Impero</span>, c'è</p>"#;
        let fixed = fix_html(html, &ItalianTypography);
        assert_eq!(fixed.matches('\u{2019}').count(), 3);
        assert!(!fixed.contains("l'acqua"));
        assert!(!fixed.contains("dell'Impero"));
        assert!(!fixed.contains("c'è"));
    }

    #[test]
    fn fix_html_handles_three_dot_ellipsis_in_text() {
        let html = r#"<p>Basta... non importa</p>"#;
        let fixed = fix_html(html, &ItalianTypography);
        assert_eq!(fixed, "<p>Basta\u{2026} non importa</p>");
    }

    #[test]
    fn fix_html_passes_through_malformed_open_tag() {
        // A bare `<` with no closing `>` is copied as-is.
        let html = "plain text < and continues l'acqua";
        let fixed = fix_html(html, &ItalianTypography);
        // Everything up to the bare < is fixed; from there on, verbatim.
        assert!(fixed.starts_with("plain text "));
        // After the malformed tag, nothing gets processed.
        assert!(fixed.contains("l'acqua"));
        // The literal < is preserved.
        assert!(fixed.contains("<"));
    }

    #[test]
    fn fix_html_files_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lesson.html");
        std::fs::write(&path, r#"<p><span lang="it">un'altra volta</span></p>"#).unwrap();

        assert_eq!(fix_html_files(dir.path(), &ItalianTypography).unwrap(), 1);

        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains("un\u{2019}altra"));
        assert!(!out.contains("un'altra"));
        // Tags intact.
        assert!(out.contains(r#"<span lang="it">"#));

        // Re-run is a no-op.
        assert_eq!(fix_html_files(dir.path(), &ItalianTypography).unwrap(), 0);
    }

    #[test]
    fn fix_html_files_skips_non_html() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep.txt"), "l'acqua").unwrap();
        std::fs::write(dir.path().join("edit.html"), "<p>l'acqua</p>").unwrap();

        fix_html_files(dir.path(), &ItalianTypography).unwrap();

        // .txt not touched by fix_html_files (only .html files).
        assert_eq!(
            std::fs::read_to_string(dir.path().join("keep.txt")).unwrap(),
            "l'acqua",
        );
        assert!(
            std::fs::read_to_string(dir.path().join("edit.html"))
                .unwrap()
                .contains("l\u{2019}acqua"),
        );
    }
}
