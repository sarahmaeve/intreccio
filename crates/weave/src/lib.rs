//! # weave
//!
//! HTML parsing and transformation for triglot-weave lesson fragments.
//!
//! A weave lesson is an HTML fragment where foreign-language content is
//! wrapped in `<span lang="fr">…</span>` or `<span lang="it">…</span>`
//! elements. This crate provides the four operations the build pipeline
//! needs on those fragments:
//!
//! 1. **Extraction** (`extract`): collect every foreign-language span
//!    with its text content, for generating drill audio and vocabulary
//!    skeletons.
//! 2. **Normalization** (`normalize`): canonicalize span text for
//!    content-addressed hashing and for TTS input (strip parenthetical
//!    Roman numerals, collapse whitespace, etc.).
//! 3. **Transformation** (`transform`): rewrite a fragment's Italian
//!    spans to carry `data-audio` attributes pointing at their drill
//!    MP3s.
//! 4. **Audit** (`audit`): count words per language to verify against
//!    DESIGN.md §4 level calibration.

pub mod audit;
pub mod extract;
pub mod normalize;
pub mod transform;

pub use audit::{count_words_by_lang, LangShares};
pub use extract::{extract_spans, LangSpan};
pub use normalize::{is_drillable, normalize_for_hash, normalize_for_tts, MIN_DRILL_LENGTH};
pub use transform::inject_drill_audio;

