use comrak::nodes::{AstNode, NodeValue};
use regex::Regex;
use std::sync::LazyLock;

use crate::error::{DiagLevel, Diagnostic};
use crate::model::*;
use super::anchors::strip_anchor;

static ADDENDUM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^addendum(?:\s+\d+)?(?:\s*[-–—]\s*(.*))?$").unwrap()
});

static RECITALS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(recitals|background)$").unwrap()
});

/// Walk a comrak AST and extract the document body as a list of BodyElements.
/// `root` should be the Document node from comrak.
pub fn extract_body<'a>(root: &'a AstNode<'a>) -> (Option<Recitals>, Option<String>, Vec<BodyElement>, Vec<Addendum>, Vec<Diagnostic>) {
    let mut body = Vec::new();
    let mut addenda = Vec::new();
    let mut diagnostics = Vec::new();
    let mut in_addendum: Option<Addendum> = None;
    let mut addendum_counter = 0u32;
    let mut recitals: Option<Recitals> = None;
    let mut in_recitals = false;
    let mut body_heading: Option<String> = None;

    for child in root.children() {
        let data = child.data.borrow();
        match &data.value {
            // Top-level heading — check for recitals, body heading, or addendum
            NodeValue::Heading(h) if h.level == 1 => {
                drop(data);
                let heading_text = collect_plain_text(child);

                if RECITALS_RE.is_match(&heading_text) {
                    if recitals.is_some() {
                        diagnostics.push(Diagnostic {
                            level: DiagLevel::Warning,
                            message: "Duplicate recitals/background heading. Only one recitals section is allowed.".to_string(),
                            location: Some("document body".to_string()),
                        });
                    } else {
                        in_recitals = true;
                        recitals = Some(Recitals {
                            heading: heading_text,
                            body: Vec::new(),
                        });
                    }
                } else if let Some(caps) = ADDENDUM_RE.captures(&heading_text) {
                    in_recitals = false;
                    // Save previous addendum if any
                    if let Some(add) = in_addendum.take() {
                        addenda.push(add);
                    }
                    addendum_counter += 1;
                    let title = caps.get(1)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();
                    in_addendum = Some(Addendum {
                        number: addendum_counter,
                        title,
                        content: Vec::new(),
                    });
                } else if in_recitals {
                    // Non-recitals, non-addendum heading after recitals = body heading
                    in_recitals = false;
                    body_heading = Some(heading_text);
                } else if recitals.is_none() {
                    // No recitals in document — unrecognised heading (existing behaviour)
                    diagnostics.push(Diagnostic {
                        level: DiagLevel::Warning,
                        message: format!(
                            "Unrecognised top-level heading '# {}'. Top-level headings must be 'RECITALS', 'BACKGROUND', or begin with 'ADDENDUM'.",
                            heading_text
                        ),
                        location: Some("document body".to_string()),
                    });
                } else {
                    // Recitals already ended, unexpected extra heading
                    diagnostics.push(Diagnostic {
                        level: DiagLevel::Warning,
                        message: format!(
                            "Unexpected top-level heading '# {}' after body section.",
                            heading_text
                        ),
                        location: Some("document body".to_string()),
                    });
                }
            }

            // Ordered list at top level = clause structure (or simple numbered list in addenda)
            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Ordered => {
                if let Some(ref mut add) = in_addendum {
                    if is_clause_list(child) {
                        let clauses = extract_clauses_from_list(child, ClauseLevel::TopLevel);
                        add.content.push(AddendumContent::ClauseList(clauses));
                    } else {
                        let items = extract_bullet_list(child);
                        add.content.push(AddendumContent::NumberedList(items));
                    }
                } else if in_recitals {
                    if let Some(ref mut rec) = recitals {
                        let clauses = extract_clauses_from_list(child, ClauseLevel::TopLevel);
                        for clause in clauses {
                            rec.body.push(BodyElement::Clause(clause));
                        }
                    }
                } else {
                    let clauses = extract_clauses_from_list(child, ClauseLevel::TopLevel);
                    for clause in clauses {
                        body.push(BodyElement::Clause(clause));
                    }
                }
            }

            // Paragraph outside clause structure
            NodeValue::Paragraph => {
                let inlines = extract_inlines(child);
                if !inlines.is_empty() {
                    if let Some(ref mut add) = in_addendum {
                        add.content.push(AddendumContent::Paragraph(inlines));
                    } else if in_recitals {
                        if let Some(ref mut rec) = recitals {
                            rec.body.push(BodyElement::Prose(inlines));
                        }
                    } else {
                        body.push(BodyElement::Prose(inlines));
                    }
                }
            }

            // Headings inside addenda (## or ###)
            NodeValue::Heading(h) if h.level >= 2 => {
                if let Some(ref mut add) = in_addendum {
                    let inlines = extract_inlines(child);
                    add.content.push(AddendumContent::Heading(h.level, inlines));
                }
            }

            // Tables
            NodeValue::Table(_) => {
                let table = extract_table(child);
                if let Some(ref mut add) = in_addendum {
                    add.content.push(AddendumContent::Table(table));
                }
            }

            // Bullet lists in addenda
            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Bullet => {
                if let Some(ref mut add) = in_addendum {
                    let items = extract_bullet_list(child);
                    add.content.push(AddendumContent::BulletList(items));
                }
            }

            _ => {}
        }
    }

    // Save last addendum
    if let Some(add) = in_addendum {
        addenda.push(add);
    }

    // Warn if recitals present but no body heading
    if recitals.is_some() && body_heading.is_none() {
        diagnostics.push(Diagnostic {
            level: DiagLevel::Warning,
            message: "Recitals section present but no body heading found. Add a top-level heading (e.g. '# Operative Provisions') before the contract clauses.".to_string(),
            location: Some("document body".to_string()),
        });
    }

    (recitals, body_heading, body, addenda, diagnostics)
}

