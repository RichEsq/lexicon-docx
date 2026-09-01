use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "lexicon",
    version,
    about = "Lexicon Markdown processor for legal contracts"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a .docx from a Lexicon Markdown file
    Build {
        /// Input Lexicon Markdown file
        input: PathBuf,

        /// Output .docx file (defaults to input stem + .docx)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Style configuration file (TOML). If not specified, searches for style.toml
        /// in the input file's directory, then in $XDG_CONFIG_HOME/lexicon/.
        #[arg(short, long)]
        style: Option<PathBuf>,

        /// Signature definitions file (TOML). If not specified, searches for signatures.toml
        /// in the input file's directory, then in $XDG_CONFIG_HOME/lexicon/.
        #[arg(long)]
        signatures: Option<PathBuf>,

        /// Fail on warnings (exit code 1)
        #[arg(long)]
        strict: bool,

        #[command(flatten)]
        overrides: Box<StyleOverrides>,
    },

    /// Lint Lexicon Markdown files — checks spec compliance and common
    /// drafting errors (unused defined terms, duplicate definitions, broken
    /// cross-references, missing exhibit files, ...) without generating output
    Lint {
        /// Input Lexicon Markdown file(s)
        #[arg(required_unless_present = "list_rules")]
        inputs: Vec<PathBuf>,

        /// Output format: human-readable text, JSON (for editors, CI, and AI
        /// agents), or GitHub Actions annotations
        #[arg(long, value_enum, default_value = "text")]
        format: LintFormatArg,

        /// Fail on warnings as well as errors (exit code 1)
        #[arg(long)]
        strict: bool,

        /// Disable a rule by code (repeatable; merged with [lint] ignore in style.toml)
        #[arg(long = "ignore", value_name = "CODE")]
        ignore: Vec<String>,

        /// Hide diagnostics below this severity
        #[arg(long, value_enum, value_name = "LEVEL")]
        min_severity: Option<SeverityArg>,

        /// Style configuration file (TOML; supplies the [lint] section and
        /// numbering convention). If not specified, searches for style.toml
        /// next to each input, then in $XDG_CONFIG_HOME/lexicon/.
        #[arg(short, long)]
        style: Option<PathBuf>,

        /// Numbering convention used when formatting clause references in messages
        #[arg(long, value_enum)]
        numbering_convention: Option<NumberingConventionArg>,

        /// List all lint rules (code, default severity, description) and exit
        #[arg(long, conflicts_with = "inputs")]
        list_rules: bool,

        /// Rewrite fixable findings in place. Currently normalises
        /// hand-numbered ordinals (10. and wider) to 1., dedenting each item's
        /// content to match. Verified structurally before writing; a rewrite
        /// that would change the document is abandoned.
        #[arg(long, conflicts_with = "list_rules")]
        fix: bool,
    },

    /// Validate a Lexicon Markdown file without generating output (alias for lint)
    Validate {
        /// Input Lexicon Markdown file
        input: PathBuf,

        /// Fail on warnings as well as errors (exit code 1)
        #[arg(long)]
        strict: bool,
    },

    /// Generate man pages to a directory
    Man {
        /// Output directory for man pages
        #[arg(short, long, default_value = "man")]
        dir: PathBuf,
    },
}

// ---------------------------------------------------------------------------
// ValueEnum wrappers for style enums (avoids adding clap to the library)
// ---------------------------------------------------------------------------

#[derive(Clone, ValueEnum)]
enum PageSizeArg {
    A4,
    Letter,
}

#[derive(Clone, ValueEnum)]
enum DefinedTermStyleArg {
    Bold,
    Quoted,
    BoldQuoted,
}

#[derive(Clone, ValueEnum)]
#[allow(clippy::enum_variant_names)]
enum PartyFormatArg {
    NameSpecRole,
    NameRole,
    NameOnly,
}

#[derive(Clone, ValueEnum)]
enum PreambleStyleArg {
    Simple,
    Prose,
    Custom,
}

#[derive(Clone, ValueEnum)]
enum SchedulePositionArg {
    End,
    AfterToc,
}

#[derive(Clone, ValueEnum)]
enum ScheduleOrderArg {
    Document,
    Alphabetical,
}

#[derive(Clone, ValueEnum)]
enum NumberingConventionArg {
    Commonwealth,
    Decimal,
    UsTraditional,
}

