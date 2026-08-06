use std::fmt;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagLevel {
    Error,
    Warning,
    Info,
}

/// A diagnostic produced while parsing, resolving, or linting a document.
///
/// `code` is a stable, kebab-case rule identifier (e.g. `unused-term`) that
/// tools and AI agents can match on. `location` is a human-readable position
/// ("clause 3.1(a)", "front-matter", "Addendum 1"); `line` is the 1-based
/// source line in the input file where the issue was detected, when known.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub level: DiagLevel,
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
}

impl Diagnostic {
    pub fn new(level: DiagLevel, code: &'static str, message: impl Into<String>) -> Self {
        Diagnostic {
            level,
            code,
            message: message.into(),
            location: None,
            line: None,
        }
    }

    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(DiagLevel::Error, code, message)
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(DiagLevel::Warning, code, message)
    }

    pub fn info(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(DiagLevel::Info, code, message)
    }

    /// Attach a human-readable location ("clause 3.1", "front-matter", ...).
    pub fn at(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Attach an optional human-readable location.
    pub fn at_opt(mut self, location: Option<impl Into<String>>) -> Self {
        self.location = location.map(Into::into);
        self
    }

    /// Attach a 1-based source line number.
    pub fn at_line(mut self, line: Option<usize>) -> Self {
        self.line = line;
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.level {
            DiagLevel::Error => "error",
            DiagLevel::Warning => "warning",
            DiagLevel::Info => "info",
        };
        write!(f, "{}[{}]: {}", prefix, self.code, self.message)?;
        match (&self.location, self.line) {
            (Some(loc), Some(line)) => write!(f, " ({}, line {})", loc, line),
            (Some(loc), None) => write!(f, " ({})", loc),
            (None, Some(line)) => write!(f, " (line {})", line),
            (None, None) => Ok(()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LexiconError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Front-matter error: {0}")]
    FrontMatter(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Render error: {0}")]
    Render(String),
}

pub type Result<T> = std::result::Result<T, LexiconError>;
