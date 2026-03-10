# Exhibit File Import

## Status: Phase 1+2 Complete (image + PDF import). Phase 3 (URL paths) is future work.

## Goal

Allow exhibits declared in the front-matter to reference external files that are imported directly into the generated .docx output, rather than producing placeholder pages.

## Proposed YAML Syntax

```yaml
exhibits:
  - title: "Property Map"
    path: "./property-map.pdf"
  - title: "Site Plan"
    path: "https://example.com/site-plan.png"
  - title: "Floor Plan"
    # No path — generates placeholder page (current behaviour)
```

The `path` field is optional. When omitted, the processor generates a centred title placeholder page (the current default behaviour). When provided, the processor imports the file.

## File Format Support

Supported formats:
- **Images** (PNG, JPG) — embed as inline images in the docx
- **PDF** — convert each page to an image and embed one per page

DOCX merging is out of scope (too complex — style/numbering conflicts).

## Implementation Considerations

- Resolve relative paths against the input document's directory
- URL paths require HTTP fetching (add `reqwest` or similar dependency)
- PDF-to-image rendering requires a library (e.g., `pdfium-render`, `pdf-render`, or shelling out to `pdftoppm`/`magick`)
- Images should be scaled to fit within page margins
- Each PDF page becomes a full-page image on its own docx page

## Suggested Phased Approach

1. **Phase 1**: Image files (PNG, JPG) — embed with `docx-rs` image support
2. **Phase 2**: PDF files — convert each page to an image and insert one per page
3. **Phase 3**: URL paths — HTTP fetch before import
