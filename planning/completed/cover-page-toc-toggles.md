# Cover Page & TOC Toggle Booleans

## Status: Implemented

Cover page and TOC toggles are now in the style TOML config (not front-matter), since they are rendering/output concerns rather than contract metadata.

## Configuration

```toml
[cover]
enabled = true          # false: inline title block instead of full cover page

[toc]
enabled = true
```

## Behaviour Matrix

| cover.enabled | toc.enabled | Result |
|---------------|-------------|--------|
| true          | true        | Cover page → page break → TOC → page break → body |
| true          | false       | Cover page → page break → body |
| false         | true        | Inline title block → TOC (same page) → page break → body |
| false         | false       | Inline title block → body (no page break before body) |

### Inline title block (no cover page)

When `cover.enabled = false`, the title appears at the top of the first page as a simple heading — no spacers, no centred parties block, no "BETWEEN" section. Just:
- Title (bold, heading size)
- Status/version line (if present)
- Date

This flows directly into the TOC (if enabled) or body content on the same page.

### Header/footer behaviour

- With cover page: first-page header/footer remain empty (via `<w:titlePg/>`), default footer has ref + page numbers
- Without cover page: no first-page suppression — footer with ref + page numbers appears on all pages
