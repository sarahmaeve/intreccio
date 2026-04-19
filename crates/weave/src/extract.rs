//! Extract foreign-language spans from a weave HTML fragment.

use scraper::{Html, Selector};
use serde::Serialize;

/// A foreign-language span found in a weave fragment.
///
/// The text is the concatenated text content of the span's descendants,
/// with interior whitespace preserved as-is. Callers that want a canonical
/// form for hashing or TTS should pass this through [`normalize_for_hash`]
/// or [`normalize_for_tts`] in the `normalize` module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LangSpan {
    /// BCP 47 language code declared on the span (e.g. `"it"`, `"fr"`).
    pub lang: String,
    /// Raw text content of the span.
    pub text: String,
}

/// Extract every `<span lang="…">` from `html`, in document order.
///
/// Spans with no `lang` attribute are not returned. Nested spans (e.g.
/// a French span inside an Italian one) are each returned independently.
pub fn extract_spans(html: &str) -> Vec<LangSpan> {
    let fragment = Html::parse_fragment(html);
    // `span[lang]` matches any <span> with a `lang` attribute.
    let selector = Selector::parse("span[lang]").expect("static selector");

    fragment
        .select(&selector)
        .map(|el| {
            let lang = el
                .value()
                .attr("lang")
                .unwrap_or_default()
                .to_string();
            let text: String = el.text().collect();
            LangSpan { lang, text }
        })
        .collect()
}

/// Extract only Italian spans, in document order. Convenience wrapper.
pub fn extract_italian_spans(html: &str) -> Vec<LangSpan> {
    extract_spans(html)
        .into_iter()
        .filter(|s| s.lang == "it")
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_italian_and_french_spans_in_order() {
        let html = r#"<p>The <span lang="fr">histoire</span> of <span lang="it">Roma</span> begins with a <span lang="fr">ville</span>.</p>"#;
        let spans = extract_spans(html);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].lang, "fr");
        assert_eq!(spans[0].text, "histoire");
        assert_eq!(spans[1].lang, "it");
        assert_eq!(spans[1].text, "Roma");
        assert_eq!(spans[2].lang, "fr");
        assert_eq!(spans[2].text, "ville");
    }

    #[test]
    fn extracts_multi_word_span_as_single_unit() {
        let html = r#"<p><span lang="it">quasi cinque secoli</span></p>"#;
        let spans = extract_spans(html);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "quasi cinque secoli");
    }

    #[test]
    fn extracts_span_with_interior_punctuation() {
        let html = r#"<p><span lang="it">"tutte le strade portano a Roma,"</span></p>"#;
        let spans = extract_spans(html);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "\"tutte le strade portano a Roma,\"");
    }

    #[test]
    fn extracts_multi_paragraph_fragment() {
        let html = r#"
            <p>First <span lang="it">paragrafo</span>.</p>
            <p>Second <span lang="fr">paragraphe</span>.</p>
        "#;
        let spans = extract_spans(html);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].lang, "it");
        assert_eq!(spans[1].lang, "fr");
    }

    #[test]
    fn extract_italian_spans_filter() {
        let html = r#"<p><span lang="fr">histoire</span> of <span lang="it">Roma</span></p>"#;
        let spans = extract_italian_spans(html);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "Roma");
    }

    #[test]
    fn empty_fragment_yields_no_spans() {
        assert!(extract_spans("").is_empty());
        assert!(extract_spans("<p>Just English</p>").is_empty());
    }

    #[test]
    fn ignores_spans_without_lang() {
        let html = r#"<p><span>no lang</span> <span lang="it">con lang</span></p>"#;
        let spans = extract_spans(html);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "con lang");
    }

    #[test]
    fn handles_preserved_entities() {
        // scraper decodes entities when yielding text.
        let html = r#"<p><span lang="it">l&#x2019;Impero</span></p>"#;
        let spans = extract_spans(html);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "l\u{2019}Impero");
    }
}
