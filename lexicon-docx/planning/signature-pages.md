# Signature Pages

## Status: Implemented

## Overview

Signature pages render execution blocks for each party at the end of the document. The feature uses a template system with a crowd-sourced definitions file, convention-based defaults from front-matter, and TOML overrides for custom cases.

## Design Principles

- **YAML = contract data** — only `entity_type` is added to parties (what kind of legal entity). No rendering information in front-matter.
- **TOML = rendering overrides** — custom signatory titles, witness preferences, template selection. Only needed for non-standard cases.
- **Definitions file = template library** — a standalone `signatures.toml` mapping `(jurisdiction, entity_type, execution_method)` to templates. Lives in the repo, not compiled into the binary. Crowd-sourceable.
- **Convention-based defaults** — the processor infers the right template from `short_title` (deed vs agreement) and `entity_type` (company, individual, etc.) without any TOML config for common cases.

## YAML Changes

A single new optional field on parties — a compound `jurisdiction-type` string:

```yaml
short_title: Deed
parties:
  - name: Google Pty Ltd
    specifier: ACN 001 002 987
    role: Google
    entity_type: au-company
  - name: Jane Smith
    role: Merchant
    entity_type: au-individual
```

`entity_type` is a freeform string in `{jurisdiction}-{type}` format. The processor splits on the first hyphen to derive the template lookup key (e.g. `au-company` → `au.company.deed`).

Common values:
- `au-company`, `au-individual`, `au-trust`, `au-partnership`
- `uk-company`, `uk-individual`
- `us-company`, `us-individual`
- `nz-company`, `nz-individual`

The processor warns if no matching template is found in the definitions file or TOML overrides.

If `entity_type` is omitted, the processor skips the signature block for that party (or falls back to a generic block — TBD).

## Execution Method Detection

Inferred from `short_title`:
- If `short_title` is `"Deed"` → execution method is `deed`
- Anything else (including the default `"Agreement"`) → execution method is `agreement`

No new YAML field needed.

## Definitions File

A standalone TOML file (`signatures.toml`) containing the default template library, organised by jurisdiction, entity type, and execution method:

```toml
[au.company.deed]
intro = "**Executed as a deed by {name}** ({specifier}) in accordance with section 127 of the Corporations Act 2001 (Cth):"
layout = "columns"
signatories = [
  { title = "Director" },
  { title = "Director/Secretary" },
]
fields = [
  { type = "line" },
  { label = "Name" },
  { label = "Title", value = "{title}" },
]

[au.company.agreement]
intro = "**Signed for and on behalf of {name}** ({specifier}):"
layout = "columns"
signatories = [
  { title = "Director" },
  { title = "Director/Secretary" },
]
fields = [
  { type = "line" },
  { label = "Name" },
  { label = "Title", value = "{title}" },
]

[au.individual.deed]
intro = "**Signed sealed and delivered by {name}**:"
layout = "columns"
signatories = [{}]
witness = true
fields = [
  { type = "line" },
  { label = "Name", value = "{name}" },
  { label = "Date" },
]

[au.individual.agreement]
intro = "**Signed by {name}**:"
layout = "columns"
signatories = [{}]
fields = [
  { type = "line" },
  { label = "Name", value = "{name}" },
  { label = "Date" },
]
```

The witness block is standard across all templates when enabled:

```toml
[witness]
fields = [
  { type = "line" },
  { label = "Name" },
  { label = "Address" },
]
```

## Template Structure

Templates have two layers:

### 1. Prose layer — `intro`

A text string with `{placeholder}` substitution, reusing the preamble template system. Available placeholders:
- `{name}` — party name
- `{specifier}` — party specifier (e.g. ACN)
- `{role}` — party role
- `{short_title}` — document short title

Supports `**bold**` markers for emphasis.

### 2. Layout layer — structured fields

```toml
layout = "columns"           # each signatory rendered as a column, side by side
signatories = [              # default signatory list (overridable in TOML)
  { title = "Director" },
  { title = "Director/Secretary" },
]
fields = [
  { type = "line" },                      # signature line (cell bottom border)
  { label = "Name" },                     # blank field for handwriting
  { label = "Title", value = "{title}" }, # pre-filled from signatory data
]
witness = true/false         # whether to add a witness column
```

Field types:
- `type = "line"` — renders a cell with a bottom border (signature/witness line)
- `label` only — blank field with a label, for handwriting
- `label` + `value = "{placeholder}"` — pre-filled from signatory or party data

## TOML Overrides

The project's `style.toml` can override templates per party role, or set global defaults:

```toml
[signatures]
enabled = true                         # opt-in, default false
heading = "EXECUTION"                  # optional heading above blocks, omit for none
default_template = "au.company.deed"   # explicit override for all parties

[signatures.party.Google]
signatories = [
  { title = "Managing Director" },
  { title = "Company Secretary" },
]

[signatures.party.Merchant]
template = "au.individual.agreement"
witness = true
```

## Template Resolution Order

For each party, the processor resolves the template as:

1. **Explicit template** — if `signatures.party.{Role}.template` is set in TOML, use that
2. **Definitions file lookup** — split `entity_type` on first hyphen → `{jurisdiction}.{type}.{execution_method}` from `signatures.toml`
3. **Hardcoded fallback** — a minimal generic block ("Signed by {name}:" + one signature line)

Signatory lists resolve similarly: TOML party override → definitions file default → generic single signatory.

## Rendering

### docx output

- Each party gets a signature block, rendered in document order
- Blocks are separated by vertical spacing
- The intro paragraph is rendered as formatted text (bold, placeholders resolved)
- The field layout is a **borderless table** (`Table::without_borders()`)
  - One column per signatory (side by side)
  - If `witness = true`, an additional witness column is appended
  - Signature lines use **cell bottom borders** (not underscore text)
  - Labels are small/grey text below each field
  - Pre-filled values (e.g. title) are rendered as regular text
- All signature blocks appear on a new page at the end of the document, before exhibits

### Page structure

```
[Page break]
SIGNATURE PAGE (optional heading — TBD)

[Intro paragraph for Party 1]
[Borderless table: signatory columns + optional witness]

[Spacing]

[Intro paragraph for Party 2]
[Borderless table: signatory columns + optional witness]
```

## Implementation Notes

- `entity_type` field added to `Party` struct in `frontmatter.rs` (optional `String`)
- New module: `render/signatures.rs` for signature page rendering
- Definitions file resolved via `--signatures` CLI flag, or auto-discovered: input document directory → `$XDG_CONFIG_HOME/lexicon/` (same logic as `--style` / `style.toml`)
- Template parsing: new `signatures.rs` module in root for template types and resolution
- Intro text rendered using the existing preamble placeholder/bold system
- Borderless tables via `Table::without_borders()` with cell-level bottom borders for signature lines

## Resolved Questions

1. **Embed or disk?** — Load from disk. Allows user edits and community contributions without recompiling.
2. **Heading above blocks?** — Configurable via TOML (`[signatures] heading = "EXECUTION"`). Optional, no heading by default.
3. **Missing `entity_type`?** — Fall back to `us-individual` (generic single-signatory block). Emit a warning.
4. **Toggleable?** — Yes. `[signatures] enabled = true` in TOML (default false — signature pages are opt-in).
5. **Jurisdiction** — Resolved: compound `entity_type` field (`au-company`) encodes jurisdiction and type together.
