# Plan: Recitals / Background Section

## Context

Legal contracts commonly have a "recitals" or "background" section before the operative clauses, providing context for why the agreement exists. Lexicon currently has no dedicated syntax for this — prose before clauses just renders as plain paragraphs. This feature adds a first-class recitals section triggered by a `# Recitals` or `# Background` heading, with clause-like structure where the top level uses letters (A), (B), (C) instead of numbers.

## Design Decisions

- **Heading**: Accept both `# Recitals` and `# Background` (case-insensitive)
- **Content**: Same content types as the body (ordered lists, prose, tables). Ordered lists become clause hierarchies but top level is lettered (A), (B), (C) instead of numbered. Reuses existing clause parsing infrastructure.
- **Body heading required when recitals present**: For Markdown readability (Lexicon files must render well in plain Markdown viewers), a body heading is **mandatory** when recitals are used. The body heading visually separates recitals from operative clauses in both rendered .docx and plain Markdown. Without recitals, no body heading is needed (backward compatible).
- **Body heading syntax**: Accept a flexible set of headings for the operative section, e.g. `# Operative Provisions`, `# Terms`, `# Agreement`, or any level-1 heading that isn't recitals/addendum. The heading text is preserved and rendered as-is. Regex: anything that doesn't match `RECITALS_RE` or `ADDENDUM_RE` becomes the body heading (instead of the current warning).
- **Position**: Recitals render after preamble, before body heading and clauses
- **Defined terms**: Validated like the body
- **Cross-references**: Supported; resolve to "Recital A", "Recital A.1", etc.

## Implementation

### 1. Model (`src/model.rs`)

Add `Recitals` struct and `recitals: Option<Recitals>` field on `Document`:
```rust
pub struct Recitals {
    pub heading: String,  // Original text: "Recitals" or "Background"
    pub body: Vec<BodyElement>,  // Reuses BodyElement (Clause + Prose)
}
```

Add `body_heading: Option<String>` field on `Document` — stores the operative section heading text when present.

Extend `ClauseNumber` with recital variants:
```rust
RecitalTopLevel(char),                        // (A)
RecitalClause(char, u32),                     // A.1
RecitalSubClause(char, u32, char),            // A.1(a)
RecitalSubSubClause(char, u32, char, String), // A.1(a)(i)
```

With `full_reference()` returning "Recital A", "Recital A.1", etc.

### 2. Parser (`src/parser/clause.rs`)

Add regex: `(?i)^(recitals|background)$`

Modify `extract_body()` state machine — add `in_recitals` flag and `body_heading` capture:

```
in_recitals = false
body_heading = None

for each node:
  if Heading(1) matching RECITALS_RE:
    in_recitals = true, capture heading
  elif Heading(1) matching ADDENDUM_RE:
    in_recitals = false, start addendum
    if recitals.is_some() && body_heading.is_none() → warning: missing body heading
  elif Heading(1):
    if in_recitals:
      in_recitals = false
      body_heading = Some(heading_text)  // this heading starts the body
    elif recitals.is_none():
      warning (existing: unrecognised heading — update message)
    else:
      warning: unexpected second body heading
  elif in_recitals:
    add to recitals.body (paragraphs, ordered lists as lettered clauses, tables, etc.)
  else:
    add to body (existing logic)
```

Key changes from current behaviour:
- Level-1 headings that aren't recitals or addendum become the **body heading** (instead of producing a warning) when recitals are present
- If recitals are present but no body heading appears before addenda/end, emit a warning
- Without recitals, unknown level-1 headings still produce the existing warning (backward compatible)

Update return type to include `Option<Recitals>` and `Option<String>` (body heading).

### 3. Parser module (`src/parser/mod.rs`)

Pass `recitals` and `body_heading` from `extract_body()` into `Document` construction.

### 4. Resolution (`src/resolve.rs`)

- **Recital numbering**: `assign_recital_numbers()` — same logic as `assign_clause_numbers()` but top level uses uppercase letters (A, B, C)
- **Anchor map**: Add recital anchors using the new `ClauseNumber` variants
- **Defined terms**: Walk `doc.recitals.body` in term collection and usage scanning
- **Cross-references**: Resolve cross-refs in recital content

### 5. Numbering (`src/render/numbering.rs`)

New recital numbering definition — identical to clause numbering but level 0 uses `upperLetter` format `(%1)` instead of `decimal` format `%1.`. Sub-levels follow body pattern.

New constants: `RECITAL_ABSTRACT_NUM_ID`, `RECITAL_NUMBERING_ID`.

### 6. Renderer (`src/render/docx.rs`)

- Register recital numbering definitions alongside clause numbering
- Insert between preamble and body:
  1. Render recitals heading (bold, heading style for TOC inclusion)
  2. Render recitals body elements using recital numbering ID
  3. Render body heading (bold, heading style for TOC inclusion) if present
- Could add a `src/render/recitals.rs` module or keep inline

### 7. Spec (`spec.md`)

- Add section documenting recitals/background syntax
- Document the body heading requirement when recitals are present
- List accepted heading variants
- Show lettering scheme

### 8. Example (`example.md`)

Add recitals and body heading before first clause:
```markdown
# Background

1. The parties have entered into a principal agreement for the provision
   of advertising services (the **Principal Agreement**).

2. The Merchant wishes to engage Google to process certain personal data
   on its behalf in connection with the Principal Agreement.

# Operative Provisions

1. ## Definitions
   ...
```

### 9. Tests

- Parser: `# Recitals` and `# Background` recognised (case-insensitive)
- Parser: body heading captured when following recitals
- Parser: warning emitted when recitals present but no body heading
- Parser: ordered lists within recitals become lettered clauses
- Parser: prose paragraphs stay in recitals
- Resolver: letters A, B, C assigned; cross-refs and terms work
- Integration: full document with recitals + body heading parses correctly
- Backward compat: documents without recitals unchanged (unknown headings still warn)

## Key Files

| File | Change |
|------|--------|
| `src/model.rs` | `Recitals` struct, recital `ClauseNumber` variants, `recitals` + `body_heading` on `Document` |
| `src/parser/clause.rs` | `RECITALS_RE` regex, `extract_body()` state machine with body heading |
| `src/parser/mod.rs` | Pass recitals + body heading to `Document` |
| `src/resolve.rs` | Recital numbering, anchors, terms, cross-refs |
| `src/render/numbering.rs` | Letter-based numbering definition |
| `src/render/docx.rs` | Register numbering, render recitals heading + content + body heading |
| `spec.md` | Document recitals syntax and body heading requirement |
| `example.md` | Add recitals + body heading example |
| `CLAUDE.md` | Update status |
| `planning/implementation-status.md` | Mark completed |

## Verification

1. `cargo test` — all tests pass
2. `cargo run -- build ../example.md -o output.docx` — recitals with (A), (B), (C) between preamble and body, body heading visible
3. Open in Word — verify numbering, both headings in TOC, cross-references
4. Build existing document without recitals — no changes (backward compatible)
