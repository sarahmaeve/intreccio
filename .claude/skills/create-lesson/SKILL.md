---
name: create-lesson
description: Create a new triglot-weave Italian lesson from an English prompt. Generates the HTML fragment with lang-tagged spans, a meta.json, and a seed vocab.json; runs typography fixes, share audit, and drill synthesis in three phases with review gates.
user_invocable: true
---

# Create Lesson

Generate a complete weave lesson for intreccio from a short prompt
describing the topic, level, and any grammar features to showcase.

## Usage

```
/create-lesson <chapter-slug> <lesson-slug> <prompt-or-prompt-file>
```

Examples:

```
/create-lesson 20-roma-antica 003-etruschi "A1-A2 lesson on the Etruscans: pre-Roman civilization of central Italy, the twelve Etruscan cities, Roman adoption of Etruscan customs. Showcase cognate pattern fr -tion / it -zione."

/create-lesson 10-grammatica-base 001-articoli prompts/articoli.md
```

## Arguments

- `<chapter-slug>` — existing chapter directory under `content/`, e.g.
  `20-roma-antica`, `00-pronuncia`, `10-grammatica-base`.
- `<lesson-slug>` — new lesson slug, e.g. `003-etruschi`. Numeric prefix
  matches the chapter's sort order.
- `<prompt-or-prompt-file>` — either a short inline prompt or a path to
  a markdown file describing the lesson.

## Workflow

Runs in three phases with a review gate after each. Do not proceed
without explicit user confirmation.

### Phase 1: Content generation

1. Read the prompt. Identify: the target level (A1-reference, A1, A2,
   A2-B1, B1), the topic, and any grammar features or cognate patterns
   the prompt requests. If the level is ambiguous, ask the user before
   continuing — level drives every downstream calibration decision.

2. **Read `DESIGN.md` §3 (language role assignment) and §4 (level
   calibration) before writing any content.** Every sentence you produce
   descends from those rules.

3. Author the lesson HTML at
   `content/<chapter-slug>/<lesson-slug>.html`.

   Conventions (from DESIGN.md §5):

   - English is unmarked (the default).
   - French in `<span lang="fr">…</span>` — structural bridge.
   - Italian in `<span lang="it">…</span>` — the target.
   - Internationally stable proper names (Rome, Napoleon, Leonardo da
     Vinci) stay unmarked; Italian forms like
     `<span lang="it">Roma</span>` or `<span lang="it">Firenze</span>`
     are marked because they're a deliberate Italian rendering.
   - Multi-word Italian phrases that form a syntactic unit go in a
     single span — `<span lang="it">quasi cinque secoli</span>`, not
     three spans. The span boundary is the audio atom.
   - Parenthetical French glosses of Italian terms sit outside the
     Italian span: `<span lang="it">acquedotti</span>
     (<span lang="fr">les aqueducs</span>)`.
   - For A1–A2 lessons, Roman numerals use the dual form inside the
     span: `<span lang="it">il quindicesimo (XV) secolo</span>`. The
     `(XV)` is stripped before TTS.
   - For B1 lessons, the numeral-only form is fine:
     `<span lang="it">il XV secolo</span>`.

   Use typographic apostrophe `'` (U+2019) and ellipsis `…` (U+2026) in
   all foreign-language content. ASCII `'` and `...` will be flagged by
   `verify-language`.

   Open with an `<h2>` heading per major paragraph; the template
   wraps the whole thing in `<article class="weave">` automatically.

4. Author `content/<chapter-slug>/<lesson-slug>.meta.json`:

   ```json
   {
     "id": "<lesson-slug>",
     "title": "…",
     "level": "A2-B1",
     "topic_area": "…",
     "grammar_features_introduced": [
       "…"
     ],
     "cognate_patterns_showcased": [
       "fr -tion / it -zione",
       "fr -té / it -tà"
     ],
     "prerequisites": [],
     "notes": "…"
   }
   ```

   Do **not** hand-write the `language_shares` field. Measured shares
   will be surfaced by `audit-shares` in phase 2.

