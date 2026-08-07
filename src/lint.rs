//! Linter for Lexicon Markdown contracts.
//!
//! Runs the full parse + resolve pipeline (which produces spec-compliance
//! diagnostics) plus additional lint-only checks that would be noise during a
//! build (metadata completeness, exhibit file availability), and packages the
//! result as a report that can be rendered as human-readable text, as JSON
//! for editor integrations and AI agents, or as GitHub Actions annotations.
//!
//! Diagnostics can be filtered three ways, all sharing one pipeline:
//! - `[lint]` in style.toml (`ignore` list + `severity` overrides)
//! - CLI flags (`--ignore`, `--min-severity`)
//! - inline suppression comments in the document:
//!   `<!-- lexicon-ignore: code, code -->` (same line or the line above the
//!   diagnostic) and `<!-- lexicon-ignore-file: code, code -->` (whole file).
//!
//! `parse-error` and `io-error` can never be filtered — a report must not
//! claim a file is valid when it could not even be read or parsed.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::error::{DiagLevel, Diagnostic};
use crate::model::Document;
use crate::style::{NumberingConvention, StyleConfig};

/// Version of the report formats (JSON structure, rule code semantics).
/// Bump on breaking changes so consumers can detect them.
pub const REPORT_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Rule registry
// ---------------------------------------------------------------------------

/// A lint rule: stable code, default severity, one-line description.
pub struct Rule {
    pub code: &'static str,
    pub severity: DiagLevel,
    pub description: &'static str,
}

/// Every rule code the pipeline can emit. Config files, CLI flags, and
/// suppression comments are validated against this list.
pub const RULES: &[Rule] = &[
    Rule {
        code: "io-error",
        severity: DiagLevel::Error,
        description: "Input file could not be read",
    },
    Rule {
        code: "parse-error",
        severity: DiagLevel::Error,
        description: "Document could not be parsed (missing or invalid front-matter)",
    },
    Rule {
        code: "invalid-date",
        severity: DiagLevel::Error,
        description: "date is not a valid YYYY-MM-DD date",
    },
    Rule {
        code: "missing-parties",
        severity: DiagLevel::Error,
        description: "No parties defined in front-matter",
    },
    Rule {
        code: "missing-party-role",
        severity: DiagLevel::Error,
        description: "A party has an empty role",
    },
    Rule {
        code: "exhibit-file-missing",
        severity: DiagLevel::Error,
        description: "A declared exhibit path does not exist",
    },
    Rule {
        code: "exhibit-unsupported-type",
        severity: DiagLevel::Error,
        description: "Exhibit file type is not png/jpg/jpeg/pdf",
    },
    Rule {
        code: "exhibit-url-unsupported",
        severity: DiagLevel::Error,
        description: "Exhibit path is a URL (not supported)",
    },
    Rule {
        code: "broken-cross-ref",
        severity: DiagLevel::Warning,
        description: "Cross-reference points to a non-existent anchor",
    },
    Rule {
        code: "duplicate-anchor",
        severity: DiagLevel::Warning,
        description: "The same {#id} anchor is declared more than once",
    },
    Rule {
        code: "duplicate-definition",
        severity: DiagLevel::Warning,
        description: "A term is bold-defined at more than one place",
    },
    Rule {
        code: "unused-term",
        severity: DiagLevel::Warning,
        description: "A defined term never appears in the document text",
    },
    Rule {
        code: "unreferenced-schedule",
        severity: DiagLevel::Warning,
        description: "A declared schedule has no referencing terms",
    },
    Rule {
        code: "undeclared-schedule",
        severity: DiagLevel::Warning,
        description: "A definition references a schedule title not declared in front-matter",
    },
    Rule {
        code: "bullet-outside-clause",
        severity: DiagLevel::Warning,
        description: "Bullet list in the clause hierarchy (unnumbered, not cross-referenceable)",
    },
    Rule {
        code: "unknown-top-heading",
        severity: DiagLevel::Warning,
        description: "Unrecognised # top-level heading",
    },
    Rule {
        code: "heading-after-body",
        severity: DiagLevel::Warning,
        description: "Unexpected # top-level heading after the body section",
    },
    Rule {
        code: "duplicate-recitals",
        severity: DiagLevel::Warning,
        description: "More than one recitals/background section",
    },
    Rule {
        code: "missing-body-heading",
        severity: DiagLevel::Warning,
        description: "Recitals present but no body heading follows",
    },
    Rule {
        code: "signatures-definitions-missing",
        severity: DiagLevel::Warning,
        description: "Signatures enabled but no definitions file found (build)",
    },
    Rule {
        code: "signatures-definitions-invalid",
        severity: DiagLevel::Warning,
        description: "Signature definitions file failed to parse (build)",
    },
    Rule {
        code: "signature-missing-entity-type",
        severity: DiagLevel::Warning,
        description: "Party has no entity_type for signature block resolution (build)",
    },
    Rule {
        code: "signature-template-missing",
        severity: DiagLevel::Warning,
        description: "No signature template found for a party (build)",
    },
    Rule {
        code: "unknown-lint-rule",
        severity: DiagLevel::Warning,
        description: "Config, flag, or suppression names a rule code that does not exist",
    },
    Rule {
        code: "invalid-suppression",
        severity: DiagLevel::Warning,
        description: "Malformed suppression comment",
    },
    Rule {
        code: "unused-anchor",
        severity: DiagLevel::Info,
        description: "An anchor is declared but never referenced",
    },
    Rule {
        code: "missing-date",
        severity: DiagLevel::Info,
        description: "No date set (rendered as a blank date line)",
    },
    Rule {
        code: "missing-party-name",
        severity: DiagLevel::Info,
        description: "A party has no name (rendered as a placeholder)",
    },
    Rule {
        code: "unused-suppression",
        severity: DiagLevel::Info,
        description: "A suppression comment matched no diagnostic",
    },
];