impl NumberingConventionArg {
    fn to_style(&self) -> lexicon_docx::style::NumberingConvention {
        match self {
            NumberingConventionArg::Commonwealth => {
                lexicon_docx::style::NumberingConvention::Commonwealth
            }
            NumberingConventionArg::Decimal => lexicon_docx::style::NumberingConvention::Decimal,
            NumberingConventionArg::UsTraditional => {
                lexicon_docx::style::NumberingConvention::UsTraditional
            }
        }
    }
}

#[derive(Clone, ValueEnum)]
enum LintFormatArg {
    /// Human-readable diagnostics, one per line
    Text,
    /// Structured JSON report on stdout
    Json,
    /// GitHub Actions workflow-command annotations
    Github,
}

#[derive(Clone, Copy, ValueEnum)]
enum SeverityArg {
    Error,
    Warning,
    Info,
}

impl SeverityArg {
    fn to_level(self) -> lexicon_docx::error::DiagLevel {
        match self {
            SeverityArg::Error => lexicon_docx::error::DiagLevel::Error,
            SeverityArg::Warning => lexicon_docx::error::DiagLevel::Warning,
            SeverityArg::Info => lexicon_docx::error::DiagLevel::Info,
        }
    }
}

// ---------------------------------------------------------------------------
// Style override flags — highest priority, applied on top of TOML config
// ---------------------------------------------------------------------------

#[derive(Args)]
struct StyleOverrides {
    // --- Typography ---
    /// Body text font family
    #[arg(long, help_heading = "Typography")]
    font_family: Option<String>,

    /// Body text font size in points
    #[arg(long, help_heading = "Typography")]
    font_size: Option<f32>,

    /// Heading font family
    #[arg(long, help_heading = "Typography")]
    heading_font_family: Option<String>,

    /// Document title font size in points
    #[arg(long, help_heading = "Typography")]
    title_size: Option<f32>,

    /// Level 1 heading size in points
    #[arg(long, help_heading = "Typography")]
    heading1_size: Option<f32>,

    /// Level 2 heading size in points
    #[arg(long, help_heading = "Typography")]
    heading2_size: Option<f32>,

    /// Space before section headings in points
    #[arg(long, help_heading = "Typography")]
    heading_space_before: Option<f32>,

    /// Space after section headings in points
    #[arg(long, help_heading = "Typography")]
    heading_space_after: Option<f32>,

    /// Space before paragraphs in points
    #[arg(long, help_heading = "Typography")]
    paragraph_space_before: Option<f32>,

    /// Space after paragraphs in points
    #[arg(long, help_heading = "Typography")]
    paragraph_space_after: Option<f32>,

    /// Line spacing multiplier
    #[arg(long, help_heading = "Typography")]
    line_spacing: Option<f32>,

    /// Defined term style
    #[arg(long, value_enum, help_heading = "Typography")]
    defined_term_style: Option<DefinedTermStyleArg>,

    /// Brand color as hex (e.g. "#2E5090" or "2E5090")
    #[arg(long, help_heading = "Typography")]
    brand_color: Option<String>,

    // --- Page Layout ---
    /// Page size
    #[arg(long, value_enum, help_heading = "Page Layout")]
    page_size: Option<PageSizeArg>,

    /// Top margin in cm
    #[arg(long, help_heading = "Page Layout")]
    margin_top: Option<f32>,

    /// Bottom margin in cm
    #[arg(long, help_heading = "Page Layout")]
    margin_bottom: Option<f32>,

    /// Left margin in cm
    #[arg(long, help_heading = "Page Layout")]
    margin_left: Option<f32>,

    /// Right margin in cm
    #[arg(long, help_heading = "Page Layout")]
    margin_right: Option<f32>,

    // --- Clause Indentation ---
    /// Indent per clause level in cm
    #[arg(long, help_heading = "Clause Indentation")]
    indent_per_level: Option<f32>,

    /// Hanging indent for clause numbers in cm
    #[arg(long, help_heading = "Clause Indentation")]
    hanging_indent: Option<f32>,

