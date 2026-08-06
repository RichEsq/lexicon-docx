//! Linter for Lexicon Markdown contracts.
//!
//! Runs the full parse + resolve pipeline (which produces spec-compliance
//! diagnostics) plus additional lint-only checks that would be noise during a
//! build (metadata completeness, exhibit file availability), and packages the
//! result as a report that can be rendered as human-readable text or as JSON
//! for editor integrations and AI agents.

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;

use crate::error::{DiagLevel, Diagnostic};
use crate::model::Document;
use crate::style::NumberingConvention;

/// The result of linting a single document.
#[derive(Debug)]
pub struct LintReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl LintReport {
    pub fn error_count(&self) -> usize {
        self.count(DiagLevel::Error)
    }

    pub fn warning_count(&self) -> usize {
        self.count(DiagLevel::Warning)
    }

    pub fn info_count(&self) -> usize {
        self.count(DiagLevel::Info)
    }

    fn count(&self, level: DiagLevel) -> usize {
        self.diagnostics.iter().filter(|d| d.level == level).count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }

    pub fn has_warnings(&self) -> bool {
        self.warning_count() > 0
    }

    /// Render as human-readable text, one diagnostic per line, followed by a
    /// summary line.
    pub fn to_text(&self, file: &str) -> String {
        let mut out = String::new();
        for d in &self.diagnostics {
            let _ = writeln!(out, "{}", d);
        }
        if self.diagnostics.is_empty() {
            let _ = writeln!(out, "{}: no issues found", file);
        } else {
            let _ = writeln!(
                out,
                "{}: {} error(s), {} warning(s), {} info",
                file,
                self.error_count(),
                self.warning_count(),
                self.info_count()
            );
        }
        out
    }

    /// Render as a JSON object for machine consumption (editors, CI, AI
    /// agents). Structure:
    ///
    /// ```json
    /// {
    ///   "file": "contract.md",
    ///   "valid": true,
    ///   "summary": { "errors": 0, "warnings": 2, "info": 1 },
    ///   "diagnostics": [
    ///     { "level": "warning", "code": "unused-term",
    ///       "message": "...", "location": "clause 3.1", "line": 42 }
    ///   ]
    /// }
    /// ```
    pub fn to_json(&self, file: &str) -> String {
        #[derive(Serialize)]
        struct Summary {
            errors: usize,
            warnings: usize,
            info: usize,
        }
        #[derive(Serialize)]
        struct JsonReport<'a> {
            file: &'a str,
            valid: bool,
            summary: Summary,
            diagnostics: &'a [Diagnostic],
        }
        let report = JsonReport {
            file,
            valid: !self.has_errors(),
            summary: Summary {
                errors: self.error_count(),
                warnings: self.warning_count(),
                info: self.info_count(),
            },
            diagnostics: &self.diagnostics,
        };
        serde_json::to_string_pretty(&report).expect("diagnostics serialize to JSON")
    }
}

/// Lint a Lexicon Markdown document.
///
/// Never fails: fatal parse errors (missing/invalid front-matter) are
/// reported as an error diagnostic in the returned report.
pub fn lint(input: &str, input_dir: Option<&Path>, convention: NumberingConvention) -> LintReport {
    let mut doc = match crate::parse(input) {
        Ok(doc) => doc,
        Err(e) => {
            return LintReport {
                diagnostics: vec![Diagnostic::error("parse-error", e.to_string())],
            };
        }
    };
    crate::resolve(&mut doc, convention);

    let mut diagnostics = std::mem::take(&mut doc.diagnostics);
    check_metadata(&doc, &mut diagnostics);
    check_exhibits(&doc, input_dir, &mut diagnostics);

    // Present diagnostics in source order where lines are known, keeping
    // front-matter and other line-less diagnostics first in original order.
    diagnostics.sort_by_key(|d| d.line.unwrap_or(0));

    LintReport { diagnostics }
}