pub fn is_known_rule(code: &str) -> bool {
    RULES.iter().any(|r| r.code == code)
}

/// Rules that can never be ignored, suppressed, or re-levelled: a report must
/// not claim a file is valid when it could not be read or parsed.
const UNFILTERABLE: &[&str] = &["parse-error", "io-error"];

/// Render the rule registry as aligned text.
pub fn rules_to_text() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{:<32} {:<8} DESCRIPTION", "CODE", "LEVEL");
    for rule in RULES {
        let _ = writeln!(
            out,
            "{:<32} {:<8} {}",
            rule.code,
            rule.severity.as_str(),
            rule.description
        );
    }
    out
}

/// Render the rule registry as JSON.
pub fn rules_to_json() -> String {
    #[derive(Serialize)]
    struct JsonRule {
        code: &'static str,
        severity: &'static str,
        description: &'static str,
    }
    #[derive(Serialize)]
    struct JsonRules {
        version: u32,
        rules: Vec<JsonRule>,
    }
    let doc = JsonRules {
        version: REPORT_VERSION,
        rules: RULES
            .iter()
            .map(|r| JsonRule {
                code: r.code,
                severity: r.severity.as_str(),
                description: r.description,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&doc).expect("rules serialize to JSON")
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Effective lint options: the `[lint]` section of style.toml merged with
/// CLI flags (`--ignore`, `--min-severity`).
#[derive(Debug, Clone)]
pub struct LintOptions {
    /// Rule codes disabled entirely.
    pub ignore: Vec<String>,
    /// Per-rule severity overrides.
    pub severity: HashMap<String, DiagLevel>,
    /// Diagnostics below this severity are dropped from the report.
    pub min_severity: DiagLevel,
}

impl Default for LintOptions {
    fn default() -> Self {
        LintOptions {
            ignore: Vec::new(),
            severity: HashMap::new(),
            min_severity: DiagLevel::Info,
        }
    }
}

impl LintOptions {
    pub fn from_style(style: &StyleConfig) -> Self {
        LintOptions {
            ignore: style.lint.ignore.clone(),
            severity: style.lint.severity.clone(),
            min_severity: DiagLevel::Info,
        }
    }

    /// Warn about configured rule codes that don't exist, and strip
    /// unfilterable codes from ignore/severity so they can't be silenced.
    fn validate(&mut self) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for code in self.ignore.iter().chain(self.severity.keys()) {
            if !is_known_rule(code) {
                diags.push(Diagnostic::warning(
                    "unknown-lint-rule",
                    format!("Unknown lint rule '{}' in lint configuration", code),
                ));
            }
        }
        self.ignore.retain(|c| !UNFILTERABLE.contains(&c.as_str()));
        self.severity
            .retain(|c, _| !UNFILTERABLE.contains(&c.as_str()));
        diags
    }
}

// ---------------------------------------------------------------------------
// Inline suppressions
// ---------------------------------------------------------------------------

static SUPPRESSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<!--\s*lexicon-ignore(-file)?\s*(?::([^>]*?))?\s*-->").unwrap()
});

