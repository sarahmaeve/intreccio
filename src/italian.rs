//! Italian voices and Roman-numeral pre-processing for TTS.
//!
//! # Voice selection
//!
//! Two Italian Chirp3-HD female voices form the drill pool. The voice
//! for a given span is chosen deterministically from the BLAKE3 hash of
//! its normalized text (first byte modulo the pool size), so the same
//! Italian phrase always produces the same voice — regenerating an MP3
//! is a no-op and does not churn the git history.
//!
//! The first-generation `it-IT-Chirp-HD-O` voice was in the pool
//! originally but was removed: it produced audibly garbled output on
//! simple two-syllable Italian nouns (e.g. `mano`, `meno`), which is
//! the exact sort of content a drill pool needs to pronounce cleanly.
//! Chirp3-HD voices are second-generation and don't have that failure
//! mode in our testing.
//!
//! # Roman-numeral expansion
//!
//! Google Cloud TTS will read a standalone Roman numeral like `XIII`
//! letter-by-letter ("ex-eye-eye-eye"), which is not what you want.
//! [`normalize_roman_numerals`] expands Roman-numeral tokens to their
//! Italian masculine ordinal form (`XIII` → `tredicesimo`) before
//! synthesis. Parenthetical `(XV)` reader aids are already stripped
//! upstream by `weave::normalize::normalize_for_tts`.
//!
//! Scope:
//!
//! - Expands tokens of length ≥ 2 that match a canonical Roman numeral
//!   from `II` through `XXV` (25). Single-character tokens (`I`, `V`,
//!   `X`) are intentionally skipped to avoid corrupting `I Medici`
//!   (plural definite article) into `primo Medici`.
//! - Italian ordinal forms agree in gender with their noun; the pool
//!   uses the masculine form (`tredicesimo`, not `tredicesima`).
//!   Content authors who need feminine agreement — e.g. `la XIII
//!   legione` → `la tredicesima legione` — should spell the ordinal
//!   out manually in the span.

use crate::tts::Voice;

// ── Drill voice pool ────────────────────────────────────────────────────

/// The Italian voices used for drill audio.
///
/// Both are Chirp3-HD female voices. Picking one voice family keeps
/// register consistent across drills; rotating within it gives enough
/// timbre variety that consecutive drills don't sound like a
/// monologue.
pub const DRILL_VOICES: [Voice; 2] = [
    Voice { language_code: "it-IT", name: "it-IT-Chirp3-HD-Aoede" },
    Voice { language_code: "it-IT", name: "it-IT-Chirp3-HD-Erinome" },
];

/// Pick a drill voice for `text` deterministically from its BLAKE3 hash.
///
/// Same input text → same voice, always. This is what makes regenerating
/// drill MP3s a no-op with respect to voice assignment.
pub fn voice_for_text(text: &str) -> &'static Voice {
    let hash = blake3::hash(text.as_bytes());
    let idx = hash.as_bytes()[0] as usize % DRILL_VOICES.len();
    &DRILL_VOICES[idx]
}

// ── Roman-numeral expansion ─────────────────────────────────────────────

/// Italian masculine ordinal forms, indexed by value (index 0 unused).
const ITALIAN_ORDINALS: [&str; 26] = [
    "",                    // 0 — unused
    "primo",               // 1
    "secondo",             // 2
    "terzo",               // 3
    "quarto",              // 4
    "quinto",              // 5
    "sesto",               // 6
    "settimo",             // 7
    "ottavo",              // 8
    "nono",                // 9
    "decimo",              // 10
    "undicesimo",          // 11
    "dodicesimo",          // 12
    "tredicesimo",         // 13
    "quattordicesimo",     // 14
    "quindicesimo",        // 15
    "sedicesimo",          // 16
    "diciassettesimo",     // 17
    "diciottesimo",        // 18
    "diciannovesimo",      // 19
    "ventesimo",           // 20
    "ventunesimo",         // 21
    "ventiduesimo",        // 22
    "ventitreesimo",       // 23
    "ventiquattresimo",    // 24
    "venticinquesimo",     // 25
];

/// Parse a canonical Roman numeral string (II through XXV) to its value.
fn parse_roman(s: &str) -> Option<u32> {
    match s {
        "II" => Some(2),
        "III" => Some(3),
        "IV" => Some(4),
        "V" => Some(5),
        "VI" => Some(6),
        "VII" => Some(7),
        "VIII" => Some(8),
        "IX" => Some(9),
        "X" => Some(10),
        "XI" => Some(11),
        "XII" => Some(12),
        "XIII" => Some(13),
        "XIV" => Some(14),
        "XV" => Some(15),
        "XVI" => Some(16),
        "XVII" => Some(17),
        "XVIII" => Some(18),
        "XIX" => Some(19),
        "XX" => Some(20),
        "XXI" => Some(21),
        "XXII" => Some(22),
        "XXIII" => Some(23),
        "XXIV" => Some(24),
        "XXV" => Some(25),
        _ => None,
    }
}

