use std::collections::HashMap;

use regex::Regex;
use std::sync::LazyLock;

use crate::error::{DiagLevel, Diagnostic};
use crate::model::*;

static FORMAL_DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*means[\s:,]").unwrap()
});

static FORMAL_DEF_ALT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(has the meaning|shall have the same meaning|have the same meaning)").unwrap()
});

pub fn resolve(doc: &mut Document) {
    assign_clause_numbers(&mut doc.body);

    // Build anchor → clause number map
    let anchor_map = build_anchor_map(&doc.body);

    // Resolve cross-references and validate
    resolve_cross_refs(&mut doc.body, &anchor_map, &mut doc.diagnostics);
    for addendum in &mut doc.addenda {
        resolve_addendum_cross_refs(addendum, &anchor_map, &mut doc.diagnostics);
    }

    // Collect schedule items
    collect_schedule_items(doc);

    // Validate defined terms
    validate_defined_terms(doc);
}

// --- Clause numbering ---

fn assign_clause_numbers(body: &mut [BodyElement]) {
    let mut top_counter = 0u32;
    for element in body.iter_mut() {
        if let BodyElement::Clause(clause) = element {
            top_counter += 1;
            clause.number = Some(ClauseNumber::TopLevel(top_counter));
            assign_children_numbers(clause, top_counter);
        }
    }
}

fn assign_children_numbers(parent: &mut Clause, top: u32) {
    let parent_number = parent.number.clone();

    for (i, child) in parent.children.iter_mut().enumerate() {
        let number = match child.level {
            ClauseLevel::TopLevel => ClauseNumber::TopLevel(i as u32 + 1),
            ClauseLevel::Clause => ClauseNumber::Clause(top, i as u32 + 1),
            ClauseLevel::SubClause => {
                let clause_num = match &parent_number {
                    Some(ClauseNumber::Clause(_, c)) => *c,
                    _ => 0,
                };
                let letter = (b'a' + i as u8) as char;
                ClauseNumber::SubClause(top, clause_num, letter)
            }
            ClauseLevel::SubSubClause => {
                let (clause_num, letter) = match &parent_number {
                    Some(ClauseNumber::SubClause(_, c, l)) => (*c, *l),
                    _ => (0, 'a'),
                };
                let roman = to_roman(i as u32 + 1);
                ClauseNumber::SubSubClause(top, clause_num, letter, roman)
            }
        };
        child.number = Some(number);
        assign_children_numbers(child, top);
    }
}

// --- Anchor map ---

fn build_anchor_map(body: &[BodyElement]) -> HashMap<String, ClauseNumber> {
    let mut map = HashMap::new();
    for element in body {
        if let BodyElement::Clause(clause) = element {
            collect_anchors(clause, &mut map);
        }
    }
    map
}

fn collect_anchors(clause: &Clause, map: &mut HashMap<String, ClauseNumber>) {
    if let (Some(anchor), Some(number)) = (&clause.anchor, &clause.number) {
        map.insert(anchor.clone(), number.clone());
    }
    for child in &clause.children {
        collect_anchors(child, map);
    }
}

// --- Cross-reference resolution ---

fn resolve_cross_refs(
    body: &mut [BodyElement],
    anchor_map: &HashMap<String, ClauseNumber>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for element in body.iter_mut() {
        match element {
            BodyElement::Clause(clause) => {
                resolve_clause_cross_refs(clause, anchor_map, diagnostics);
            }
            BodyElement::Prose(inlines) => {
                resolve_inlines_cross_refs(inlines, anchor_map, diagnostics, None);
            }
        }
    }
}

fn resolve_clause_cross_refs(
    clause: &mut Clause,
    anchor_map: &HashMap<String, ClauseNumber>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let clause_loc = clause.number.as_ref().map(|n| n.full_reference());

    for content in &mut clause.content {
        match content {
            ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                resolve_inlines_cross_refs(inlines, anchor_map, diagnostics, clause_loc.as_deref());
            }
            _ => {}
        }
    }
    if let Some(ref mut heading) = clause.heading {
        resolve_inlines_cross_refs(
            &mut heading.text,
            anchor_map,
            diagnostics,
            clause_loc.as_deref(),
        );
    }
    for child in &mut clause.children {
        resolve_clause_cross_refs(child, anchor_map, diagnostics);
    }
}