    /// Align first-level body clauses with second level
    #[arg(
        long,
        conflicts_with = "no_body_align_first_level",
        help_heading = "Clause Indentation"
    )]
    body_align_first_level: bool,

    /// Do not align first-level body clauses with second level
    #[arg(
        long,
        conflicts_with = "body_align_first_level",
        help_heading = "Clause Indentation"
    )]
    no_body_align_first_level: bool,

    /// Align first-level recital clauses with second level
    #[arg(
        long,
        conflicts_with = "no_recitals_align_first_level",
        help_heading = "Clause Indentation"
    )]
    recitals_align_first_level: bool,

    /// Do not align first-level recital clauses with second level
    #[arg(
        long,
        conflicts_with = "recitals_align_first_level",
        help_heading = "Clause Indentation"
    )]
    no_recitals_align_first_level: bool,

    /// Numbering convention for clause hierarchy
    #[arg(long, value_enum, help_heading = "Clause Indentation")]
    numbering_convention: Option<NumberingConventionArg>,

    // --- Formatting ---
    /// Date format string (chrono strftime syntax)
    #[arg(long, help_heading = "Formatting")]
    date_format: Option<String>,

    /// Placeholder text for missing party names (default: "[Name]")
    #[arg(long, help_heading = "Formatting")]
    name_placeholder: Option<String>,

    // --- Cover Page ---
    /// Enable cover page
    #[arg(
        long = "cover",
        conflicts_with = "no_cover",
        help_heading = "Cover Page"
    )]
    cover: bool,

    /// Disable cover page
    #[arg(
        long = "no-cover",
        conflicts_with = "cover",
        help_heading = "Cover Page"
    )]
    no_cover: bool,

    /// Cover page "between" label
    #[arg(long, help_heading = "Cover Page")]
    cover_between_label: Option<String>,

    /// Cover page party format
    #[arg(long, value_enum, help_heading = "Cover Page")]
    cover_party_format: Option<PartyFormatArg>,

    /// Show reference on cover page
    #[arg(long, conflicts_with = "no_cover_ref", help_heading = "Cover Page")]
    cover_ref: bool,

    /// Hide reference on cover page
    #[arg(long, conflicts_with = "cover_ref", help_heading = "Cover Page")]
    no_cover_ref: bool,

    /// Show author on cover page
    #[arg(long, conflicts_with = "no_cover_author", help_heading = "Cover Page")]
    cover_author: bool,

    /// Hide author on cover page
    #[arg(long, conflicts_with = "cover_author", help_heading = "Cover Page")]
    no_cover_author: bool,

    /// Show status on cover page
    #[arg(long, conflicts_with = "no_cover_status", help_heading = "Cover Page")]
    cover_status: bool,

    /// Hide status on cover page
    #[arg(long, conflicts_with = "cover_status", help_heading = "Cover Page")]
    no_cover_status: bool,

    // --- Table of Contents ---
    /// Enable table of contents
    #[arg(
        long = "toc",
        conflicts_with = "no_toc",
        help_heading = "Table of Contents"
    )]
    toc: bool,

    /// Disable table of contents
    #[arg(
        long = "no-toc",
        conflicts_with = "toc",
        help_heading = "Table of Contents"
    )]
    no_toc: bool,

    /// Table of contents heading text
    #[arg(long, help_heading = "Table of Contents")]
    toc_heading: Option<String>,

    // --- Footer ---
    /// Show reference in footer
    #[arg(long, conflicts_with = "no_footer_ref", help_heading = "Footer")]
    footer_ref: bool,

    /// Hide reference in footer
    #[arg(long, conflicts_with = "footer_ref", help_heading = "Footer")]
    no_footer_ref: bool,

    /// Show page numbers in footer
    #[arg(
        long,
        conflicts_with = "no_footer_page_number",
        help_heading = "Footer"
    )]
    footer_page_number: bool,

    /// Hide page numbers in footer
    #[arg(long, conflicts_with = "footer_page_number", help_heading = "Footer")]
    no_footer_page_number: bool,

    /// Show version in footer (appended to reference)
    #[arg(long, conflicts_with = "no_footer_version", help_heading = "Footer")]
    footer_version: bool,

    /// Hide version in footer
    #[arg(long, conflicts_with = "footer_version", help_heading = "Footer")]
    no_footer_version: bool,

    // --- Preamble ---
    /// Enable parties preamble
    #[arg(
        long = "preamble",
        conflicts_with = "no_preamble",
        help_heading = "Preamble"
    )]
    preamble: bool,

    /// Disable parties preamble
    #[arg(
        long = "no-preamble",
        conflicts_with = "preamble",
        help_heading = "Preamble"
    )]
    no_preamble: bool,

    /// Preamble style
    #[arg(long, value_enum, help_heading = "Preamble")]
    preamble_style: Option<PreambleStyleArg>,

    // --- Schedule ---
    /// Schedule position in the document
    #[arg(long, value_enum, help_heading = "Schedule")]
    schedule_position: Option<SchedulePositionArg>,

    /// Schedule item ordering
    #[arg(long, value_enum, help_heading = "Schedule")]
    schedule_order: Option<ScheduleOrderArg>,

    // --- Signatures ---
    /// Enable signature pages
    #[arg(
        long = "enable-signatures",
        conflicts_with = "no_signatures",
        help_heading = "Signatures"
    )]
    enable_signatures: bool,

    /// Disable signature pages
    #[arg(
        long = "no-signatures",
        conflicts_with = "enable_signatures",
        help_heading = "Signatures"
    )]
    no_signatures: bool,

    /// Signature pages heading text
    #[arg(long, help_heading = "Signatures")]
    signatures_heading: Option<String>,

    /// Default signature template key
    #[arg(long, help_heading = "Signatures")]
    signatures_template: Option<String>,

    /// Each signature block on its own page
    #[arg(long, help_heading = "Signatures")]
    signatures_separate_pages: bool,
}

