# Triglot Italian — Design Document

A personal language-learning repository that generates Italian reading material for a specific polyglot profile, using a **triglot weave** technique: English as frame, French as structural bridge, Italian as the target. Output is HTML with semantic language tagging, suitable for reading in a browser and for programmatic transformation (vocabulary extraction, alternate views, future audio).

This document is the source of truth for content generation. When Claude Code (or any human collaborator) generates a new lesson, the rules below should be followed unless the lesson's own front-matter explicitly overrides them.

---

## 1. Learner Profile

The entire system is calibrated to one specific reader. Do not genericize.

- **Native language:** English
- **Strong Romance:** French, B2 (effortless reading, comfortable with subjunctive, conditional, compound tenses, clitic pronouns, relative constructions)
- **Weaker Romance:** Spanish, A2–B1 (reliable for basic vocabulary, phonology, and high-frequency grammar; not reliable for complex syntax)
- **Target language:** Italian, entering at A2 with goal of comfortable B1 reading
- **Primary goal:** receptive skills (reading first, listening second); productive skills are a secondary bonus
- **Secondary goal:** metalinguistic awareness across the three Romance languages and English, so that future work in Portuguese, Catalan, or Romanian benefits from the same reflexes

This profile matters because it drives every language-assignment decision below. A different profile (e.g., Spanish-dominant, no French) would require a different design.

---

## 2. Pedagogical Foundations

The method synthesizes four traditions. Each contributes something the others don't.

**Intercomprehension (EuroCom / Seven Sieves).** The assumption that receptive access to related languages can be taught directly through systematic cognate recognition and sound-correspondence rules, rather than through conventional four-skills instruction. Source material: Klein & Stegmann's *EuroComRom*, Hufeisen & Marx's *EuroComGerm*. From this tradition: the systematic exploitation of French→Italian correspondence patterns (`-oire/-oria`, `-tion/-zione`, `-té/-tà`, `-eur/-ore`, etc.).

**Pedagogical translanguaging (Cenoz & Gorter; García & Li).** The principle that a multilingual learner's full linguistic repertoire is a resource to be explicitly activated, not suppressed. From this tradition: the deliberate surfacing of all three languages within a single passage, with prompts that encourage cross-linguistic comparison rather than monolingual immersion.

**Diglot weave (Burling 1968; Blair / Power-Glide).** The technique of embedding target-language items into native-language text and progressively increasing the target-language share. From this tradition: the core formal structure of every lesson — a passage that is primarily L1 with calibrated L2 substitution, tuned so context always sustains comprehension.

**Juntos-style comparative textbook (Donato et al.).** The pedagogical stance that cognate-rich language trios should be taught *as trios*, with explicit comparative tables and cross-linguistic reflection. From this tradition: the trilingual vocabulary notebook convention and the "notice the pattern" metalinguistic prompts appended to each lesson.

The innovation of this project, if it is one, is extending Burling's two-language weave into a three-language weave and letting each language do the work it does best.

---

## 3. Language Role Assignment

This is the most important section. Every generation decision descends from it.

**English** is the frame. It handles:
- Function words and connectives when no pedagogical gain comes from translating them
- Proper nouns that are identical or near-identical in all three languages (Rome, Napoleon, Europe)
- Occasional clarifying glosses in parentheses when an Italian or French item might otherwise fail

**French** is the structural bridge. It handles:
- Connective tissue where French-Italian mapping is tight (*après que, pendant que, bien que, parce que*)
- Complex tenses and moods that Italian shares with French more than with Spanish (subjunctive after *pour que / perché*, conditional, pluperfect)
- Clitic and partitive constructions (the French *y / en* and Italian *ci / ne* are closer to each other than to anything in Spanish or English)
- Any vocabulary item where French is phonologically or orthographically closer to Italian than English is (which is the vast majority of abstract/Latinate vocabulary)