fn resolve_inlines_cross_refs(
    inlines: &mut [InlineContent],
    anchor_map: &HashMap<String, ClauseNumber>,
    diagnostics: &mut Vec<Diagnostic>,
    location: Option<&str>,
) {
    for inline in inlines.iter_mut() {
        if let InlineContent::CrossRef {
            anchor_id,
            resolved,
            display,
        } = inline
        {
            if let Some(number) = anchor_map.get(anchor_id.as_str()) {
                *resolved = Some(number.full_reference());
            } else {
                diagnostics.push(Diagnostic {
                    level: DiagLevel::Warning,
                    message: format!(
                        "Cross-reference '{}' (#{}) points to non-existent anchor",
                        display, anchor_id
                    ),
                    location: location.map(String::from),
                });
            }
        }
    }
}

fn resolve_addendum_cross_refs(
    addendum: &mut Addendum,
    anchor_map: &HashMap<String, ClauseNumber>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let loc = addendum.heading();
    for content in &mut addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) => {
                resolve_inlines_cross_refs(
                    inlines,
                    anchor_map,
                    diagnostics,
                    Some(&loc),
                );
            }
            AddendumContent::Heading(_, inlines) => {
                resolve_inlines_cross_refs(
                    inlines,
                    anchor_map,
                    diagnostics,
                    Some(&loc),
                );
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    resolve_clause_cross_refs(clause, anchor_map, diagnostics);
                }
            }
            AddendumContent::NumberedList(items)
            | AddendumContent::BulletList(items) => {
                for item_inlines in items {
                    resolve_inlines_cross_refs(
                        item_inlines,
                        anchor_map,
                        diagnostics,
                        Some(&loc),
                    );
                }
            }
            _ => {}
        }
    }
}

// --- Schedule item collection ---

fn collect_schedule_items(doc: &mut Document) {
    let mut items = Vec::new();

    for element in &doc.body {
        match element {
            BodyElement::Clause(clause) => {
                collect_clause_schedule_items(clause, &mut items);
            }
            BodyElement::Prose(inlines) => {
                collect_inline_schedule_items(inlines, &mut items);
            }
        }
    }

    for addendum in &doc.addenda {
        collect_addendum_schedule_items(addendum, &mut items);
    }

    doc.schedule_items = items;
}

fn collect_clause_schedule_items(clause: &Clause, items: &mut Vec<ScheduleItem>) {
    for content in &clause.content {
        match content {
            ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                collect_inline_schedule_items(inlines, items);
            }
            _ => {}
        }
    }
    for child in &clause.children {
        collect_clause_schedule_items(child, items);
    }
}

fn collect_inline_schedule_items(inlines: &[InlineContent], items: &mut Vec<ScheduleItem>) {
    for inline in inlines {
        if let InlineContent::ScheduleRef {
            display,
            resolved_value,
            ..
        } = inline
        {
            items.push(ScheduleItem {
                description: display.clone(),
                value: resolved_value.clone(),
            });
        }
    }
}

fn collect_addendum_schedule_items(addendum: &Addendum, items: &mut Vec<ScheduleItem>) {
    for content in &addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) | AddendumContent::Heading(_, inlines) => {
                collect_inline_schedule_items(inlines, items);
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    collect_clause_schedule_items(clause, items);
                }
            }
            AddendumContent::NumberedList(items_list)
            | AddendumContent::BulletList(items_list) => {
                for item_inlines in items_list {
                    collect_inline_schedule_items(item_inlines, items);
                }
            }
            _ => {}
        }
    }
}

// --- Defined term validation ---
// Bold text marks definition sites only. References are plain text.
// We collect definitions from bold, then scan all text for usage.

#[derive(Debug)]
struct TermDefinition {
    term: String,
    location: Option<String>,
}

#[derive(Debug, PartialEq)]
enum TermKind {
    FormalDefinition,   // **Term** means ...
    InlineDefinition,   // ("**Term**")
    FieldLabel,         // **Label**: structural label, not a term
}

