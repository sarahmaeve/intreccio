//! Rewrite Italian spans in a weave fragment to carry `data-audio`
//! attributes pointing at their drill MP3s.
//!
//! The fragment is assumed to be well-formed and to follow DESIGN.md §5's
//! markup convention: foreign-language content is wrapped in
//! `<span lang="…">…</span>`, spans do not nest inside themselves, and
//! all foreign text appears inside a span.
//!
//! We transform by matching `<span lang="it"…>…</span>` with a simple
//! non-greedy byte scan. For each match, the callback produces the
//! `data-audio` URL (typically by hashing the normalized text). The
//! scan preserves the surrounding HTML verbatim — only the opening tag
//! of matched spans is rewritten.

/// Inject `data-audio` attributes into every `<span lang="it">` in `html`.
///
/// The callback is invoked once per Italian span with the span's raw
/// inner text (HTML-escaped entities preserved); it should return the
/// path to write into the `data-audio` attribute, or `None` to skip
/// that span.
pub fn inject_drill_audio<F>(html: &str, mut audio_path: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    let mut out = String::with_capacity(html.len() + 64);
    let bytes = html.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        match find_italian_span_open(html, cursor) {
            Some(tag) => {
                // Copy everything up to the opening tag verbatim.
                out.push_str(&html[cursor..tag.open_start]);

                // Find the matching closing </span>. We assume no nesting
                // of <span> inside <span lang="it">.
                let body_start = tag.open_end;
                match html[body_start..].find("</span>") {
                    Some(close_rel) => {
                        let body_end = body_start + close_rel;
                        let inner = &html[body_start..body_end];
                        match audio_path(inner) {
                            Some(path) => {
                                // Emit the rewritten opening tag.
                                out.push_str(&html[tag.open_start..tag.open_end - 1]);
                                out.push_str(" data-audio=\"");
                                push_attr_escaped(&mut out, &path);
                                out.push('"');
                                out.push('>');
                            }
                            None => {
                                // No drill assigned — emit original opening tag.
                                out.push_str(&html[tag.open_start..tag.open_end]);
                            }
                        }
                        // Emit body + closing tag verbatim.
                        out.push_str(inner);
                        out.push_str("</span>");
                        cursor = body_end + "</span>".len();
                    }
                    None => {
                        // Malformed: no closing tag. Emit the rest verbatim.
                        out.push_str(&html[tag.open_start..]);
                        return out;
                    }
                }
            }
            None => {
                // No more Italian spans — emit the tail.
                out.push_str(&html[cursor..]);
                return out;
            }
        }
    }

    out
}

struct OpenTag {
    /// Byte offset of the leading `<`.
    open_start: usize,
    /// Byte offset just past the closing `>`.
    open_end: usize,
}

/// Find the next `<span … lang="it" …>` opening tag at or after `from`.
///
/// Looks for the substring `<span` followed, before the next `>`, by
/// `lang="it"` (exact attribute form, no single-quote support — we
/// author spans with double-quoted attributes by convention).
fn find_italian_span_open(html: &str, from: usize) -> Option<OpenTag> {
    let mut cursor = from;
    while let Some(rel) = html[cursor..].find("<span") {
        let open_start = cursor + rel;
        let open_end_rel = html[open_start..].find('>')?;
        let open_end = open_start + open_end_rel + 1;
        let tag = &html[open_start..open_end];

        // Match only spans whose lang attribute is explicitly "it".
        // Accept any attribute ordering, but require the literal
        // `lang="it"` to appear (no escaped quotes, no interior whitespace).
        if tag.contains(r#"lang="it""#) {
            return Some(OpenTag { open_start, open_end });
        }
        cursor = open_end;
    }
    None
}

/// Append `value` to `out` with the minimum escaping needed for a
/// double-quoted HTML attribute value.
fn push_attr_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_data_audio_into_single_italian_span() {
        let html = r#"<p>Hello <span lang="it">Roma</span> world.</p>"#;
        let out = inject_drill_audio(html, |_| Some("audio/drills/abc.mp3".to_string()));
        assert_eq!(
            out,
            r#"<p>Hello <span lang="it" data-audio="audio/drills/abc.mp3">Roma</span> world.</p>"#,
        );
    }

    #[test]
    fn leaves_french_spans_untouched() {
        let html = r#"<p><span lang="fr">histoire</span> of <span lang="it">Roma</span></p>"#;
        let out = inject_drill_audio(html, |_| Some("a.mp3".to_string()));
        assert!(out.contains(r#"<span lang="fr">histoire</span>"#));
        assert!(out.contains(r#"<span lang="it" data-audio="a.mp3">Roma</span>"#));
    }

    #[test]
    fn uses_callback_text_for_each_span() {
        let html = r#"<p><span lang="it">Roma</span> e <span lang="it">Venezia</span></p>"#;
        let out = inject_drill_audio(html, |text| {
            Some(format!("audio/{}.mp3", text.to_lowercase()))
        });
        assert!(out.contains(r#"data-audio="audio/roma.mp3""#));
        assert!(out.contains(r#"data-audio="audio/venezia.mp3""#));
    }

    #[test]
    fn skips_span_when_callback_returns_none() {
        let html = r#"<p><span lang="it">Roma</span></p>"#;
        let out = inject_drill_audio(html, |_| None);
        assert_eq!(out, html);
    }

    #[test]
    fn preserves_other_attributes_on_the_span() {
        let html = r#"<p><span class="place" lang="it">Roma</span></p>"#;
        let out = inject_drill_audio(html, |_| Some("r.mp3".to_string()));
        assert_eq!(
            out,
            r#"<p><span class="place" lang="it" data-audio="r.mp3">Roma</span></p>"#,
        );
    }

    #[test]
    fn does_not_match_spans_with_other_lang_values() {
        let html = r#"<p><span lang="es">casa</span></p>"#;
        let out = inject_drill_audio(html, |_| Some("x.mp3".to_string()));
        assert_eq!(out, html);
    }

    #[test]
    fn handles_malformed_fragment_without_panic() {
        let html = r#"<p><span lang="it">never closed"#;
        let out = inject_drill_audio(html, |_| Some("x.mp3".to_string()));
        // With no </span>, we emit the tail verbatim starting from the open tag.
        assert!(out.contains(r#"<span lang="it">never closed"#));
    }

    #[test]
    fn escapes_quotes_and_ampersands_in_path() {
        let html = r#"<p><span lang="it">x</span></p>"#;
        let out = inject_drill_audio(html, |_| Some(r#"a"&b.mp3"#.to_string()));
        assert!(out.contains(r#"data-audio="a&quot;&amp;b.mp3""#));
    }

    #[test]
    fn multiple_italian_spans_in_one_paragraph() {
        let html = r#"<p><span lang="it">il Rinascimento</span> e <span lang="it">il Risorgimento</span></p>"#;
        let mut counter = 0;
        let out = inject_drill_audio(html, |_| {
            counter += 1;
            Some(format!("drill-{counter}.mp3"))
        });
        assert!(out.contains(r#"data-audio="drill-1.mp3""#));
        assert!(out.contains(r#"data-audio="drill-2.mp3""#));
    }
}