**Italian** is the target. It handles:
- Content-bearing nouns, especially those showcasing a cognate correspondence pattern
- Verbs in tenses the learner is actively acquiring (present, passato prossimo, imperfetto at A2; add congiuntivo presente, futuro, and condizionale at A2-B1; add passato remoto recognition at B1)
- Fixed phrases and cultural touchstones (*tutte le strade portano a Roma*, *dolce vita*, *la bella figura*)
- Anything the learner should encounter repeatedly to build productive familiarity

**Spanish** is *not* present in the text itself (the weave is triglot, not quadriglot). But Spanish may appear:
- In vocabulary-table glosses, where the Spanish form illuminates phonology or cognate relationships better than French does
- In metalinguistic notes at the bottom of a lesson (e.g., "notice that Italian *acqua* is closer to Spanish *agua* than to French *eau*")

The pattern to internalize: **French pulls you through the grammar, Spanish pulls you through the pronunciation, English catches you when both fail.**

---

## 4. Level Calibration

Three levels are supported. Each lesson declares exactly one in its metadata.

| Level | Italian share | Italian verb forms | French share | English share |
|---|---|---|---|---|
| **A2** | 30–40% | infinitives, present tense, passato prossimo, one or two imperfetto tokens as recognition-only | 30–40% | 25–35% |
| **A2-B1** | 45–55% | above + full imperfetto, futuro semplice, condizionale presente, congiuntivo presente in receptive contexts | 25–35% | 15–25% |
| **B1** | 60–70% | above + congiuntivo imperfetto, passato remoto (recognition), trapassato, si-passivante | 15–25% | 10–20% |

Within a single lesson, paragraph-level ramping is allowed and encouraged. A B1 lesson may open at 55% Italian and close at 70%. A2-B1 is the default target for this learner and should be the level for any lesson whose metadata doesn't specify otherwise.

Above B1 is out of scope for this project. At that point, the learner should switch to Italian-French parallel texts (Folio bilingue, LingQ with French gloss) and the weave is no longer the right training tool.

---

## 5. HTML Markup Convention

Every generated lesson is an HTML fragment using the native `lang` attribute on `<span>` tags. This is the only structural decision the repo commits to.

```html
<p>
  The <span lang="fr">histoire</span> of
  <span lang="fr">l'Empire romain</span> begins with a small
  <span lang="fr">ville</span> — a <span lang="fr">ville</span>
  the <span lang="fr">Romains</span> called
  <span lang="it">Roma</span>.
</p>
```

Rules:
- **Do not** wrap English content in any span. English is the unmarked default. This keeps the markup clean and makes the language distribution visually obvious in the source.
- **Do** wrap every French and Italian item, even single words, in `<span lang="fr">` or `<span lang="it">` respectively. No exceptions.
- **Do not** use `<i>` or `<em>` to indicate foreign language. Italic styling is applied via CSS on the lang attribute; using `<em>` confuses emphasis with language marking.
- **Do** treat multi-word foreign phrases as a single span when they form a syntactic unit (`<span lang="it">la dolce vita</span>`, not three separate spans).
- **Do** keep punctuation outside the span when it belongs to the surrounding English sentence, inside the span when it belongs to the foreign-language unit (quoted phrases, parenthetical glosses).

Styling lives in `assets/styles.css` and uses CSS variables so it can be themed:

```css
:root {
  --color-fr: #1e3a8a;
  --color-it: #14532d;
  --weight-it: 600;
}

[lang="fr"] { color: var(--color-fr); font-style: italic; }
[lang="it"] { color: var(--color-it); font-style: italic; font-weight: var(--weight-it); }
```

The choice of blue for French and green for Italian is arbitrary and adjustable. What matters is that the two foreign languages are visually distinct from each other and from English at a glance.

