use comrak::nodes::{AstNode, NodeValue};
use regex::Regex;
use std::sync::LazyLock;

use super::anchors::strip_anchor;
use crate::error::{Diagnostic, SourcePos};
use crate::model::*;

static ADDENDUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^addendum(?:\s+\d+)?(?:\s*[-–—]\s*(.*))?$").unwrap());

static RECITALS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(recitals|background)$").unwrap());

/// Return type for `extract_body`: (recitals, body_heading, body, addenda, diagnostics).
type ExtractBodyResult = (
    Option<Recitals>,
    Option<String>,
    Vec<BodyElement>,
    Vec<Addendum>,
    Vec<Diagnostic>,
);

/// 1-based position in the source file for a comrak node, given the number of
/// lines consumed before the body (front-matter and delimiters).
fn node_pos<'a>(node: &'a AstNode<'a>, line_offset: usize) -> Option<SourcePos> {
    let start = node.data.borrow().sourcepos.start;
    if start.line == 0 {
        None
    } else {
        Some(SourcePos {
            line: start.line + line_offset,
            column: start.column.max(1),
        })
    }
}

/// 1-based source column of a list item's marker.
fn item_marker_col<'a>(item: &'a AstNode<'a>) -> usize {
    item.data.borrow().sourcepos.start.column.max(1)
}

/// 1-based source column at which a list item's content begins.
///
/// This is a property of the marker **as written in the source**, not of the
/// nesting level: comrak's `padding` counts the marker literal plus the spaces
/// after it, so `1. ` gives 3, `10. ` gives 4 and `100. ` gives 5. Continuation
/// content must reach this column to stay inside the clause (spec 3.4).
fn item_content_col<'a>(item: &'a AstNode<'a>) -> usize {
    let data = item.data.borrow();
    let padding = match &data.value {
        NodeValue::Item(list) => list.padding,
        _ => 0,
    };
    data.sourcepos.start.column.max(1) + padding
}

/// Number of leading spaces corresponding to a 1-based source column.
fn col_to_indent(col: usize) -> usize {
    col.saturating_sub(1)
}

fn last_item<'a>(list_node: &'a AstNode<'a>) -> Option<&'a AstNode<'a>> {
    list_node
        .children()
        .filter(|n| matches!(n.data.borrow().value, NodeValue::Item(_)))
        .last()
}

fn last_ordered_sublist<'a>(item: &'a AstNode<'a>) -> Option<&'a AstNode<'a>> {
    item.children()
        .filter(|c| {
            matches!(&c.data.borrow().value,
                NodeValue::List(l) if l.list_type == comrak::nodes::ListType::Ordered)
        })
        .last()
}

/// The chain of list items a trailing nested list leaves "open": the list's
/// last item, then that item's own trailing nested list's last item, and so on.
/// Continuation content that under-indents falls out of one of these and lands
/// on an ancestor. Ordered shallowest-first.
fn open_item_chain<'a>(list_node: &'a AstNode<'a>) -> Vec<&'a AstNode<'a>> {
    let mut chain = Vec::new();
    let mut current = last_item(list_node);
    while let Some(item) = current {
        chain.push(item);
        current = last_ordered_sublist(item).and_then(last_item);
    }
    chain
}

/// Short label for a list item, for diagnostics emitted before numbering.
fn item_label<'a>(item: &'a AstNode<'a>) -> String {
    for child in item.children() {
        let is_para = matches!(child.data.borrow().value, NodeValue::Paragraph);
        if !is_para {
            continue;
        }
        let text = collect_plain_text(child);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            let snippet: String = trimmed.chars().take(40).collect();
            return format!("clause starting '{}…'", snippet);
        }
    }
    "the preceding clause".to_string()
}