5. Add the lesson to `content/<chapter-slug>/chapter.toml`:

   ```toml
   [[sections.pages]]
   slug = "<lesson-slug>"
   title = "…"
   description = "…"
   type = "weave"
   ```

6. **Run Italian typography fixer:**

   ```bash
   cargo run -- verify-language <chapter-slug> --fix
   ```

7. Present a summary: files created, word count, grammar features
   and cognate patterns introduced, and a preview of the first
   paragraph. **Stop here and wait for user confirmation** before
   proceeding to phase 2.

### Phase 2: Calibration and review

After the user confirms the content looks good:

1. Build the lesson:

   ```bash
   cargo run -- build <chapter-slug>
   ```

2. Audit language shares:

   ```bash
   cargo run -- audit-shares <chapter-slug>
   ```

   Review the measured `en / fr / it` percentages against the
   declared level's range (DESIGN.md §4). If the lesson is outside
   the range, propose specific edits — e.g. "swap `<span lang="fr">la
   péninsule</span>` for `<span lang="it">la penisola</span>` in
   paragraph 2 to raise Italian share by ~1%". Apply edits if the
   user approves; re-run `audit-shares`.

3. Seed the vocabulary skeleton:

   ```bash
   cargo run -- extract-vocab <chapter-slug>
   ```

   This produces `<lesson-slug>.vocab.json` with one entry per unique
   Italian span, fields `fr`, `en`, `es`, `pattern` blank. Fill in the
   glosses in a single pass, following DESIGN.md §3's policy (Spanish
   only when it illuminates phonology or cognate structure).

4. Verify CSP compliance:

   ```bash
   cargo run -- check-csp
   ```

5. **Stop here and wait for user approval** before proceeding to
   phase 3 (which costs TTS API credits).

### Phase 3: Drill-audio synthesis

After the user approves the validated content:

1. Confirm `GOOGLE_TTS_API_KEY` is set. If not, ask the user to
   provide it inline — never persist it to any file in the repo.

2. Synthesize missing drill MP3s:

   ```bash
   cargo run -- drills <chapter-slug>
   ```

   The report prints the number of unique drills, how many were
   reused from the existing MP3 cache, and how many were newly
   synthesized. Every span whose text hashes to an unseen content
   address produces one MP3; the mapping is deterministic so reruns
   are no-ops.

3. Rebuild so any updated `data-audio` attributes are emitted:

   ```bash
   cargo run -- build <chapter-slug>
   ```

4. Final CSP check:

   ```bash
   cargo run -- check-csp
   ```

5. Report: files created, total drill MP3s, audio size on disk,
   CSP status.

## Scope reminders

- **No dialogs.** Intreccio has no dialog parser. Weave lessons are
  narrative prose only.
- **No full-passage audio.** Drill audio is per-span, Italian-only.
  French and English spans are silent.
- **One voice per text, deterministic.** The two-voice pool
  (`it-IT-Chirp3-HD-Aoede`, `it-IT-Chirp3-HD-Erinome`) is assigned by
  BLAKE3 first-byte modulo, so the same Italian phrase always gets
  the same voice.

## Common pitfalls

- **Wrapping English in spans.** Don't. English is unmarked.
- **Splitting a multi-word Italian phrase into one span per word.**
  Don't. The span is the audio atom — you want
  `<span lang="it">la bella figura</span>` played as one drill, not
  three.
- **Using passato remoto in A1-A2 content.** It's a recognition-only
  form at B1.
- **Introducing a new grammar feature without a cognate-pattern
  anchor in the same paragraph.** The learner should always have
  something to grip (DESIGN.md §10).
- **Claiming `language_shares` by hand in meta.json.** Don't —
  `audit-shares` measures them.
