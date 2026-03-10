pub mod error;
pub mod frontmatter;
pub mod model;
pub mod parser;
pub mod render;
pub mod resolve;
pub mod style;

use std::path::Path;

use error::{Diagnostic, Result};
use model::{Document, Status};
use style::StyleConfig;

pub fn parse(input: &str) -> Result<Document> {
    parser::parse(input)
}

pub fn resolve(doc: &mut Document) {
    resolve::resolve(doc);
}

pub fn render_docx(doc: &Document, style: &StyleConfig, input_dir: Option<&Path>) -> Result<Vec<u8>> {
    render::docx::render_docx(doc, style, input_dir)
}

pub fn process(input: &str, style: &StyleConfig, input_dir: Option<&Path>) -> Result<(Vec<u8>, Vec<Diagnostic>)> {
    let mut doc = parse(input)?;
    resolve(&mut doc);
    let mut bytes = render_docx(&doc, style, input_dir)?;
    if doc.meta.status == Some(Status::Draft) {
        bytes = render::watermark::inject_watermark(bytes, "DRAFT")?;
    }
    Ok((bytes, doc.diagnostics))
}
