# Configurable Cover Page

## Goal

Make the cover page layout and content configurable via the style TOML file, so different firms/courts can customise the output.

## What should be configurable

### Layout
- Element ordering (e.g., title, date, parties, ref — reorderable)
- Spacing between elements
- Alignment per element (currently all centered)
- Whether to include a cover page at all (some contracts start directly with clauses)

### Content
- Title font size (currently 20pt, hardcoded)
- "BETWEEN" label text (some jurisdictions use different wording)
- Party block format (e.g., whether role appears as `(the "Employer")` or `("Employer")` or just `Employer`)
- Date format (e.g., "15 January 2026" vs "15/01/2026" vs "January 15, 2026")
- Whether to show ref, author, status, version on cover
- Custom logo/image at top of cover page

### Party specifier display
- Whether specifier appears in parentheses, on a new line, or is hidden
- Whether to include an ABN/ACN label prefix

## Current implementation

All cover page rendering is in `src/render/docx.rs` in the `render_cover_page` function. Styles are partially sourced from `StyleConfig` but most cover-specific formatting is hardcoded.

## Approach

Extend `StyleConfig` with a `[cover]` section in the TOML:

```toml
[cover]
enabled = true
title_size = 20.0
date_format = "%e %B %Y"
between_label = "BETWEEN"
party_format = "name_spec_role"   # or "name_role", "name_only"
show_ref = true
show_author = true
show_status = true
```

All fields optional with sensible defaults matching the current hardcoded values.