impl StyleOverrides {
    fn apply(self, config: &mut lexicon_docx::style::StyleConfig) {
        // Typography
        if let Some(v) = self.font_family {
            config.font_family = v;
        }
        if let Some(v) = self.font_size {
            config.font_size = v;
        }
        if let Some(v) = self.heading_font_family {
            config.heading_font_family = v;
        }
        if let Some(v) = self.title_size {
            config.title_size = v;
        }
        if let Some(v) = self.heading1_size {
            config.heading1_size = v;
        }
        if let Some(v) = self.heading2_size {
            config.heading2_size = v;
        }
        if let Some(v) = self.heading_space_before {
            config.heading_space_before = v;
        }
        if let Some(v) = self.heading_space_after {
            config.heading_space_after = v;
        }
        if let Some(v) = self.paragraph_space_before {
            config.paragraph_space_before = v;
        }
        if let Some(v) = self.paragraph_space_after {
            config.paragraph_space_after = v;
        }
        if let Some(v) = self.line_spacing {
            config.line_spacing = v;
        }
        if let Some(v) = self.brand_color {
            config.brand_color = Some(v);
        }
        if let Some(v) = self.defined_term_style {
            config.defined_term_style = match v {
                DefinedTermStyleArg::Bold => lexicon_docx::style::DefinedTermStyle::Bold,
                DefinedTermStyleArg::Quoted => lexicon_docx::style::DefinedTermStyle::Quoted,
                DefinedTermStyleArg::BoldQuoted => {
                    lexicon_docx::style::DefinedTermStyle::BoldQuoted
                }
            };
        }

        // Page Layout
        if let Some(v) = self.page_size {
            config.page_size = match v {
                PageSizeArg::A4 => lexicon_docx::style::PageSize::A4,
                PageSizeArg::Letter => lexicon_docx::style::PageSize::Letter,
            };
        }
        if let Some(v) = self.margin_top {
            config.margin_top_cm = v;
        }
        if let Some(v) = self.margin_bottom {
            config.margin_bottom_cm = v;
        }
        if let Some(v) = self.margin_left {
            config.margin_left_cm = v;
        }
        if let Some(v) = self.margin_right {
            config.margin_right_cm = v;
        }

        // Clause Indentation
        if let Some(v) = self.indent_per_level {
            config.indent_per_level_cm = v;
        }
        if let Some(v) = self.hanging_indent {
            config.hanging_indent_cm = v;
        }
        if self.body_align_first_level {
            config.body_align_first_level = true;
        }
        if self.no_body_align_first_level {
            config.body_align_first_level = false;
        }
        if self.recitals_align_first_level {
            config.recitals_align_first_level = true;
        }
        if self.no_recitals_align_first_level {
            config.recitals_align_first_level = false;
        }
        if let Some(v) = self.numbering_convention {
            config.numbering_convention = v.to_style();
        }

        // Formatting
        if let Some(v) = self.date_format {
            config.date_format = v;
        }
        if let Some(v) = self.name_placeholder {
            config.name_placeholder = v;
        }

        // Cover Page
        if self.cover {
            config.cover.enabled = true;
        }
        if self.no_cover {
            config.cover.enabled = false;
        }
        if let Some(v) = self.cover_between_label {
            config.cover.between_label = v;
        }
        if let Some(v) = self.cover_party_format {
            config.cover.party_format = match v {
                PartyFormatArg::NameSpecRole => lexicon_docx::style::PartyFormat::NameSpecRole,
                PartyFormatArg::NameRole => lexicon_docx::style::PartyFormat::NameRole,
                PartyFormatArg::NameOnly => lexicon_docx::style::PartyFormat::NameOnly,
            };
        }
        if self.cover_ref {
            config.cover.show_ref = true;
        }
        if self.no_cover_ref {
            config.cover.show_ref = false;
        }
        if self.cover_author {
            config.cover.show_author = true;
        }
        if self.no_cover_author {
            config.cover.show_author = false;
        }
        if self.cover_status {
            config.cover.show_status = true;
        }
        if self.no_cover_status {
            config.cover.show_status = false;
        }

        // Table of Contents
        if self.toc {
            config.toc.enabled = true;
        }
        if self.no_toc {
            config.toc.enabled = false;
        }
        if let Some(v) = self.toc_heading {
            config.toc.heading = v;
        }

        // Footer
        if self.footer_ref {
            config.footer.show_ref = true;
        }
        if self.no_footer_ref {
            config.footer.show_ref = false;
        }
        if self.footer_page_number {
            config.footer.show_page_number = true;
        }
        if self.no_footer_page_number {
            config.footer.show_page_number = false;
        }
        if self.footer_version {
            config.footer.show_version = true;
        }
        if self.no_footer_version {
            config.footer.show_version = false;
        }

        // Preamble
        if self.preamble {
            config.preamble.enabled = true;
        }
        if self.no_preamble {
            config.preamble.enabled = false;
        }
        if let Some(v) = self.preamble_style {
            config.preamble.style = match v {
                PreambleStyleArg::Simple => lexicon_docx::style::PreambleStyle::Simple,
                PreambleStyleArg::Prose => lexicon_docx::style::PreambleStyle::Prose,
                PreambleStyleArg::Custom => lexicon_docx::style::PreambleStyle::Custom,
            };
        }

        // Schedule
        if let Some(v) = self.schedule_position {
            config.schedule_position = match v {
                SchedulePositionArg::End => lexicon_docx::style::SchedulePosition::End,
                SchedulePositionArg::AfterToc => lexicon_docx::style::SchedulePosition::AfterToc,
            };
        }
        if let Some(v) = self.schedule_order {
            config.schedule_order = match v {
                ScheduleOrderArg::Document => lexicon_docx::style::ScheduleOrder::Document,
                ScheduleOrderArg::Alphabetical => lexicon_docx::style::ScheduleOrder::Alphabetical,
            };
        }

        // Signatures
        if self.enable_signatures {
            config.signatures.enabled = true;
        }
        if self.no_signatures {
            config.signatures.enabled = false;
        }
        if let Some(v) = self.signatures_heading {
            config.signatures.heading = Some(v);
        }
        if let Some(v) = self.signatures_template {
            config.signatures.default_template = Some(v);
        }
        if self.signatures_separate_pages {
            config.signatures.separate_pages = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            input,
            output,
            style,
            signatures,
            strict,
            overrides,
        } => {
            let output_path = output.unwrap_or_else(|| input.with_extension("docx"));
            let input_dir = input.parent();

            // Load style config: explicit --style flag → input dir → XDG → defaults
            let mut style_config = match style {
                Some(path) => match lexicon_docx::style::StyleConfig::load(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error loading style config: {}", e);
                        std::process::exit(1);
                    }
                },
                None => match lexicon_docx::resolve_config_path("style.toml", input_dir) {
                    Some(path) => match lexicon_docx::style::StyleConfig::load(&path) {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Error loading style config from {}: {}", path.display(), e);
                            std::process::exit(1);
                        }
                    },
                    None => lexicon_docx::style::StyleConfig::default(),
                },
            };

