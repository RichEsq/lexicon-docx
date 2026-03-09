# Cover Page & TOC Toggle Booleans

## Goal

Add two optional front-matter booleans:
- `cover_page: true|false` (default: `true`)
- `toc: true|false` (default: `true`)

## Behaviour Matrix

| cover_page | toc   | Result |
|------------|-------|--------|
| true       | true  | Cover page → page break → TOC → page break → body (current behaviour) |
| true       | false | Cover page → page break → body |
| false      | true  | Inline title block → TOC (same page) → page break → body |
| false      | false | Inline title block → body (no page break before body) |

### Inline title block (no cover page)

When `cover_page: false`, the title appears at the top of the first page as a simple heading — no spacers, no centred parties block, no "BETWEEN" section. Just:
- Title (bold, heading size)
- Status/version line (if present)
- Date

This flows directly into the TOC (if enabled) or body content on the same page.

### Header/footer behaviour

- With cover page: first-page header/footer remain empty (current behaviour with `<w:titlePg/>`), default footer has ref + page numbers
- Without cover page: no first-page suppression needed — footer with ref + page numbers appears on all pages

## Implementation

### model.rs
Add to `DocumentMeta`:
```rust
#[serde(default = "default_true")]
pub cover_page: bool,
#[serde(default = "default_true")]
pub toc: bool,
```

### render/docx.rs
Restructure the rendering flow in `render_docx()`:
1. If `cover_page`: render cover page + first-page header/footer suppression + page break
2. If `!cover_page`: render inline title block, skip first-page header/footer setup
3. If `toc`: render TOC + page break
4. Render body content
