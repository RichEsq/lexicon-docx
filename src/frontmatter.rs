use crate::error::{Diagnostic, LexiconError, Result};
use crate::model::DocumentMeta;

pub struct FrontMatterResult {
    pub meta: DocumentMeta,
    pub body: String,
    /// Number of source lines preceding the body (front-matter + delimiters),
    /// so body-relative line numbers can be mapped back to the input file.
    pub body_line_offset: usize,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse_frontmatter(input: &str) -> Result<FrontMatterResult> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with("---") {
        return Err(LexiconError::FrontMatter(
            "Document must begin with YAML front-matter (---)".to_string(),
        ));
    }

    // Find the closing ---
    let after_open = &trimmed[3..];
    let close_pos = after_open
        .find("\n---")
        .ok_or_else(|| LexiconError::FrontMatter("No closing --- for front-matter".to_string()))?;

    let yaml_str = &after_open[..close_pos];
    let body_start = close_pos + 4; // skip past \n---
    let body = after_open[body_start..]
        .trim_start_matches('\n')
        .to_string();

    // The body is a suffix of the input; everything before it (front-matter,
    // delimiters, leading blank lines) contributes to the line offset.
    let consumed = input.len() - body.len();
    let body_line_offset = input[..consumed].matches('\n').count();

    let meta: DocumentMeta = serde_yaml::from_str(yaml_str)
        .map_err(|e| LexiconError::FrontMatter(format!("Invalid YAML front-matter: {}", e)))?;

    let mut diagnostics = Vec::new();

    // Validate date format (only when present)
    if let Some(ref date) = meta.date
        && !is_valid_date(date)
    {
        diagnostics.push(
            Diagnostic::error(
                "invalid-date",
                format!("Date '{}' is not a valid YYYY-MM-DD date", date),
            )
            .at("front-matter"),
        );
    }

    // Validate parties
    if meta.parties.is_empty() {
        diagnostics.push(
            Diagnostic::error("missing-parties", "No parties defined in front-matter")
                .at("front-matter"),
        );
    }

    for (i, party) in meta.parties.iter().enumerate() {
        if party.role.is_empty() {
            diagnostics.push(
                Diagnostic::error(
                    "missing-party-role",
                    format!("Party {} has empty role", i + 1),
                )
                .at("front-matter"),
            );
        }
    }

    Ok(FrontMatterResult {
        meta,
        body,
        body_line_offset,
        diagnostics,
    })
}

fn is_valid_date(date_str: &str) -> bool {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").is_ok()
}
