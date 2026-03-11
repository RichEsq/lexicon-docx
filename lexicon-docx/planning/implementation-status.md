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
- `parser/clause.rs` — recursive clause extraction from nested ordered lists, inline content extraction (bold, italic, links, cross-refs, schedule refs), addendum parsing (headings, paragraphs, tables, bullet lists, clause lists)
- `parser/anchors.rs` — regex-based `{#id}` stripping from text nodes
- `resolve.rs` — clause number assignment (1., 1.1, (a), (i) with roman numerals), anchor→number map, cross-reference resolution
- `render/docx.rs` — clause rendering with headings, legal numbering as text prefixes, indentation per level (720 twips/level), blockquotes, tables, addendum pages with cover headings, exhibit placeholder pages, bullet lists

**Key finding**: comrak produces proper `Heading` nodes inside list `Item` nodes for `1. ## Heading` syntax. Anchors appear as literal text — stripped by regex. Schedule reference links are fully resolved by comrak (URL = `#schedule`, title = value).

### Phase 3: Inline Content + Cross-References + Term Validation ✓
- `resolve.rs` — defined term validation:
  - Classifies bold text as: FormalDefinition (`**Term** means`), InlineDefinition (`("**Term**")`), PartyRole (from front-matter), FieldLabel (`**Label**:`), or Reference
  - Handles grouped definitions (`The terms "**X**", "**Y**" shall have the same meaning...`)
  - Fuzzy matching via multi-variant normalisation (possessives, plurals, verb forms like -ing, -ed, -es, -ies)
  - Filters structural field labels in addenda
  - Cross-reference validation with broken anchor warnings
- Validation output: 10 warnings on example.md, all legitimate (1 undefined `Addendum`, 8 unused US privacy acronyms, 1 unused `include`)

**Note**: `parser/terms.rs`, `parser/references.rs`, and `parser/schedule.rs` were NOT created as separate files. All term detection, cross-reference handling, and schedule item extraction is handled within `parser/clause.rs` (inline extraction) and `resolve.rs` (validation). This is simpler than the original plan since comrak handles most of the heavy lifting during AST parsing.

### Phase 4: Schedule Generation ✓
- `model.rs` — replaced `ScheduleDef` with `ScheduleItem { description, value }`, renamed `schedule_defs` → `schedule_items`
- `resolve.rs` — `collect_schedule_items()` walks entire document (body, addenda) collecting all `ScheduleRef` inline elements
- `render/docx.rs` — "SCHEDULE" page with two-column table (Item | Value), blank items show "____________"

### Phase 5: Styling, TOC, Headers/Footers, Polish ✓
- **Line spacing** — `style.line_spacing` applied as document-wide default via `default_line_spacing()` (Auto rule, value = spacing × 240 twips)
- **Hanging indents** — clause paragraphs with numbers use `SpecialIndentType::Hanging(360)` so wrapped lines align past the number; continuation paragraphs indent to the text position
- **Table of contents** — auto-generated TOC via `TableOfContents` field code with heading styles range 1–3, on its own page after the cover
- **Footer** — ref (italic, left) | Page X of Y (center) | Version N (italic, right) on all pages except the cover page
- **First-page suppression** — empty first-page header/footer so the cover page is clean

## Remaining / Future Work

### Not yet implemented
- **Exhibit URL import** — `path` field with HTTP/HTTPS URLs for fetching remote exhibit files. Phase 3 of exhibit file import.

