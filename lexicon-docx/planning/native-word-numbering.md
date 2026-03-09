# Native Word Numbering

## Goal

Replace the current text-prefix clause numbering with Word's native numbering engine via docx-rs, for proper hanging indents, automatic counting, and better Word integration.

## Current approach

Clause numbers are rendered as text prefixes in paragraphs with left indentation:
```
[indent=0]    "1.\tDefinitions"
[indent=720]  "1.1\tApplicable Laws means..."
[indent=1440] "(a)\tEuropean Union or..."
[indent=2160] "(i)\tthe pseudonymisation..."
```

This works but wrapped lines don't hang-indent (they align to the left margin of that indent level, not past the number).

## Native numbering approach

### docx-rs API

```rust
// 1. Define abstract numbering with 4 levels
let abstract_num = AbstractNumbering::new(1)
    .add_level(
        Level::new(0, Start::new(1), NumberFormat::new("decimal"), LevelText::new("%1."), LevelJc::new("left"))
            .indent(Some(720), Some(SpecialIndentType::Hanging(360)), None, None)
    )
    .add_level(
        Level::new(1, Start::new(1), NumberFormat::new("decimal"), LevelText::new("%1.%2"), LevelJc::new("left"))
            .indent(Some(1440), Some(SpecialIndentType::Hanging(360)), None, None)
    )
    .add_level(
        Level::new(2, Start::new(1), NumberFormat::new("lowerLetter"), LevelText::new("(%3)"), LevelJc::new("left"))
            .indent(Some(2160), Some(SpecialIndentType::Hanging(360)), None, None)
    )
    .add_level(
        Level::new(3, Start::new(1), NumberFormat::new("lowerRoman"), LevelText::new("(%4)"), LevelJc::new("left"))
            .indent(Some(2880), Some(SpecialIndentType::Hanging(360)), None, None)
    );

// 2. Create numbering instance
let numbering = Numbering::new(1, 1); // id=1, abstract_id=1

// 3. Register with document
docx = docx
    .add_abstract_numbering(abstract_num)
    .add_numbering(numbering);

// 4. Tag paragraphs
Paragraph::new()
    .numbering(NumberingId::new(1), IndentLevel::new(0))  // top-level: "1."
    .add_run(Run::new().add_text("Definitions"))
```

### Level mapping

| Clause level | Numbering level | Format | Pattern | Example |
|-------------|----------------|--------|---------|---------|
| TopLevel | 0 | decimal | `%1.` | 1. |
| Clause | 1 | decimal | `%1.%2` | 1.1 |
| SubClause | 2 | lowerLetter | `(%3)` | (a) |
| SubSubClause | 3 | lowerRoman | `(%4)` | (i) |

### What changes

- `resolve.rs` — clause number assignment logic can be simplified (Word counts automatically), but we still need `ClauseNumber` for cross-reference resolution (anchor → "clause 1.2")
- `render/docx.rs` — register abstract numbering + numbering instance on the Docx, replace text prefix rendering with `.numbering(id, level)` on clause paragraphs
- Remove manual number formatting from paragraph text

### Benefits

- Hanging indents — wrapped lines align past the number
- Word-native — users can restyle numbering in Word UI
- Cleaner TOC integration
- Automatic restart of sub-numbering per parent clause

### Risks

- Need to verify docx-rs `Level` API supports `LevelText` with multi-level patterns like `%1.%2`
- Restart behaviour: sub-clause counters must reset when a new parent clause starts — may need `LevelRestart` settings
- Top-level clauses with headings (## Heading) are currently separate paragraphs from the number — need to combine or ensure numbering attaches correctly to the heading paragraph

### Status

**Implemented.** Native Word numbering is now active in `render/docx.rs`. The abstract numbering defines 4 levels (decimal → decimal → lowerLetter → lowerRoman) with `level_restart` for automatic counter resets. Annexure clause lists get separate `Numbering` instances with `LevelOverride` start resets. Level 0 has bold/heading-font run properties; levels 1-3 inherit document defaults.