/// Warn when a content block is indented past its clause's content column but
/// not far enough to reach the clause it was evidently written for. The block
/// still renders, one or more levels too shallow, under the wrong clause —
/// nothing is missing from the output, so proofreading will not catch it
/// (spec 3.4.1, failure mode 2).
fn check_reattached<'a>(
    block: &'a AstNode<'a>,
    prev_list: Option<&'a AstNode<'a>>,
    location: String,
    diagnostics: &mut Vec<Diagnostic>,
    line_offset: usize,
) {
    let Some(list) = prev_list else { return };
    let col = block.data.borrow().sourcepos.start.column.max(1);

    // The deepest open item the block reaches past the marker of, but falls
    // short of the content column of — that is the clause it was aimed at.
    let chain = open_item_chain(list);
    let Some(target) = chain
        .iter()
        .rev()
        .find(|item| item_marker_col(item) < col && col < item_content_col(item))
    else {
        return;
    };

    diagnostics.push(
        Diagnostic::warning(
            "continuation-reattached",
            format!(
                "Continuation content is indented {} spaces, short of the {} needed to stay inside {}. It will render under {} instead — one level too shallow.",
                col_to_indent(col),
                col_to_indent(item_content_col(target)),
                item_label(target),
                location,
            ),
        )
        .at(location)
        .at_pos(node_pos(block, line_offset)),
    );
}

/// Error on an indented code block inside the clause hierarchy. Lexicon has no
/// use for indented code in a contract, so one is always mis-indented
/// continuation content: it has fallen 4 or more spaces short of its clause's
/// content column and CommonMark has reinterpreted it as code, which means it
/// is dropped from the output entirely (spec 3.4.1, failure mode 1).
fn report_indented_code<'a>(
    block: &'a AstNode<'a>,
    required_col: Option<usize>,
    location: String,
    diagnostics: &mut Vec<Diagnostic>,
    line_offset: usize,
) {
    let fix = match required_col {
        Some(col) => format!(" Indent it to {} spaces.", col_to_indent(col)),
        None => String::new(),
    };
    diagnostics.push(
        Diagnostic::error(
            "continuation-indent",
            format!(
                "Content is indented too far below its clause's content column and has been parsed as an indented code block. It will be dropped from the output.{}",
                fix
            ),
        )
        .at(location)
        .at_pos(node_pos(block, line_offset)),
    );
}

/// Catch-all for block-level nodes the parser has no representation for. These
/// would otherwise fall through silently and vanish from the output.
fn report_unsupported_block<'a>(
    block: &'a AstNode<'a>,
    location: String,
    diagnostics: &mut Vec<Diagnostic>,
    line_offset: usize,
) {
    let kind = match &block.data.borrow().value {
        NodeValue::CodeBlock(_) => "fenced code block",
        NodeValue::HtmlBlock(_) => "raw HTML block",
        NodeValue::ThematicBreak => "thematic break",
        NodeValue::FootnoteDefinition(_) => "footnote definition",
        NodeValue::DescriptionList => "description list",
        _ => return,
    };
    diagnostics.push(
        Diagnostic::warning(
            "unsupported-block",
            format!(
                "A {} is not part of the Lexicon format and will not appear in the output.",
                kind
            ),
        )
        .at(location)
        .at_pos(node_pos(block, line_offset)),
    );
}