This markup choice unlocks several capabilities almost for free:
- **Alternate views** via CSS toggle: hide all French to see the pure Italian+English, hide all Italian to see the scaffolding
- **Programmatic extraction:** a script can pull every `[lang="it"]` span into a vocabulary list
- **Accessibility:** screen readers switch voices on `lang` attribute boundaries
- **Drill-down audio:** the build step walks every `<span lang="it">`, synthesizes its text once with a single Italian voice, and attaches a `data-audio` attribute to the span. See §9. French and English spans are silent — the weave is a reading exercise, not a listening one.

---

## 6. Lesson File Structure

Each lesson lives in its chapter directory as three slug-prefixed files:

```
content/20-weave-roma-antica/
├── chapter.toml
├── 001-roman-empire.html         # the weave itself, as an HTML fragment
├── 001-roman-empire.meta.json    # level, topic, shares, grammar features
└── 001-roman-empire.vocab.json   # Italian items with French and English glosses
```

Lessons are authored directly as HTML fragments, not as Markdown. (Markdown with embedded `<span lang="…">` tags is fine for drafting and review — the file in `examples/roman-empire-triglot.md` is an instance — but the source of truth that the build pipeline reads is `.html`.) The fragment is not a complete document; it's the content that a Tera template wraps, which lets the site style evolve without rewriting lessons.

**`meta.json` schema:**

```json
{
  "id": "003-roman-empire",
  "title": "The Roman Empire",
  "level": "A2-B1",
  "topic_area": "Italian history",
  "word_count": 420,
  "language_shares": {
    "en": 0.22,
    "fr": 0.28,
    "it": 0.50
  },
  "grammar_features_introduced": [
    "passato prossimo with essere",
    "imperfetto",
    "congiuntivo presente (receptive)"
  ],
  "cognate_patterns_showcased": [
    "fr -oire / it -oria",
    "fr -tion / it -zione",
    "fr -té / it -tà"
  ],
  "prerequisites": ["001-rome-foundations"],
  "created": "2026-04-18",
  "notes": "First lesson to introduce receptive congiuntivo."
}
```

**`vocab.json` schema:**

```json
{
  "items": [
    {
      "it": "la storia",
      "fr": "l'histoire",
      "es": "la historia",
      "en": "history / story",
      "pattern": "fr -oire / it -oria",
      "first_appearance": "paragraph 1"
    }
  ]
}
```

The `es` (Spanish) field is optional and appears only when the Spanish form illuminates something the French form doesn't.

---

## 7. Repository Layout

The implementation is a single Rust workspace. No Node, no npm, no JavaScript build step. A fresh clone plus `cargo run -- build` produces a deployable `site/`.

```
intreccio/
├── DESIGN.md              # this file — the pedagogical and structural spec
├── README.md              # human-facing usage instructions
├── CLAUDE.md              # Claude Code instructions (points here for details)
├── Cargo.toml             # Rust workspace manifest
├── src/                   # root-crate CLI dispatcher + TTS client (see §9)
├── crates/                # supporting libraries (site generation, image-strip, etc.)
├── prompts/
│   ├── generate-lesson.md # the prompt used to generate a new lesson
│   ├── level-rubrics.md   # explicit A2 / A2-B1 / B1 criteria with examples
│   └── revision.md        # the prompt used to revise an existing lesson
├── content/               # source of truth — lessons, chapter configs, site config
│   ├── site.toml                       # top-level site index
│   └── <chapter>/
│       ├── chapter.toml
│       ├── <lesson>.html               # weave fragment
│       ├── <lesson>.meta.json
│       └── <lesson>.vocab.json
├── templates/             # Tera templates applied on build
│   ├── base.html, weave.html,
│   ├── fragment.html, chapter_index.html, site_index.html
└── site/                  # deployable output (generated HTML + committed MP3s)
    ├── _headers                        # strict CSP + cache rules
    ├── shared/                         # fonts, styles, a minimal drill.js
    ├── chapters/<chapter>/
    │   ├── *.html, style.css
    │   └── audio/drills/<hash>.mp3     # per-span drill audio
    └── index.html
```

