pub mod anchors;
pub mod clause;

use comrak::{parse_document, Arena, Options};

use crate::error::Result;
use crate::model::Document;
use crate::frontmatter::{self, FrontMatterResult};

pub fn parse(input: &str) -> Result<Document> {
    let FrontMatterResult {
        meta,
        body,
        diagnostics,
    } = frontmatter::parse_frontmatter(input)?;

    // Parse body with comrak
    let arena = Arena::new();
    let mut opts = Options::default();
    opts.extension.superscript = true;
    let root = parse_document(&arena, &body, &opts);

    // Extract clause structure and annexures
    let (body_elements, annexures) = clause::extract_body(root);

    Ok(Document {
        meta,
        body: body_elements,
        annexures,
        schedule_items: Vec::new(),
        diagnostics,
    })
}
