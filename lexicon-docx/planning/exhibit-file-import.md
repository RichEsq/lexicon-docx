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

## PDF Rendering Dependency

Phase 2 currently shells out to `pdftoppm` (from poppler-utils) to rasterise PDF pages to PNG. This is an external system dependency: `brew install poppler` on macOS, `apt install poppler-utils` on Debian/Ubuntu. It is only required when an exhibit has a `.pdf` path — PNG/JPEG exhibits are handled natively via the `image` crate.

### Alternatives evaluated (March 2025)

| Crate | Approach | License | Verdict |
|-------|----------|---------|---------|
| **pdfium-render** | Rust wrapper around Google's Pdfium; loads `.so`/`.dylib` at runtime | Apache-2.0/MIT (wrapper) + BSD-3 (Pdfium) | Best quality and API, but still requires an external binary (~25MB Pdfium shared library) |
| **mupdf** | Rust bindings; compiles MuPDF C source at build time, no runtime dep | **AGPL-3.0** | Excellent rendering, but AGPL infects the entire project unless commercially licensed |
| **hayro** | Pure Rust PDF renderer, no C dependencies, forbids unsafe | MIT/Apache-2.0 | Only truly zero-dependency option with a permissive license, but still early-stage (self-described). Worth revisiting as it matures |
| **poppler-rs** | Rust bindings to libpoppler-glib + cairo | **GPL** | Same underlying library as `pdftoppm`, same system deps, worse license — no advantage |

**Update (March 2025)**: Switched to **hayro** as the primary PDF renderer with `pdftoppm` as a fallback. The `--pdf-renderer` CLI flag controls backend selection: `auto` (default, tries hayro first then pdftoppm) or `pdftoppm` (forces the external tool). This eliminates the external dependency for most users while preserving a fallback for edge cases where hayro's rendering is insufficient.