/// Walk a comrak AST and extract the document body as a list of BodyElements.
/// `root` should be the Document node from comrak. `line_offset` is the number
/// of source lines preceding the body (used to map node positions back to the
/// input file).
pub fn extract_body<'a>(root: &'a AstNode<'a>, line_offset: usize) -> ExtractBodyResult {
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
                let raw_heading = collect_plain_text(child);
                let (heading_text, heading_anchor) = strip_anchor(&raw_heading);

                if RECITALS_RE.is_match(&heading_text) {
                    if recitals.is_some() {
                        diagnostics.push(
                            Diagnostic::warning(
                                "duplicate-recitals",
                                "Duplicate recitals/background heading. Only one recitals section is allowed.",
                            )
                            .at("document body")
                            .at_pos(node_pos(child, line_offset)),
                        );
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
                    let title = caps
                        .get(1)
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();
                    in_addendum = Some(Addendum {
                        number: addendum_counter,
                        title,
                        anchor: heading_anchor.clone(),
                        source_pos: node_pos(child, line_offset),
                        content: Vec::new(),
                    });
                } else if in_recitals {
                    // Non-recitals, non-addendum heading after recitals = body heading
                    in_recitals = false;
                    body_heading = Some(heading_text);
                } else if recitals.is_none() {
                    // No recitals in document — unrecognised heading (existing behaviour)
                    diagnostics.push(
                        Diagnostic::warning(
                            "unknown-top-heading",
                            format!(
                                "Unrecognised top-level heading '# {}'. Top-level headings must be 'RECITALS', 'BACKGROUND', or begin with 'ADDENDUM'.",
                                heading_text
                            ),
                        )
                        .at("document body")
                        .at_pos(node_pos(child, line_offset)),
                    );
                } else {
                    // Recitals already ended, unexpected extra heading
                    diagnostics.push(
                        Diagnostic::warning(
                            "heading-after-body",
                            format!(
                                "Unexpected top-level heading '# {}' after body section.",
                                heading_text
                            ),
                        )
                        .at("document body")
                        .at_pos(node_pos(child, line_offset)),
                    );
                }
            }

            // Ordered list at top level = clause structure (or simple numbered list in addenda)
            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Ordered => {
                if let Some(ref mut add) = in_addendum {
                    if is_clause_list(child) {
                        let clauses = extract_clauses_from_list(
                            child,
                            ClauseLevel::TopLevel,
                            &mut diagnostics,
                            line_offset,
                        );
                        add.content.push(AddendumContent::ClauseList(clauses));
                    } else {
                        let items = extract_bullet_list(child);
                        add.content.push(AddendumContent::NumberedList(items));
                    }
                } else if in_recitals {
                    if let Some(ref mut rec) = recitals {
                        let clauses = extract_clauses_from_list(
                            child,
                            ClauseLevel::TopLevel,
                            &mut diagnostics,
                            line_offset,
                        );
                        for clause in clauses {
                            rec.body.push(BodyElement::Clause(clause));
                        }
                    }
                } else {
                    let clauses = extract_clauses_from_list(
                        child,
                        ClauseLevel::TopLevel,
                        &mut diagnostics,
                        line_offset,
                    );
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

            // Bullet lists — permitted as ordinary content in prose sections and
            // addendum content (spec 3.10). Outside those, they fall through the
            // clause hierarchy at the source-implied indentation level, without
            // a clause number, and produce a warning.
            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Bullet => {
                let items = extract_bullet_list(child);
                if let Some(ref mut add) = in_addendum {
                    add.content.push(AddendumContent::BulletList(items));
                } else if in_recitals {
                    if let Some(ref mut rec) = recitals {
                        diagnostics.push(
                            Diagnostic::warning(
                                "bullet-outside-clause",
                                "Bullet point in recitals — bullets are not part of the structured outline and will not be numbered.",
                            )
                            .at("recitals")
                            .at_pos(node_pos(child, line_offset)),
                        );
                        rec.body.push(BodyElement::BulletList(items));
                    }
                } else {
                    diagnostics.push(
                        Diagnostic::warning(
                            "bullet-outside-clause",
                            "Bullet point at top level of document body — bullets are not part of the structured outline and will not be numbered.",
                        )
                        .at("document body")
                        .at_pos(node_pos(child, line_offset)),
                    );
                    body.push(BodyElement::BulletList(items));
                }
            }

            // Indented code block outside any clause — same mis-indentation
            // hazard as inside one, and equally silent without this.
            NodeValue::CodeBlock(cb) if !cb.fenced => {
                drop(data);
                let location = if in_addendum.is_some() {
                    "addendum content"
                } else if in_recitals {
                    "recitals"
                } else {
                    "document body"
                };
                report_indented_code(
                    child,
                    None,
                    location.to_string(),
                    &mut diagnostics,
                    line_offset,
                );
            }

            _ => {
                drop(data);
                let location = if in_addendum.is_some() {
                    "addendum content"
                } else if in_recitals {
                    "recitals"
                } else {
                    "document body"
                };
                report_unsupported_block(
                    child,
                    location.to_string(),
                    &mut diagnostics,
                    line_offset,
                );
            }
        }
    }

    // Save last addendum
    if let Some(add) = in_addendum {
        addenda.push(add);
    }

    // Warn if recitals present but no body heading
    if recitals.is_some() && body_heading.is_none() {
        diagnostics.push(
            Diagnostic::warning(
                "missing-body-heading",
                "Recitals section present but no body heading found. Add a top-level heading (e.g. '# Operative Provisions') before the contract clauses.",
            )
            .at("document body"),
        );
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
                NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Ordered => {
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
    diagnostics: &mut Vec<Diagnostic>,
    line_offset: usize,
) -> Vec<Clause> {
    let mut clauses = Vec::new();

    for item in list_node.children() {
        let item_data = item.data.borrow();
        let NodeValue::Item(list) = &item_data.value else {
            continue;
        };
        let ordinal = list.start;
        drop(item_data);

        // A marker of `10.` or wider widens the clause's content column, which
        // is what makes under-indented continuation content possible at all.
        // The processor renumbers every item on render, so the typed ordinal
        // is discarded — writing `1.` costs nothing and removes the hazard.
        if ordinal >= 10 {
            diagnostics.push(
                Diagnostic::info(
                    "hand-numbered-ordinal",
                    format!(
                        "Source ordinal '{}.' is {} characters wide, which shifts this clause's content column to {} spaces. Write '1.' instead — the rendered numbering is unaffected.",
                        ordinal,
                        ordinal.to_string().len() + 1,
                        col_to_indent(item_content_col(item)),
                    ),
                )
                .at_pos(node_pos(item, line_offset)),
            );
        }

        let clause = extract_clause_from_item(item, level, diagnostics, line_offset);
        clauses.push(clause);
    }

    clauses
}

/// Extract a single Clause from a list Item node.
fn extract_clause_from_item<'a>(
    item: &'a AstNode<'a>,
    level: ClauseLevel,
    diagnostics: &mut Vec<Diagnostic>,
    line_offset: usize,
) -> Clause {
    let mut heading = None;
    let mut anchor = None;
    let mut body: Vec<ClauseBody> = Vec::new();
    // Most recent nested ordered list, used to work out which clause a
    // mis-indented continuation block was written for.
    let mut prev_list: Option<&'a AstNode<'a>> = None;

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
                if let Some(InlineContent::Text(t)) = inlines.last() {
                    let (cleaned, para_anchor) = strip_anchor(t);
                    if para_anchor.is_some() {
                        if let Some(ref previous) = anchor {
                            diagnostics.push(
                                Diagnostic::warning(
                                    "multiple-anchors",
                                    format!(
                                        "Clause declares more than one anchor; '#{}' is dropped in favour of '#{}'. Cross-references to the dropped anchor will not resolve — move it to its own sub-clause",
                                        previous,
                                        para_anchor.as_deref().unwrap_or_default()
                                    ),
                                )
                                .at_pos(node_pos(child, line_offset)),
                            );
                        }
                        anchor = para_anchor;
                        if cleaned.is_empty() {
                            inlines.pop();
                        } else {
                            let len = inlines.len();
                            inlines[len - 1] = InlineContent::Text(cleaned);
                        }
                    }
                }

                if !inlines.is_empty() {
                    check_reattached(
                        child,
                        prev_list,
                        clause_location_hint(&heading, &body),
                        diagnostics,
                        line_offset,
                    );
                    body.push(ClauseBody::Content(ClauseContent::Paragraph(inlines)));
                }
            }

            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Ordered => {
                drop(data);
                let child_level = next_level(level);
                let child_clauses =
                    extract_clauses_from_list(child, child_level, diagnostics, line_offset);
                body.push(ClauseBody::Children(child_clauses));
                prev_list = Some(child);
            }

            // Bullet list nested inside a clause body — captured with no
            // clause number, at the source-implied indentation level.
            NodeValue::List(list) if list.list_type == comrak::nodes::ListType::Bullet => {
                drop(data);
                let items = extract_bullet_list(child);
                let location = clause_location_hint(&heading, &body);
                diagnostics.push(
                    Diagnostic::warning(
                        "bullet-outside-clause",
                        "Bullet point inside clause body — bullets are not part of the structured outline, will not be numbered, and cannot be cross-referenced.",
                    )
                    .at(location)
                    .at_pos(node_pos(child, line_offset)),
                );
                body.push(ClauseBody::Content(ClauseContent::BulletList(items)));
            }

            NodeValue::BlockQuote => {
                drop(data);
                check_reattached(
                    child,
                    prev_list,
                    clause_location_hint(&heading, &body),
                    diagnostics,
                    line_offset,
                );
                let inlines = extract_blockquote_inlines(child);
                body.push(ClauseBody::Content(ClauseContent::Blockquote(inlines)));
            }

            NodeValue::Table(_) => {
                drop(data);
                check_reattached(
                    child,
                    prev_list,
                    clause_location_hint(&heading, &body),
                    diagnostics,
                    line_offset,
                );
                let table = extract_table(child);
                body.push(ClauseBody::Content(ClauseContent::Table(table)));
            }

            // An indented code block in the clause hierarchy is always
            // mis-indented continuation content, never intentional code.
            NodeValue::CodeBlock(cb) if !cb.fenced => {
                drop(data);
                let required = prev_list
                    .and_then(|l| open_item_chain(l).last().map(|i| item_content_col(i)))
                    .unwrap_or_else(|| item_content_col(item));
                report_indented_code(
                    child,
                    Some(required),
                    clause_location_hint(&heading, &body),
                    diagnostics,
                    line_offset,
                );
            }

            _ => {
                drop(data);
                report_unsupported_block(
                    child,
                    clause_location_hint(&heading, &body),
                    diagnostics,
                    line_offset,
                );
            }
        }
    }

    Clause {
        level,
        heading,
        anchor,
        number: None,
        source_pos: node_pos(item, line_offset),
        body,
    }
}