/// Check if an ordered list contains clause structure (headings or nested sub-lists).
/// If it's just simple paragraph items, it's a plain numbered list.
fn is_clause_list<'a>(list_node: &'a AstNode<'a>) -> bool {
    for item in list_node.children() {
        let item_data = item.data.borrow();
        if !matches!(item_data.value, NodeValue::Item(_)) {
            continue;
        }
        drop(item_data);

        for child in item.children() {
            let child_data = child.data.borrow();
            match &child_data.value {
                NodeValue::Heading(_) => return true,
                NodeValue::List(list)
                    if list.list_type == comrak::nodes::ListType::Ordered =>
                {
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// Extract clauses from an ordered List node.
fn extract_clauses_from_list<'a>(
    list_node: &'a AstNode<'a>,
    level: ClauseLevel,
) -> Vec<Clause> {
    let mut clauses = Vec::new();

    for item in list_node.children() {
        let item_data = item.data.borrow();
        if !matches!(item_data.value, NodeValue::Item(_)) {
            continue;
        }
        drop(item_data);

        let clause = extract_clause_from_item(item, level);
        clauses.push(clause);
    }

    clauses
}

/// Extract a single Clause from a list Item node.
fn extract_clause_from_item<'a>(
    item: &'a AstNode<'a>,
    level: ClauseLevel,
) -> Clause {
    let mut heading = None;
    let mut anchor = None;
    let mut body: Vec<ClauseBody> = Vec::new();

    for child in item.children() {
        let data = child.data.borrow();
        match &data.value {
            NodeValue::Heading(h) => {
                let hlevel = h.level;
                drop(data);
                let raw_inlines = extract_inlines(child);
                let heading_text = inlines_to_plain_text(&raw_inlines);
                let (cleaned_text, head_anchor) = strip_anchor(&heading_text);

                if head_anchor.is_some() {
                    anchor = head_anchor;
                }

                // Rebuild inlines with cleaned text for the heading
                let cleaned_inlines = if cleaned_text != heading_text {
                    rebuild_inlines_stripped(child)
                } else {
                    raw_inlines
                };

                heading = Some(ClauseHeading {
                    text: cleaned_inlines,
                    level: hlevel,
                });
            }

            NodeValue::Paragraph => {
                drop(data);
                let mut inlines = extract_inlines(child);

                // Check last inline for anchor
                if let Some(last) = inlines.last() {
                    if let InlineContent::Text(t) = last {
                        let (cleaned, para_anchor) = strip_anchor(t);
                        if para_anchor.is_some() {
                            anchor = para_anchor;
                            if cleaned.is_empty() {
                                inlines.pop();
                            } else {
                                let len = inlines.len();
                                inlines[len - 1] = InlineContent::Text(cleaned);
                            }
                        }
                    }
                }

                if !inlines.is_empty() {
                    body.push(ClauseBody::Content(ClauseContent::Paragraph(inlines)));
                }
            }

            NodeValue::List(list)
                if list.list_type == comrak::nodes::ListType::Ordered =>
            {
                drop(data);
                let child_level = next_level(level);
                let child_clauses = extract_clauses_from_list(child, child_level);
                body.push(ClauseBody::Children(child_clauses));
            }

            NodeValue::BlockQuote => {
                drop(data);
                let inlines = extract_blockquote_inlines(child);
                body.push(ClauseBody::Content(ClauseContent::Blockquote(inlines)));
            }

            NodeValue::Table(_) => {
                drop(data);
                let table = extract_table(child);
                body.push(ClauseBody::Content(ClauseContent::Table(table)));
            }

            _ => {
                drop(data);
            }
        }
    }

    Clause {
        level,
        heading,
        anchor,
        number: None,
        body,
    }
}

fn next_level(level: ClauseLevel) -> ClauseLevel {
    match level {
        ClauseLevel::TopLevel => ClauseLevel::Clause,
        ClauseLevel::Clause => ClauseLevel::SubClause,
        ClauseLevel::SubClause => ClauseLevel::SubSubClause,
        ClauseLevel::SubSubClause => ClauseLevel::SubSubClause, // cap at this level
    }
}

/// Extract inline content from a node's children.
pub fn extract_inlines<'a>(node: &'a AstNode<'a>) -> Vec<InlineContent> {
    let mut inlines = Vec::new();

    for child in node.children() {
        let data = child.data.borrow();
        match &data.value {
            NodeValue::Text(t) => {
                inlines.push(InlineContent::Text(t.clone()));
            }
            NodeValue::Strong => {
                drop(data);
                let inner = collect_plain_text(child);
                inlines.push(InlineContent::Bold(inner));
            }
            NodeValue::Emph => {
                drop(data);
                let inner = collect_plain_text(child);
                inlines.push(InlineContent::Italic(inner));
            }
            NodeValue::Link(link) => {
                let link_url = link.url.clone();
                drop(data);
                let display = collect_plain_text(child);
                if link_url.starts_with('#') {
                    inlines.push(InlineContent::CrossRef {
                        display,
                        anchor_id: link_url[1..].to_string(),
                        resolved: None,
                    });
                } else {
                    inlines.push(InlineContent::Link {
                        text: display,
                        url: link_url,
                    });
                }
            }
            NodeValue::SoftBreak => {
                inlines.push(InlineContent::SoftBreak);
            }
            NodeValue::LineBreak => {
                inlines.push(InlineContent::LineBreak);
            }
            NodeValue::Code(c) => {
                inlines.push(InlineContent::Text(c.literal.clone()));
            }
            NodeValue::Superscript => {
                drop(data);
                let inner = collect_plain_text(child);
                inlines.push(InlineContent::Superscript(inner));
            }
            _ => {
                drop(data);
                // Recurse into unknown nodes to get their text
                let inner = extract_inlines(child);
                inlines.extend(inner);
            }
        }
    }

    inlines
}

/// Collect all text from a node as plain text (ignoring formatting).
fn collect_plain_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut result = String::new();
    for child in node.children() {
        let data = child.data.borrow();
        match &data.value {
            NodeValue::Text(t) => result.push_str(t),
            NodeValue::SoftBreak => result.push(' '),
            NodeValue::LineBreak => result.push('\n'),
            NodeValue::Code(c) => result.push_str(&c.literal),
            _ => {
                drop(data);
                result.push_str(&collect_plain_text(child));
            }
        }
    }
    result
}