/// Replace Roman-numeral tokens in `text` with their Italian masculine
/// ordinal forms. Preserves surrounding whitespace and trailing
/// punctuation.
///
/// See the module documentation for scope and limitations.
pub fn normalize_roman_numerals(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());

    for word in &words {
        let (core, trailing) = split_trailing_non_alnum(word);
        if core.len() >= 2 {
            if let Some(num) = parse_roman(core) {
                if let Some(ordinal) = ITALIAN_ORDINALS.get(num as usize) {
                    out.push(format!("{ordinal}{trailing}"));
                    continue;
                }
            }
        }
        out.push((*word).to_string());
    }

    // Preserve the original whitespace structure approximately by joining
    // on single spaces. Multi-space runs are collapsed to one space — an
    // acceptable loss for TTS input (the voice doesn't distinguish).
    out.join(" ")
}

// ── Article-aware lexical helpers ───────────────────────────────────────
//
// Used by the vocabolario aggregator to collapse entries that differ
// only in whether they carry a leading article: `pasta`, `la pasta`,
// and `una pasta` share a bare form and should appear once on the
// page, with the highest-priority article-bearing variant preferred.

/// Leading Italian articles that are whole words, followed by a space.
const ARTICLES_WITH_SPACE_DEFINITE: &[&str] = &[
    "il ", "lo ", "la ", "i ", "gli ", "le ",
];
const ARTICLES_WITH_SPACE_INDEFINITE: &[&str] = &[
    "un ", "uno ", "una ",
];

/// Elided articles that run directly into the next word without a space.
const ARTICLES_ELIDED_DEFINITE: &[&str] = &["l'", "l\u{2019}"];
const ARTICLES_ELIDED_INDEFINITE: &[&str] = &["un'", "un\u{2019}"];

/// Return the "bare form" of an Italian phrase: the phrase with its
/// leading article removed (if any), lowercased for comparison.
///
/// Used to recognize that `pasta`, `la pasta`, and `La pasta` share a
/// common headword so that the vocabolario page can present only one
/// of them.
pub fn bare_form(text: &str) -> String {
    let lower = text.to_lowercase();
    for prefix in ARTICLES_WITH_SPACE_DEFINITE
        .iter()
        .chain(ARTICLES_WITH_SPACE_INDEFINITE.iter())
    {
        if let Some(rest) = lower.strip_prefix(prefix) {
            return rest.to_string();
        }
    }
    for prefix in ARTICLES_ELIDED_DEFINITE
        .iter()
        .chain(ARTICLES_ELIDED_INDEFINITE.iter())
    {
        if let Some(rest) = lower.strip_prefix(prefix) {
            return rest.to_string();
        }
    }
    lower
}

/// Priority a vocab entry has when two entries share a bare form.
///
/// - `3` — entry starts with a definite article (`il libro`, `la casa`,
///   `l'acqua`). Pedagogically the most useful form because it reveals
///   gender; this is the dictionary convention for Italian.
/// - `2` — entry starts with an indefinite article (`un libro`,
///   `un'amica`). Also reveals gender, just less conventionally.
/// - `1` — bare headword (`libro`, `casa`, `acqua`). Fallback.
pub fn article_priority(text: &str) -> u32 {
    let lower = text.to_lowercase();
    for prefix in ARTICLES_WITH_SPACE_DEFINITE {
        if lower.starts_with(prefix) {
            return 3;
        }
    }
    for prefix in ARTICLES_ELIDED_DEFINITE {
        if lower.starts_with(prefix) {
            return 3;
        }
    }
    for prefix in ARTICLES_WITH_SPACE_INDEFINITE {
        if lower.starts_with(prefix) {
            return 2;
        }
    }
    for prefix in ARTICLES_ELIDED_INDEFINITE {
        if lower.starts_with(prefix) {
            return 2;
        }
    }
    1
}

