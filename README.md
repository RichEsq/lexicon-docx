# lexicon-docx

A Rust CLI that converts [Lexicon Markdown](https://github.com/RichEsq/lexicon) legal contracts into formatted Word (`.docx`) documents.

[Website](https://lexicon.esq) | [Playground](https://play.lexicon.esq) | [Specification](https://github.com/RichEsq/lexicon/blob/main/spec.md) | [Example Document](https://github.com/RichEsq/lexicon/blob/main/example.md?plain=1)

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
      --strict                Fail on warnings (exit code 1)
```

### Config resolution and priority

Style settings are resolved in this order (highest priority first):

1. **CLI flags** — `--font-size 11`, `--no-cover`, etc.
2. **TOML file in the input directory** — `style.toml` next to the contract
3. **TOML file in XDG config** — `$XDG_CONFIG_HOME/lexicon/style.toml` (defaults to `~/.config/lexicon/`)
4. **Built-in defaults**

An explicit `--style` flag replaces steps 2–3 (the specified file is loaded, then CLI flags still override it).

Signature definitions (`signatures.toml`) follow the same discovery order (input dir → XDG), overridden by an explicit `--signatures` flag.

### CLI style overrides

Every style.toml setting can also be set from the command line. This is useful for one-off builds or scripting without creating a TOML file.

**Typography:**

| Flag | Description | Default |
|------|-------------|---------|
| `--font-family <NAME>` | Body text font family | Times New Roman |
| `--font-size <PT>` | Body text size in points | 12 |
| `--heading-font-family <NAME>` | Heading font family | Times New Roman |
| `--title-size <PT>` | Document title size in points | 20 |
| `--heading1-size <PT>` | Level 1 heading size in points | 14 |
| `--heading2-size <PT>` | Level 2 heading size in points | 12 |
| `--heading-space-before <PT>` | Space before section headings in points | 18 |
| `--heading-space-after <PT>` | Space after section headings in points | 12 |
| `--paragraph-space-before <PT>` | Space before paragraphs in points | 0 |
| `--paragraph-space-after <PT>` | Space after paragraphs in points | 6 |
| `--line-spacing <N>` | Line spacing multiplier | 1.5 |
| `--defined-term-style <STYLE>` | `bold`, `quoted`, or `bold-quoted` | bold |
| `--brand-color <HEX>` | Brand color (e.g. `"#2E5090"`) | none |

**Page layout:**

| Flag | Description | Default |
|------|-------------|---------|
| `--page-size <SIZE>` | `a4` or `letter` | a4 |
| `--margin-top <CM>` | Top margin in cm | 2.54 |
| `--margin-bottom <CM>` | Bottom margin in cm | 2.54 |
| `--margin-left <CM>` | Left margin in cm | 2.54 |
| `--margin-right <CM>` | Right margin in cm | 2.54 |

**Clause indentation:**

| Flag | Description | Default |
|------|-------------|---------|
| `--indent-per-level <CM>` | Indent per clause level in cm | 1.27 |
| `--hanging-indent <CM>` | Hanging indent for numbers in cm | 1.27 |
| `--body-align-first-level` | Align first-level body clauses with second level | off |
| `--no-body-align-first-level` | (opposite of above) | |
| `--recitals-align-first-level` | Align first-level recital clauses with second level | off |
| `--no-recitals-align-first-level` | (opposite of above) | |

**Formatting:**

| Flag | Description | Default |
|------|-------------|---------|
| `--date-format <FMT>` | chrono strftime format string | `%e %B %Y` |

**Cover page:**

| Flag | Description | Default |
|------|-------------|---------|
| `--cover` / `--no-cover` | Enable/disable cover page | on |
| `--cover-between-label <TEXT>` | "Between" label on cover | BETWEEN |
| `--cover-party-format <FMT>` | `name-spec-role`, `name-role`, or `name-only` | name-spec-role |
| `--cover-ref` / `--no-cover-ref` | Show/hide reference on cover | on |
| `--cover-author` / `--no-cover-author` | Show/hide author on cover | on |
| `--cover-status` / `--no-cover-status` | Show/hide status on cover | on |

**Table of contents:**

| Flag | Description | Default |
|------|-------------|---------|
| `--toc` / `--no-toc` | Enable/disable table of contents | on |
| `--toc-heading <TEXT>` | TOC heading text | Contents |

**Footer:**

| Flag | Description | Default |
|------|-------------|---------|
| `--footer-ref` / `--no-footer-ref` | Show/hide reference in footer | on |
| `--footer-page-number` / `--no-footer-page-number` | Show/hide page numbers | on |
| `--footer-version` / `--no-footer-version` | Show/hide version in footer | off |

**Preamble:**

| Flag | Description | Default |
|------|-------------|---------|
| `--preamble` / `--no-preamble` | Enable/disable parties preamble | off |
| `--preamble-style <STYLE>` | `simple`, `prose`, or `custom` | simple |

**Schedule:**

| Flag | Description | Default |
|------|-------------|---------|
| `--schedule-position <POS>` | `end` or `after-toc` | end |
| `--schedule-order <ORDER>` | `document` or `alphabetical` | document |

**Signatures:**

| Flag | Description | Default |
|------|-------------|---------|
| `--enable-signatures` / `--no-signatures` | Enable/disable signature pages | on |
| `--signatures-heading <TEXT>` | Heading text for signature section | none |
| `--signatures-template <KEY>` | Default signature template key | none |
| `--signatures-separate-pages` | Each signature block on its own page | off |

> **Note:** Preamble templates (`preamble.template`, `preamble.party_template`, `preamble.party_separator`) and per-party signature overrides (`signatures.party.*`) are TOML-only — they contain structured data that doesn't lend itself to CLI flags.

### Man pages

Generate man pages with:

```bash
lexicon-docx man --dir man/
```

This creates `lexicon-docx.1`, `lexicon-docx-build.1`, and `lexicon-docx-validate.1` in the output directory. Install them to your man path (e.g. `/usr/local/share/man/man1/`) to use with `man lexicon-docx`.

## Features

| Feature | Description |
|---------|-------------|
| Cover page | Title, parties, date, status, version, author, reference |
| Table of contents | Auto-generated from clause headings |
| Legal numbering | Native Word numbering: `1.`, `1.1`, `(a)`, `(i)`, `(A)`, `(I)` |
| Cross-references | `{#id}` anchors resolved to clickable Word hyperlinks |
| Defined terms | Bold terms validated for usage; warnings on unused terms |
| Recitals / Background | Optional pre-body section with independent numbering |
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
body_align_first_level = false      # true: body levels 0 and 1 share the same indent
recitals_align_first_level = false  # true: recitals levels 0 and 1 share the same indent
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

### Spacing

```toml
heading_space_before = 18.0   # space before section headings (pt)
heading_space_after = 12.0    # space after section headings (pt)
paragraph_space_before = 0.0  # space before paragraphs (pt)
paragraph_space_after = 6.0   # space after paragraphs (pt)
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

Supported file types: PNG, JPEG, and PDF (rendered to images via [hayro](https://github.com/LaurenzV/hayro), a native Rust PDF renderer — no external dependencies required). When `path` is omitted, a placeholder page is generated. Relative paths are resolved against the input file's directory.

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
