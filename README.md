# Intreccio

*Interweaving.* Triglot Italian reading material for English speakers with strong French, built as a Rust CLI that generates static HTML.

Every Italian word and phrase in a lesson has click-to-hear audio, synthesized once via Google Cloud Text-to-Speech and committed to git as content-addressed MP3s. No JavaScript framework, no Node toolchain, no database — one Rust binary, some Tera templates, a strict Content Security Policy, and a local dev server.

See [`DESIGN.md`](DESIGN.md) for the pedagogical rationale, markup conventions, and level calibration.

## Pre-commit hook

After cloning, activate the pre-commit hook once per clone:

```bash
git config core.hooksPath scripts/hooks
```

The hook runs `cargo test --workspace` and
`cargo clippy --workspace --all-targets -- -D warnings` whenever a
commit touches Rust sources or Cargo metadata. Content-only commits
(lesson HTML, meta.json, vocab.json, audio MP3s) skip the hook so
authoring work doesn't pay a compile tax. Bypass with
`git commit --no-verify` when you're certain.

## Quickstart

```bash
# Build HTML (no API key required)
cargo run -- build

# Preview locally
cargo run -- serve
# open http://127.0.0.1:8000

# Synthesize drill audio for a chapter (needs a Google Cloud key)
export GOOGLE_TTS_API_KEY="…"
cargo run -- drills 20-roma-antica

# Verify language shares match DESIGN.md §4 level ranges
cargo run -- audit-shares

# Check CSP compliance of the built site
cargo run -- check-csp
```

## Repository layout

```
intreccio/
├── Cargo.toml              # workspace manifest
├── src/                    # root binary: CLI, TTS client, drill orchestrator, dev server
├── crates/
│   ├── site-gen/           # chapter/site config, Tera pipeline, typography, CSP checker
│   ├── weave/              # HTML span extraction, normalization, drill-audio injection, audit
│   └── image-strip/        # EXIF/XMP/IPTC scrubbing for JPEG/PNG
├── templates/              # Tera templates: base, weave, fragment, chapter_index, site_index
├── content/                # source of truth — lessons, chapter configs, site config
│   ├── site.toml
│   ├── 00-pronuncia/           # A1 pronunciation reference
│   ├── 10-grammatica-base/     # A1 grammar reference
│   └── 20-roma-antica/         # first A1 → A2-B1 weave chapter
├── site/                   # generated output (HTML) + committed drill MP3s
│   ├── _headers                # CSP + cache rules for Cloudflare Pages
│   ├── shared/                 # fonts, styles, drill.js
│   └── chapters/<chapter>/
│       ├── *.html, index.html
│       └── audio/drills/<hash>.mp3
└── examples/
    └── roman-empire-triglot.md # readable draft format (not an input to the build)
```

`content/` is the source of truth. `site/` is generated on every `build`; the drill MP3s are content-addressed (BLAKE3-based filenames), so regenerating a chapter is idempotent — unchanged spans keep their existing audio.

## Commands

| Command | What it does |
|---|---|
| `build [<chapter>]` | Render HTML from `content/` into `site/`, injecting `data-audio` URLs on Italian spans. No API calls. |
| `drills <chapter>` | Synthesize missing drill MP3s via Google Cloud TTS. Idempotent; existing files are reused. |
| `serve [--port N]` | Serve `site/` on `http://127.0.0.1:N` (default 8000) with correct MIME types. |
| `audit-shares [<chapter>]` | Count words per language in each weave lesson and compare to the level's expected range. |
| `extract-vocab [<chapter>]` | Emit a skeleton `<slug>.vocab.json` from each weave lesson's `<span lang="it">` items. Merges with existing files. |
| `verify-language [<chapter>] [--fix]` | Italian typographic rules (apostrophes, ellipsis). |
| `check-csp` | Scan built HTML for Content Security Policy violations. |
| `strip-metadata <path>` | Strip EXIF/XMP/IPTC from JPEG/PNG, preserving orientation. |
| `file <input.txt> <output.mp3>` | One-off text-to-MP3 with a single Italian voice. |

Full help: `cargo run -- --help`.

## Authoring a new lesson

1. Draft an HTML fragment with `<span lang="fr">…</span>` and `<span lang="it">…</span>` markers. See `examples/roman-empire-triglot.md` for the style.
2. Save it as `content/<chapter>/<slug>.html` and add an entry in `content/<chapter>/chapter.toml` with `type = "weave"`.
3. Create `content/<chapter>/<slug>.meta.json` with `level`, `grammar_features_introduced`, etc. (see DESIGN.md §6).
4. `cargo run -- extract-vocab <chapter>` to seed `<slug>.vocab.json`; fill in glosses.
5. `cargo run -- build` to render.
6. `cargo run -- audit-shares <chapter>` to verify calibration.
7. `cargo run -- drills <chapter>` to synthesize the new drill MP3s.
8. `cargo run -- serve` to preview, then commit.

When Claude Code generates a lesson, it follows `.claude/skills/create-lesson/SKILL.md`.

## License

Content and code are for personal and educational use.
