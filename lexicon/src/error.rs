use std::fmt;

#[derive(Debug, Clone)]
pub enum DiagLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagLevel,
    pub message: String,
    pub location: Option<String>,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.level {
            DiagLevel::Error => "error",
            DiagLevel::Warning => "warning",
        };
        if let Some(ref loc) = self.location {
            write!(f, "{}: {} ({})", prefix, self.message, loc)
        } else {
            write!(f, "{}: {}", prefix, self.message)
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