struct LineDirective {
    line: usize,
    /// None = bare directive, suppresses every rule on the target lines.
    codes: Option<Vec<String>>,
    used: bool,
}

struct FileDirective {
    line: usize,
    code: String,
    used: bool,
}

#[derive(Default)]
struct Suppressions {
    lines: Vec<LineDirective>,
    file: Vec<FileDirective>,
}

impl Suppressions {
    /// True if a diagnostic with `code` at `diag_line` is suppressed.
    /// Marks matching directives as used.
    fn matches(&mut self, code: &str, diag_line: Option<usize>) -> bool {
        let mut hit = false;
        for d in &mut self.file {
            if d.code == code {
                d.used = true;
                hit = true;
            }
        }
        if let Some(line) = diag_line {
            for d in &mut self.lines {
                // A directive applies to its own line and the line below it.
                let targets = d.line == line || d.line + 1 == line;
                let code_match = match &d.codes {
                    None => true,
                    Some(codes) => codes.iter().any(|c| c == code),
                };
                if targets && code_match {
                    d.used = true;
                    hit = true;
                }
            }
        }
        hit
    }
}

/// Scan the raw source for suppression comments. Returns the directives and
/// any diagnostics about malformed or unknown ones.
fn scan_suppressions(input: &str) -> (Suppressions, Vec<Diagnostic>) {
    let mut supp = Suppressions::default();
    let mut diags = Vec::new();

    for (idx, line_text) in input.lines().enumerate() {
        let line = idx + 1;
        for caps in SUPPRESSION_RE.captures_iter(line_text) {
            let file_level = caps.get(1).is_some();
            let codes: Vec<String> = caps
                .get(2)
                .map(|m| {
                    m.as_str()
                        .split(',')
                        .map(|c| c.trim().to_string())
                        .filter(|c| !c.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            let is_bare = codes.is_empty();

            for code in &codes {
                if !is_known_rule(code) {
                    diags.push(
                        Diagnostic::warning(
                            "unknown-lint-rule",
                            format!("Suppression comment names unknown rule '{}'", code),
                        )
                        .at_line(Some(line)),
                    );
                } else if UNFILTERABLE.contains(&code.as_str()) {
                    diags.push(
                        Diagnostic::warning(
                            "invalid-suppression",
                            format!("Rule '{}' cannot be suppressed", code),
                        )
                        .at_line(Some(line)),
                    );
                }
            }
            let codes: Vec<String> = codes
                .into_iter()
                .filter(|c| is_known_rule(c) && !UNFILTERABLE.contains(&c.as_str()))
                .collect();

            if file_level {
                if is_bare {
                    diags.push(
                        Diagnostic::warning(
                            "invalid-suppression",
                            "lexicon-ignore-file requires one or more rule codes \
                             (e.g. <!-- lexicon-ignore-file: unused-anchor -->)",
                        )
                        .at_line(Some(line)),
                    );
                } else {
                    // Invalid codes were already reported; register the rest.
                    for code in codes {
                        supp.file.push(FileDirective {
                            line,
                            code,
                            used: false,
                        });
                    }
                }
            } else {
                // A directive that listed only invalid codes must not become
                // a bare suppress-everything directive — keep the (empty)
                // code list so it matches nothing.
                supp.lines.push(LineDirective {
                    line,
                    codes: if is_bare { None } else { Some(codes) },
                    used: false,
                });
            }
        }
    }

    (supp, diags)
}

// ---------------------------------------------------------------------------
// Filtering pipeline
// ---------------------------------------------------------------------------

/// Apply severity overrides, suppressions, and ignores to a diagnostic list.
/// Order per diagnostic: re-level, then inline suppressions (so directives
/// get credited as used even when the rule is also globally ignored), then
/// the ignore list. `parse-error`/`io-error` pass through untouched.
fn apply_filters(
    diagnostics: Vec<Diagnostic>,
    options: &LintOptions,
    supp: &mut Suppressions,
) -> Vec<Diagnostic> {
    let mut kept = Vec::new();
    for mut d in diagnostics {
        if UNFILTERABLE.contains(&d.code) {
            kept.push(d);
            continue;
        }
        if let Some(level) = options.severity.get(d.code) {
            d.level = *level;
        }
        if supp.matches(d.code, d.line) {
            continue;
        }
        if options.ignore.iter().any(|c| c == d.code) {
            continue;
        }
        kept.push(d);
    }
    kept
}

/// Report suppression directives that matched nothing.
fn unused_suppression_diags(supp: &Suppressions) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for d in &supp.lines {
        if !d.used {
            diags.push(
                Diagnostic::info(
                    "unused-suppression",
                    "Suppression comment does not match any diagnostic on this or the next line",
                )
                .at_line(Some(d.line)),
            );
        }
    }
    for d in &supp.file {
        if !d.used {
            diags.push(
                Diagnostic::info(
                    "unused-suppression",
                    format!(
                        "File-level suppression of '{}' does not match any diagnostic",
                        d.code
                    ),
                )
                .at_line(Some(d.line)),
            );
        }
    }
    diags
}

/// Filter build diagnostics through the same pipeline as lint: severity
/// overrides, inline suppressions, and the config ignore list all apply, so
/// `build` and `lint` never disagree about a finding. Unused suppressions are
/// not reported here (that's a lint concern).
pub fn filter_build_diagnostics(
    input: &str,
    diagnostics: Vec<Diagnostic>,
    style: &StyleConfig,
) -> Vec<Diagnostic> {
    let mut options = LintOptions::from_style(style);
    // Config mistakes are reported by `lint`; build just drops the invalid bits.
    let _ = options.validate();
    let (mut supp, _) = scan_suppressions(input);
    apply_filters(diagnostics, &options, &mut supp)
}

// ---------------------------------------------------------------------------
// Lint entry point
// ---------------------------------------------------------------------------

/// The result of linting a single document.
#[derive(Debug)]
pub struct LintReport {
    pub diagnostics: Vec<Diagnostic>,
}

impl LintReport {
    /// Wrap a single diagnostic (used for unreadable files).
    pub fn from_diagnostic(diagnostic: Diagnostic) -> Self {
        LintReport {
            diagnostics: vec![diagnostic],
        }
    }

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
}

/// Lint a Lexicon Markdown document.
///
/// Never fails: fatal parse errors (missing/invalid front-matter) are
/// reported as an error diagnostic in the returned report.
pub fn lint(
    input: &str,
    input_dir: Option<&Path>,
    convention: NumberingConvention,
    options: &LintOptions,
) -> LintReport {
    let mut options = options.clone();
    let mut diagnostics = options.validate();

    let (mut supp, supp_diags) = scan_suppressions(input);
    diagnostics.extend(supp_diags);

    match crate::parse(input) {
        Ok(mut doc) => {
            crate::resolve(&mut doc, convention);
            diagnostics.extend(std::mem::take(&mut doc.diagnostics));
            check_metadata(&doc, &mut diagnostics);
            check_exhibits(&doc, input_dir, &mut diagnostics);
        }
        Err(e) => {
            diagnostics.push(Diagnostic::error("parse-error", e.to_string()));
        }
    }

    let mut diagnostics = apply_filters(diagnostics, &options, &mut supp);
    let unused = apply_filters(unused_suppression_diags(&supp), &options, &mut supp);
    diagnostics.extend(unused);

    // Display floor — applied last so it hides but never un-suppresses.
    diagnostics.retain(|d| d.level.rank() >= options.min_severity.rank());

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

// ---------------------------------------------------------------------------
// Report rendering (single- and multi-file)
// ---------------------------------------------------------------------------

/// Render reports as human-readable text: per-file diagnostics and summary,
/// plus an overall total when more than one file was linted.
pub fn reports_to_text(reports: &[(String, LintReport)]) -> String {
    let mut out = String::new();
    for (i, (file, report)) in reports.iter().enumerate() {
        if i > 0 {
            let _ = writeln!(out);
        }
        for d in &report.diagnostics {
            let _ = writeln!(out, "{}", d);
        }
        if report.diagnostics.is_empty() {
            let _ = writeln!(out, "{}: no issues found", file);
        } else {
            let _ = writeln!(
                out,
                "{}: {} error(s), {} warning(s), {} info",
                file,
                report.error_count(),
                report.warning_count(),
                report.info_count()
            );
        }
    }
    if reports.len() > 1 {
        let errors: usize = reports.iter().map(|(_, r)| r.error_count()).sum();
        let warnings: usize = reports.iter().map(|(_, r)| r.warning_count()).sum();
        let info: usize = reports.iter().map(|(_, r)| r.info_count()).sum();
        let _ = writeln!(
            out,
            "\n{} files checked: {} error(s), {} warning(s), {} info",
            reports.len(),
            errors,
            warnings,
            info
        );
    }
    out
}

#[derive(Serialize)]
struct JsonSummary {
    errors: usize,
    warnings: usize,
    info: usize,
}

#[derive(Serialize)]
struct JsonFileReport<'a> {
    file: &'a str,
    valid: bool,
    summary: JsonSummary,
    diagnostics: &'a [Diagnostic],
}

fn json_file_report<'a>(file: &'a str, report: &'a LintReport) -> JsonFileReport<'a> {
    JsonFileReport {
        file,
        valid: !report.has_errors(),
        summary: JsonSummary {
            errors: report.error_count(),
            warnings: report.warning_count(),
            info: report.info_count(),
        },
        diagnostics: &report.diagnostics,
    }
}

/// Render reports as JSON. A single file keeps the flat shape:
///
/// ```json
/// { "version": 1, "file": "contract.md", "valid": true,
///   "summary": { "errors": 0, "warnings": 2, "info": 1 },
///   "diagnostics": [ { "level": "warning", "code": "unused-term",
///     "message": "...", "location": "clause 3.1", "line": 42, "column": 5 } ] }
/// ```
///
/// Multiple files nest per-file reports under `files` with an overall summary:
///
/// ```json
/// { "version": 1, "valid": false,
///   "summary": { "errors": 1, "warnings": 0, "info": 0 },
///   "files": [ { "file": "a.md", ... }, { "file": "b.md", ... } ] }
/// ```
pub fn reports_to_json(reports: &[(String, LintReport)]) -> String {
    #[derive(Serialize)]
    struct SingleReport<'a> {
        version: u32,
        #[serde(flatten)]
        report: JsonFileReport<'a>,
    }
    #[derive(Serialize)]
    struct MultiReport<'a> {
        version: u32,
        valid: bool,
        summary: JsonSummary,
        files: Vec<JsonFileReport<'a>>,
    }

    let json = if let [(file, report)] = reports {
        serde_json::to_string_pretty(&SingleReport {
            version: REPORT_VERSION,
            report: json_file_report(file, report),
        })
    } else {
        serde_json::to_string_pretty(&MultiReport {
            version: REPORT_VERSION,
            valid: !reports.iter().any(|(_, r)| r.has_errors()),
            summary: JsonSummary {
                errors: reports.iter().map(|(_, r)| r.error_count()).sum(),
                warnings: reports.iter().map(|(_, r)| r.warning_count()).sum(),
                info: reports.iter().map(|(_, r)| r.info_count()).sum(),
            },
            files: reports
                .iter()
                .map(|(f, r)| json_file_report(f, r))
                .collect(),
        })
    };
    json.expect("reports serialize to JSON")
}

