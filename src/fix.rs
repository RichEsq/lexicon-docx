//! Source rewrites for lint rules that can be fixed automatically.
//!
//! Currently one fix: normalising hand-numbered ordinals. A marker of `10.` or
//! wider widens the clause's content column (spec 3.4), which is what makes
//! under-indented continuation content possible at all. Because the processor
//! renumbers every item on render, the typed ordinal carries no information,
//! so rewriting it to `1.` is free — provided the item's content is dedented by
//! the same amount the marker shrinks, so that every column relationship in the
//! source is preserved exactly.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options, parse_document};

use crate::frontmatter;
use crate::model::*;

/// One ordinal that needs rewriting, in source-line terms.
struct Target {
    /// 1-based line of the marker.
    line: usize,
    /// 0-based byte offset of the first digit within that line.
    offset: usize,
    /// Number of digits to replace with a single `1`.
    digits: usize,
    /// 1-based last line of the item, inclusive.
    end_line: usize,
}

/// Rewrite the first hand-numbered ordinal found in the clause hierarchy.
/// Returns `None` when there is nothing left to fix.
fn next_target(body: &str) -> Option<Target> {
    let arena = Arena::new();
    let mut opts = Options::default();
    opts.extension.table = true;
    opts.extension.superscript = true;
    let root = parse_document(&arena, body, &opts);

    fn walk<'a>(node: &'a AstNode<'a>) -> Option<Target> {
        for child in node.children() {
            let data = child.data.borrow();
            if let NodeValue::Item(list) = &data.value
                && list.start >= 10
            {
                let sp = data.sourcepos;
                return Some(Target {
                    line: sp.start.line,
                    offset: sp.start.column.max(1) - 1,
                    digits: list.start.to_string().len(),
                    end_line: sp.end.line.max(sp.start.line),
                });
            }
            drop(data);
            if let Some(found) = walk(child) {
                return Some(found);
            }
        }
        None
    }

    walk(root)
}

/// Apply one target: shrink the marker to `1.` and dedent the item's remaining
/// lines by the width the marker lost, so nested markers and continuation
/// content keep their position relative to the new content column.
fn apply(lines: &mut [String], target: &Target) -> bool {
    let delta = target.digits - 1;
    let idx = target.line - 1;
    if idx >= lines.len() {
        return false;
    }

    // The marker line must actually carry the digits we expect.
    let marker_line = &lines[idx];
    let end = target.offset + target.digits;
    if end > marker_line.len()
        || !marker_line[target.offset..end]
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        return false;
    }

    let mut rewritten = String::with_capacity(marker_line.len());
    rewritten.push_str(&marker_line[..target.offset]);
    rewritten.push('1');
    rewritten.push_str(&marker_line[end..]);
    lines[idx] = rewritten;

    // Dedent the rest of the item. Blank lines and lazy continuation lines
    // (fewer than `delta` leading spaces) are left alone — removing what is
    // not there would corrupt them.
    let last = target.end_line.min(lines.len());
    for line in lines.iter_mut().take(last).skip(idx + 1) {
        if line.trim().is_empty() {
            continue;
        }
        let leading = line.len() - line.trim_start_matches(' ').len();
        if leading >= delta {
            line.drain(..delta);
        }
    }

    true
}