/// Metadata completeness checks — legitimate in drafts, so info-level only.
fn check_metadata(doc: &Document, diagnostics: &mut Vec<Diagnostic>) {
    if doc.meta.date.is_none() {
        diagnostics.push(
            Diagnostic::info(
                "missing-date",
                "No date set; the rendered document will show a blank date line",
            )
            .at("front-matter"),
        );
    }

    for party in &doc.meta.parties {
        if party.name.is_none() {
            diagnostics.push(
                Diagnostic::info(
                    "missing-party-name",
                    format!(
                        "Party '{}' has no name; the rendered document will show a placeholder",
                        party.role
                    ),
                )
                .at("front-matter"),
            );
        }
    }
}

/// Check that declared exhibit files exist and are of a supported type.
/// These fail a build at render time; linting surfaces them early.
fn check_exhibits(doc: &Document, input_dir: Option<&Path>, diagnostics: &mut Vec<Diagnostic>) {
    for exhibit in &doc.meta.exhibits {
        let Some(ref path_str) = exhibit.path else {
            continue;
        };
        if path_str.starts_with("http://") || path_str.starts_with("https://") {
            diagnostics.push(
                Diagnostic::error(
                    "exhibit-url-unsupported",
                    format!(
                        "Exhibit '{}' uses a URL path; URL import is not supported — download the file and use a local path",
                        exhibit.title
                    ),
                )
                .at("front-matter"),
            );
            continue;
        }
        let path = Path::new(path_str);
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            match input_dir {
                Some(dir) => dir.join(path),
                None => path.to_path_buf(),
            }
        };

        if !resolved.exists() {
            diagnostics.push(
                Diagnostic::error(
                    "exhibit-file-missing",
                    format!(
                        "Exhibit '{}' file not found: {}",
                        exhibit.title,
                        resolved.display()
                    ),
                )
                .at("front-matter"),
            );
        } else if crate::render::exhibit::detect_file_type(&resolved).is_err() {
            diagnostics.push(
                Diagnostic::error(
                    "exhibit-unsupported-type",
                    format!(
                        "Exhibit '{}' has unsupported file type '{}' (supported: png, jpg, jpeg, pdf)",
                        exhibit.title, path_str
                    ),
                )
                .at("front-matter"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint_str(input: &str) -> LintReport {
        lint(input, None, NumberingConvention::Commonwealth)
    }

    #[test]
    fn parse_error_reported_as_diagnostic() {
        let report = lint_str("no front matter here");
        assert!(report.has_errors());
        assert_eq!(report.diagnostics[0].code, "parse-error");
    }

    #[test]
    fn missing_date_and_party_name_reported_as_info() {
        let input = r#"---
title: Test
parties:
  - role: Seller
---

1. ## Definitions

    1. The Seller agrees.
"#;
        let report = lint_str(input);
        assert!(!report.has_errors());
        let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code).collect();
        assert!(codes.contains(&"missing-date"));
        assert!(codes.contains(&"missing-party-name"));
    }

    #[test]
    fn missing_exhibit_file_reported_as_error() {
        let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Seller
exhibits:
  - title: Plan
    path: does-not-exist.png
---

1. ## Definitions

    1. The Seller agrees.
"#;
        let report = lint(
            input,
            Some(Path::new("/nonexistent-dir")),
            NumberingConvention::Commonwealth,
        );
        assert!(report.has_errors());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "exhibit-file-missing")
        );
    }

    #[test]
    fn json_output_is_valid_and_structured() {
        let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Seller
---

1. ## Definitions

    1. **Widget** means a thing nobody mentions again.
"#;
        let report = lint_str(input);
        let json = report.to_json("test.md");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["file"], "test.md");
        assert_eq!(parsed["valid"], true);
        assert!(parsed["summary"]["warnings"].as_u64().unwrap() >= 1);
        let diags = parsed["diagnostics"].as_array().unwrap();
        assert!(
            diags
                .iter()
                .any(|d| d["code"] == "unused-term" && d["level"] == "warning")
        );
    }

    #[test]
    fn clean_document_produces_no_diagnostics() {
        let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Seller
---

1. ## Obligations

    1. The Seller must deliver the goods under this Agreement.
"#;
        let report = lint_str(input);
        assert!(
            report.diagnostics.is_empty(),
            "expected clean, got: {:?}",
            report.diagnostics
        );
    }
}