`content/` is the source of truth. Generated HTML is written into `site/` on every build; drill MP3s are content-addressed by BLAKE3 of the Italian span's normalized text and checked in alongside the HTML so regeneration is idempotent — only spans whose text has changed incur a new TTS call.

---

## 8. Claude Code Integration

A `CLAUDE.md` file at the repo root tells Claude Code how to behave when asked to generate or revise lessons. It should be minimal and defer to this document for anything substantive. Suggested contents:

```markdown
# Instructions for Claude

When generating a new lesson, follow `DESIGN.md` strictly. In particular:

1. Use `lang="fr"` and `lang="it"` spans for all non-English content.
2. Do not wrap English in spans.
3. Default to level A2-B1 unless otherwise specified.
4. Produce three files per lesson: `<slug>.html`, `<slug>.meta.json`, `<slug>.vocab.json`.
5. After generating, run `cargo run -- audit-shares <chapter>` and verify
   the measured language shares fall within the declared level's range in §4.
6. Run `cargo run -- drills <chapter>` to synthesize drill-down MP3s for
   any new Italian words or phrases. Full-passage audio is out of scope.

When revising, preserve the lesson id and filename. Update `meta.json` if any
grammar feature, cognate pattern, or share shifts meaningfully.

For prompts used during generation, see `prompts/generate-lesson.md`.
```

The `prompts/generate-lesson.md` file holds the actual text passed to Claude Code as a template, with placeholder fields for topic, level, and length. Keeping prompts under version control means they can be iterated on and their history tracked.

---
## 9. Tooling

All build steps are driven by a single Rust CLI at the workspace root. The goal is that a fresh clone, `$GOOGLE_TTS_API_KEY` in the environment, and `cargo run -- build` produce a deployable `site/`.

Commands:

- `cargo run -- build [<chapter>]`
  Render HTML from `content/` into `site/`, applying Tera templates. Walks every `[lang="it"]` span in each fragment and injects a `data-audio` attribute pointing at `audio/drills/<hash>.mp3`. Does not synthesize audio.

- `cargo run -- drills <chapter>`
  Parse every lesson in the chapter, extract `[lang="it"]` span texts, hash each with BLAKE3 (first 16 hex chars), and synthesize a missing MP3 via Google Cloud TTS. A single Italian Studio voice reads every drill. Idempotent: existing files are reused. Scope is strictly Italian words and phrases — there is no full-passage audio, no dialog support, no multi-voice assignment.

- `cargo run -- audit-shares [<chapter>]`
  Walk each lesson's HTML, count words per language span, and verify the measured shares fall within the declared level's range in §4. Counting rule: unmarked text is English; hyphenated compounds count as one word; proper nouns count in the language of their surrounding span.

- `cargo run -- extract-vocab [<chapter>]`
  Emit a skeleton `vocab.json` for a lesson from its `[lang="it"]` spans, with `fr`/`en`/`es` gloss fields left blank for manual or Claude-assisted fill-in.

- `cargo run -- verify-language [<chapter>] [--fix]`
  Enforce Italian typographic rules (apostrophes, quotation marks, ellipsis).

- `cargo run -- check-csp`
  Scan generated HTML for Content Security Policy violations (no inline scripts, styles, event handlers, or external resources).

- `cargo run -- strip-metadata <path>` / `prepare-image <path>`
  Remove EXIF, XMP, IPTC, and comment metadata from JPEG/PNG images before they land in `site/`; optionally resize and convert to WebP.

TTS is Google Cloud Text-to-Speech over a simple REST call: `text → MP3 bytes` with one Italian voice. Roman numerals are expanded to Italian ordinals before synthesis (`XIII secolo` → `tredicesimo secolo`, `Guglielmo IX` → `Guglielmo nono`).

---

