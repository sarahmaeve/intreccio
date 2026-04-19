//! Count words per language in a weave fragment to verify a lesson's
//! declared language shares against DESIGN.md §4.
//!
//! Counting rules (from DESIGN.md §9):
//!
//! - Unmarked text (outside any `lang` span) counts as English.
//! - Text inside `<span lang="fr">` counts as French.
//! - Text inside `<span lang="it">` counts as Italian.
//! - Hyphenated compounds count as one word.
//! - Proper nouns count in the language of their surrounding span; when
//!   outside any span, they count as English.
//!
//! The implementation walks the parsed DOM, descending into children and
//! attributing text nodes to whichever ancestor span (if any) most recently
//! declared a `lang`.

use std::collections::HashMap;

use ego_tree::NodeRef;
use scraper::{node::Node, Html};
use serde::Serialize;

/// Word counts per language in a weave fragment.
///
/// Keys are BCP 47 language codes (`"en"`, `"fr"`, `"it"`). Values are
/// word counts. Languages with zero words are omitted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LangShares {
    pub counts: HashMap<String, usize>,
}

impl LangShares {
    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }

    pub fn ratio(&self, lang: &str) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.counts.get(lang).copied().unwrap_or(0) as f64 / total as f64
        }
    }

    pub fn get(&self, lang: &str) -> usize {
        self.counts.get(lang).copied().unwrap_or(0)
    }
}

/// Count words per language in a weave HTML fragment.
///
/// See the module documentation for the counting rules.
pub fn count_words_by_lang(html: &str) -> LangShares {
    let fragment = Html::parse_fragment(html);
    let root = fragment.tree.root();
    let mut counts: HashMap<String, usize> = HashMap::new();

    // Start with an implicit outer language of "en" (unmarked text).
    visit(root, "en", &mut counts);

    LangShares { counts }
}

fn visit<'a>(
    node: NodeRef<'a, Node>,
    current_lang: &str,
    counts: &mut HashMap<String, usize>,
) {
    match node.value() {
        Node::Text(text) => {
            let n = count_words(&text[..]);
            if n > 0 {
                *counts.entry(current_lang.to_string()).or_insert(0) += n;
            }
        }
        Node::Element(el) => {
            // If this element declares a `lang`, text beneath it is attributed
            // to that language (until a deeper element overrides it).
            let lang = el.attr("lang").unwrap_or(current_lang);
            for child in node.children() {
                visit(child, lang, counts);
            }
        }
        _ => {
            // Document, comment, etc. — recurse but don't change lang.
            for child in node.children() {
                visit(child, current_lang, counts);
            }
        }
    }
}

/// Count words in a text run. Hyphenated compounds count as one word.
///
/// A word is a maximal run of alphabetic characters (Unicode-aware),
/// optionally joined by apostrophes or hyphens to more alphabetic
/// characters. Digits and punctuation do not form words.
fn count_words(text: &str) -> usize {
    let mut count = 0;
    let mut in_word = false;

    for ch in text.chars() {
        if ch.is_alphabetic() {
            if !in_word {
                count += 1;
                in_word = true;
            }
        } else if matches!(ch, '-' | '\'' | '\u{2019}') {
            // Hyphens and apostrophes extend the current word rather than
            // terminating it. If we're not currently in a word, they
            // don't start one (e.g. `—` followed by space).
        } else {
            in_word = false;
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_all_english_when_no_spans() {
        let shares = count_words_by_lang("<p>The Roman Empire begins with a small city.</p>");
        assert_eq!(shares.get("en"), 8);
        assert_eq!(shares.get("it"), 0);
        assert_eq!(shares.total(), 8);
    }

    #[test]
    fn counts_mixed_languages() {
        let html = r#"<p>The <span lang="fr">histoire</span> of <span lang="it">Roma</span>.</p>"#;
        let shares = count_words_by_lang(html);
        assert_eq!(shares.get("en"), 2); // "The", "of"
        assert_eq!(shares.get("fr"), 1); // "histoire"
        assert_eq!(shares.get("it"), 1); // "Roma"
    }

    #[test]
    fn multi_word_spans_count_each_word() {
        let html = r#"<p><span lang="it">quasi cinque secoli</span></p>"#;
        let shares = count_words_by_lang(html);
        assert_eq!(shares.get("it"), 3);
    }

    #[test]
    fn hyphenated_compound_counts_as_one_word() {
        let shares = count_words_by_lang("<p>état-nation</p>");
        assert_eq!(shares.get("en"), 1);
    }

    #[test]
    fn apostrophe_does_not_split_a_word() {
        // Typographic apostrophe inside a word: `l'acqua` is one word.
        let html = "<p><span lang=\"it\">l\u{2019}acqua</span></p>";
        let shares = count_words_by_lang(html);
        assert_eq!(shares.get("it"), 1);
    }

    #[test]
    fn numbers_and_punctuation_do_not_count() {
        // Digits are not alphabetic, so "753" does not count as a word.
        let shares = count_words_by_lang("<p>753 years of history</p>");
        // "years", "of", "history" → 3 words.
        assert_eq!(shares.get("en"), 3);
    }

    #[test]
    fn ratio_reports_fractional_share() {
        let html = r#"<p>The <span lang="it">Roma</span> story.</p>"#;
        let shares = count_words_by_lang(html);
        // 3 total words: "The", "Roma", "story".
        assert!((shares.ratio("it") - (1.0 / 3.0)).abs() < 1e-9);
        assert!((shares.ratio("en") - (2.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn ratio_of_empty_fragment_is_zero() {
        let shares = count_words_by_lang("");
        assert_eq!(shares.total(), 0);
        assert_eq!(shares.ratio("it"), 0.0);
    }

    #[test]
    fn nested_span_takes_inner_lang() {
        // A French gloss inside an Italian span: the French text is French.
        let html = r#"<p><span lang="it">il Colosseo <span lang="fr">l'amphithéâtre</span></span></p>"#;
        let shares = count_words_by_lang(html);
        assert_eq!(shares.get("it"), 2); // "il", "Colosseo"
        assert_eq!(shares.get("fr"), 1); // "l'amphithéâtre" (hyphen-joined by apostrophe)
    }

    #[test]
    fn multi_paragraph_fragment() {
        let html = r#"
            <p>The <span lang="it">Rinascimento</span>.</p>
            <p>Then <span lang="it">il Risorgimento</span>.</p>
        "#;
        let shares = count_words_by_lang(html);
        assert_eq!(shares.get("en"), 2); // "The", "Then"
        assert_eq!(shares.get("it"), 3); // "Rinascimento", "il", "Risorgimento"
    }
}