fn inlines_to_plain_text(inlines: &[InlineContent]) -> String {
    inlines.iter().map(|i| i.as_plain_text()).collect()
}

/// Rebuild inlines from a heading node, stripping any anchor from text nodes.
fn rebuild_inlines_stripped<'a>(node: &'a AstNode<'a>) -> Vec<InlineContent> {
    let mut inlines = extract_inlines(node);
    // Strip anchor from last text element
    if let Some(last) = inlines.last_mut() {
        if let InlineContent::Text(t) = last {
            let (cleaned, _) = strip_anchor(t);
            *t = cleaned;
        }
    }
    inlines
}

fn extract_blockquote_inlines<'a>(node: &'a AstNode<'a>) -> Vec<InlineContent> {
    let mut inlines = Vec::new();
    for child in node.children() {
        let data = child.data.borrow();
        if matches!(data.value, NodeValue::Paragraph) {
            drop(data);
            inlines.extend(extract_inlines(child));
            inlines.push(InlineContent::LineBreak);
        }
    }
    // Remove trailing linebreak
    if matches!(inlines.last(), Some(InlineContent::LineBreak)) {
        inlines.pop();
    }
    inlines
}

fn extract_table<'a>(node: &'a AstNode<'a>) -> Table {
    let mut headers = Vec::new();
    let mut rows = Vec::new();

    for child in node.children() {
        let data = child.data.borrow();
        if let NodeValue::TableRow(header) = &data.value {
            let is_h = *header;
            drop(data);
            let mut row = Vec::new();
            for cell in child.children() {
                let cell_data = cell.data.borrow();
                if matches!(cell_data.value, NodeValue::TableCell) {
                    drop(cell_data);
                    row.push(extract_inlines(cell));
                }
            }
            if is_h {
                headers = row;
            } else {
                rows.push(row);
            }
        }
    }

    Table { headers, rows }
}

fn extract_bullet_list<'a>(node: &'a AstNode<'a>) -> Vec<Vec<InlineContent>> {
    let mut items = Vec::new();
    for child in node.children() {
        let data = child.data.borrow();
        if matches!(data.value, NodeValue::Item(_)) {
            drop(data);
            for inner in child.children() {
                let inner_data = inner.data.borrow();
                if matches!(inner_data.value, NodeValue::Paragraph) {
                    drop(inner_data);
                    items.push(extract_inlines(inner));
                }
            }
        }
    }
    items
}
