//! # site-gen
//!
//! Chapter and site rendering for intreccio. This crate handles the
//! non-language-specific parts of the build pipeline:
//!
//! - **`config`** — deserialize `chapter.toml` and `site.toml`.
//! - **`build`** — apply Tera templates to lessons and generate the
//!   chapter and site index pages.
//! - **`typography`** — verify and auto-fix typographic rules per
//!   language (currently Italian).
//! - **`csp`** — scan built HTML for Content Security Policy violations.
//!
//! Drill-audio injection is delegated to the `weave` crate, which this
//! crate depends on. The root binary's drill generator separately
//! synthesizes the MP3 files that the injected attributes reference.

pub mod build;
pub mod config;
pub mod csp;
pub mod typography;
