//! Canonicalize span text for content-addressed hashing and TTS input.
//!
//! Two distinct normalizations:
//!
//! - [`normalize_for_hash`] — produce a stable canonical form of a span's
//!   text so that cosmetic differences (trailing punctuation, whitespace,
//!   quote marks) don't create spurious new drill files. This is the form
//!   hashed by BLAKE3 to produce the `audio/drills/<hash>.mp3` filename.
//!
//! - [`normalize_for_tts`] — produce the string that actually gets
//!   synthesized. Strips parenthetical Roman-numeral reader aids (e.g.
//!   `quindicesimo (XV)` → `quindicesimo`) so the voice doesn't speak
//!   the numeral redundantly. Intended for Italian input.
//!
//! Roman-numeral *expansion* (turning standalone `XV secolo` into
//! `quindicesimo secolo`) is language-specific and lives in the root
//! binary's `italian` module, not here.
//!
//! [`is_drillable`] encodes the project's drill-eligibility policy:
//! spans shorter than [`MIN_DRILL_LENGTH`] characters don't get MP3s.
//! Function words like `e`, `il`, `la`, `da`, `in` have predictable
//! pronunciation and cluttering the drill cache with them wastes API
//! calls and visual affordance.

/// Canonicalize a span's text for content-addressed hashing.
///
/// - Trims leading and trailing whitespace.
/// - Strips leading and trailing quotation marks (`"` `'` `«` `»` `"` `"` `'` `'`).
/// - Strips trailing sentence-ending punctuation (`,` `.` `;` `:` `!` `?`).
/// - Collapses interior runs of whitespace to single ASCII spaces.
///
/// The input is returned if normalization produces an empty string, to
/// avoid accidentally hashing the empty string for pure-punctuation spans.
pub fn normalize_for_hash(text: &str) -> String {
    let collapsed = collapse_whitespace(text);
    let trimmed = collapsed
        .trim_matches(is_edge_punctuation)
        .trim()
        .to_string();

    // Apostrophe canonicalization: U+2019 (typographic) and U+0027 (ASCII)
    // both flatten to ASCII for hashing. Two consequences:
    //   1. A typography pass that replaces ASCII `'` with U+2019 inside
    //      spans does NOT invalidate already-synthesized drill MP3s —
    //      their filenames stay the same across the orthographic cleanup.
    //   2. Both forms produce identical drill audio, which is correct:
    //      they represent the same phoneme sequence, and the Google Cloud
    //      TTS API treats them equivalently.
    let canonical = trimmed.replace('\u{2019}', "'");

    if canonical.is_empty() {
        collapsed
    } else {
        canonical
    }
}

/// Canonicalize a span's text for TTS synthesis.
///
/// On top of the hash normalization, this also:
///
/// - Strips parenthetical Roman numerals that follow a word, e.g.
///   `il quindicesimo (XV) secolo` → `il quindicesimo secolo`.
///   The parenthetical is a reader aid; the TTS voice shouldn't repeat
///   the numeral after already speaking the spelled-out ordinal.
///
/// Callers that want the TTS to *expand* standalone Roman numerals
/// (e.g. `XV secolo` → `quindicesimo secolo`) must do so in a
/// language-specific pre-processor before calling the TTS API.
pub fn normalize_for_tts(text: &str) -> String {
    let hash_form = normalize_for_hash(text);
    let stripped = strip_parenthetical_roman_numerals(&hash_form);
    // The hash form uses ASCII apostrophes for stability. Convert back
    // to typographic U+2019 before handing the string to the TTS API,
    // so the voice receives well-typeset Italian — matches what the
    // rendered HTML shows the reader.
    stripped.replace('\'', "\u{2019}")
}

/// Collapse all runs of Unicode whitespace to single ASCII spaces.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = true;
        } else {
            out.push(ch);
            in_ws = false;
        }
    }
    // `out` may have a trailing space if the input ended with whitespace.
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Punctuation characters that, when they bracket a span, are cosmetic
/// and should not contribute to the canonical form.
fn is_edge_punctuation(c: char) -> bool {
    matches!(
        c,
        // Quotation marks: ASCII, typographic, guillemets.
        '"' | '\''
            | '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}'
            | '\u{00AB}' | '\u{00BB}'
            // Sentence-ending punctuation.
            | ',' | '.' | ';' | ':' | '!' | '?'
            // Ellipsis.
            | '\u{2026}'
    )
}

