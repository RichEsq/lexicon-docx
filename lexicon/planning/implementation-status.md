# Implementation Status

## Completed

### Phase 1: Skeleton + Front-matter → Cover Page ✓
- Cargo.toml with all dependencies (clap, comrak, docx-rs, serde, serde_yaml, toml, regex, chrono, thiserror)
- `main.rs` — CLI with `build` and `validate` subcommands
- `lib.rs` — public facade: `parse()`, `resolve()`, `render_docx()`, `process()`
- `error.rs` — `LexiconError` enum + `Diagnostic` type
- `model.rs` — full IR types (Document, Clause, InlineContent, etc.)
- `frontmatter.rs` — YAML parsing + date/party validation
- `style.rs` — StyleConfig with TOML loading, page size/margin helpers
- `render/docx.rs` — cover page with title, date, status/version, ref, author, parties block

### Phase 2: Clause Parsing + Legal Numbering ✓
- `parser/mod.rs` — orchestrator: comrak parse → AST walk → IR
- `parser/clause.rs` — recursive clause extraction from nested ordered lists, inline content extraction (bold, italic, links, cross-refs, schedule refs), annexure parsing (headings, paragraphs, tables, bullet lists, clause lists)
- `parser/anchors.rs` — regex-based `{#id}` stripping from text nodes
- `resolve.rs` — clause number assignment (1., 1.1, (a), (i) with roman numerals), anchor→number map, cross-reference resolution
- `render/docx.rs` — clause rendering with headings, legal numbering as text prefixes, indentation per level (720 twips/level), blockquotes, tables, annexure pages with cover headings, bullet lists

**Key finding**: comrak produces proper `Heading` nodes inside list `Item` nodes for `1. ## Heading` syntax. Anchors appear as literal text — stripped by regex. Schedule reference links are fully resolved by comrak (URL = `#schedule`, title = value).

### Phase 3: Inline Content + Cross-References + Term Validation ✓
- `resolve.rs` — defined term validation:
  - Classifies bold text as: FormalDefinition (`**Term** means`), InlineDefinition (`("**Term**")`), PartyRole (from front-matter), FieldLabel (`**Label**:`), or Reference
  - Handles grouped definitions (`The terms "**X**", "**Y**" shall have the same meaning...`)
  - Fuzzy matching via multi-variant normalisation (possessives, plurals, verb forms like -ing, -ed, -es, -ies)
  - Filters structural field labels in annexures
  - Cross-reference validation with broken anchor warnings
- Validation output: 10 warnings on example.md, all legitimate (1 undefined `Addendum`, 8 unused US privacy acronyms, 1 unused `include`)

**Note**: `parser/terms.rs`, `parser/references.rs`, and `parser/schedule.rs` were NOT created as separate files. All term detection, cross-reference handling, and schedule item extraction is handled within `parser/clause.rs` (inline extraction) and `resolve.rs` (validation). This is simpler than the original plan since comrak handles most of the heavy lifting during AST parsing.

### Phase 4: Schedule Annexure Generation ✓
- `model.rs` — replaced `ScheduleDef` with `ScheduleItem { description, value }`, renamed `schedule_defs` → `schedule_items`
- `resolve.rs` — `collect_schedule_items()` walks entire document (body, annexures) collecting all `ScheduleRef` inline elements
- `render/docx.rs` — "SCHEDULE" annexure page with two-column table (Item | Value), blank items show "____________"

### Phase 5: Styling, TOC, Headers/Footers, Polish ✓
- **Line spacing** — `style.line_spacing` applied as document-wide default via `default_line_spacing()` (Auto rule, value = spacing × 240 twips)
- **Hanging indents** — clause paragraphs with numbers use `SpecialIndentType::Hanging(360)` so wrapped lines align past the number; continuation paragraphs indent to the text position
- **Table of contents** — auto-generated TOC via `TableOfContents` field code with heading styles range 1–3, on its own page after the cover
- **Footer** — ref (italic, left) | Page X of Y (center) | Version N (italic, right) on all pages except the cover page
- **First-page suppression** — empty first-page header/footer so the cover page is clean

## Remaining / Future Work

### Not yet implemented
- **Draft watermark** when `status: draft` — docx-rs 0.4 does not expose watermark/VML shape APIs; would require XML post-processing or a future crate version
- **Sub-headings (`###`)** within clauses — rendering exists but may need styling refinement for specific use cases

### Recently completed
- **Native Word numbering** — replaced text-prefix numbers with Word's native numbering engine (`AbstractNumbering` + `Numbering` via docx-rs). See `planning/native-word-numbering.md` for details.

## Architecture Notes

### Processing Pipeline
```
.md input
  → frontmatter.rs: split on ---, serde_yaml deserialize, validate
  → parser/mod.rs: comrak::parse_document → AST
  → parser/clause.rs: recursive AST walk → Document IR
  → resolve.rs: numbering + cross-refs + term validation
  → render/docx.rs: IR → docx-rs → .docx bytes
  → main.rs: write to disk, print diagnostics
```

### Comrak AST Structure (discovered empirically)
```
Document
  List(ordered)           ← top-level clause list
    Item                  ← one per top-level clause
      Heading(level=2)    ← ## heading, with {#anchor} as literal text
      List(ordered)       ← sub-clauses
        Item
          Paragraph       ← clause text with Strong/Link/Text children
          List(ordered)   ← sub-sub-clauses (recursive)
  Heading(level=1)        ← # ANNEX headings
  Paragraph               ← prose/annexure content
  Table                   ← markdown tables
```

### Key Design Decisions
- Clause numbers as text prefixes (not Word numbering engine) — simpler, more predictable
- Single crate now, structured for workspace extraction later
- comrak resolves reference links during parsing, so `[text][ref-id]` becomes `Link(url="#schedule", title="value")` — the ref-id is lost but title carries the value
- Defined term matching uses multi-variant stemming, not a full NLP stemmer — pragmatic for legal text
