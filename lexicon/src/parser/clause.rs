use comrak::nodes::{AstNode, NodeValue};

use crate::model::*;
use super::anchors::strip_anchor;

/// Walk a comrak AST and extract the document body as a list of BodyElements.
/// `root` should be the Document node from comrak.
pub fn extract_body<'a>(root: &'a AstNode<'a>) -> (Vec<BodyElement>, Vec<Annexure>) {
    let mut body = Vec::new();
    let mut annexures = Vec::new();
    let mut in_annexure: Option<Annexure> = None;

    for child in root.children() {
        let data = child.data.borrow();
        match &data.value {
            // Top-level heading (# ANNEX ...) marks annexure start
            NodeValue::Heading(h) if h.level == 1 => {
                // Save previous annexure if any
                if let Some(annex) = in_annexure.take() {
                    annexures.push(annex);
                }
                let heading_text = collect_plain_text(child);
                in_annexure = Some(Annexure {
                    heading: heading_text,
                    content: Vec::new(),
                });
            }

            // Ordered list at top level = clause structure
            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Ordered => {
                if let Some(ref mut annex) = in_annexure {
                    // Inside an annexure, clauses become annexure content
                    let clauses = extract_clauses_from_list(child, ClauseLevel::TopLevel);
                    annex.content.push(AnnexureContent::ClauseList(clauses));
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
                    if let Some(ref mut annex) = in_annexure {
                        annex.content.push(AnnexureContent::Paragraph(inlines));
                    } else {
                        body.push(BodyElement::Prose(inlines));
                    }
                }
            }

            // Headings inside annexures (## or ###)
            NodeValue::Heading(h) if h.level >= 2 => {
                if let Some(ref mut annex) = in_annexure {
                    let inlines = extract_inlines(child);
                    annex.content.push(AnnexureContent::Heading(h.level, inlines));
                }
            }

            // Tables
            NodeValue::Table(_) => {
                let table = extract_table(child);
                if let Some(ref mut annex) = in_annexure {
                    annex.content.push(AnnexureContent::Table(table));
                }
            }

            // Bullet lists in annexures
            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Bullet => {
                if let Some(ref mut annex) = in_annexure {
                    let items = extract_bullet_list(child);
                    annex.content.push(AnnexureContent::BulletList(items));
                }
            }

            _ => {}
        }
    }

    // Save last annexure
    if let Some(annex) = in_annexure {
        annexures.push(annex);
    }

    (body, annexures)
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
    let mut content = Vec::new();
    let mut children = Vec::new();

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
                    content.push(ClauseContent::Paragraph(inlines));
                }
            }

            NodeValue::List(list)
                if list.list_type == comrak::nodes::ListType::Ordered =>
            {
                drop(data);
                let child_level = next_level(level);
                let child_clauses = extract_clauses_from_list(child, child_level);
                children.extend(child_clauses);
            }

            NodeValue::BlockQuote => {
                drop(data);
                let inlines = extract_blockquote_inlines(child);
                content.push(ClauseContent::Blockquote(inlines));
            }

            NodeValue::Table(_) => {
                drop(data);
                let table = extract_table(child);
                content.push(ClauseContent::Table(table));
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
        content,
        children,
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
                let link_title = link.title.clone();
                drop(data);
                let display = collect_plain_text(child);
                if link_url == "#schedule" {
                    inlines.push(InlineContent::ScheduleRef {
                        display,
                        ref_id: String::new(), // comrak resolves away the ref-id
                        resolved_value: if link_title.is_empty() {
                            None
                        } else {
                            Some(link_title)
                        },
                    });
                } else if link_url.starts_with('#') {
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
                inlines.push(InlineContent::Text(inner));
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