/// Strip `(XV)`, `(IX)`, etc. that follow a word. The parenthetical must
/// contain only Roman-numeral characters (I, V, X, L, C, D, M) and must
/// be preceded by whitespace.
fn strip_parenthetical_roman_numerals(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'(' && i > 0 && bytes[i - 1] == b' ' {
            // Scan forward to `)`.
            if let Some(close_rel) = s[i + 1..].find(')') {
                let inner = &s[i + 1..i + 1 + close_rel];
                if !inner.is_empty() && inner.chars().all(is_roman_numeral_char) {
                    // Drop the preceding space as well — "word (XV)" → "word".
                    if out.ends_with(' ') {
                        out.pop();
                    }
                    // Skip past `(XV)`.
                    i = i + 1 + close_rel + 1;
                    continue;
                }
            }
        }
        // Default: copy the byte. Safe because the check above only
        // matches ASCII characters that are single-byte in UTF-8.
        out.push(bytes[i] as char);
        i += 1;
    }

    out
}

fn is_roman_numeral_char(c: char) -> bool {
    matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M')
}

// ── Drill-eligibility policy ────────────────────────────────────────────

/// Minimum canonical length (in Unicode scalar values) for a span to
/// warrant drill audio.
///
/// Spans shorter than this are skipped by both the build-time
/// `data-audio` injector and the drill synthesizer. The threshold is
/// a pedagogical judgement: tiny function words like `e` (and), `il`
/// (the), `da` (from), `in` (in) have predictable pronunciation and
/// don't benefit from a drill MP3.
pub const MIN_DRILL_LENGTH: usize = 3;

