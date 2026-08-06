use crate::error::Diagnostic;
use serde::{Deserialize, Deserializer};

/// Deserialize version as a string, accepting YAML integers, floats, or strings.
fn deserialize_version<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let value: Option<serde_yaml::Value> = Option::deserialize(deserializer)?;
    Ok(value.map(|v| match v {
        serde_yaml::Value::Number(n) => {
            // Preserve integer formatting (no trailing ".0")
            if let Some(i) = n.as_u64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                f.to_string()
            } else {
                n.to_string()
            }
        }
        serde_yaml::Value::String(s) => s,
        other => other.as_str().unwrap_or("").to_string(),
    }))
}

/// The fully parsed and resolved document.
#[derive(Debug)]
pub struct Document {
    pub meta: DocumentMeta,
    pub recitals: Option<Recitals>,
    pub body_heading: Option<String>,
    pub body: Vec<BodyElement>,
    pub addenda: Vec<Addendum>,
    pub schedule_items: Vec<ScheduleItem>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
pub struct Recitals {
    pub heading: String,
    pub body: Vec<BodyElement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentMeta {
    pub title: String,
    #[serde(rename = "type")]
    pub doc_type: Option<String>,
    pub date: Option<String>,
    #[serde(rename = "ref")]
    pub ref_: Option<String>,
    pub author: Option<String>,
    pub status: Option<Status>,
    #[serde(default, deserialize_with = "deserialize_version")]
    pub version: Option<String>,
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
    pub name: Option<String>,
    pub specifier: Option<String>,
    pub role: String,
    pub entity_type: Option<String>,
}

#[derive(Debug)]
pub enum BodyElement {
    Clause(Clause),
    Prose(Vec<InlineContent>),
    BulletList(Vec<Vec<InlineContent>>),
}

#[derive(Debug)]
pub struct Clause {
    pub level: ClauseLevel,
    pub heading: Option<ClauseHeading>,
    pub anchor: Option<String>,
    pub number: Option<ClauseNumber>,
    /// 1-based line in the source file where this clause starts.
    pub source_line: Option<usize>,
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
    Paragraph,
    SubParagraph,
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
    SubClause(u32, u32, u32),
    SubSubClause(u32, u32, u32, u32),
    Paragraph(u32, u32, u32, u32, u32),
    SubParagraph(u32, u32, u32, u32, u32, u32),
}

impl std::fmt::Display for ClauseNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClauseNumber::TopLevel(a) => write!(f, "{}.", a),
            ClauseNumber::Clause(a, b) => write!(f, "{}.{}", a, b),
            ClauseNumber::SubClause(a, b, c) => write!(f, "{}.{}.{}", a, b, c),
            ClauseNumber::SubSubClause(a, b, c, d) => write!(f, "{}.{}.{}.{}", a, b, c, d),
            ClauseNumber::Paragraph(a, b, c, d, e) => {
                write!(f, "{}.{}.{}.{}.{}", a, b, c, d, e)
            }
            ClauseNumber::SubParagraph(a, b, c, d, e, g) => {
                write!(f, "{}.{}.{}.{}.{}.{}", a, b, c, d, e, g)
            }
        }
    }
}

use crate::style::NumberingConvention;

impl ClauseNumber {
    pub fn full_reference(&self, prefix: &str, convention: NumberingConvention) -> String {
        let num = match convention {
            NumberingConvention::Commonwealth => self.format_commonwealth(),
            NumberingConvention::Decimal => self.format_decimal(),
            NumberingConvention::UsTraditional => self.format_us_traditional(),
        };
        format!("{} {}", prefix, num)
    }