fn validate_defined_terms(doc: &mut Document) {
    let mut definitions: Vec<TermDefinition> = Vec::new();

    // Party roles are automatic definitions
    for party in &doc.meta.parties {
        definitions.push(TermDefinition {
            term: party.role.clone(),
            location: Some("front-matter".to_string()),
        });
    }

    // Short title is an automatic definition
    let short_title = doc.meta.short_title.as_deref().unwrap_or("Agreement");
    definitions.push(TermDefinition {
        term: short_title.to_string(),
        location: Some("front-matter".to_string()),
    });

    // Collect all bold terms that are definitions (not field labels)
    for element in &doc.body {
        match element {
            BodyElement::Clause(clause) => {
                collect_clause_definitions(clause, &mut definitions);
            }
            BodyElement::Prose(inlines) => {
                collect_inline_definitions(inlines, &mut definitions, None);
            }
        }
    }
    for addendum in &doc.addenda {
        collect_addendum_definitions(addendum, &mut definitions);
    }

    // Build definition set (term → first location)
    let mut def_map: HashMap<String, String> = HashMap::new();
    for def in &definitions {
        let loc = def.location.clone().unwrap_or_default();
        def_map.entry(def.term.clone()).or_insert(loc);
    }

    // Collect all plain text from the document for usage scanning
    let mut all_text = String::new();
    for element in &doc.body {
        collect_element_text(element, &mut all_text);
    }
    for addendum in &doc.addenda {
        collect_addendum_text(addendum, &mut all_text);
    }
    let text_lower = all_text.to_lowercase();

    // Warn on definitions never used in the document text (with fuzzy matching)
    for (term, loc) in &def_map {
        let variants = term_variants(term);
        let is_used = variants.iter().any(|v| text_lower.contains(v));
        if !is_used {
            doc.diagnostics.push(Diagnostic {
                level: DiagLevel::Warning,
                message: format!("'{}' is defined but never used in the document", term),
                location: Some(loc.clone()),
            });
        }
    }
}

fn collect_clause_definitions(clause: &Clause, defs: &mut Vec<TermDefinition>) {
    let clause_loc = clause.number.as_ref().map(|n| n.full_reference());

    for content in &clause.content {
        match content {
            ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                collect_inline_definitions(inlines, defs, clause_loc.as_deref());
            }
            _ => {}
        }
    }
    for child in &clause.children {
        collect_clause_definitions(child, defs);
    }
}

fn collect_addendum_definitions(addendum: &Addendum, defs: &mut Vec<TermDefinition>) {
    let heading = addendum.heading();
    let loc = Some(heading.as_str());
    for content in &addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) | AddendumContent::Heading(_, inlines) => {
                collect_inline_definitions(inlines, defs, loc);
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    collect_clause_definitions(clause, defs);
                }
            }
            AddendumContent::NumberedList(items)
            | AddendumContent::BulletList(items) => {
                for item_inlines in items {
                    collect_inline_definitions(item_inlines, defs, loc);
                }
            }
            _ => {}
        }
    }
}

/// Collect bold terms that are definitions (formal or inline), skipping field labels.
fn collect_inline_definitions(
    inlines: &[InlineContent],
    defs: &mut Vec<TermDefinition>,
    location: Option<&str>,
) {
    for (i, inline) in inlines.iter().enumerate() {
        if let InlineContent::Bold(term) = inline {
            let kind = classify_term(term, inlines, i);
            match kind {
                TermKind::FormalDefinition | TermKind::InlineDefinition => {
                    defs.push(TermDefinition {
                        term: term.clone(),
                        location: location.map(String::from),
                    });
                }
                TermKind::FieldLabel => {}
            }
        }
    }
}

/// Collect all plain text from a body element for term usage scanning.
fn collect_element_text(element: &BodyElement, out: &mut String) {
    match element {
        BodyElement::Clause(clause) => collect_clause_text(clause, out),
        BodyElement::Prose(inlines) => collect_inlines_text(inlines, out),
    }
}

fn collect_clause_text(clause: &Clause, out: &mut String) {
    if let Some(ref heading) = clause.heading {
        collect_inlines_text(&heading.text, out);
    }
    for content in &clause.content {
        match content {
            ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                collect_inlines_text(inlines, out);
            }
            _ => {}
        }
    }
    for child in &clause.children {
        collect_clause_text(child, out);
    }
}