            // Apply CLI overrides (highest priority)
            overrides.apply(&mut style_config);

            // Resolve signatures definitions: explicit flag, then auto-discover
            let signatures_path = match signatures {
                Some(path) => Some(path),
                None => lexicon_docx::resolve_config_path("signatures.toml", input_dir),
            };

            let input_text = match std::fs::read_to_string(&input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Error reading {}: {}", input.display(), e);
                    std::process::exit(1);
                }
            };

            match lexicon_docx::process(
                &input_text,
                &style_config,
                input_dir,
                signatures_path.as_deref(),
            ) {
                Ok((bytes, diagnostics)) => {
                    // Apply the same [lint] config and inline suppressions the
                    // linter uses, so build and lint never disagree.
                    let diagnostics = lexicon_docx::lint::filter_build_diagnostics(
                        &input_text,
                        diagnostics,
                        &style_config,
                    );
                    let has_errors = print_diagnostics(&diagnostics);

                    // --strict counts warnings only — info diagnostics are
                    // lint-only and not even printed here, so failing on them
                    // would be undiagnosable (and would disagree with lint).
                    let has_warnings = diagnostics
                        .iter()
                        .any(|d| d.level == lexicon_docx::error::DiagLevel::Warning);
                    if has_errors || (strict && has_warnings) {
                        eprintln!("Build failed due to errors.");
                        std::process::exit(1);
                    }

                    match std::fs::write(&output_path, bytes) {
                        Ok(_) => {
                            eprintln!("Written: {}", output_path.display());
                        }
                        Err(e) => {
                            eprintln!("Error writing {}: {}", output_path.display(), e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Lint {
            inputs,
            format,
            strict,
            ignore,
            min_severity,
            style,
            numbering_convention,
            list_rules,
            fix,
        } => {
            if list_rules {
                match format {
                    LintFormatArg::Json => print!("{}", lexicon_docx::lint::rules_to_json()),
                    _ => print!("{}", lexicon_docx::lint::rules_to_text()),
                }
                std::process::exit(0);
            }
            if fix {
                run_fix(&inputs);
            }
            run_lint(
                &inputs,
                &format,
                strict,
                &ignore,
                min_severity,
                style.as_deref(),
                numbering_convention.map(|c| c.to_style()),
            );
        }

        Commands::Validate { input, strict } => {
            run_lint(
                std::slice::from_ref(&input),
                &LintFormatArg::Text,
                strict,
                &[],
                None,
                None,
                None,
            );
        }

        Commands::Man { dir } => {
            generate_man_pages(&dir);
        }
    }
}

/// Print build diagnostics to stderr. Info-level diagnostics are lint-only
/// and skipped here — run `lexicon-docx lint` to see them.
fn print_diagnostics(diagnostics: &[lexicon_docx::error::Diagnostic]) -> bool {
    let mut has_errors = false;
    for d in diagnostics {
        if matches!(d.level, lexicon_docx::error::DiagLevel::Info) {
            continue;
        }
        eprintln!("{}", d);
        if matches!(d.level, lexicon_docx::error::DiagLevel::Error) {
            has_errors = true;
        }
    }
    has_errors
}

/// Apply autofixes in place, then fall through to a normal lint run so the
/// user sees what is left. Exits with status 2 if a file cannot be read or
/// written, or if a rewrite could not be verified as structure-preserving.
fn run_fix(inputs: &[PathBuf]) {
    use lexicon_docx::fix::FixOutcome;

    let mut failed = false;
    for input in inputs {
        let source = match std::fs::read_to_string(input) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", input.display(), e);
                failed = true;
                continue;
            }
        };
        match lexicon_docx::fix::normalise_ordinals(&source) {
            FixOutcome::Unchanged => {}
            FixOutcome::Fixed(output, count) => {
                if let Err(e) = std::fs::write(input, output) {
                    eprintln!("error: cannot write {}: {}", input.display(), e);
                    failed = true;
                } else {
                    eprintln!(
                        "fixed {}: normalised {} hand-numbered ordinal{}",
                        input.display(),
                        count,
                        if count == 1 { "" } else { "s" }
                    );
                }
            }
            FixOutcome::Unsafe => {
                eprintln!(
                    "error: {}: normalising ordinals would have changed the document structure; left unchanged.\n  \
                     This happens when continuation content is already mis-indented: narrowing a marker moves \
                     the content column, which changes where that content lands. Fix the continuation-indent \
                     and continuation-reattached findings first, then re-run --fix.",
                    input.display()
                );
                failed = true;
            }
        }
    }

    if failed {
        std::process::exit(2);
    }
}

/// Run the linter over one or more files and exit with an appropriate status
/// code: 0 = clean (or warnings without --strict), 1 = errors (or warnings
/// with --strict), 2 = any input file could not be read.
///
/// Each file is linted with its own resolved style.toml (unless --style pins
/// one), so the [lint] section and numbering convention follow the document.
fn run_lint(
    inputs: &[PathBuf],
    format: &LintFormatArg,
    strict: bool,
    cli_ignore: &[String],
    min_severity: Option<SeverityArg>,
    style_flag: Option<&std::path::Path>,
    convention_flag: Option<lexicon_docx::style::NumberingConvention>,
) -> ! {
    let mut reports: Vec<(String, lexicon_docx::lint::LintReport)> = Vec::new();
    let mut any_unreadable = false;

    for input in inputs {
        let file_label = input.display().to_string();
        let input_dir = input.parent();

        // Resolve style config for this file: --style flag, else the file's
        // own directory, else XDG, else defaults. A config that fails to load
        // becomes a style-error diagnostic for this file (keeping JSON output
        // valid and other files' reports intact) and exits 2 like an
        // unreadable input.
        let style_path = style_flag
            .map(|p| p.to_path_buf())
            .or_else(|| lexicon_docx::resolve_config_path("style.toml", input_dir));
        let style_config = match style_path {
            Some(path) => match lexicon_docx::style::StyleConfig::load(&path) {
                Ok(c) => c,
                Err(e) => {
                    any_unreadable = true;
                    reports.push((
                        file_label,
                        lexicon_docx::lint::LintReport::from_diagnostic(
                            lexicon_docx::error::Diagnostic::error(
                                "style-error",
                                format!(
                                    "Error loading style config from {}: {}",
                                    path.display(),
                                    e
                                ),
                            ),
                        ),
                    ));
                    continue;
                }
            },
            None => lexicon_docx::style::StyleConfig::default(),
        };

        let mut options = lexicon_docx::lint::LintOptions::from_style(&style_config);
        options.ignore.extend(cli_ignore.iter().cloned());
        if let Some(level) = min_severity {
            options.min_severity = level.to_level();
        }
        let convention = convention_flag.unwrap_or(style_config.numbering_convention);

        let report = match std::fs::read_to_string(input) {
            Ok(text) => lexicon_docx::lint::lint(&text, input_dir, convention, &options),
            Err(e) => {
                any_unreadable = true;
                lexicon_docx::lint::LintReport::from_diagnostic(
                    lexicon_docx::error::Diagnostic::error(
                        "io-error",
                        format!("Error reading {}: {}", file_label, e),
                    ),
                )
            }
        };
        reports.push((file_label, report));
    }

    match format {
        LintFormatArg::Text => print!("{}", lexicon_docx::lint::reports_to_text(&reports)),
        LintFormatArg::Json => println!("{}", lexicon_docx::lint::reports_to_json(&reports)),
        LintFormatArg::Github => print!("{}", lexicon_docx::lint::reports_to_github(&reports)),
    }

    let has_errors = reports.iter().any(|(_, r)| r.has_errors());
    let has_warnings = reports.iter().any(|(_, r)| r.has_warnings());
    if any_unreadable {
        std::process::exit(2);
    }
    if has_errors || (strict && has_warnings) {
        std::process::exit(1);
    }
    std::process::exit(0);
}

fn generate_man_pages(dir: &std::path::Path) {
    use clap::CommandFactory;

    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("Error creating directory {}: {}", dir.display(), e);
        std::process::exit(1);
    }

    let cmd = Cli::command();

    // Main man page: lexicon-docx(1)
    let man = clap_mangen::Man::new(cmd.clone());
    let path = dir.join("lexicon-docx.1");
    let mut file = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error creating {}: {}", path.display(), e);
            std::process::exit(1);
        }
    };
    if let Err(e) = man.render(&mut file) {
        eprintln!("Error writing {}: {}", path.display(), e);
        std::process::exit(1);
    }
    eprintln!("Written: {}", path.display());

    // Subcommand man pages: lexicon-docx-build(1), lexicon-docx-validate(1)
    for subcmd in cmd.get_subcommands() {
        let name = format!("lexicon-docx-{}", subcmd.get_name());
        let man = clap_mangen::Man::new(subcmd.clone());
        let path = dir.join(format!("{}.1", name));
        let mut file = match std::fs::File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error creating {}: {}", path.display(), e);
                std::process::exit(1);
            }
        };
        if let Err(e) = man.render(&mut file) {
            eprintln!("Error writing {}: {}", path.display(), e);
            std::process::exit(1);
        }
        eprintln!("Written: {}", path.display());
    }
}