    fn format_commonwealth(&self) -> String {
        match self {
            ClauseNumber::TopLevel(a) => a.to_string(),
            ClauseNumber::Clause(a, b) => format!("{}.{}", a, b),
            ClauseNumber::SubClause(a, b, c) => {
                format!("{}.{}({})", a, b, to_lower_letter(*c))
            }
            ClauseNumber::SubSubClause(a, b, c, d) => {
                format!(
                    "{}.{}({})({})",
                    a,
                    b,
                    to_lower_letter(*c),
                    to_lower_roman(*d)
                )
            }
            ClauseNumber::Paragraph(a, b, c, d, e) => {
                format!(
                    "{}.{}({})({})({})",
                    a,
                    b,
                    to_lower_letter(*c),
                    to_lower_roman(*d),
                    to_upper_letter(*e)
                )
            }
            ClauseNumber::SubParagraph(a, b, c, d, e, g) => {
                format!(
                    "{}.{}({})({})({})({})",
                    a,
                    b,
                    to_lower_letter(*c),
                    to_lower_roman(*d),
                    to_upper_letter(*e),
                    to_upper_roman(*g)
                )
            }
        }
    }

    fn format_decimal(&self) -> String {
        match self {
            ClauseNumber::TopLevel(a) => a.to_string(),
            ClauseNumber::Clause(a, b) => format!("{}.{}", a, b),
            ClauseNumber::SubClause(a, b, c) => format!("{}.{}.{}", a, b, c),
            ClauseNumber::SubSubClause(a, b, c, d) => format!("{}.{}.{}.{}", a, b, c, d),
            ClauseNumber::Paragraph(a, b, c, d, e) => {
                format!("{}.{}.{}.{}.{}", a, b, c, d, e)
            }
            ClauseNumber::SubParagraph(a, b, c, d, e, g) => {
                format!("{}.{}.{}.{}.{}.{}", a, b, c, d, e, g)
            }
        }
    }

    fn format_us_traditional(&self) -> String {
        match self {
            ClauseNumber::TopLevel(a) => to_upper_roman(*a),
            ClauseNumber::Clause(a, b) => {
                format!("{}.{}", to_upper_roman(*a), to_upper_letter(*b))
            }
            ClauseNumber::SubClause(a, b, c) => {
                format!("{}.{}.{}", to_upper_roman(*a), to_upper_letter(*b), c)
            }
            ClauseNumber::SubSubClause(a, b, c, d) => {
                format!(
                    "{}.{}.{}.{}",
                    to_upper_roman(*a),
                    to_upper_letter(*b),
                    c,
                    to_lower_letter(*d)
                )
            }
            ClauseNumber::Paragraph(a, b, c, d, e) => {
                format!(
                    "{}.{}.{}.{}({})",
                    to_upper_roman(*a),
                    to_upper_letter(*b),
                    c,
                    to_lower_letter(*d),
                    e
                )
            }
            ClauseNumber::SubParagraph(a, b, c, d, e, g) => {
                format!(
                    "{}.{}.{}.{}({})({})",
                    to_upper_roman(*a),
                    to_upper_letter(*b),
                    c,
                    to_lower_letter(*d),
                    e,
                    to_lower_letter(*g)
                )
            }
        }
    }
}

fn to_lower_letter(n: u32) -> char {
    (b'a' + (n - 1) as u8) as char
}

fn to_upper_letter(n: u32) -> char {
    (b'A' + (n - 1) as u8) as char
}

pub(crate) fn to_lower_roman(mut n: u32) -> String {
    let table = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut result = String::new();
    for &(value, numeral) in &table {
        while n >= value {
            result.push_str(numeral);
            n -= value;
        }
    }
    result
}

fn to_upper_roman(n: u32) -> String {
    to_lower_roman(n).to_uppercase()
}

#[derive(Debug)]
pub enum ClauseContent {
    Paragraph(Vec<InlineContent>),
    Blockquote(Vec<InlineContent>),
    Table(Table),
    BulletList(Vec<Vec<InlineContent>>),
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
            InlineContent::CrossRef {
                display, resolved, ..
            } => resolved.as_ref().unwrap_or(display).clone(),
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
    pub anchor: Option<String>,
    /// 1-based line in the source file where this addendum's heading appears.
    pub source_line: Option<usize>,
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