fn collect_addendum_text(addendum: &Addendum, out: &mut String) {
    for content in &addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) | AddendumContent::Heading(_, inlines) => {
                collect_inlines_text(inlines, out);
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    collect_clause_text(clause, out);
                }
            }
            AddendumContent::NumberedList(items)
            | AddendumContent::BulletList(items) => {
                for item in items {
                    collect_inlines_text(item, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_inlines_text(inlines: &[InlineContent], out: &mut String) {
    for inline in inlines {
        match inline {
            InlineContent::Text(t) => {
                out.push_str(t);
                out.push(' ');
            }
            InlineContent::Bold(t) | InlineContent::Italic(t) => {
                out.push_str(t);
                out.push(' ');
            }
            InlineContent::CrossRef { display, resolved, .. } => {
                out.push_str(resolved.as_ref().unwrap_or(display));
                out.push(' ');
            }
            InlineContent::ScheduleRef { display, .. } => {
                out.push_str(display);
                out.push(' ');
            }
            InlineContent::Link { text, .. } => {
                out.push_str(text);
                out.push(' ');
            }
            _ => {}
        }
    }
}

/// Generate multiple normalised variants for a term, for fuzzy matching.
/// Produces several candidate stems so different inflections of the same
/// base word share at least one common variant.
fn term_variants(term: &str) -> Vec<String> {
    let mut s = term.to_string();

    // Strip possessive
    if s.ends_with("'s") {
        s.truncate(s.len() - 2);
    } else if s.ends_with("s'") {
        s.truncate(s.len() - 2);
    }

    let lower = s.to_lowercase();
    let mut variants = vec![lower.clone()];

    // Apply suffix rules, each producing a variant
    let suffix_rules: &[(&str, &str)] = &[
        ("ies", "y"),       // Authorities → authority
        ("ing", ""),        // Processing → process
        ("ed", ""),         // Processed → process
        ("es", "e"),        // Affiliates → affiliate
        ("es", ""),         // Breaches → breach
        ("s", ""),          // Members → member
    ];

    for &(suffix, replacement) in suffix_rules {
        if lower.ends_with(suffix) && lower.len() > suffix.len() + 2 {
            let stem = &lower[..lower.len() - suffix.len()];
            let variant = format!("{}{}", stem, replacement);
            if variant != lower {
                variants.push(variant);
            }
        }
    }

    variants
}

/// Classify a bold term based on what follows/precedes it in the inline sequence.
/// In the source, bold marks definition sites only. This classifies the type of
/// definition, or identifies field labels (structural bold, not a term).
fn classify_term(_term: &str, inlines: &[InlineContent], index: usize) -> TermKind {
    // Check for inline definition pattern: ("**Term**") or (the "**Term**")
    if index > 0 {
        if let Some(InlineContent::Text(before)) = inlines.get(index - 1) {
            let trimmed = before.trim_end();
            if trimmed.ends_with("(\"") || trimmed.ends_with("(the \"") {
                return TermKind::InlineDefinition;
            }
            // Also match: "**Term**" (quoted without parens, used in grouped defs)
            if trimmed.ends_with('"') || trimmed.ends_with("\", \"") {
                // Check if this is part of a "shall have the same meaning" pattern
                // by scanning the rest of the inlines for that phrase
                if inlines_contain_meaning_phrase(inlines) {
                    return TermKind::FormalDefinition;
                }
            }
        }
    }

    // Check for formal definition: **Term** means ...
    if let Some(InlineContent::Text(after)) = inlines.get(index + 1) {
        if FORMAL_DEF_RE.is_match(after) || FORMAL_DEF_ALT_RE.is_match(after) {
            return TermKind::FormalDefinition;
        }
        // Check for field label pattern: **Label**: (bold followed by colon)
        // These are structural labels, not defined terms
        if after.starts_with(':') {
            return TermKind::FieldLabel;
        }
    }

    // Default: bold text in source is a definition (bold = definition sites only)
    TermKind::FormalDefinition
}

static GROUPED_DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(shall have the same meaning|have the meaning given|shall be construed)").unwrap()
});

/// Check if any text in the inline sequence contains a phrase indicating
/// a grouped definition (e.g., "shall have the same meaning as in the GDPR").
fn inlines_contain_meaning_phrase(inlines: &[InlineContent]) -> bool {
    for inline in inlines {
        if let InlineContent::Text(t) = inline {
            if GROUPED_DEF_RE.is_match(t) {
                return true;
            }
        }
    }
    false
}

// --- Roman numerals ---

fn to_roman(mut n: u32) -> String {
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
