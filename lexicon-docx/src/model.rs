use crate::error::Diagnostic;
use serde::Deserialize;

/// The fully parsed and resolved document.
#[derive(Debug)]
pub struct Document {
    pub meta: DocumentMeta,
    pub body: Vec<BodyElement>,
    pub addenda: Vec<Addendum>,
    pub schedule_items: Vec<ScheduleItem>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentMeta {
    pub title: String,
    #[serde(rename = "type")]
    pub doc_type: Option<String>,
    pub date: String,
    #[serde(rename = "ref")]
    pub ref_: Option<String>,
    pub author: Option<String>,
    pub status: Option<Status>,
    pub version: Option<u32>,
    pub parties: Vec<Party>,
    #[serde(default)]
    pub exhibits: Vec<Exhibit>,
    #[serde(default)]
    pub schedule: Vec<ScheduleDecl>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScheduleDecl {
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Exhibit {
    pub title: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Final,
    Executed,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Draft => write!(f, "DRAFT"),
            Status::Final => write!(f, "FINAL"),
            Status::Executed => write!(f, "EXECUTED"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Party {
    pub name: String,
    pub specifier: Option<String>,
    pub role: String,
    pub entity_type: Option<String>,
}

#[derive(Debug)]
pub enum BodyElement {
    Clause(Clause),
    Prose(Vec<InlineContent>),
}

#[derive(Debug)]
pub struct Clause {
    pub level: ClauseLevel,
    pub heading: Option<ClauseHeading>,
    pub anchor: Option<String>,
    pub number: Option<ClauseNumber>,
    /// Interleaved content and children, preserving source order.
    /// This ensures continuation paragraphs after sub-lists render
    /// in the correct position.
    pub body: Vec<ClauseBody>,
}

#[derive(Debug)]
pub enum ClauseBody {
    Content(ClauseContent),
    Children(Vec<Clause>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClauseLevel {
    TopLevel,
    Clause,
    SubClause,
    SubSubClause,
}

#[derive(Debug)]
pub struct ClauseHeading {
    pub text: Vec<InlineContent>,
    pub level: u8,
}

#[derive(Debug, Clone)]
pub enum ClauseNumber {
    TopLevel(u32),
    Clause(u32, u32),
    SubClause(u32, u32, char),
    SubSubClause(u32, u32, char, String),
}

impl std::fmt::Display for ClauseNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClauseNumber::TopLevel(a) => write!(f, "{}.", a),
            ClauseNumber::Clause(a, b) => write!(f, "{}.{}", a, b),
            ClauseNumber::SubClause(_, _, c) => write!(f, "({})", c),
            ClauseNumber::SubSubClause(_, _, _, r) => write!(f, "({})", r),
        }
    }
}

impl ClauseNumber {
    pub fn full_reference(&self) -> String {
        match self {
            ClauseNumber::TopLevel(a) => format!("clause {}", a),
            ClauseNumber::Clause(a, b) => format!("clause {}.{}", a, b),
            ClauseNumber::SubClause(a, b, c) => format!("clause {}.{}({})", a, b, c),
            ClauseNumber::SubSubClause(a, b, c, r) => {
                format!("clause {}.{}({})({})", a, b, c, r)
            }
        }
    }
}

#[derive(Debug)]
pub enum ClauseContent {
    Paragraph(Vec<InlineContent>),
    Blockquote(Vec<InlineContent>),
    Table(Table),
}

#[derive(Debug, Clone)]
pub enum InlineContent {
    Text(String),
    Bold(String),
    Italic(String),
    Superscript(String),
    CrossRef {
        display: String,
        anchor_id: String,
        resolved: Option<String>,
    },
    Link {
        text: String,
        url: String,
    },
    SoftBreak,
    LineBreak,
}

impl InlineContent {
    pub fn as_plain_text(&self) -> String {
        match self {
            InlineContent::Text(s)
            | InlineContent::Bold(s)
            | InlineContent::Italic(s)
            | InlineContent::Superscript(s) => s.clone(),
            InlineContent::CrossRef { display, resolved, .. } => {
                resolved.as_ref().unwrap_or(display).clone()
            }
            InlineContent::Link { text, .. } => text.clone(),
            InlineContent::SoftBreak => " ".to_string(),
            InlineContent::LineBreak => "\n".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct Table {
    pub headers: Vec<Vec<InlineContent>>,
    pub rows: Vec<Vec<Vec<InlineContent>>>,
}

#[derive(Debug)]
pub struct Addendum {
    pub number: u32,
    pub title: String,
    pub content: Vec<AddendumContent>,
}

impl Addendum {
    /// The full rendered heading, e.g. "ADDENDUM 1 - Details of Processing"
    pub fn heading(&self) -> String {
        if self.title.is_empty() {
            format!("ADDENDUM {}", self.number)
        } else {
            format!("ADDENDUM {} - {}", self.number, self.title)
        }
    }
}

#[derive(Debug)]
pub enum AddendumContent {
    Paragraph(Vec<InlineContent>),
    Heading(u8, Vec<InlineContent>),
    ClauseList(Vec<Clause>),
    NumberedList(Vec<Vec<InlineContent>>),
    Table(Table),
    BulletList(Vec<Vec<InlineContent>>),
}

#[derive(Debug, Clone)]
pub struct ScheduleItem {
    pub term: String,
    pub schedule_index: usize,
}
