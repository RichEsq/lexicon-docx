pub mod error;
pub mod frontmatter;
pub mod model;
pub mod parser;
pub mod render;
pub mod resolve;
pub mod signatures;
pub mod style;

use std::path::{Path, PathBuf};

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

pub fn render_docx(
    doc: &Document,
    style: &StyleConfig,
    input_dir: Option<&Path>,
    signature_blocks: &[SignatureBlock],
) -> Result<Vec<u8>> {
    render::docx::render_docx(doc, style, input_dir, signature_blocks)
}

/// Resolve a config file path by searching:
/// 1. The input document's directory
/// 2. $XDG_CONFIG_HOME/lexicon/ (defaults to ~/.config/lexicon/)
///
/// Returns the first path that exists, or None.
pub fn resolve_config_path(filename: &str, input_dir: Option<&Path>) -> Option<PathBuf> {
    // 1. Same directory as the input document
    if let Some(dir) = input_dir {
        let local = dir.join(filename);
        if local.exists() {
            return Some(local);
        }
    }

    // 2. $XDG_CONFIG_HOME/lexicon/ or ~/.config/lexicon/
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        })
        .join("lexicon");

    let global = config_dir.join(filename);
    if global.exists() {
        return Some(global);
    }

    None
}

pub fn process(
    input: &str,
    style: &StyleConfig,
    input_dir: Option<&Path>,
    signatures_path: Option<&Path>,
) -> Result<(Vec<u8>, Vec<Diagnostic>)> {
    let mut doc = parse(input)?;
    resolve(&mut doc);

    // Resolve signature blocks if enabled
    let mut sig_diagnostics = Vec::new();
    let signature_blocks = if style.signatures.enabled {
        let definitions = match signatures_path {
            Some(path) => signatures::load_definitions(path, &mut sig_diagnostics),
            None => {
                sig_diagnostics.push(Diagnostic {
                    level: error::DiagLevel::Warning,
                    message: "Signatures enabled but no definitions file found (searched input directory and $XDG_CONFIG_HOME/lexicon/)".to_string(),
                    location: None,
                });
                None
            }
        };

        signatures::resolve_signature_blocks(
            &doc.meta.parties,
            doc.meta.doc_type.as_deref(),
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
