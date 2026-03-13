# Signature Panel Layout Modes: "Short" vs "Long"

## Context

The current signature rendering produces the same field layout for all templates — each field is a single table row with either a bottom-bordered cell (signature line) or an inline `Label: Value` cell. This matches the compact "short" style common in US documents, but doesn't match the "long" style used in AU/UK/NZ documents where each writable field is a pair: a tall bordered writing space + a caption label underneath.

**Goal**: Add two layout modes so templates can produce either style.

## Design

### "Short" layout (US-style, current behaviour)
- Prose intro paragraph
- Each field = one table row
- `type = "line"` → empty cell with bottom border
- Default → `Label:` or `Label: Value` inline text

### "Long" layout (AU/UK/NZ-style, new)
- Prose intro paragraph
- Each field = **two** table rows:
  1. **Space row**: cell with bottom border for writing. Taller for `type = "line"` (signatures), shorter for default fields (names/dates).
  2. **Label row**: smaller grey caption text (e.g. "Director Signature", "Print Name")
- Multi-column support (e.g. Director | gap | Director/Secretary)
- Labels support `{title}` placeholder expansion (e.g. `label = "{title} Signature"` → "Director Signature")

### Template `layout` field
- Per-template `layout` field in `signatures.toml`: `"short"` or `"long"`
- **Default: `"long"`**
- US templates explicitly set `layout = "short"`

## Changes

### 1. Data model (`src/signatures.rs`)

**Add `Layout` enum** (near `FieldType` enum, ~line 33):
```rust
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Layout { Short, Long }

impl Default for Layout {
    fn default() -> Self { Layout::Long }
}
```

**Add `layout` field** to:
- `SignatureBlock` (line 12) — `pub layout: Layout`
- `TemplateDefinition` (line 46) — `#[serde(default)] pub layout: Layout`

**Thread layout through resolution** in `resolve_party_block()` (line 149):
- Extract `t.layout` from the template in the `Some(t)` arm (line 216)
- Set `layout` on the `SignatureBlock` at line 252
- `hardcoded_fallback()` (line 277) returns `Layout::Short` as part of its tuple (becomes 6-tuple)

### 2. Placeholder expansion in labels

The existing `expand_field_value()` (line 339) already handles `{title}`, `{name}`, `{specifier}`, `{role}`. The renderer will call this same function for label text in long mode. No changes to `signatures.rs` needed — just call it from the renderer.

### 3. Rendering (`src/render/signatures.rs`)

**Add `Layout` to imports** (line 8).

**Branch on layout** in the row-generation loop (lines 92–136). Extract the current loop body into a helper for short mode, add new long mode logic:

**Short mode**: unchanged — one `TableRow` per field via existing `render_field_cell()`.

**Long mode**: each field produces two `TableRow`s:
- **Space row**: cell with bottom border (all fields, not just `type = "line"`). Height via non-breaking space `"\u{00A0}"` sized to:
  - `FieldType::Line`: ~56 half-pts (28pt — signature writing space)
  - `FieldType::Blank`: ~32 half-pts (16pt — name/date writing space)
  - If field has a pre-filled `value`, render it in this cell instead of the NBSP
- **Label row**: smaller grey text (`label_half_pts`, color `#666666`), no borders. Label text passed through `expand_field_value()` for per-signatory placeholder expansion (e.g. `{title}`).

Multi-signatory alignment: when `field_idx >= fields.len()` for one column, produce two empty rows (space + label both empty) to keep columns aligned.

New helper functions:
- `render_long_space_cell()` — bordered cell with height-controlling content
- `render_long_label_cell()` — small grey caption text, no borders

### 4. Template updates (`signatures.toml`)

**US templates** — add `layout = "short"`, fields unchanged:
```toml
[us.company.agreement]
layout = "short"
# ... rest unchanged
```

**AU/UK/NZ templates** — omit `layout` (defaults to "long"), update fields to caption style:
```toml
[au.company.deed]
# layout defaults to "long"
intro = "**Executed as a deed by {name}** ({specifier}) in accordance with section 127 of the Corporations Act 2001 (Cth):"
signatories = [
  { title = "Director" },
  { title = "Director/Secretary" },
]
fields = [
  { type = "line", label = "{title} Signature" },
  { label = "Print Name" },
]
witness = false
```

Key changes to AU/UK/NZ templates:
- `{ type = "line" }` gains `label = "{title} Signature"` or similar caption
- `{ label = "Name" }` becomes `{ label = "Print Name" }` (caption style)
- `{ label = "Title", value = "{title}" }` removed — the signatory role is already conveyed by the signature line label
- Witness fields similarly updated with caption-style labels

### 5. Style config (`src/style.rs`)

No new config fields needed. Use hardcoded defaults for space heights (28pt / 16pt). Can add configurable heights later if needed.

## Files to modify

| File | Change |
|------|--------|
| `src/signatures.rs` | Add `Layout` enum, add `layout` to `TemplateDefinition` + `SignatureBlock`, thread through resolution |
| `src/render/signatures.rs` | Branch on layout in row generation, add long-mode two-row rendering, placeholder expansion for labels |
| `signatures.toml` | Add `layout = "short"` to US templates, update AU/UK/NZ field labels for long-mode captions |

## Verification

1. `cargo test` — all existing tests pass
2. `cargo run -- build ../example.md -o output.docx` — example.md uses `au-company` and `au-individual` entity types, so should produce "long" layout signature blocks
3. Open `output.docx` in Word and verify:
   - AU signature blocks have two-row field pairs (space + caption)
   - Signature lines are visually taller than name/date lines
   - Labels expand placeholders correctly (e.g. "Director Signature")
4. Test with a US-style document to verify "short" layout is unchanged