/// A structural fingerprint of a parsed document: every piece of rendered text,
/// in order, tagged with the clause depth it sits at. Two sources with the same
/// fingerprint produce the same output, so comparing before and after proves a
/// rewrite changed nothing but the source ordinals.
fn fingerprint(doc: &Document) -> Vec<String> {
    let mut out = Vec::new();

    fn inlines(items: &[InlineContent]) -> String {
        items
            .iter()
            .map(|i| match i {
                InlineContent::Text(t) => t.clone(),
                InlineContent::Bold(t) => t.clone(),
                InlineContent::Italic(t) => t.clone(),
                InlineContent::Superscript(t) => t.clone(),
                InlineContent::CrossRef { display, .. } => display.clone(),
                _ => String::new(),
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn content(c: &ClauseContent, depth: usize, out: &mut Vec<String>) {
        match c {
            ClauseContent::Paragraph(i) => out.push(format!("{depth}:p:{}", inlines(i))),
            ClauseContent::Blockquote(i) => out.push(format!("{depth}:q:{}", inlines(i))),
            ClauseContent::BulletList(items) => {
                for item in items {
                    out.push(format!("{depth}:b:{}", inlines(item)));
                }
            }
            ClauseContent::Table(t) => out.push(format!("{depth}:t:{}", t.rows.len())),
        }
    }

    fn clause(c: &Clause, depth: usize, out: &mut Vec<String>) {
        if let Some(h) = &c.heading {
            out.push(format!("{depth}:h:{}", inlines(&h.text)));
        }
        if let Some(a) = &c.anchor {
            out.push(format!("{depth}:a:{a}"));
        }
        for element in &c.body {
            match element {
                ClauseBody::Content(x) => content(x, depth, out),
                ClauseBody::Children(kids) => {
                    for kid in kids {
                        clause(kid, depth + 1, out);
                    }
                }
            }
        }
    }

    fn body_element(e: &BodyElement, out: &mut Vec<String>) {
        match e {
            BodyElement::Clause(c) => clause(c, 0, out),
            BodyElement::Prose(i) => out.push(format!("0:prose:{}", inlines(i))),
            BodyElement::BulletList(items) => {
                for item in items {
                    out.push(format!("0:b:{}", inlines(item)));
                }
            }
        }
    }

    if let Some(rec) = &doc.recitals {
        out.push(format!("recitals:{}", rec.heading));
        for e in &rec.body {
            body_element(e, &mut out);
        }
    }
    for e in &doc.body {
        body_element(e, &mut out);
    }
    for add in &doc.addenda {
        out.push(format!("addendum:{}", add.title));
        for c in &add.content {
            match c {
                AddendumContent::Paragraph(i) => out.push(format!("add:p:{}", inlines(i))),
                AddendumContent::Heading(l, i) => out.push(format!("add:h{l}:{}", inlines(i))),
                AddendumContent::ClauseList(cs) => {
                    for c in cs {
                        clause(c, 0, &mut out);
                    }
                }
                AddendumContent::NumberedList(items) | AddendumContent::BulletList(items) => {
                    for item in items {
                        out.push(format!("add:l:{}", inlines(item)));
                    }
                }
                AddendumContent::Table(t) => out.push(format!("add:t:{}", t.rows.len())),
            }
        }
    }

    out
}

/// Outcome of an autofix run.
pub enum FixOutcome {
    /// Nothing to fix.
    Unchanged,
    /// Rewritten source, and how many ordinals were normalised.
    Fixed(String, usize),
    /// The rewrite would have changed the document's structure, so it was
    /// abandoned. The source is left untouched.
    Unsafe,
}

/// Normalise every hand-numbered ordinal in the clause hierarchy to `1.`.
///
/// Fixes one ordinal per pass and re-parses between passes, so each pass works
/// from positions that are known to be current. The result is verified against
/// the original by structural fingerprint before being returned: if the rewrite
/// would move, drop or re-level any content, it is discarded rather than
/// written out.
pub fn normalise_ordinals(input: &str) -> FixOutcome {
    let Ok(before) = crate::parse(input) else {
        return FixOutcome::Unchanged;
    };
    let before = fingerprint(&before);

    let Ok(parsed) = frontmatter::parse_frontmatter(input) else {
        return FixOutcome::Unchanged;
    };
    let offset = parsed.body_line_offset;

    let mut lines: Vec<String> = input.split('\n').map(|s| s.to_string()).collect();
    let mut fixed = 0usize;

    loop {
        let body = lines[offset.min(lines.len())..].join("\n");
        let Some(mut target) = next_target(&body) else {
            break;
        };
        // Map body-relative lines back onto the whole file.
        target.line += offset;
        target.end_line += offset;

        if !apply(&mut lines, &target) {
            break;
        }
        fixed += 1;

        // Guard against a pathological loop if a rewrite fails to reduce the
        // number of wide ordinals.
        if fixed > 10_000 {
            return FixOutcome::Unsafe;
        }
    }

    if fixed == 0 {
        return FixOutcome::Unchanged;
    }

    let output = lines.join("\n");
    match crate::parse(&output) {
        Ok(after) if fingerprint(&after) == before => FixOutcome::Fixed(output, fixed),
        _ => FixOutcome::Unsafe,
    }
}
