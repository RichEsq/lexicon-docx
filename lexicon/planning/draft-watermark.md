# Draft Watermark Implementation

## Goal

When `status: draft` in the YAML front-matter, add a diagonal "DRAFT" watermark behind all pages of the generated .docx.

## How Word Watermarks Work

Word watermarks are **VML WordArt shapes** (shapetype #136) placed inside header XML parts. Each section's headers (even, default, first) contain an identical shape. The shape is:

- Centered on the page (horizontal + vertical, relative to margin)
- Rotated 315° (diagonal bottom-left to top-right)
- Silver fill, no stroke
- Large size (442.55pt × 193.6pt)
- Negative z-index (behind text)
- Font: Calibri, font-size: 1pt (auto-scaled by `fitshape="t"`)

## Why Post-Processing

docx-rs 0.4 does not expose VML shape APIs or watermark support. The watermark must be injected by post-processing the .docx ZIP after docx-rs generates it.

## Implementation Plan

### New module: `src/render/watermark.rs`

A single public function:

```rust
pub fn inject_watermark(docx_bytes: Vec<u8>, text: &str) -> Result<Vec<u8>>
```

Steps:

1. **Open** the .docx bytes as a ZIP archive (using `zip` crate for read, write)
2. **Parse** `word/document.xml` to find the `<w:sectPr>` elements and identify existing header references
3. **Inject watermark VML** into existing header XML parts:
   - The lexicon processor already creates a first-page header (empty) and a default header (empty, since footer carries the content). Need to check if docx-rs puts these in header1.xml/header2.xml or similar.
   - For each header part: parse the XML, find/create a `<w:p>` with `<w:pPr><w:pStyle w:val="Header"/></w:pPr>`, insert a `<w:r>` containing `<w:pict>` with the VML shapetype + shape.
   - If no header part exists for a section type, create one and add the corresponding `<w:headerReference>` to `<w:sectPr>` in document.xml.
4. **Update `[Content_Types].xml`** if new header parts were added (ensure header content type is registered).
5. **Update `word/_rels/document.xml.rels`** if new header parts were added.
6. **Re-pack** everything into a new ZIP and return the bytes.

### VML Template

The watermark VML is static except for the `string` attribute:

```xml
<w:r>
  <w:rPr><w:noProof/></w:rPr>
  <w:pict>
    <v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800"
      path="m@7,l@8,m@5,21600l@6,21600e">
      <v:formulas>...</v:formulas>
      <v:path textpathok="t" o:connecttype="custom" .../>
      <v:textpath on="t" fitshape="t"/>
      <v:handles><v:h position="#0,bottomRight" xrange="6629,14971"/></v:handles>
      <o:lock v:ext="edit" text="t" shapetype="t"/>
    </v:shapetype>
    <v:shape type="#_x0000_t136"
      style="position:absolute;margin-left:0;margin-top:0;
             width:442.55pt;height:193.6pt;rotation:315;z-index:-251651072;
             mso-position-horizontal:center;mso-position-horizontal-relative:margin;
             mso-position-vertical:center;mso-position-vertical-relative:margin"
      fillcolor="silver" stroked="f">
      <v:textpath style="font-family:&quot;Calibri&quot;;font-size:1pt" string="DRAFT"/>
    </v:shape>
  </w:pict>
</w:r>
```

### Integration Point

In `lib.rs` `process()`, after `render_docx()` returns bytes:

```rust
pub fn process(input: &str, style: &StyleConfig) -> Result<(Vec<u8>, Vec<Diagnostic>)> {
    let mut doc = parse(input)?;
    resolve(&mut doc);
    let mut bytes = render_docx(&doc, style)?;
    if doc.meta.status == Some(Status::Draft) {
        bytes = render::watermark::inject_watermark(bytes, "DRAFT")?;
    }
    Ok((bytes, doc.diagnostics))
}
```

### Dependencies

- Add `zip` crate to Cargo.toml for ZIP read/write

### Approach to XML Manipulation

Use string-based XML manipulation (find/replace with known patterns) rather than a full XML parser. The docx-rs output is predictable and we control the structure. This avoids adding an XML parsing dependency.

Specifically:
- For header XML: we know docx-rs creates headers with a simple `<w:hdr ...><w:p ...>...</w:p></w:hdr>` structure. Insert VML run into the paragraph.
- For document.xml sectPr: find `<w:sectPr` blocks and check/add `<w:headerReference>` entries.
- For rels and content types: append entries before closing tags.

### Testing

- Build example.md with `status: draft` and verify watermark appears in Word
- Build with `status: final` and verify no watermark
- Inspect generated .docx XML to confirm VML is well-formed
