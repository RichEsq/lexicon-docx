pub mod error;
pub mod frontmatter;
pub mod model;
pub mod parser;
pub mod render;
pub mod resolve;
pub mod signatures;
pub mod style;

use std::path::Path;

use error::{Diagnostic, Result};
use model::{Document, Status};
use signatures::SignatureBlock;
use style::StyleConfig;

pub fn parse(input: &str) -> Result<Document> {
    parser::parse(input)
}

pub fn resolve(doc: &mut Document) {
    resolve::resolve(doc);
}

pub fn render_docx(doc: &Document, style: &StyleConfig, input_dir: Option<&Path>, signature_blocks: &[SignatureBlock]) -> Result<Vec<u8>> {
    render::docx::render_docx(doc, style, input_dir, signature_blocks)
}

pub fn process(input: &str, style: &StyleConfig, input_dir: Option<&Path>) -> Result<(Vec<u8>, Vec<Diagnostic>)> {
    let mut doc = parse(input)?;
    resolve(&mut doc);

    // Resolve signature blocks if enabled
    let mut sig_diagnostics = Vec::new();
    let signature_blocks = if style.signatures.enabled {
        // Load definitions file
        let defs_path = style.signatures.definitions.as_deref()
            .unwrap_or("signatures.toml");
        let defs_path = if Path::new(defs_path).is_absolute() {
            defs_path.to_string()
        } else {
            // Resolve relative to input directory
            input_dir
                .map(|d| d.join(defs_path).display().to_string())
                .unwrap_or_else(|| defs_path.to_string())
        };
        let definitions = signatures::load_definitions(Path::new(&defs_path), &mut sig_diagnostics);

        signatures::resolve_signature_blocks(
            &doc.meta.parties,
            doc.meta.short_title.as_deref(),
            style,
            &definitions,
            &mut sig_diagnostics,
        )
    } else {
        vec![]
    };

    doc.diagnostics.extend(sig_diagnostics);

    let mut bytes = render_docx(&doc, style, input_dir, &signature_blocks)?;
    if doc.meta.status == Some(Status::Draft) {
        bytes = render::watermark::inject_watermark(bytes, "DRAFT")?;
    }
    Ok((bytes, doc.diagnostics))
}