/// Split a word into `(core, trailing_punct)` by finding the last
/// alphanumeric character. If the word has no alphanumeric character,
/// returns `(word, "")`.
fn split_trailing_non_alnum(word: &str) -> (&str, &str) {
    match word.rfind(|c: char| c.is_alphanumeric()) {
        Some(last_alnum_start) => {
            // Compute the byte index just past the last alphanumeric char.
            let end = last_alnum_start
                + word[last_alnum_start..]
                    .chars()
                    .next()
                    .map(char::len_utf8)
                    .unwrap_or(0);
            (&word[..end], &word[end..])
        }
        None => (word, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Voice selection ─────────────────────────────────────────────

    #[test]
    fn voice_selection_is_deterministic() {
        let v1 = voice_for_text("la storia");
        let v2 = voice_for_text("la storia");
        assert_eq!(v1.name, v2.name);
    }

    #[test]
    fn voice_selection_varies_across_texts() {
        // It's possible by luck that several distinct texts all hash to
        // the same voice, but the pool is only 3 deep — across a dozen
        // samples we should see at least two voices picked.
        let samples = [
            "la storia", "l'acqua", "la famiglia", "Roma",
            "il Rinascimento", "il Colosseo", "la Divina Commedia",
            "tutte le strade portano a Roma", "il Risorgimento",
            "il Medioevo", "Firenze", "Venezia",
        ];
        let voices: std::collections::HashSet<_> =
            samples.iter().map(|s| voice_for_text(s).name).collect();
        assert!(
            voices.len() >= 2,
            "expected voice rotation across {} samples; saw only one voice ({:?})",
            samples.len(),
            voices,
        );
    }

    #[test]
    fn all_drill_voices_are_italian() {
        for v in &DRILL_VOICES {
            assert_eq!(v.language_code, "it-IT");
            assert!(v.name.starts_with("it-IT-"));
        }
    }

    // ── Roman-numeral expansion ─────────────────────────────────────

    #[test]
    fn expands_standalone_century() {
        assert_eq!(
            normalize_roman_numerals("il XIII secolo"),
            "il tredicesimo secolo",
        );
    }

    #[test]
    fn expands_all_centuries_11_through_21() {
        for (roman, ordinal) in [
            ("XI", "undicesimo"),
            ("XII", "dodicesimo"),
            ("XIII", "tredicesimo"),
            ("XIV", "quattordicesimo"),
            ("XV", "quindicesimo"),
            ("XVI", "sedicesimo"),
            ("XVII", "diciassettesimo"),
            ("XVIII", "diciottesimo"),
            ("XIX", "diciannovesimo"),
            ("XX", "ventesimo"),
            ("XXI", "ventunesimo"),
        ] {
            let input = format!("nel {roman} secolo");
            let expected = format!("nel {ordinal} secolo");
            assert_eq!(normalize_roman_numerals(&input), expected);
        }
    }

    #[test]
    fn expands_royal_ordinals() {
        assert_eq!(
            normalize_roman_numerals("Vittorio Emanuele II"),
            "Vittorio Emanuele secondo",
        );
        assert_eq!(
            normalize_roman_numerals("Papa Paolo VI"),
            "Papa Paolo sesto",
        );
        assert_eq!(
            normalize_roman_numerals("Giovanni XXIII"),
            "Giovanni ventitreesimo",
        );
    }

    #[test]
    fn preserves_trailing_comma_and_period() {
        assert_eq!(
            normalize_roman_numerals("nel XIII, il Medioevo"),
            "nel tredicesimo, il Medioevo",
        );
        assert_eq!(
            normalize_roman_numerals("Vittorio Emanuele II."),
            "Vittorio Emanuele secondo.",
        );
    }

    #[test]
    fn does_not_expand_single_character_i() {
        // "I Medici" — capital-I plural definite article, not a numeral.
        assert_eq!(
            normalize_roman_numerals("I Medici erano ricchi"),
            "I Medici erano ricchi",
        );
    }

    #[test]
    fn does_not_expand_single_character_v_or_x() {
        assert_eq!(normalize_roman_numerals("V come vittoria"), "V come vittoria");
        assert_eq!(normalize_roman_numerals("X marca il punto"), "X marca il punto");
    }

    #[test]
    fn does_not_expand_out_of_range_numerals() {
        // XXVI (26) and above are not in our table.
        assert_eq!(
            normalize_roman_numerals("il XXVI secolo"),
            "il XXVI secolo",
        );
        // MCMLXIV (1964) — way out of range.
        assert_eq!(
            normalize_roman_numerals("anno MCMLXIV"),
            "anno MCMLXIV",
        );
    }

    #[test]
    fn does_not_expand_non_roman_uppercase_tokens() {
        // "CD" in Italian tech writing = compact disc.
        assert_eq!(normalize_roman_numerals("un CD musicale"), "un CD musicale");
        // Random initials.
        assert_eq!(normalize_roman_numerals("la sigla RAI"), "la sigla RAI");
    }

    #[test]
    fn multiple_numerals_in_one_phrase() {
        assert_eq!(
            normalize_roman_numerals("nel XIII e XIV secolo"),
            "nel tredicesimo e quattordicesimo secolo",
        );
    }

    #[test]
    fn preserves_surrounding_whitespace_through_join() {
        // We collapse multi-space runs to single spaces — acceptable for
        // TTS input. This test documents the behavior.
        assert_eq!(
            normalize_roman_numerals("nel  XIII  secolo"),
            "nel tredicesimo secolo",
        );
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(normalize_roman_numerals(""), "");
    }

    #[test]
    fn text_without_numerals_is_unchanged() {
        assert_eq!(
            normalize_roman_numerals("la bella figura"),
            "la bella figura",
        );
    }

    // ── split_trailing_non_alnum ────────────────────────────────────

    #[test]
    fn split_trailing_handles_common_cases() {
        assert_eq!(split_trailing_non_alnum("XIII"), ("XIII", ""));
        assert_eq!(split_trailing_non_alnum("XIII,"), ("XIII", ","));
        assert_eq!(split_trailing_non_alnum("XIII."), ("XIII", "."));
        assert_eq!(split_trailing_non_alnum("XIII);"), ("XIII", ");"));
        assert_eq!(split_trailing_non_alnum(""), ("", ""));
        assert_eq!(split_trailing_non_alnum(".,!"), (".,!", ""));
    }

    // ── bare_form ───────────────────────────────────────────────────

    #[test]
    fn bare_form_strips_definite_articles_with_space() {
        assert_eq!(bare_form("il libro"), "libro");
        assert_eq!(bare_form("la casa"), "casa");
        assert_eq!(bare_form("lo studente"), "studente");
        assert_eq!(bare_form("i libri"), "libri");
        assert_eq!(bare_form("gli amici"), "amici");
        assert_eq!(bare_form("le case"), "case");
    }

    #[test]
    fn bare_form_strips_indefinite_articles_with_space() {
        assert_eq!(bare_form("un libro"), "libro");
        assert_eq!(bare_form("uno studente"), "studente");
        assert_eq!(bare_form("una casa"), "casa");
    }

    #[test]
    fn bare_form_strips_elided_articles_typographic_and_ascii() {
        // typographic apostrophe
        assert_eq!(bare_form("l\u{2019}acqua"), "acqua");
        assert_eq!(bare_form("un\u{2019}amica"), "amica");
        // ASCII apostrophe
        assert_eq!(bare_form("l'acqua"), "acqua");
        assert_eq!(bare_form("un'amica"), "amica");
    }

    #[test]
    fn bare_form_returns_lowercased_when_no_article() {
        assert_eq!(bare_form("pasta"), "pasta");
        assert_eq!(bare_form("Pasta"), "pasta");
        assert_eq!(bare_form("ROMA"), "roma");
    }

    #[test]
    fn bare_form_treats_mixed_case_articles() {
        // Sentence-initial capitalization shouldn't hide the article.
        assert_eq!(bare_form("La casa"), "casa");
        assert_eq!(bare_form("Un libro"), "libro");
    }

    #[test]
    fn bare_form_preserves_multi_word_remainder() {
        assert_eq!(bare_form("la pasta secca"), "pasta secca");
        assert_eq!(bare_form("l'olio d'oliva"), "olio d'oliva");
    }

    #[test]
    fn bare_form_does_not_strip_non_article_prefixes() {
        // `del`, `alla`, `nel` etc. are prepositions or contractions,
        // not articles — bare_form leaves them intact.
        assert_eq!(bare_form("del pane"), "del pane");
        assert_eq!(bare_form("alla milanese"), "alla milanese");
        assert_eq!(bare_form("nel forno"), "nel forno");
    }

    // ── article_priority ────────────────────────────────────────────

    #[test]
    fn article_priority_definite_is_3() {
        for t in &[
            "il libro",
            "la casa",
            "lo studente",
            "i libri",
            "gli amici",
            "le case",
            "l'acqua",
            "l\u{2019}acqua",
        ] {
            assert_eq!(article_priority(t), 3, "definite article priority for {t:?}");
        }
    }

    #[test]
    fn article_priority_indefinite_is_2() {
        for t in &[
            "un libro",
            "uno studente",
            "una casa",
            "un'amica",
            "un\u{2019}amica",
        ] {
            assert_eq!(article_priority(t), 2, "indefinite article priority for {t:?}");
        }
    }

    #[test]
    fn article_priority_bare_is_1() {
        for t in &["pasta", "Roma", "carbonara", "del pane"] {
            assert_eq!(article_priority(t), 1, "bare priority for {t:?}");
        }
    }
}