### Recently completed
- **Native Word cross-references** — cross-references (`[text](#anchor)`) now render as clickable internal hyperlinks in Word, with bookmarks placed at anchor targets (clause headings, first content paragraphs, addendum headings). Uses docx-rs's `Hyperlink::new(name, HyperlinkType::Anchor)` for hyperlinks and `Paragraph::add_bookmark_start/end` for bookmark targets. Bookmark names are sanitised (`lx_` prefix, hyphens → underscores, truncated to 40 chars). Ctrl+click in Word jumps to the referenced clause. No field codes or ZIP post-processing needed.
- **Recitals / Background section** — dedicated section for contract recitals or background, triggered by `# Recitals` or `# Background` heading (case-insensitive). Content supports the same types as the document body. Ordered lists are lettered at the top level: (A), (B), (C) instead of numbered. Sub-levels follow the clause hierarchy pattern: A.1, A.1(a), A.1(a)(i). When recitals are present, a body heading (e.g. `# Operative Provisions`) is required before the operative clauses, ensuring the document remains readable in plain Markdown. Cross-references resolve to "Recital A", "Recital A.1", etc. Defined terms in recitals are validated like the body. New model types: `Recitals` struct, `RecitalTopLevel`/`RecitalClause`/`RecitalSubClause`/`RecitalSubSubClause` variants on `ClauseNumber`. New Word numbering definition with `upperLetter` format at level 0. Spec section 3.9 added.
- **Signature panel layout modes** — two layout modes for signature blocks: "long" (AU/UK/NZ-style, each field is two table rows: writing space with bottom border + label caption below) and "short" (US-style, one row per field with inline label/value). Per-template `layout` field in `signatures.toml` (default "long"). In long mode, `type = "line"` fields get taller writing space (28pt) vs blank fields (16pt). Field labels support `{title}` placeholder expansion for per-signatory captions (e.g. `"{title} Signature"` → "Director Signature"). US templates explicitly set `layout = "short"`; AU/UK/NZ templates updated with caption-style labels.
- **Signature pages** — configurable signature blocks rendered as borderless tables with cell-border signature lines. New `entity_type` field on parties (`{jurisdiction}-{type}` compound string, e.g. `au-company`). Execution method inferred from `type` (Deed → deed, anything else → agreement). Two-layer template system: prose intro with `{placeholder}` substitution + structured field layout. External definitions file (`signatures.toml`) with templates for AU, UK, US, NZ jurisdictions. TOML config: `[signatures]` section with `enabled`, `heading`, `definitions`, `default_template`, and per-party overrides. Template resolution: explicit TOML → definitions file → hardcoded fallback. New modules: `signatures.rs` (template types, loading, resolution), `render/signatures.rs` (docx rendering). See `planning/signature-pages.md`.
- **Schedule refactor (phrase-based detection)** — replaced reference-link syntax (`[display][ref-id]` + `[ref-id]: #schedule "value"`) with phrase-based detection from defined terms. Schedules are declared in front-matter YAML (`schedule: [{title: "Schedule"}]`). Terms whose definition text contains phrases like "has the meaning given by the Schedule" are auto-collected into schedule pages. Multiple schedules supported. Pre-filled values dropped (schedules are always blank for completion). New TOML config: `schedule_order` (`document` or `alphabetical`). Removed `InlineContent::ScheduleRef` from model. Schedule collection and term validation merged into a single pass in `resolve.rs`.
- **Exhibit file import** — optional `path` field on exhibit entries. Supports PNG, JPEG (converted to PNG), and PDF. PDF rendering uses hayro (native Rust) by default with pdftoppm as a fallback. CLI flag `--pdf-renderer` controls backend: `auto` (default, hayro first then pdftoppm) or `pdftoppm` (force external tool). Images are scaled to fit within page margins preserving aspect ratio. Relative paths resolved against the input document's directory. When `path` is omitted, the existing placeholder page behaviour is preserved. New module: `render/exhibit.rs`. Dependencies: `image` 0.25 (PNG/JPEG decode), `hayro` 0.5 (native PDF rendering), `tempfile` 3 (pdftoppm workspace).
- **Attachment terminology refactor** — renamed "annexures" to three distinct concepts: **Schedule** (inline reference-linked values, unchanged), **Addendum** (body sections with `# ADDENDUM` headings, formerly "ANNEX"), **Exhibit** (front-matter `exhibits` list of external documents, generates placeholder pages with centred title). `Annexure`/`AnnexureContent` types renamed to `Addendum`/`AddendumContent`. Front-matter `annexures: Vec<String>` replaced with `exhibits: Vec<Exhibit>` (objects with `title` field). Addenda are auto-numbered sequentially, case-insensitive heading match, unrecognised `#` headings produce warnings.
- **Configurable defined term style** — `defined_term_style` in style TOML: `bold` (default), `quoted` (curly quotes, no bold), or `bold_quoted`. Applies to all `**bold**` text in body, preamble party roles/type, and custom templates.
- **Parties preamble** — parties preamble block renders before the contract body (independent of cover page). Three styles: `simple` (block layout), `prose` (single flowing paragraph), and `custom` (user-defined templates with `{title}`, `{type}`, `{date}`, `{name}`, `{specifier}`, `{role}` placeholders, `**bold**` markers, `\n` for paragraph breaks). Configured via `[preamble]` section in style TOML. Default disabled.
- **`type` front-matter field** — optional (defaults to "Agreement"), used in preamble text, automatically treated as a defined term.
- **Promoted `title_size` to top-level** — single `title_size` (default 20pt) controls the document title font size for both cover page and inline title. Removed from `[cover]`.
- **Promoted `date_format` to top-level** — single `date_format` (default `%e %B %Y`) used by cover page, preamble, and any future date rendering. Removed from `[cover]`.
- **Centred inline title** — when cover page is disabled, the title is centre-aligned. Status/version and date lines removed (handled by preamble/watermark/footer).
- **Sub-heading numbering styling** — clause numbers on heading paragraphs now inherit bold + heading size via paragraph `rPr`, so `###` sub-heading numbers match the heading text.
- **Simple numbered lists in addenda** — ordered lists without headings or nested sub-lists are now rendered as plain numbered lists (`1.`, `2.`, `3.`) rather than being fed through the clause numbering system.
- **Cover page / TOC toggles** — `[cover] enabled` and `[toc] enabled` in style TOML (default true). Without cover page, an inline title block is rendered. See `planning/cover-page-toc-toggles.md`.
- **Configurable cover page** — `[cover]` section in style TOML: title_size, date_format, between_label, party_format, show_ref, show_author, show_status. See `planning/configurable-cover-page.md`.
- **Footer config** — `[footer]` section in style TOML: show_ref, show_page_number, show_version (appends version to ref). See `planning/footer-and-schedule-config.md`.
- **Schedule position** — `schedule_position` in style TOML: `end` (default, after addenda/exhibits) or `after_toc` (before contract body). See `planning/footer-and-schedule-config.md`.
- **Draft watermark** when `status: draft` — VML WordArt shape injected via ZIP post-processing of the .docx output. See `planning/draft-watermark.md` for details.
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
  Heading(level=1)        ← # ADDENDUM headings
  Paragraph               ← prose/addendum content
  Table                   ← markdown tables
```

### Key Design Decisions
- Clause numbers via Word's native numbering engine (`AbstractNumbering` + `Numbering`) — proper hanging indents, automatic counting, Word-native restyling
- Single crate now, structured for workspace extraction later
- comrak resolves reference links during parsing, so `[text][ref-id]` becomes `Link(url="#schedule", title="value")` — the ref-id is lost but title carries the value
- Defined term matching uses multi-variant stemming, not a full NLP stemmer — pragmatic for legal text