/// Escape a GitHub Actions workflow-command property value.
fn github_escape_property(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
        .replace(':', "%3A")
        .replace(',', "%2C")
}

/// Escape a GitHub Actions workflow-command message.
fn github_escape_message(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Render reports as GitHub Actions workflow commands, one annotation per
/// diagnostic, so findings appear inline on pull request diffs:
///
/// ```text
/// ::warning file=contract.md,line=42,col=5,title=unused-term::'Widget' is defined but never used
/// ```
pub fn reports_to_github(reports: &[(String, LintReport)]) -> String {
    let mut out = String::new();
    for (file, report) in reports {
        for d in &report.diagnostics {
            let command = match d.level {
                DiagLevel::Error => "error",
                DiagLevel::Warning => "warning",
                DiagLevel::Info => "notice",
            };
            let mut props = format!("file={}", github_escape_property(file));
            if let Some(line) = d.line {
                let _ = write!(props, ",line={}", line);
            }
            if let Some(col) = d.column {
                let _ = write!(props, ",col={}", col);
            }
            let _ = write!(props, ",title={}", github_escape_property(d.code));
            let message = match &d.location {
                Some(loc) => format!("{} ({})", d.message, loc),
                None => d.message.clone(),
            };
            let _ = writeln!(
                out,
                "::{} {}::{}",
                command,
                props,
                github_escape_message(&message)
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lint_str(input: &str) -> LintReport {
        lint(
            input,
            None,
            NumberingConvention::Commonwealth,
            &LintOptions::default(),
        )
    }

    fn lint_opts(input: &str, options: &LintOptions) -> LintReport {
        lint(input, None, NumberingConvention::Commonwealth, options)
    }

    const CLEAN_BASE: &str = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Seller
---
"#;

    #[test]
    fn parse_error_reported_as_diagnostic() {
        let report = lint_str("no front matter here");
        assert!(report.has_errors());
        assert_eq!(report.diagnostics[0].code, "parse-error");
    }

    #[test]
    fn parse_error_cannot_be_ignored() {
        let mut options = LintOptions::default();
        options.ignore.push("parse-error".to_string());
        let report = lint_opts("no front matter here", &options);
        assert!(report.has_errors());
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
            &LintOptions::default(),
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
        let input = format!(
            "{}\n1. ## Definitions\n\n    1. **Widget** means a thing nobody mentions again.\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        let json = reports_to_json(&[("test.md".to_string(), report)]);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["file"], "test.md");
        assert_eq!(parsed["valid"], true);
        assert!(parsed["summary"]["warnings"].as_u64().unwrap() >= 1);
        let diags = parsed["diagnostics"].as_array().unwrap();
        let unused = diags
            .iter()
            .find(|d| {
                d["code"] == "unused-term" && d["message"].as_str().unwrap().contains("Widget")
            })
            .expect("unused-term for Widget");
        assert_eq!(unused["level"], "warning");
        assert!(unused["line"].as_u64().is_some());
        assert!(unused["column"].as_u64().is_some());
    }

    #[test]
    fn multi_file_json_nests_reports() {
        let clean = format!(
            "{}\n1. ## Obligations\n\n    1. The Seller must deliver under this Agreement.\n",
            CLEAN_BASE
        );
        let a = lint_str(&clean);
        let b = lint_str("broken");
        let json = reports_to_json(&[("a.md".to_string(), a), ("b.md".to_string(), b)]);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["version"], 1);
        assert_eq!(parsed["valid"], false);
        assert_eq!(parsed["summary"]["errors"], 1);
        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["file"], "a.md");
        assert_eq!(files[0]["valid"], true);
        assert_eq!(files[1]["valid"], false);
    }

    #[test]
    fn github_format_emits_annotations() {
        let input = format!(
            "{}\n1. ## Definitions\n\n    1. **Widget** means a thing nobody mentions again.\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        let out = reports_to_github(&[("test.md".to_string(), report)]);
        assert!(
            out.lines()
                .any(|l| l.starts_with("::warning file=test.md,line=")
                    && l.contains("title=unused-term")
                    && l.contains("Widget")),
            "got: {}",
            out
        );
    }

    #[test]
    fn ignore_option_drops_rule() {
        let input = format!(
            "{}\n1. ## Definitions\n\n    1. **Widget** means a thing nobody mentions again. This Agreement binds the Seller.\n",
            CLEAN_BASE
        );
        let mut options = LintOptions::default();
        options.ignore.push("unused-term".to_string());
        let report = lint_opts(&input, &options);
        assert!(
            !report.diagnostics.iter().any(|d| d.code == "unused-term"),
            "got: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn severity_override_relevels_rule() {
        let input = format!(
            "{}\n1. ## Definitions\n\n    1. **Widget** means a thing nobody mentions again. This Agreement binds the Seller.\n",
            CLEAN_BASE
        );
        let mut options = LintOptions::default();
        options
            .severity
            .insert("unused-term".to_string(), DiagLevel::Error);
        let report = lint_opts(&input, &options);
        assert!(report.has_errors());
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "unused-term" && d.level == DiagLevel::Error)
        );
    }

    #[test]
    fn min_severity_hides_lower_levels() {
        let input = r#"---
title: Test
parties:
  - role: Seller
---

1. ## Obligations

    1. The Seller agrees to this Agreement.
"#;
        let options = LintOptions {
            min_severity: DiagLevel::Warning,
            ..Default::default()
        };
        let report = lint_opts(input, &options);
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| d.level == DiagLevel::Info),
            "got: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn unknown_rule_in_config_reported() {
        let mut options = LintOptions::default();
        options.ignore.push("not-a-rule".to_string());
        let clean = format!(
            "{}\n1. ## Obligations\n\n    1. The Seller must deliver under this Agreement.\n",
            CLEAN_BASE
        );
        let report = lint_opts(&clean, &options);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "unknown-lint-rule" && d.message.contains("not-a-rule"))
        );
    }

    #[test]
    fn inline_suppression_same_line() {
        let input = format!(
            "{}\n1. ## Definitions\n\n    1. **Widget** means a thing nobody mentions again. <!-- lexicon-ignore: unused-term --> This Agreement binds the Seller.\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        assert!(
            !report.diagnostics.iter().any(|d| d.code == "unused-term"),
            "got: {:?}",
            report.diagnostics
        );
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| d.code == "unused-suppression"),
            "suppression matched, must not be reported unused: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn file_level_suppression() {
        let input = format!(
            "{}\n<!-- lexicon-ignore-file: unused-anchor -->\n\n1. ## Definitions {{#defs}}\n\n    1. The Seller signs this Agreement.\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        assert!(
            !report.diagnostics.iter().any(|d| d.code == "unused-anchor"),
            "got: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn unused_suppression_reported() {
        let input = format!(
            "{}\n1. ## Obligations\n\n    1. The Seller must deliver under this Agreement. <!-- lexicon-ignore: unused-term -->\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "unused-suppression"),
            "got: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn suppression_with_unknown_code_reported() {
        let input = format!(
            "{}\n1. ## Obligations\n\n    1. The Seller must deliver under this Agreement. <!-- lexicon-ignore: bogus-rule -->\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "unknown-lint-rule" && d.message.contains("bogus-rule"))
        );
    }

    #[test]
    fn bare_file_suppression_is_invalid() {
        let input = format!(
            "{}\n<!-- lexicon-ignore-file -->\n\n1. ## Obligations\n\n    1. The Seller must deliver under this Agreement.\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.code == "invalid-suppression")
        );
    }

    #[test]
    fn clean_document_produces_no_diagnostics() {
        let input = format!(
            "{}\n1. ## Obligations\n\n    1. The Seller must deliver the goods under this Agreement.\n",
            CLEAN_BASE
        );
        let report = lint_str(&input);
        assert!(
            report.diagnostics.is_empty(),
            "expected clean, got: {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn every_emitted_code_is_registered() {
        // The registry drives config validation; a code missing from it could
        // never be ignored or re-levelled.
        for rule in RULES {
            assert!(is_known_rule(rule.code));
        }
        // Spot-check registry completeness against a document that trips
        // several rules at once.
        let input = r#"---
title: Test
parties:
  - role: Seller
schedule:
  - title: Schedule
---

1. ## Definitions {#defs}

    1. **Widget** means a thing.

2. ## Scope {#defs}

    1. The **Widget** breaks [clause 9](#gone). The Seller signs this Agreement.
"#;
        let report = lint_str(input);
        for d in &report.diagnostics {
            assert!(
                is_known_rule(d.code),
                "emitted code '{}' is not in RULES",
                d.code
            );
        }
    }

    #[test]
    fn build_filtering_applies_config_and_suppressions() {
        let input = format!(
            "{}\n1. ## Definitions\n\n    1. **Widget** means a thing. <!-- lexicon-ignore: unused-term -->\n\n    2. **Cog** means a gear.\n\n    3. The Seller signs this Agreement.\n",
            CLEAN_BASE
        );
        let mut style = StyleConfig::default();
        style
            .lint
            .severity
            .insert("unused-term".to_string(), DiagLevel::Error);

        let mut doc = crate::parse(&input).unwrap();
        crate::resolve(&mut doc, NumberingConvention::Commonwealth);
        let diagnostics =
            filter_build_diagnostics(&input, std::mem::take(&mut doc.diagnostics), &style);

        // Widget suppressed inline; Cog re-levelled to error by config.
        assert!(!diagnostics.iter().any(|d| d.message.contains("Widget")));
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("Cog") && d.level == DiagLevel::Error),
            "got: {:?}",
            diagnostics
        );
    }

    #[test]
    fn rules_to_json_is_valid() {
        let parsed: serde_json::Value = serde_json::from_str(&rules_to_json()).unwrap();
        assert_eq!(parsed["version"], 1);
        assert!(parsed["rules"].as_array().unwrap().len() >= 20);
    }
}