/// Return `true` if a span's canonical text is long enough to warrant
/// a drill MP3.
///
/// Counts Unicode scalar values, so accented letters count as one
/// character. The input should already be canonicalized via
/// [`normalize_for_hash`] — whitespace and edge punctuation collapsed.
///
/// Examples: `"e"` → `false`, `"è"` → `false`, `"da"` → `false`,
/// `"c'è"` → `true` (apostrophe counts as a character), `"chi"` →
/// `true`, `"Roma"` → `true`.
pub fn is_drillable(canonical: &str) -> bool {
    canonical.chars().count() >= MIN_DRILL_LENGTH
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── normalize_for_hash ──────────────────────────────────────────────

    #[test]
    fn hash_trims_leading_and_trailing_whitespace() {
        assert_eq!(normalize_for_hash("  la storia  "), "la storia");
    }

    #[test]
    fn hash_strips_trailing_comma() {
        assert_eq!(normalize_for_hash("tutte le strade,"), "tutte le strade");
    }

    #[test]
    fn hash_strips_surrounding_quotes() {
        assert_eq!(
            normalize_for_hash("\"tutte le strade portano a Roma\""),
            "tutte le strade portano a Roma",
        );
    }

    #[test]
    fn hash_strips_typographic_quotes() {
        assert_eq!(
            normalize_for_hash("\u{201C}la dolce vita\u{201D}"),
            "la dolce vita",
        );
    }

    #[test]
    fn hash_strips_trailing_period_and_exclamation() {
        assert_eq!(normalize_for_hash("Ciao!"), "Ciao");
        assert_eq!(normalize_for_hash("basta."), "basta");
    }

    #[test]
    fn hash_collapses_interior_whitespace() {
        assert_eq!(
            normalize_for_hash("la   bella    figura"),
            "la bella figura",
        );
    }

    #[test]
    fn hash_is_stable_across_calls() {
        let a = normalize_for_hash("  la storia,  ");
        let b = normalize_for_hash("la storia");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_canonicalizes_apostrophes_to_ascii() {
        // Both ASCII and typographic apostrophes hash to the same form.
        // This is what lets a typography pass replace ASCII with U+2019
        // in HTML without invalidating already-synthesized drill MP3s.
        assert_eq!(normalize_for_hash("l'acqua"), "l'acqua");
        assert_eq!(normalize_for_hash("l\u{2019}acqua"), "l'acqua");
        assert_eq!(
            normalize_for_hash("l'acqua"),
            normalize_for_hash("l\u{2019}acqua"),
        );
        // Several real-world examples in one go.
        for word in &[
            "un'altra",
            "dell'Impero",
            "c'è",
            "vent'anni",
            "d'Italia",
            "all'ultimo momento",
        ] {
            let ascii = word.to_string();
            let curly = word.replace('\'', "\u{2019}");
            assert_eq!(
                normalize_for_hash(&ascii),
                normalize_for_hash(&curly),
                "{word} should hash the same in ASCII and typographic forms",
            );
        }
    }

    #[test]
    fn hash_preserves_interior_comma_punctuation() {
        assert_eq!(
            normalize_for_hash("Mazzini, Cavour e Garibaldi"),
            "Mazzini, Cavour e Garibaldi",
        );
    }

    #[test]
    fn hash_empty_input_is_empty() {
        assert_eq!(normalize_for_hash(""), "");
    }

    #[test]
    fn hash_pure_punctuation_returns_collapsed_input() {
        // Edge case: trimming everything would leave nothing. Preserve the
        // collapsed-whitespace form so callers get a non-empty result.
        assert_eq!(normalize_for_hash(",,"), ",,");
    }

    // ── normalize_for_tts ───────────────────────────────────────────────

    #[test]
    fn tts_strips_parenthetical_roman_numeral() {
        assert_eq!(
            normalize_for_tts("il quindicesimo (XV) secolo"),
            "il quindicesimo secolo",
        );
    }

    #[test]
    fn tts_strips_parenthetical_with_various_numerals() {
        for (input, expected) in [
            ("il nono (IX) capitolo", "il nono capitolo"),
            ("il XIII (XIII) secolo", "il XIII secolo"),
            ("Vittorio Emanuele (II)", "Vittorio Emanuele"),
            ("Luigi (XIV) di Francia", "Luigi di Francia"),
        ] {
            assert_eq!(normalize_for_tts(input), expected, "input: {input}");
        }
    }

    #[test]
    fn tts_preserves_parenthetical_that_is_not_roman_numeral() {
        assert_eq!(
            normalize_for_tts("il termine (informale)"),
            "il termine (informale)",
        );
    }

    #[test]
    fn tts_delegates_to_hash_normalization_first() {
        // Trimming and quote-stripping still apply.
        assert_eq!(
            normalize_for_tts("  \"quindicesimo (XV) secolo\"  "),
            "quindicesimo secolo",
        );
    }

    #[test]
    fn tts_leaves_standalone_roman_numeral_untouched() {
        // Expansion of standalone Roman numerals (XV secolo → quindicesimo
        // secolo) is language-specific and lives in the root binary.
        assert_eq!(normalize_for_tts("XV secolo"), "XV secolo");
    }

    #[test]
    fn tts_emits_typographic_apostrophes() {
        // The TTS API gets well-typeset Italian regardless of how the
        // author wrote the apostrophe in the source.
        assert_eq!(normalize_for_tts("l'acqua"), "l\u{2019}acqua");
        assert_eq!(normalize_for_tts("l\u{2019}acqua"), "l\u{2019}acqua");
        assert_eq!(normalize_for_tts("un'altra volta"), "un\u{2019}altra volta");
    }

    // ── is_drillable ────────────────────────────────────────────────

    #[test]
    fn drillable_rejects_single_character() {
        assert!(!is_drillable("e"));
        assert!(!is_drillable("o"));
        assert!(!is_drillable("è"));
        assert!(!is_drillable("I"));
    }

    #[test]
    fn drillable_rejects_two_character_function_words() {
        for s in ["il", "la", "da", "in", "di", "no", "Io", "A"] {
            if s.chars().count() <= 2 {
                assert!(!is_drillable(s), "expected {s:?} to be non-drillable");
            }
        }
    }

    #[test]
    fn drillable_accepts_three_character_words() {
        assert!(is_drillable("chi"));
        assert!(is_drillable("già"));
        assert!(is_drillable("c\u{2019}è")); // c + apostrophe + è = 3 chars
        assert!(is_drillable("non"));
    }

    #[test]
    fn drillable_accepts_longer_phrases() {
        assert!(is_drillable("Roma"));
        assert!(is_drillable("la bella figura"));
        assert!(is_drillable("tutte le strade portano a Roma"));
    }

    #[test]
    fn drillable_rejects_empty_string() {
        assert!(!is_drillable(""));
    }

    #[test]
    fn drillable_counts_unicode_scalars_not_bytes() {
        // "è" is 2 bytes in UTF-8 but 1 char. Must count as 1, not 2.
        assert!(!is_drillable("è"));
        assert!(!is_drillable("pà")); // 2 chars, 3 bytes
    }
}
