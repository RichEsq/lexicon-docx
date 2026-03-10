use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lexicon", version, about = "Lexicon Markdown processor for legal contracts")]
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
    },

    /// Validate a Lexicon Markdown file without generating output
    Validate {
        /// Input Lexicon Markdown file
        input: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            input,
            output,
            style,
            signatures,
            strict,
        } => {
            let output_path = output.unwrap_or_else(|| input.with_extension("docx"));
            let input_dir = input.parent();

            let style_config = match style {
                // Explicit --style flag: use that path directly
                Some(path) => match lexicon_docx::style::StyleConfig::load(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error loading style config: {}", e);
                        std::process::exit(1);
                    }
                },
                // No flag: search input dir, then XDG
                None => {
                    match lexicon_docx::resolve_config_path("style.toml", input_dir) {
                        Some(path) => match lexicon_docx::style::StyleConfig::load(&path) {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("Error loading style config from {}: {}", path.display(), e);
                                std::process::exit(1);
                            }
                        },
                        None => lexicon_docx::style::StyleConfig::default(),
                    }
                }
            };

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

            match lexicon_docx::process(&input_text, &style_config, input_dir, signatures_path.as_deref()) {
                Ok((bytes, diagnostics)) => {
                    let has_errors = print_diagnostics(&diagnostics);

                    if has_errors || (strict && !diagnostics.is_empty()) {
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

        Commands::Validate { input } => {
            let input_text = match std::fs::read_to_string(&input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Error reading {}: {}", input.display(), e);
                    std::process::exit(1);
                }
            };

            match lexicon_docx::parse(&input_text) {
                Ok(mut doc) => {
                    lexicon_docx::resolve(&mut doc);
                    let has_errors = print_diagnostics(&doc.diagnostics);

                    if has_errors {
                        std::process::exit(1);
                    } else if doc.diagnostics.is_empty() {
                        eprintln!("Valid.");
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn print_diagnostics(diagnostics: &[lexicon_docx::error::Diagnostic]) -> bool {
    let mut has_errors = false;
    for d in diagnostics {
        eprintln!("{}", d);
        if matches!(d.level, lexicon_docx::error::DiagLevel::Error) {
            has_errors = true;
        }
    }
    has_errors
}
