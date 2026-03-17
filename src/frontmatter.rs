use crate::error::{DiagLevel, Diagnostic, LexiconError, Result};
use crate::model::DocumentMeta;

pub struct FrontMatterResult {
    pub meta: DocumentMeta,
    pub body: String,
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

    let meta: DocumentMeta = serde_yaml::from_str(yaml_str)
        .map_err(|e| LexiconError::FrontMatter(format!("Invalid YAML front-matter: {}", e)))?;

    let mut diagnostics = Vec::new();

    // Validate date format
    if !is_valid_date(&meta.date) {
        diagnostics.push(Diagnostic {
            level: DiagLevel::Error,
            message: format!("Date '{}' is not a valid YYYY-MM-DD date", meta.date),
            location: Some("front-matter".to_string()),
        });
    }

    // Validate parties
    if meta.parties.is_empty() {
        diagnostics.push(Diagnostic {
            level: DiagLevel::Error,
            message: "No parties defined in front-matter".to_string(),
            location: Some("front-matter".to_string()),
        });
    }

    for (i, party) in meta.parties.iter().enumerate() {
        if party.name.is_empty() {
            diagnostics.push(Diagnostic {
                level: DiagLevel::Error,
                message: format!("Party {} has empty name", i + 1),
                location: Some("front-matter".to_string()),
            });
        }
        if party.role.is_empty() {
            diagnostics.push(Diagnostic {
                level: DiagLevel::Error,
                message: format!("Party {} has empty role", i + 1),
                location: Some("front-matter".to_string()),
            });
        }
    }

    Ok(FrontMatterResult {
        meta,
        body,
        diagnostics,
    })
}

fn is_valid_date(date_str: &str) -> bool {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").is_ok()
}
