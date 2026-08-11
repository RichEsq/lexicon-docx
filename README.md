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

### Lint a contract

```bash
lexicon-docx lint contract.md
lexicon-docx lint contracts/*.md              # multiple files, per-file reports + total
lexicon-docx lint contract.md --format json   # machine-readable, for editors / CI / AI agents
lexicon-docx lint contract.md --format github # GitHub Actions annotations for PR diffs
lexicon-docx lint contract.md --strict        # fail on warnings too
lexicon-docx lint contract.md --ignore unused-anchor --min-severity warning
lexicon-docx lint contract.md --fix           # normalise hand-numbered ordinals in place
lexicon-docx lint --list-rules                # all rules with default severity
```

Checks documents without producing output: spec compliance (front-matter, cross-references, clause structure) plus drafting checks driven by the document's own metadata (unused defined terms, duplicate definitions, undeclared schedule references, missing exhibit files).

Each diagnostic carries a severity (`error` / `warning` / `info`), a stable rule code, a human-readable location (e.g. `clause 3.1(a)`), and the source line/column where known:

```
warning[unused-term]: 'Widget' is defined but never used in the document (clause 1.1, line 17)
```

Exit codes: `0` clean (warnings allowed unless `--strict`), `1` errors found (or warnings with `--strict`), `2` an input file was unreadable. With `--format json` the report is a single JSON object on stdout — always valid JSON, even when parsing fails:

```json
{
  "version": 1,
  "file": "contract.md",
  "valid": false,
  "summary": { "errors": 1, "warnings": 2, "info": 1 },
  "diagnostics": [
    {
      "level": "warning",
      "code": "unused-term",
      "message": "'Widget' is defined but never used in the document",
      "location": "clause 1.1",
      "line": 17,
      "column": 5
    }
  ]
}
```

`version` identifies the report format and is bumped on breaking changes. When linting multiple files, per-file reports nest under a `files` array next to an overall `valid` and `summary`.

