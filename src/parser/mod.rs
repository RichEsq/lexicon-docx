pub mod anchors;
pub mod clause;

use comrak::{Arena, Options, parse_document};

use crate::error::Result;
use crate::frontmatter::{self, FrontMatterResult};
use crate::model::Document;

pub fn parse(input: &str) -> Result<Document> {
    let FrontMatterResult {
        meta,
        body,
        mut diagnostics,
    } = frontmatter::parse_frontmatter(input)?;

    // Parse body with comrak
    let arena = Arena::new();
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.superscript = true;
    let root = parse_document(&arena, &body, &opts);

    // Extract clause structure, recitals, and addenda
    let (recitals, body_heading, body_elements, addenda, parser_diags) = clause::extract_body(root);
    diagnostics.extend(parser_diags);

    Ok(Document {
        meta,
        recitals,
        body_heading,
        body: body_elements,
        addenda,
        schedule_items: Vec::new(),
        diagnostics,
    })
}