## 10. Quality Heuristics

When generating a lesson, prefer the choice that:

- **Clusters cognate patterns.** A paragraph that showcases three or four words from the same correspondence pattern (e.g., *nation/nazione, tradition/tradizione, religion/religione, construction/costruzione*) teaches the pattern more effectively than four words from four different patterns.
- **Repeats high-value Italian items.** A word that appears twice in a lesson is far more likely to stick than one that appears once. Aim for every content-bearing Italian noun to recur at least once.
- **Lets French do its job.** When a connective could be in English or French, prefer French if it's a B2-comfortable form. This trains the reader to parse French at reading speed, which is a skill worth maintaining anyway.
- **Uses Italian verbs receptively before actively.** A new tense should first appear as a clearly contextualized form with French scaffolding around it. Only after two or three receptive exposures across different lessons should the same tense appear in a context where the reader has to actively parse it.
- **Flags false friends explicitly.** Whenever a French-Italian pair looks identical but diverges in meaning or register (*actuellement* ≠ *attualmente*, *assister à* ≠ *assistere a*), the vocabulary file must flag it and the metalinguistic notes should address it.

Counterheuristics — things to avoid:

- Do not use the passato remoto in A2 or A2-B1 active contexts. It's a recognition-only form at B1.
- Do not introduce a grammar feature without at least one cognate-pattern anchor in the same paragraph. The learner should always have something to grip.
- Do not let any single paragraph exceed 70% Italian, even at B1. Beyond that, the weave collapses into ordinary Italian text and the pedagogical value is lost.

---

## 11. Extension Points

Not required for v1, but the architecture should not preclude them:

- **Interactive toggles.** A `toggle.js` that lets the reader hide French (to test whether they can still follow), hide Italian (to see the scaffolding alone), or reveal hover-glosses on any foreign span.
- **Vocabulary review.** A spaced-repetition deck auto-generated from `vocab.json` files across all lessons, exportable to Anki or a lightweight built-in reviewer.
- **Expanded drill-audio.** Drill-audio for `[lang="it"]` spans ships in v1 (§9). Possible extensions: drill audio for `[lang="fr"]` spans (for refreshing French pronunciation of connectives); a slow-playback hotkey; a second voice for phrases longer than some threshold.
- **Cross-lesson pattern index.** A generated page that lists every cognate pattern showcased across the corpus, with links to the lessons that use them.
- **Portuguese mode.** The same design with Portuguese swapped in as the target language. The learner's French B2 + Italian (once acquired) + Spanish A2-B1 would make Portuguese trivially accessible; this is the natural second project.
- **Reverse mode.** Generate lessons targeted at a French-dominant learner acquiring English through Italian as the bridge. Useful as a test of the design's generality.

---

## 12. Open Questions

Things not yet decided, flagged for future resolution.

- **Verse and proverbs.** Italian has an enormous proverb and folk-verse tradition. Should lessons occasionally include a short poem or proverb in its original Italian, with a French prose gloss? This would break the weave convention but add cultural density. Provisional answer: yes, in a designated `<blockquote class="proverb">` that is exempt from the language-share targets.
- **Regional variation.** Should lessons stick to standard Italian, or gradually introduce regional features (Neapolitan, Venetian, Sicilian) as cultural color? Provisional answer: standard only in v1; regional features become a possible future axis.
- **Dialogues.** Out of scope for v1. The weave is narrative prose only. The tooling has no dialog parsing or multi-voice audio pipeline. If dialogues are added later, they would be a separate page type with their own single-voice or narrator-monologue audio model.
- **Measuring shares.** Resolved in §9 (`audit-shares`): word count, with hyphenated compounds counting as one word and proper nouns counting in the language of their surrounding span. Unmarked text is English.

---

*Last revised: 2026-04-18. This document is the living spec; update it when design decisions change, and cite sections by number in commit messages when a commit implements or alters a rule.*
