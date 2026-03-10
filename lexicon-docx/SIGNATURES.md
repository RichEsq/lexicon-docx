# Signature Pages

Lexicon can generate signature (execution) pages for your contracts. Signature blocks are rendered as borderless tables with proper signature lines, signatory columns, and optional witness fields.

## Quick Start

1. Add `entity_type` to your parties in the YAML front-matter:

```yaml
parties:
  - name: Acme Corp Pty Ltd
    specifier: ACN 123 456 789
    role: Acme
    entity_type: au-company
  - name: Jane Smith
    role: Consultant
    entity_type: au-individual
```

2. Enable signatures in your `style.toml`:

```toml
[signatures]
enabled = true
```

3. Place the `signatures.toml` definitions file alongside your contract, or in `$XDG_CONFIG_HOME/lexicon/` for a global default.

4. Build:

```bash
cargo run -- build contract.md -o contract.docx

# Or with explicit paths:
cargo run -- build contract.md --style style.toml --signatures signatures.toml
```

The processor automatically selects the right signature block template based on each party's `entity_type` and whether the document is a deed or agreement.

## How It Works

### Execution method detection

The execution method is inferred from the `short_title` front-matter field:

| `short_title` | Execution method |
|---------------|-----------------|
| `Deed`        | `deed`          |
| Anything else | `agreement`     |

The default `short_title` is `"Agreement"`, so documents without an explicit `short_title` use agreement-style execution wording.

### Entity type

The `entity_type` field on each party is a compound string in `{jurisdiction}-{type}` format:

| Value | Meaning |
|-------|---------|
| `au-company` | Australian company |
| `au-individual` | Australian individual |
| `au-sole_director` | Australian company with sole director |
| `uk-company` | UK company |
| `uk-individual` | UK individual |
| `us-company` | US company |
| `us-individual` | US individual |
| `nz-company` | New Zealand company |
| `nz-individual` | New Zealand individual |

The processor splits on the first hyphen to look up templates: `au-company` becomes `au.company.deed` or `au.company.agreement` in the definitions file.

If `entity_type` is omitted, the processor emits a warning and uses a generic fallback block.

### Template resolution

For each party, the processor resolves which template to use in this order:

1. **Explicit TOML override** -- if `signatures.party.{Role}.template` is set, use that template key
2. **Definitions file lookup** -- split `entity_type` on first hyphen, combine with execution method, look up in `signatures.toml`
3. **Hardcoded fallback** -- a minimal "Signed by {name}:" block with a single signature line

## Definitions File

The `signatures.toml` file contains the template library. It ships with templates for Australia, UK, US, and New Zealand, and can be extended with additional jurisdictions and entity types.

### Structure

Templates are keyed by `[jurisdiction.entity_type.execution_method]`:

```toml
[au.company.deed]
intro = "**Executed as a deed by {name}** ({specifier}) in accordance with section 127 of the Corporations Act 2001 (Cth):"
signatories = [
  { title = "Director" },
  { title = "Director/Secretary" },
]
fields = [
  { type = "line" },
  { label = "Name" },
  { label = "Title", value = "{title}" },
]
witness = false
```

### Template fields

| Field | Description |
|-------|-------------|
| `intro` | Introductory paragraph. Supports `{name}`, `{specifier}`, `{role}`, `{short_title}` placeholders and `**bold**` markers. |
| `signatories` | List of signatory objects. Each becomes a column in the rendered table. |
| `fields` | List of field definitions, rendered as rows in each signatory column. |
| `witness` | Whether to add a witness column alongside the signatories. |
| `witness_fields` | Custom fields for the witness column. Defaults to signature line, name, and address. |

### Field types

| Syntax | Renders as |
|--------|-----------|
| `{ type = "line" }` | A cell with a bottom border (signature line) |
| `{ label = "Name" }` | A blank field labelled "Name:" for handwriting |
| `{ label = "Title", value = "{title}" }` | A pre-filled field with the signatory's title |
| `{ label = "Name", value = "{name}" }` | A pre-filled field with the party's name |

Available value placeholders: `{title}` (from signatory), `{name}`, `{specifier}`, `{role}` (from party).

### Adding new templates

To add templates for a new jurisdiction, simply add new sections to `signatures.toml`:

```toml
[sg.company.agreement]
intro = "**Signed for and on behalf of {name}** ({specifier}):"
signatories = [
  { title = "Authorised Signatory" },
]
fields = [
  { type = "line" },
  { label = "Name" },
  { label = "Title", value = "{title}" },
  { label = "Date" },
]
witness = false
```

Then use `entity_type: sg-company` in your front-matter.

## TOML Configuration

All signature configuration lives in the `[signatures]` section of `style.toml`:

```toml
[signatures]
enabled = true                          # default: false (opt-in)
heading = "EXECUTION"                   # optional heading above all blocks
default_template = "au.company.deed"    # override template for all parties
```

The definitions file (`signatures.toml`) is resolved separately via the `--signatures` CLI flag, or auto-discovered from the input document's directory, then `$XDG_CONFIG_HOME/lexicon/`.

### Per-party overrides

Override the template, signatories, or witness setting for specific parties by role:

```toml
[signatures.party.Acme]
template = "au.sole_director.deed"
signatories = [
  { title = "Sole Director and Sole Company Secretary" },
]

[signatures.party.Consultant]
witness = true
```

The party key must match the `role` field from the YAML front-matter.

### Override precedence

Per-party TOML overrides take priority over the definitions file:

- `template` -- selects a different template entirely
- `signatories` -- replaces the signatory list (titles) while keeping the template's fields and intro
- `witness` -- overrides the template's witness setting

## Rendering

Signature blocks are rendered as:

1. A page break (signature blocks start on a new page)
2. An optional centred heading (e.g. "EXECUTION")
3. For each party:
   - The intro paragraph with placeholders resolved and bold text applied
   - A borderless table with one column per signatory (side by side), plus a witness column if enabled
   - Signature lines are rendered as cell bottom borders
   - Labels appear as small grey text; pre-filled values appear as regular text

Signature pages appear after the body clauses, before addenda and exhibits.

## Examples

### Australian company executing a deed

```yaml
parties:
  - name: Acme Corp Pty Ltd
    specifier: ACN 123 456 789
    role: Acme
    entity_type: au-company
```

With `short_title: Deed`, this renders:

> **Executed as a deed by Acme Corp Pty Ltd** (ACN 123 456 789) in accordance with section 127 of the Corporations Act 2001 (Cth):
>
> | _________________________ | _________________________ |
> | Name:                     | Name:                     |
> | Title: Director           | Title: Director/Secretary |

### Australian individual signing an agreement with witness

```yaml
parties:
  - name: Jane Smith
    role: Consultant
    entity_type: au-individual
```

```toml
[signatures.party.Consultant]
witness = true
```

With default `short_title` (Agreement), this renders:

> **Signed by Jane Smith**:
>
> | _________________________ | _________________________ |
> | Name: Jane Smith          | Witness name:             |
> | Date:                     | Address:                  |