`--format github` emits one [workflow command](https://docs.github.com/en/actions/reference/workflow-commands-for-github-actions) per diagnostic so findings appear as inline annotations on pull requests:

```
::warning file=contract.md,line=17,col=5,title=unused-term::'Widget' is defined but never used in the document (clause 1.1)
```

#### Configuring the linter

Rules can be disabled or re-levelled per project in the `[lint]` section of `style.toml` (resolved per input file: next to the document, then `$XDG_CONFIG_HOME/lexicon/`):

```toml
[lint]
ignore = ["unused-anchor"]

[lint.severity]
unused-term = "error"     # promote: fails the lint (and the build)
missing-date = "warning"
```

CLI flags merge on top: `--ignore <code>` (repeatable) adds to the ignore list, `--min-severity <error|warning|info>` hides lower-severity diagnostics from the report.

Individual findings can be suppressed inline with an HTML comment; `lexicon-ignore-file` disables rules for the whole file:

```markdown
1. **UCPA** means the Utah Consumer Privacy Act. <!-- lexicon-ignore: unused-term -->

<!-- lexicon-ignore-file: unused-anchor -->
```

Diagnostics are anchored to the **first line of the clause** (or addendum) that contains the finding — the line reported in the diagnostic — so place the comment on that line or the line directly above it. For a clause whose text spans several source lines, a comment on a continuation line will not match (the linter reports it as `unused-suppression` so it can't fail silently). A bare `<!-- lexicon-ignore -->` with no codes suppresses every rule anchored to those two lines — prefer naming the rule.

Comments pass through Markdown renderers invisibly and never appear in the .docx output. Directives only count in ordinary document text — inside code spans, code fences, or front-matter strings they are literal content and have no effect. Suppressions that match nothing are reported as `unused-suppression` (info) so stale ones don't accumulate, and suppression machinery diagnostics (`unused-suppression`, `unknown-lint-rule`, `invalid-suppression`, `invalid-lint-config`) cannot themselves be suppressed inline (use the config ignore list). `build` applies the same `[lint]` config and inline suppressions, so build and lint never disagree about a finding.

Guard rails: `parse-error`, `io-error`, and `style-error` can never be ignored or suppressed, and rules whose default severity is error (`invalid-date`, `exhibit-file-missing`, ...) cannot be demoted below warning — otherwise a report could claim `valid: true` for a document the renderer refuses to build. Invalid config entries are reported as `invalid-lint-config`.

#### Continuation indent

Continuation content — a paragraph, blockquote or table after a blank line inside a clause — must be indented to the clause's **content column**, which depends on the width of the marker as written in the source, not on the nesting level (spec 3.4):

```
content column = marker indent + len(marker literal) + spaces after marker
```

So `1. ` at indent 4 has a content column of 7, but `10. ` has 8 and `100. ` has 9. Indenting below that column is silent in both directions, which is why there are two rules for it:

- **`continuation-indent` (error)** — the block fell 4 or more spaces short, so CommonMark reinterpreted it as an indented code block and it is dropped from the output entirely.
- **`continuation-reattached` (warning)** — the block fell short by less, so it still renders, but under an ancestor clause rather than the one it was written for. Nothing is missing from the output, so proofreading will not catch it; in a contract this means a proviso silently governs the wrong clause. The message names both the clause it was aimed at and the clause it will actually render under.

The processor renumbers every item on render, so the ordinal you type is discarded. Writing `1.` for every marker keeps the content column fixed per level and makes the whole class unreachable — that is what `hand-numbered-ordinal` (info) recommends, and `--fix` applies it:

```bash
lexicon-docx lint contract.md --fix
```

`--fix` rewrites each wide ordinal to `1.` and dedents that item's content by the width the marker loses, so every column relationship in the source is preserved. The result is verified against the original by structural fingerprint before anything is written: if the rewrite would move, drop or re-level any content, it is abandoned and the file is left untouched. That happens when the document already has `continuation-indent` findings — narrowing a marker moves the content column under content that is already in the wrong place — so fix those first, then re-run.

#### Known limitations

- Diagnostics carry the start line of the containing clause or addendum, not the exact line of the offending text within it.
- A clause keeps only its **last** `{#id}` anchor; declaring more than one produces a `multiple-anchors` warning (put each anchor on its own sub-clause).
- Table cells are not scanned: terms, term usage, and cross-references inside tables are invisible to the linter (the Lexicon spec does not anticipate defined terms in tables).
- Schedule-phrase detection is heuristic. References to statutory schedules ("the Schedule to the Corporations Act") are recognised and skipped, but unusual phrasing may still be misread — check the generated schedule table when in doubt.

#### Lint rules

| Code | Severity | Meaning |
|------|----------|---------|
| `parse-error` | error | Document could not be parsed (missing/invalid front-matter or YAML) |
| `invalid-date` | error | `date` is not a valid `YYYY-MM-DD` date |
| `missing-parties` | error | No parties defined in front-matter |
| `missing-party-role` | error | A party has an empty `role` |
| `exhibit-file-missing` | error | A declared exhibit `path` does not exist |
| `exhibit-unsupported-type` | error | Exhibit file type is not png/jpg/jpeg/pdf |
| `exhibit-url-unsupported` | error | Exhibit `path` is a URL (not supported) |
| `continuation-indent` | error | Continuation content is indented too far below its clause and will be dropped |
| `broken-cross-ref` | warning | Cross-reference points to a non-existent anchor |
| `continuation-reattached` | warning | Continuation content will render under an ancestor clause, not the one it was written for |
| `unsupported-block` | warning | A block-level element the Lexicon format has no representation for |
| `duplicate-anchor` | warning | The same `{#id}` anchor is declared more than once |
| `duplicate-definition` | warning | A term is bold-defined at more than one place (bold marks definitions, not references) |
| `unused-term` | warning | A defined term (including party roles) never appears in the document text |
| `unreferenced-schedule` | warning | A declared schedule has no referencing terms |
| `undeclared-schedule` | warning | A definition references a schedule title not declared in front-matter |
| `bullet-outside-clause` | warning | Bullet list in the clause hierarchy (unnumbered, not cross-referenceable) |
| `unknown-top-heading` / `heading-after-body` | warning | Unexpected `#` top-level heading |
| `duplicate-recitals` / `missing-body-heading` | warning | Recitals structure issues |
| `signatures-*` / `signature-*` | warning | Signature template resolution issues (build only) |
| `unknown-lint-rule` | warning | Config, flag, or suppression names a rule code that doesn't exist |
| `invalid-suppression` | warning | Malformed suppression comment |
| `invalid-lint-config` | warning | Lint configuration entry that cannot take effect |
| `multiple-anchors` | warning | A clause declares more than one `{#id}` anchor; only the last is kept |
| `style-error` | error | Style configuration file could not be loaded |
| `unused-anchor` | info | An anchor is declared but never referenced |
| `missing-date` | info | No `date` set (rendered as a blank date line) |
| `missing-party-name` | info | A party has no `name` (rendered as a placeholder) |
| `unused-suppression` | info | A suppression comment matched no diagnostic |
| `hand-numbered-ordinal` | info | A source list ordinal of `10.` or wider (widens the clause content column) |

Run `lexicon-docx lint --list-rules` (add `--format json` for machine consumption) for the authoritative list. `validate` is an alias for `lint` with text output. Info-level diagnostics are shown by `lint`/`validate` but suppressed during `build`.

### Options

```
lexicon-docx build <INPUT> [OPTIONS]

Options:
  -o, --output <FILE>         Output .docx path (default: <input>.docx)
  -s, --style <FILE>          Style configuration (TOML)
      --signatures <FILE>     Signature template definitions (TOML)
      --strict                Fail on warnings (exit code 1)

lexicon-docx lint <INPUT>... [OPTIONS]

Options:
      --format <FORMAT>            Output format: text (default), json, or github
      --strict                     Fail on warnings as well as errors
      --ignore <CODE>              Disable a rule (repeatable)
      --min-severity <LEVEL>       Hide diagnostics below error|warning|info
  -s, --style <FILE>              Style configuration (supplies [lint] section)
      --numbering-convention <C>   Convention for clause references in messages
      --list-rules                 List all lint rules and exit
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
| `--numbering-convention <CONV>` | `commonwealth`, `decimal`, or `us-traditional` | commonwealth |
| `--body-align-first-level` | Align first-level body clauses with second level | off |
| `--no-body-align-first-level` | (opposite of above) | |
| `--recitals-align-first-level` | Align first-level recital clauses with second level | off |
| `--no-recitals-align-first-level` | (opposite of above) | |

**Formatting:**

| Flag | Description | Default |
|------|-------------|---------|
| `--date-format <FMT>` | chrono strftime format string | `%e %B %Y` |
| `--name-placeholder <TEXT>` | Placeholder for missing party names | `[Name]` |

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
| Legal numbering | Native Word numbering with three conventions: Commonwealth (`1.1`, `(a)`, `(i)`), Decimal (`1.1.1`), US Traditional (`I.`, `A.`, `1.`) |
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
numbering_convention = "commonwealth"  # "commonwealth", "decimal", or "us_traditional"
```

### Defined term rendering

```toml
defined_term_style = "bold"   # "bold", "quoted" (curly quotes), or "bold_quoted"
```

### Date formatting

```toml
date_format = "%e %B %Y"     # chrono strftime format
```

### Placeholder text

```toml
name_placeholder = "[Name]"  # shown when a party has no name
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
- Invalid front-matter (bad dates, missing party roles)

Use `--strict` to treat warnings as errors:

```bash
lexicon-docx build contract.md --strict
```

## License

MIT
