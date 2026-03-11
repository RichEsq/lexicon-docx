# lexicon-docx

A Rust CLI that converts [Lexicon Markdown](../README.md) legal contracts into formatted Word (`.docx`) documents.

## Requirements

- [Rust](https://rustup.rs/) (2024 edition)

## Installation

```bash
cd lexicon-docx
cargo build --release
```

The binary is at `target/release/lexicon-docx`.

## Usage

### Build a `.docx`

```bash
lexicon-docx build contract.md -o contract.docx
```

The output flag is optional — without `-o`, the output file uses the input filename with a `.docx` extension.

### Validate without building

```bash
lexicon-docx validate contract.md
```

Parses the document, resolves cross-references, validates defined terms, and prints diagnostics without producing output.

### Options

```
lexicon-docx build <INPUT> [OPTIONS]

Options:
  -o, --output <FILE>         Output .docx path (default: <input>.docx)
  -s, --style <FILE>          Style configuration (TOML)
      --signatures <FILE>     Signature template definitions (TOML)
      --pdf-renderer <MODE>   PDF renderer: "auto" (default) or "pdftoppm"
      --strict                Fail on warnings (exit code 1)
```

### Config file resolution

Both `style.toml` and `signatures.toml` are auto-discovered without flags. The processor searches:

1. The input file's directory
2. `$XDG_CONFIG_HOME/lexicon/` (defaults to `~/.config/lexicon/`)

Explicit `--style` or `--signatures` flags override auto-discovery.

## Features

| Feature | Description |
|---------|-------------|
| Cover page | Title, parties, date, status, version, author, reference |
| Table of contents | Auto-generated from clause headings |
| Legal numbering | Native Word numbering: `1.`, `1.1`, `(a)`, `(i)` |
| Cross-references | `{#id}` anchors resolved to clause numbers |
| Defined terms | Bold terms validated for usage; warnings on unused terms |
| Parties preamble | Configurable introductory block with party details |
| Schedule pages | Terms referencing a schedule auto-collected into a completion table |
| Signature pages | Template-based execution blocks with jurisdiction-aware defaults |
| Exhibit pages | Imported images (PNG/JPG) and PDFs, or placeholder pages |
| Draft watermark | Diagonal "DRAFT" watermark when `status: draft` |
| Headers/footers | Document reference, page numbering, optional version |

## Style Configuration

Copy [`style.example.toml`](style.example.toml) and customise it. All fields are optional — sensible defaults are built in.

```bash
lexicon-docx build contract.md --style style.toml
```

### Typography and layout

```toml
font_family = "Times New Roman"
font_size = 12.0
heading_font_family = "Times New Roman"
title_size = 20.0
heading1_size = 14.0
heading2_size = 12.0
line_spacing = 1.5

page_size = "a4"              # "a4" or "letter"
margin_top_cm = 2.54
margin_bottom_cm = 2.54
margin_left_cm = 2.54
margin_right_cm = 2.54
```

### Clause indentation

```toml
indent_per_level_cm = 1.27
hanging_indent_cm = 1.27
align_first_level = false     # true: levels 0 and 1 share the same indent
```

### Defined term rendering

```toml
defined_term_style = "bold"   # "bold", "quoted" (curly quotes), or "bold_quoted"
```

### Date formatting

```toml
date_format = "%e %B %Y"     # chrono strftime format
```

### Cover page

```toml
[cover]
enabled = true
between_label = "BETWEEN"
party_format = "name_spec_role"  # "name_spec_role", "name_role", or "name_only"
show_ref = true
show_author = true
show_status = true
```

Set `enabled = false` for a minimal inline title instead of a full cover page.

### Parties preamble

```toml
[preamble]
enabled = false
style = "simple"              # "simple", "prose", or "custom"
```

With `style = "custom"`, you define templates with placeholders:

```toml
[preamble]
enabled = true
style = "custom"
template = "This {title} (**{type}**) is dated {date} between"
party_template = "{name} ({specifier}) (**{role}**)"
party_separator = "; and"
```

### Table of contents

```toml
[toc]
enabled = true
heading = "Contents"
```

### Footer

```toml
[footer]
show_ref = true
show_page_number = true
show_version = false          # appends version to ref, e.g. "OK:RP:20260115v3"
```

### Schedule

```toml
schedule_position = "end"     # "end" (after addenda/exhibits) or "after_toc"
schedule_order = "document"   # "document" (source order) or "alphabetical"
```

### Branding

```toml
brand_color = "#2E5090"       # applies to title and heading text
```

## Signature Pages

Signature blocks are generated from templates based on each party's `entity_type` and whether the document is a deed or agreement.

### Setup

1. Add `entity_type` to parties in the front-matter:

```yaml
parties:
  - name: Acme Corp Pty Ltd
    specifier: ACN 123 456 789
    role: Acme
    entity_type: au-company
```

2. Enable in `style.toml`:

```toml
[signatures]
enabled = true
```

3. Place `signatures.toml` alongside your contract or in `~/.config/lexicon/`.

### Entity types

| Value | Meaning |
|-------|---------|
| `au-company` | Australian company |
| `au-individual` | Australian individual |
| `au-sole_director` | Australian sole director company |
| `uk-company` | UK company |
| `uk-individual` | UK individual |
| `us-company` | US company |
| `us-individual` | US individual |
| `nz-company` | New Zealand company |
| `nz-individual` | New Zealand individual |

The execution method is inferred from the `type` front-matter field: `Deed` uses deed-style wording, anything else uses agreement-style.

### Template resolution

For each party, the processor resolves a template in this order:

1. Explicit TOML override (`signatures.party.{Role}.template`)
2. Definitions file lookup (from `entity_type` + execution method)
3. Hardcoded fallback (minimal signature block)

### Per-party overrides

```toml
[signatures.party.Acme]
template = "au.sole_director.deed"
signatories = [
  { title = "Sole Director and Sole Company Secretary" },
]

[signatures.party.Consultant]
witness = true
```

See [`SIGNATURES.md`](SIGNATURES.md) for the full signature page documentation, including how to write custom templates.

## Exhibits

Exhibits attach external documents to the contract. Declare them in the front-matter:

```yaml
exhibits:
  - title: Floor Plan
    path: ./floor-plan.png
  - title: Technical Specifications
```

Supported file types: PNG, JPEG, and PDF (rendered to images). When `path` is omitted, a placeholder page is generated. Relative paths are resolved against the input file's directory.

PDF rendering uses a built-in native renderer ([hayro](https://github.com/LaurenzV/hayro)) by default. If you encounter visual issues with a specific PDF, you can fall back to `pdftoppm`:

```bash
lexicon-docx build contract.md --pdf-renderer pdftoppm
```

This requires poppler-utils: `brew install poppler` (macOS) or `apt install poppler-utils` (Debian/Ubuntu).

## Diagnostics

The processor emits warnings and errors during validation:

- Undefined cross-references (broken `#anchor` links)
- Defined terms that are never used in the document
- Declared schedules with no referencing terms
- Missing signature definitions
- Invalid front-matter (bad dates, empty parties)

Use `--strict` to treat warnings as errors:

```bash
lexicon-docx build contract.md --strict
```

## License

MIT