/// Best-effort location hint for diagnostics emitted before clause numbering.
/// Uses the clause heading if present, otherwise a snippet of the first
/// paragraph, otherwise a generic "clause body" label.
fn clause_location_hint(heading: &Option<ClauseHeading>, body: &[ClauseBody]) -> String {
    if let Some(h) = heading {
        let text = inlines_to_plain_text(&h.text);
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return format!("clause '{}'", trimmed);
        }
    }
    for element in body {
        if let ClauseBody::Content(ClauseContent::Paragraph(inlines)) = element {
            let text = inlines_to_plain_text(inlines);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let snippet: String = trimmed.chars().take(40).collect();
                return format!("clause starting '{}…'", snippet);
            }
        }
    }
    "clause body".to_string()
}

fn next_level(level: ClauseLevel) -> ClauseLevel {
    match level {
        ClauseLevel::TopLevel => ClauseLevel::Clause,
        ClauseLevel::Clause => ClauseLevel::SubClause,
        ClauseLevel::SubClause => ClauseLevel::SubSubClause,
        ClauseLevel::SubSubClause => ClauseLevel::Paragraph,
        ClauseLevel::Paragraph => ClauseLevel::SubParagraph,
        ClauseLevel::SubParagraph => ClauseLevel::SubParagraph, // cap at this level
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
                if let Some(anchor_id_str) = link_url.strip_prefix('#') {
                    inlines.push(InlineContent::CrossRef {
                        display,
                        anchor_id: anchor_id_str.to_string(),
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
    if let Some(InlineContent::Text(t)) = inlines.last_mut() {
        let (cleaned, _) = strip_anchor(t);
        *t = cleaned;
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
