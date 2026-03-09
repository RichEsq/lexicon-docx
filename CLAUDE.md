# CLAUDE.md

## Project Overview

Lexicon is a plain-text legal contract format built on standard Markdown, plus a Rust CLI processor that converts Lexicon Markdown contracts into formatted .docx files.

The repository contains:
- `spec.md` — the Lexicon Markdown specification (v1.0-draft)
- `example.md` — a real-world Data Processing Addendum written in Lexicon format
- `lexicon/` — the Rust CLI processor

## Specification

Lexicon Markdown extends standard Markdown with conventions for legal documents:
- **YAML front-matter** for contract metadata (title, date, parties, status, version, cover_page, toc, annexures)
- **Nested ordered lists** for clause hierarchy (`1. ## Heading` → `1. text` → indented sub-clauses)
- **Bold = defined terms** — `**Term** means ...` is a definition; any other `**Term**` is a reference
- **Pandoc-style anchors** (`{#id}`) + standard links (`[clause X](#id)`) for cross-references
- **Reference links as schedule items** (`[display][ref-id]` with `[ref-id]: #schedule "value"`)

Full spec is in `spec.md`. The spec is the source of truth for all parsing and validation rules.

## Development Commands

```bash
# Build the processor
cd lexicon
cargo build

# Build a .docx from a Lexicon contract
cargo run -- build ../example.md -o output.docx

# Validate a contract without generating output
cargo run -- validate ../example.md

# Build with a custom style config
cargo run -- build ../example.md -o output.docx --style style.toml

# Run with --strict to fail on warnings
cargo run -- build ../example.md --strict

# Run tests
cargo test
```

## Architecture

### Processor Pipeline

```
.md input
  → frontmatter.rs: YAML front-matter parsing + validation
  → parser/mod.rs: comrak Markdown AST parsing
  → parser/clause.rs: AST walk → Document IR (clauses, inlines, annexures)
  → parser/anchors.rs: {#id} anchor extraction
  → resolve.rs: clause numbering, cross-ref resolution, term validation
  → render/docx.rs: Document IR → .docx via docx-rs
  → main.rs: CLI, file I/O, diagnostic output
```

### Key Source Files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point (clap). Thin wrapper over lib.rs |
| `src/lib.rs` | Public API: `parse()`, `resolve()`, `render_docx()`, `process()` |
| `src/model.rs` | Intermediate representation — Document, Clause, InlineContent, etc. |
| `src/frontmatter.rs` | YAML front-matter parsing with serde_yaml |
| `src/parser/clause.rs` | Core parser: comrak AST → clause tree, inline extraction |
| `src/parser/anchors.rs` | Regex-based `{#id}` stripping |
| `src/resolve.rs` | Numbering (1., 1.1, (a), (i)), cross-refs, defined term validation |
| `src/render/docx.rs` | DOCX generation — cover page, clauses, annexures, tables |
| `src/render/watermark.rs` | Draft watermark injection via ZIP post-processing |
| `src/style.rs` | Style configuration with TOML override support |
| `src/error.rs` | Error types and diagnostics |

### Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` 4 | CLI argument parsing |
| `comrak` | CommonMark + GFM Markdown → AST |
| `docx-rs` 0.4 | .docx file generation |
| `serde` + `serde_yaml` | YAML front-matter deserialization |
| `toml` | Style config file parsing |
| `regex` | Anchor pattern matching |
| `chrono` | Date validation |
| `thiserror` | Error type derivation |
| `zip` 2 | ZIP read/write for .docx post-processing (watermark) |

### Design Decisions

- **Native Word numbering** — clause numbers use Word's numbering engine (`AbstractNumbering` + `Numbering` via docx-rs) with `multiLevelType="multilevel"` for proper hanging indents and Word integration.
- **Single crate** — structured so `lib.rs` can be extracted to a `lexicon-core` workspace crate later.
- **comrak for parsing** — produces a full AST. Headings inside list items work correctly. Reference links are resolved during parsing (schedule item values come from the link title attribute).
- **Defined term matching** — multi-variant stemming for possessives/plurals/verb forms, not a full NLP stemmer.
- **Draft watermark via ZIP post-processing** — docx-rs doesn't expose VML/watermark APIs, so `render/watermark.rs` post-processes the .docx ZIP to inject VML WordArt shapes into header XML parts. Triggered when `status: draft`.

### docx-rs Pitfalls

- **Do NOT use `Level::level_restart()`** — docx-rs 0.4 emits `<w:lvlRestart>` in the wrong position within `<w:lvl>` (after `pPr`/`rPr` instead of after `numFmt` as OOXML requires). Word silently ignores the entire level when this happens, while LibreOffice is lenient and renders it fine. Instead, rely on `multiLevelType="multilevel"` which gives automatic sub-level restarts.
- **Use `AbstractNumbering` IDs starting at 2** — docx-rs adds a default `abstractNum` with ID 1; using the same ID causes conflicts.
- **Set `multi_level_type` directly** — docx-rs has no builder method: `numbering.multi_level_type = Some("multilevel".to_string())`
- **Always test .docx output in Word**, not just LibreOffice — Word is much stricter about OOXML compliance.
- **docx-rs does not expose VML or watermark APIs** — watermarks require ZIP post-processing. The `zip` crate reads/rewrites the .docx archive to inject raw XML into header parts.

## Planning

Future work and design notes are in `lexicon/planning/`:
- `implementation-status.md` — what's done, what's remaining, architecture notes
- `library-extraction.md` — plan for extracting lexicon-core as a separate crate
- `configurable-cover-page.md` — plan for making cover page elements configurable
- `native-word-numbering.md` — native Word numbering (implemented)
- `draft-watermark.md` — draft watermark via VML injection (implemented)
- `cover-page-toc-toggles.md` — cover_page and toc front-matter booleans (implemented)

## Implementation Status

Phases 1-5 are complete (cover page, clause parsing, legal numbering, cross-references, defined term validation, schedule annexures, TOC, headers/footers, native Word numbering, draft watermark, cover page/TOC toggles).

See `lexicon/planning/implementation-status.md` for detailed status.
