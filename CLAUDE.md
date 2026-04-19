# Instructions for Claude

You are working on **intreccio**, a Rust build tool that generates triglot
Italian reading material for an English speaker with strong French. The
pedagogical design, markup convention, level calibration, and tooling
contract live in [`DESIGN.md`](DESIGN.md) and are the authoritative
source of truth for all content and code decisions.

## Code style

- Expert, idiomatic Rust. Simplify for correctness.
- Always include tests when a changelist exceeds ~25 lines.
- Run `cargo clippy --workspace --all-targets` and fix all warnings before
  considering a task complete.
- Never use `unsafe` unless you also write a `// SAFETY:` comment and a
  test that exercises the preconditions.

## Content authoring

When generating a new lesson, follow **`DESIGN.md` strictly**:

1. Use `<span lang="fr">…</span>` and `<span lang="it">…</span>` for all
   non-English content. Do not wrap English in spans.
2. Multi-word foreign phrases that form a syntactic unit go in a single
   span — `<span lang="it">quasi cinque secoli</span>`, not three
   separate spans. The span boundary *is* the audio atom.
3. Default to level **A2-B1** unless otherwise specified.
4. Produce three files per lesson:
   - `<slug>.html` (the weave fragment)
   - `<slug>.meta.json` (level, grammar features, cognate patterns)
   - `<slug>.vocab.json` (optional; will be seeded by `extract-vocab`)
5. For A1 → A2 content, Roman numerals are dual-form:
   `<span lang="it">il quindicesimo (XV) secolo</span>`. The `(XV)` is
   stripped before TTS by `weave::normalize_for_tts`.
   For B1 content, Roman-numeral-only form is fine:
   `<span lang="it">il XV secolo</span>`. The Italian ordinal table in
   `src/italian.rs` expands it to `tredicesimo secolo` for the voice.

## Build workflow

After generating or revising a lesson:

```bash
cargo run -- verify-language <chapter> --fix   # Italian typography
cargo run -- build <chapter>                   # render HTML
cargo run -- audit-shares <chapter>            # verify §4 calibration
cargo run -- drills <chapter>                  # synthesize new drills (needs TTS key)
cargo run -- check-csp                         # CSP compliance
```

## Scope boundaries

- **No dialogs in v1.** Intreccio has no dialog parser, no multi-voice
  code, no combined-audio pipeline. Lessons are narrative prose only.
- **No full-passage audio.** Drill audio is scoped strictly to Italian
  words and phrases. French and English spans are silent.
- **No Node, no npm.** The entire build is Rust.
- **No inline styles, scripts, or event handlers.** The site runs under
  a strict CSP. All JS/CSS is external; `drill.js` attaches handlers
  in code, not via `onclick=`.

For the generation skill, see `.claude/skills/create-lesson/SKILL.md`.
