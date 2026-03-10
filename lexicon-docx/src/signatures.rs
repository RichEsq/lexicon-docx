use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::{Diagnostic, DiagLevel};
use crate::model::Party;
use crate::style::{SignaturesConfig, StyleConfig};

/// A resolved signature block ready for rendering.
#[derive(Debug, Clone)]
pub struct SignatureBlock {
    pub intro: String,
    pub signatories: Vec<Signatory>,
    pub fields: Vec<SignatureField>,
    pub witness: bool,
    pub witness_fields: Vec<SignatureField>,
}

#[derive(Debug, Clone)]
pub struct Signatory {
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SignatureField {
    pub label: Option<String>,
    pub field_type: FieldType,
    pub value: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldType {
    Line,
    Blank,
}

// --- Definitions file types (deserialisable) ---

/// The top-level definitions file structure.
/// Keys are jurisdiction codes (e.g. "au", "uk"), containing entity types,
/// containing execution methods.
type DefinitionsFile = HashMap<String, HashMap<String, HashMap<String, TemplateDefinition>>>;

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateDefinition {
    pub intro: String,
    #[serde(default)]
    pub signatories: Vec<SignatoryDef>,
    #[serde(default)]
    pub fields: Vec<FieldDef>,
    #[serde(default)]
    pub witness: bool,
    #[serde(default)]
    pub witness_fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignatoryDef {
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FieldDef {
    #[serde(rename = "type", default)]
    pub field_type: Option<String>,
    pub label: Option<String>,
    pub value: Option<String>,
}

/// Load the definitions file from disk. Returns None with a diagnostic if not found.
pub fn load_definitions(
    path: &Path,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<DefinitionsFile> {
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(defs) => Some(defs),
            Err(e) => {
                diagnostics.push(Diagnostic {
                    level: DiagLevel::Warning,
                    message: format!("Failed to parse signatures definitions file: {}", e),
                    location: Some(path.display().to_string()),
                });
                None
            }
        },
        Err(_) => {
            diagnostics.push(Diagnostic {
                level: DiagLevel::Warning,
                message: format!(
                    "Signatures definitions file not found: {}",
                    path.display()
                ),
                location: None,
            });
            None
        }
    }
}

/// Look up a template from the definitions file.
/// `entity_type` is e.g. "au-company", `execution` is "deed" or "agreement".
fn lookup_definition(
    defs: &DefinitionsFile,
    entity_type: &str,
    execution: &str,
) -> Option<TemplateDefinition> {
    let (jurisdiction, etype) = split_entity_type(entity_type)?;
    defs.get(jurisdiction)?
        .get(etype)?
        .get(execution)
        .cloned()
}

/// Split "au-company" into ("au", "company"). Returns None if no hyphen.
fn split_entity_type(entity_type: &str) -> Option<(&str, &str)> {
    let idx = entity_type.find('-')?;
    Some((&entity_type[..idx], &entity_type[idx + 1..]))
}

/// Determine the execution method from the document's short_title.
pub fn execution_method(short_title: Option<&str>) -> &str {
    match short_title {
        Some(t) if t.eq_ignore_ascii_case("deed") => "deed",
        _ => "agreement",
    }
}

/// Resolve signature blocks for all parties.
pub fn resolve_signature_blocks(
    parties: &[Party],
    short_title: Option<&str>,
    style: &StyleConfig,
    definitions: &Option<DefinitionsFile>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<SignatureBlock> {
    let execution = execution_method(short_title);
    let config = &style.signatures;

    parties
        .iter()
        .map(|party| {
            resolve_party_block(party, execution, short_title, config, definitions, diagnostics)
        })
        .collect()
}

fn resolve_party_block(
    party: &Party,
    execution: &str,
    short_title: Option<&str>,
    config: &SignaturesConfig,
    definitions: &Option<DefinitionsFile>,
    diagnostics: &mut Vec<Diagnostic>,
) -> SignatureBlock {
    // 1. Check for explicit template override in TOML
    let party_override = config.party.get(&party.role);

    let template_key = party_override
        .and_then(|o| o.template.as_deref())
        .or(config.default_template.as_deref());

    // 2. Try to find a template definition
    let template = if let Some(key) = template_key {
        // Explicit template key — parse as "jurisdiction.entity_type.execution"
        definitions.as_ref().and_then(|defs| {
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() == 3 {
                defs.get(parts[0])?.get(parts[1])?.get(parts[2]).cloned()
            } else {
                None
            }
        })
    } else if let Some(ref entity_type) = party.entity_type {
        // Convention-based lookup from entity_type + execution method
        definitions
            .as_ref()
            .and_then(|defs| lookup_definition(defs, entity_type, execution))
    } else {
        // No entity_type — warn and use fallback
        diagnostics.push(Diagnostic {
            level: DiagLevel::Warning,
            message: format!(
                "Party '{}' has no entity_type; using generic signature block",
                party.role
            ),
            location: Some("front-matter".to_string()),
        });
        None
    };

    // If no template found from definitions, try us-individual fallback
    let template = template.or_else(|| {
        if party.entity_type.is_some() || template_key.is_some() {
            diagnostics.push(Diagnostic {
                level: DiagLevel::Warning,
                message: format!(
                    "No signature template found for party '{}' (entity_type: {}, execution: {}); using fallback",
                    party.role,
                    party.entity_type.as_deref().unwrap_or("none"),
                    execution
                ),
                location: None,
            });
        }
        // Try us-individual from definitions as fallback
        definitions
            .as_ref()
            .and_then(|defs| lookup_definition(defs, "us-individual", execution))
    });

    // 3. Build the SignatureBlock from template (or hardcoded fallback)
    let (base_intro, base_signatories, base_fields, base_witness, base_witness_fields) =
        match template {
            Some(t) => (
                t.intro,
                t.signatories
                    .into_iter()
                    .map(|s| Signatory { title: s.title })
                    .collect(),
                t.fields.into_iter().map(convert_field_def).collect(),
                t.witness,
                t.witness_fields.into_iter().map(convert_field_def).collect(),
            ),
            None => hardcoded_fallback(),
        };

    // 4. Apply TOML overrides
    let signatories = if let Some(override_sigs) = party_override.and_then(|o| o.signatories.as_ref()) {
        override_sigs
            .iter()
            .map(|s| Signatory {
                title: s.title.clone(),
            })
            .collect()
    } else {
        base_signatories
    };

    let witness = party_override
        .and_then(|o| o.witness)
        .unwrap_or(base_witness);

    // 5. Expand placeholders in intro
    let intro = expand_placeholders(
        &base_intro,
        party,
        short_title,
    );

    SignatureBlock {
        intro,
        signatories,
        fields: base_fields,
        witness,
        witness_fields: if base_witness_fields.is_empty() {
            default_witness_fields()
        } else {
            base_witness_fields
        },
    }
}

fn convert_field_def(def: FieldDef) -> SignatureField {
    let field_type = match def.field_type.as_deref() {
        Some("line") => FieldType::Line,
        _ => FieldType::Blank,
    };
    SignatureField {
        label: def.label,
        field_type,
        value: def.value,
    }
}

fn hardcoded_fallback() -> (String, Vec<Signatory>, Vec<SignatureField>, bool, Vec<SignatureField>) {
    (
        "**Signed by {name}**:".to_string(),
        vec![Signatory { title: None }],
        vec![
            SignatureField {
                label: None,
                field_type: FieldType::Line,
                value: None,
            },
            SignatureField {
                label: Some("Name".to_string()),
                field_type: FieldType::Blank,
                value: Some("{name}".to_string()),
            },
            SignatureField {
                label: Some("Date".to_string()),
                field_type: FieldType::Blank,
                value: None,
            },
        ],
        false,
        vec![],
    )
}

fn default_witness_fields() -> Vec<SignatureField> {
    vec![
        SignatureField {
            label: None,
            field_type: FieldType::Line,
            value: None,
        },
        SignatureField {
            label: Some("Witness name".to_string()),
            field_type: FieldType::Blank,
            value: None,
        },
        SignatureField {
            label: Some("Address".to_string()),
            field_type: FieldType::Blank,
            value: None,
        },
    ]
}

/// Expand `{name}`, `{specifier}`, `{role}`, `{short_title}` in a template string.
fn expand_placeholders(
    template: &str,
    party: &Party,
    short_title: Option<&str>,
) -> String {
    let result = template
        .replace("{name}", &party.name)
        .replace("{specifier}", party.specifier.as_deref().unwrap_or(""))
        .replace("{role}", &party.role)
        .replace("{short_title}", short_title.unwrap_or("Agreement"));

    clean_empty_parens(&result)
}

/// Expand `{title}`, `{name}` etc. within a field value, using signatory + party data.
pub fn expand_field_value(
    value: &str,
    party: &Party,
    signatory: &Signatory,
) -> String {
    value
        .replace("{title}", signatory.title.as_deref().unwrap_or(""))
        .replace("{name}", &party.name)
        .replace("{specifier}", party.specifier.as_deref().unwrap_or(""))
        .replace("{role}", &party.role)
}

/// Remove empty parentheses left over when {specifier} is absent.
fn clean_empty_parens(text: &str) -> String {
    let result = text.replace("()", "").replace("( )", "");
    let mut prev = String::new();
    let mut current = result;
    while current != prev {
        prev = current.clone();
        current = current.replace("  ", " ");
    }
    current.trim().to_string()
}
