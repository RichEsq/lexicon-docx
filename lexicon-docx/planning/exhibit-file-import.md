# Exhibit File Import

## Status: Future Work

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

Potential formats to support:
- **Images** (PNG, JPG) — embed as inline images in the docx
- **PDF** — convert pages to images and embed, or use OLE embedding
- **DOCX** — merge content from the external docx into the output

PDF and DOCX import are non-trivial. Images are the simplest starting point.

## Implementation Considerations

- Need to resolve relative paths against the input document's directory
- URL paths would require HTTP fetching (add `reqwest` or similar dependency)
- PDF rendering would require a PDF-to-image library (e.g., `pdf-render`, `pdfium`)
- DOCX merging is complex — would need to parse and merge styles, numbering, etc.

## Suggested Phased Approach

1. **Phase 1**: Image files (PNG, JPG) — embed with `docx-rs` image support
2. **Phase 2**: PDF files — convert to images and embed
3. **Phase 3**: URL paths — HTTP fetch before import
4. **Phase 4**: DOCX merging (if needed)
