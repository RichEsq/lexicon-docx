# Plan: Attachment Terminology Refactor (Annexure → Addendum + Exhibit)

## Context

The project conflates two different legal concepts under "annexure":
- **Body content** (`# ANNEX N - Title` sections) — these are actually **addenda**: attached parts of the contract with substantive content that can be omitted without breaking the body.
- **Front-matter declarations** (`annexures: [...]`) — these are actually **exhibits**: pre-existing external documents included for reference, containing no legal terms (e.g. a property map).

This refactor introduces three clear terms:
1. **Schedule** — unchanged, already implemented (inline `#schedule` reference-link system)
2. **Addendum** — replaces "annexure" for body content sections
3. **Exhibit** — replaces "annexures" in front-matter YAML; renders a placeholder page per exhibit

## Changes

### 1. spec.md

- **Section 2.2.9**: Rename `annexures` → `exhibits`. Change type from list of strings to list of objects with `title` field. Update description: pre-existing documents included for reference, not legal terms. Note future `path` field for file import.
- **Section 8**: Rewrite. Split into:
  - **Addenda** — body sections marked with `# ADDENDUM N - Title`. Free-form markdown content. Same parsing rules as current annexures.
  - **Exhibits** — declared in front-matter `exhibits` field. Processor generates a placeholder page with centred title (e.g. "EXHIBIT 1 - Property Map"). Future: file import.
- **Section 3.8**: Update "annexure content" → "addendum content"
- **Section 9 example**: Update front-matter and body headings
- **Section 11 summary table**: Update annexure rows to addendum + exhibit

### 2. example.md

- Front-matter: `annexures: [...]` → `exhibits: []` (empty list — the example DPA has no external exhibits)
- Body headings: `# ANNEX N - ...` → `# ADDENDUM N - ...`

### 3. Rust code — model.rs

- `Annexure` → `Addendum`
- `AnnexureContent` → `AddendumContent`
- `Document.annexures: Vec<Annexure>` → `Document.addenda: Vec<Addendum>`
- `DocumentMeta.annexures: Vec<String>` → `DocumentMeta.exhibits: Vec<Exhibit>`
- Add `Exhibit` struct: `{ title: String }` (with serde Deserialize)

### 4. Rust code — parser/clause.rs

- Change `# ANNEX` heading detection to `# ADDENDUM` (clean break, no backwards compat)
- Rename all `annexure`/`annex` variables → `addendum`/`addenda`

### 5. Rust code — resolve.rs

- Rename `resolve_annexure_cross_refs` and related functions
- Update variable names

### 6. Rust code — render/docx.rs

- Rename `render_annexure()` → `render_addendum()`
- Add `render_exhibits()`: for each exhibit in `doc.meta.exhibits`, render a page break + centred title heading (e.g. "EXHIBIT 1 - {title}"), same style as addendum cover headings
- Exhibit pages render after addenda (and after/before schedule depending on `schedule_position`)

### 7. Rust code — frontmatter.rs

- No special validation needed beyond serde deserialization of the new `Exhibit` struct

### 8. Planning docs

- Create `lexicon-docx/planning/exhibit-file-import.md` — future feature: `path` field on exhibits for importing external documents (PDF, docx, images) into the generated output
- Update `implementation-status.md` — move annexure references to new terminology, add exhibit placeholder pages to completed
- Update `todo.md` — close item #2 (annexures in YAML), update item #4 (addenda cross-referencing)

### 9. CLAUDE.md

- Update all references from annexure → addendum/exhibit terminology

## File List

| File | Action |
|------|--------|
| `spec.md` | Rewrite sections 2.2.9, 3.8, 8, 9, 11 |
| `example.md` | Update front-matter and body headings |
| `lexicon-docx/src/model.rs` | Rename types and fields |
| `lexicon-docx/src/parser/clause.rs` | Update heading detection, rename variables |
| `lexicon-docx/src/parser/mod.rs` | Rename variables |
| `lexicon-docx/src/resolve.rs` | Rename functions and variables |
| `lexicon-docx/src/render/docx.rs` | Rename functions, add exhibit rendering |
| `lexicon-docx/src/frontmatter.rs` | No changes expected (serde handles it) |
| `lexicon-docx/src/style.rs` | Check for annexure references |
| `lexicon-docx/planning/exhibit-file-import.md` | New planning doc |
| `lexicon-docx/planning/implementation-status.md` | Update |
| `lexicon-docx/planning/todo.md` | Update |
| `CLAUDE.md` | Update terminology |

## Verification

1. `cargo test` — all tests pass
2. `cargo run -- build ../example.md -o test.docx` — builds successfully
3. Open test.docx in Word/LibreOffice:
   - Body headings say "ADDENDUM 1", "ADDENDUM 2", etc.
   - No exhibit pages (example has empty exhibits list)
4. Create a test document with `exhibits: [{title: "Property Map"}]` and verify a centred "EXHIBIT 1 - Property Map" placeholder page appears
5. Verify schedule still renders correctly (unchanged)
