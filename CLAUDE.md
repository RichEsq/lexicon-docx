# CLAUDE.md

## Project Overview

Lexicon is a plain-text legal contract format built on standard Markdown, plus a Rust CLI processor that converts Lexicon Markdown contracts into formatted .docx files.

The repository contains:
- `lexicon/` — git submodule ([RichEsq/lexicon](https://github.com/RichEsq/lexicon)) containing:
  - `spec.md` — the Lexicon Markdown specification (v1.0-draft)
  - `example.md` — a real-world Data Processing Addendum written in Lexicon format
- `src/`, `Cargo.toml`, etc. — the Rust CLI processor (root-level crate)

## First Steps

Before starting any work:

1. Update the lexicon submodule: `git submodule update --init --remote`
1. read **`lexicon/spec.md`** — the Lexicon Markdown specification. The spec is the source of truth for all parsing and validation rules.
2. read **`planning/implementation-status.md`** — what's done and what's remaining.
3. read **`planning/todo.md`** — open tasks and questions.
4. **Other planning files in `planning/`** — design notes for specific features.

## Specification

Lexicon Markdown extends standard Markdown with conventions for legal documents:
- **YAML front-matter** for contract metadata (title, date, parties, status, version, exhibits)
- **Nested ordered lists** for clause hierarchy (`1. ## Heading` → `1. text` → indented sub-clauses)
- **Bold = defined terms** — `**Term** means ...` is a definition; any other `**Term**` is a reference
- **Pandoc-style anchors** (`{#id}`) + standard links (`[clause X](#id)`) for cross-references
- **Reference links as schedule items** (`[display][ref-id]` with `[ref-id]: #schedule "value"`)

Full spec is in `lexicon/spec.md`. The spec is the source of truth for all parsing and validation rules.

## Development Commands

```bash
# Build the processor
cargo build

# Build a .docx from a Lexicon contract
cargo run -- build lexicon/example.md -o output.docx

# Validate a contract without generating output
cargo run -- validate lexicon/example.md

# Build with a custom style config
cargo run -- build lexicon/example.md -o output.docx --style style.toml

# Build with CLI style overrides (override TOML or defaults)
cargo run -- build lexicon/example.md --no-cover --page-size letter --font-family Arial

# Run with --strict to fail on warnings
cargo run -- build lexicon/example.md --strict

# Generate man pages
cargo run -- man --dir man/

# Run tests
cargo test
```

## Architecture

### Processor Pipeline

```
.md input
  → frontmatter.rs: YAML front-matter parsing + validation
  → parser/mod.rs: comrak Markdown AST parsing
  → parser/clause.rs: AST walk → Document IR (recitals, clauses, inlines, addenda)
  → parser/anchors.rs: {#id} anchor extraction
  → resolve.rs: clause numbering, cross-ref resolution, term validation, schedule phrase detection
  → signatures.rs: signature template loading, resolution, placeholder expansion
  → render/docx.rs: Document IR → .docx via docx-rs
  → render/signatures.rs: signature block rendering (borderless tables, cell borders)
  → main.rs: CLI, file I/O, diagnostic output
```

### Key Source Files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point (clap). Style override flags, man page generation |
| `src/lib.rs` | Public API: `parse()`, `resolve()`, `render_docx()`, `process()` |
| `src/model.rs` | Intermediate representation — Document, Clause, InlineContent, etc. |
| `src/frontmatter.rs` | YAML front-matter parsing with serde_yaml |
| `src/parser/clause.rs` | Core parser: comrak AST → clause tree, inline extraction, addendum parsing |
| `src/parser/anchors.rs` | Regex-based `{#id}` stripping |
| `src/resolve.rs` | Numbering (1., 1.1, (a), (i)), cross-refs, defined term validation, schedule phrase detection |
| `src/signatures.rs` | Signature template types, definitions file loading, resolution, placeholder expansion |
| `src/render/docx.rs` | DOCX generation — cover page, clauses, addenda, exhibits, tables, TOC item collection |
| `src/render/cover.rs` | Cover page and inline title rendering |
| `src/render/common.rs` | Shared rendering helpers — inline runs, tables, paragraphs |
| `src/render/addendum.rs` | Addendum page rendering |
| `src/render/schedule.rs` | Schedule page rendering — heading + item/particulars table |
| `src/render/signatures.rs` | Signature page rendering — borderless tables, cell-border signature lines, keep_next |
| `src/render/exhibit.rs` | Exhibit file import — image loading, PDF rendering, sizing |
| `src/render/watermark.rs` | Draft watermark injection via ZIP post-processing |
| `src/style.rs` | Style configuration with TOML override support |
| `src/error.rs` | Error types and diagnostics |

### Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` 4 | CLI argument parsing |
| `clap_mangen` | Man page generation from clap definitions |
| `comrak` | CommonMark + GFM Markdown → AST |
| `docx-rs` 0.4 | .docx file generation |
| `serde` + `serde_yaml` | YAML front-matter deserialization |
| `toml` | Style config file parsing |
| `regex` | Anchor pattern matching |
| `chrono` | Date validation |
| `thiserror` | Error type derivation |
| `zip` 2 | ZIP read/write for .docx post-processing (watermark) |
| `image` 0.25 | PNG/JPEG decoding + JPEG→PNG conversion for exhibit import |
| `tempfile` 3 | Temporary directories for PDF-to-image rendering (pdftoppm fallback) |
| `hayro` 0.5 | Native Rust PDF rendering for exhibit import (primary backend) |

### Design Decisions

- **Native Word numbering** — clause numbers use Word's numbering engine (`AbstractNumbering` + `Numbering` via docx-rs) with `multiLevelType="multilevel"` for proper hanging indents and Word integration.
- **Single crate** — structured so `lib.rs` can be extracted to a `lexicon-core` workspace crate later.
- **comrak for parsing** — produces a full AST. Headings inside list items work correctly. Reference links are resolved during parsing (schedule item values come from the link title attribute).
- **Defined term matching** — multi-variant stemming for possessives/plurals/verb forms, not a full NLP stemmer.
- **Draft watermark via ZIP post-processing** — docx-rs doesn't expose VML/watermark APIs, so `render/watermark.rs` post-processes the .docx ZIP to inject VML WordArt shapes into header XML parts. Triggered when `status: draft`.
- **Signature pages via external template definitions** — templates defined in `signatures.toml` (loaded from disk), keyed by `{jurisdiction}.{entity_type}.{execution_method}`. Two-layer templates: prose intro with `{placeholder}` substitution + structured field layout. Rendered as borderless tables with cell-border signature lines. `entity_type` on parties is a compound `{jurisdiction}-{type}` string (e.g. `au-company`). Execution method inferred from `type`.

### docx-rs Pitfalls

- **Do NOT use `Level::level_restart()`** — docx-rs 0.4 emits `<w:lvlRestart>` in the wrong position within `<w:lvl>` (after `pPr`/`rPr` instead of after `numFmt` as OOXML requires). Word silently ignores the entire level when this happens, while LibreOffice is lenient and renders it fine. Instead, rely on `multiLevelType="multilevel"` which gives automatic sub-level restarts.
- **Use `AbstractNumbering` IDs starting at 2** — docx-rs adds a default `abstractNum` with ID 1; using the same ID causes conflicts.
- **Set `multi_level_type` directly** — docx-rs has no builder method: `numbering.multi_level_type = Some("multilevel".to_string())`
- **Always test .docx output in Word**, not just LibreOffice — Word is much stricter about OOXML compliance.
- **docx-rs does not expose VML or watermark APIs** — watermarks require ZIP post-processing. The `zip` crate reads/rewrites the .docx archive to inject raw XML into header parts.
- **Do NOT use `TableCell::set_border()` for selective borders** — `set_border()` calls `unwrap_or_default()` internally, and `TableCellBorders::default()` creates all six borders (top, left, bottom, right, insideH, insideV). So setting a single bottom border gives you a full box. Instead, use `cell.set_borders(TableCellBorders::with_empty().set(border))` to start from no borders.
- **No cell-level margin methods on `TableCell`** — use `table.margins(TableCellMargins::new().margin(...))` at the table level instead.
- **docx-rs `.auto()` on `TableOfContents` double-escapes XML entities** — `raw_text()` returns pre-escaped text, then `Text::new()` escapes again. Build TOC items manually from the Document IR instead, passing raw un-escaped text to `TableOfContentsItem::text()`.
- **Use Word native spacing instead of blank paragraphs** — blank `Paragraph::new()` for vertical gaps should be replaced with `LineSpacing::new().before()/after()` on styles or paragraphs. This gives precise control and avoids extra elements.
- **Use `cantSplit` + `keep_next` to prevent table page breaks** — `TableRow::cant_split()` prevents a row from splitting across pages. To keep an entire table on one page, also set `keep_next = Some(true)` on all cell paragraphs (accessible via public `row.cells` / `cell.children` / `para.property.keep_next`).

## Planning

Future work and design notes are in `planning/`:
- `implementation-status.md` — what's done, what's remaining, architecture notes
- `library-extraction.md` — plan for extracting lexicon-core as a separate crate
- `exhibit-file-import.md` — exhibit file import (Phase 1+2 complete, URL import is future)
- `configurable-cover-page.md` — plan for making cover page elements configurable
- `native-word-numbering.md` — native Word numbering (implemented)
- `draft-watermark.md` — draft watermark via VML injection (implemented)
- `cover-page-toc-toggles.md` — cover page and TOC toggles in style TOML (implemented)
- `footer-and-schedule-config.md` — footer config options and schedule position (implemented)
- `signature-pages.md` — signature pages with template system and external definitions file (implemented)

## Implementation Status

Phases 1-5 are complete (cover page, clause parsing, legal numbering, cross-references, defined term validation, schedules (phrase-based detection), TOC, headers/footers, native Word numbering, draft watermark, cover page/TOC toggles, configurable cover page, footer config, schedule position config, parties preamble, type field, defined term style, custom preamble templates, attachment terminology refactor (addenda + exhibits), exhibit file import (PNG/JPEG/PDF with native hayro renderer + pdftoppm fallback), signature pages (template-based, external definitions file, short/long layout modes, separate_pages toggle, default enabled), recitals/background section (lettered (A)/(B)/(C), body heading requirement), native Word cross-references (bookmarks + internal hyperlinks, Ctrl+click navigation), CLI style override flags (all style.toml options as --flags, priority: CLI > local TOML > XDG TOML > defaults), man page generation (`lexicon-docx man`), TOC fixes (manual TOC item building to avoid docx-rs double-escaping, black TOC text via style), consistent Heading1 styling (section headings, addendum, exhibit, schedule, execution headings all use Heading1 with brand colour via style), heading/paragraph spacing config (heading_space_before/after, paragraph_space_before/after replacing blank paragraphs with Word native spacing), table layout (cantSplit on all table rows, keep_next on signature block cells)).

See `planning/implementation-status.md` for detailed status.

## Post-Work Checklist

After every successful piece of work (new feature, bug fix, spec change), complete ALL of the following before considering the task done:

1. **Update `CLAUDE.md`** — reflect any new files, dependencies, design decisions, or planning docs. Keep the Implementation Status line current.
2. **Update `planning/implementation-status.md`** — move completed items to "Recently completed", remove from "Not yet implemented".
3. **Run `cargo test`** — ensure all tests pass.
4. **Commit and push** — commit all changes with a descriptive message, then push to the remote.
