use std::collections::HashMap;

use regex::Regex;
use std::sync::LazyLock;

use crate::error::{DiagLevel, Diagnostic};
use crate::model::*;

static FORMAL_DEF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*means[\s:,]").unwrap());

static FORMAL_DEF_ALT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(has the meaning|shall have the same meaning|have the same meaning)")
        .unwrap()
});

pub fn resolve(doc: &mut Document) {
    // Number recitals and body clauses (both use the same scheme: 1., 1.1, (a), etc.)
    if let Some(ref mut recitals) = doc.recitals {
        assign_clause_numbers(&mut recitals.body);
    }
    assign_clause_numbers(&mut doc.body);

    // Build anchor → reference text map (from both recitals and body)
    let mut anchor_map: HashMap<String, String> = HashMap::new();
    if let Some(ref recitals) = doc.recitals {
        collect_body_anchors(&recitals.body, &mut anchor_map, "Recital");
    }
    collect_body_anchors(&doc.body, &mut anchor_map, "clause");

    // Register addendum heading anchors
    for addendum in &doc.addenda {
        if let Some(ref anchor_id) = addendum.anchor {
            anchor_map.insert(anchor_id.clone(), format!("Addendum {}", addendum.number));
        }
    }

    // Resolve cross-references and validate
    if let Some(ref mut recitals) = doc.recitals {
        resolve_cross_refs(&mut recitals.body, &anchor_map, &mut doc.diagnostics);
    }
    resolve_cross_refs(&mut doc.body, &anchor_map, &mut doc.diagnostics);
    for addendum in &mut doc.addenda {
        resolve_addendum_cross_refs(addendum, &anchor_map, &mut doc.diagnostics);
    }

    // Build schedule phrase patterns from front-matter
    let schedule_patterns = build_schedule_phrase_patterns(&doc.meta.schedule);

    // Collect schedule items and validate defined terms (single pass)
    collect_and_validate_terms(doc, &schedule_patterns);
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
    let mut i = 0usize;

    for element in &mut parent.body {
        if let ClauseBody::Children(kids) = element {
            for child in kids.iter_mut() {
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
                    ClauseLevel::Paragraph => {
                        let (clause_num, letter, roman) = match &parent_number {
                            Some(ClauseNumber::SubSubClause(_, c, l, r)) => (*c, *l, r.clone()),
                            _ => (0, 'a', "i".to_string()),
                        };
                        let upper = (b'A' + i as u8) as char;
                        ClauseNumber::Paragraph(top, clause_num, letter, roman, upper)
                    }
                    ClauseLevel::SubParagraph => {
                        let (clause_num, letter, roman, upper) = match &parent_number {
                            Some(ClauseNumber::Paragraph(_, c, l, r, u)) => (*c, *l, r.clone(), *u),
                            _ => (0, 'a', "i".to_string(), 'A'),
                        };
                        let upper_roman = to_roman(i as u32 + 1).to_uppercase();
                        ClauseNumber::SubParagraph(
                            top,
                            clause_num,
                            letter,
                            roman,
                            upper,
                            upper_roman,
                        )
                    }
                };
                child.number = Some(number);
                assign_children_numbers(child, top);
                i += 1;
            }
        }
    }
}

// --- Anchor map ---

fn collect_body_anchors(body: &[BodyElement], map: &mut HashMap<String, String>, prefix: &str) {
    for element in body {
        if let BodyElement::Clause(clause) = element {
            collect_anchors(clause, map, prefix);
        }
    }
}

fn collect_anchors(clause: &Clause, map: &mut HashMap<String, String>, prefix: &str) {
    if let (Some(anchor), Some(number)) = (&clause.anchor, &clause.number) {
        map.insert(anchor.clone(), number.full_reference(prefix));
    }
    for element in &clause.body {
        if let ClauseBody::Children(kids) = element {
            for child in kids {
                collect_anchors(child, map, prefix);
            }
        }
    }
}

// --- Cross-reference resolution ---

fn resolve_cross_refs(
    body: &mut [BodyElement],
    anchor_map: &HashMap<String, String>,
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
    anchor_map: &HashMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let clause_loc = clause.number.as_ref().map(|n| n.full_reference("clause"));

    if let Some(ref mut heading) = clause.heading {
        resolve_inlines_cross_refs(
            &mut heading.text,
            anchor_map,
            diagnostics,
            clause_loc.as_deref(),
        );
    }
    for element in &mut clause.body {
        match element {
            ClauseBody::Content(content) => match content {
                ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                    resolve_inlines_cross_refs(
                        inlines,
                        anchor_map,
                        diagnostics,
                        clause_loc.as_deref(),
                    );
                }
                _ => {}
            },
            ClauseBody::Children(kids) => {
                for child in kids {
                    resolve_clause_cross_refs(child, anchor_map, diagnostics);
                }
            }
        }
    }
}

fn resolve_inlines_cross_refs(
    inlines: &mut [InlineContent],
    anchor_map: &HashMap<String, String>,
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
            if let Some(ref_text) = anchor_map.get(anchor_id.as_str()) {
                *resolved = Some(ref_text.clone());
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
    anchor_map: &HashMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let loc = addendum.heading();
    for content in &mut addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) => {
                resolve_inlines_cross_refs(inlines, anchor_map, diagnostics, Some(&loc));
            }
            AddendumContent::Heading(_, inlines) => {
                resolve_inlines_cross_refs(inlines, anchor_map, diagnostics, Some(&loc));
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    resolve_clause_cross_refs(clause, anchor_map, diagnostics);
                }
            }
            AddendumContent::NumberedList(items) | AddendumContent::BulletList(items) => {
                for item_inlines in items {
                    resolve_inlines_cross_refs(item_inlines, anchor_map, diagnostics, Some(&loc));
                }
            }
            _ => {}
        }
    }
}

// --- Schedule phrase pattern building ---

/// Build regex patterns for matching schedule-referencing phrases in defined term text.
/// Each schedule title produces one compiled regex with all phrases as alternations.
fn build_schedule_phrase_patterns(schedules: &[ScheduleDecl]) -> Vec<(usize, Regex)> {
    let phrase_templates = [
        "given by the {title}",
        "set out in the {title}",
        "specified in the {title}",
        "described in the {title}",
        "defined in the {title}",
        "provided in the {title}",
        "contained in the {title}",
        "stated in the {title}",
        "referred to in the {title}",
        "as per the {title}",
        "in accordance with the {title}",
        "pursuant to the {title}",
        "detailed in the {title}",
    ];

    schedules
        .iter()
        .enumerate()
        .map(|(idx, sched)| {
            let escaped_title = regex::escape(&sched.title);
            let alternations: Vec<String> = phrase_templates
                .iter()
                .map(|t| t.replace("{title}", &escaped_title))
                .collect();
            let pattern = format!(r"(?i)({})", alternations.join("|"));
            (idx, Regex::new(&pattern).unwrap())
        })
        .collect()
}

/// Check if inline text following a bold term contains a schedule phrase.
/// Returns the schedule index if found.
fn check_schedule_phrase(
    inlines: &[InlineContent],
    bold_index: usize,
    patterns: &[(usize, Regex)],
) -> Option<usize> {
    if patterns.is_empty() {
        return None;
    }

    // Concatenate all text content after the bold term in this inline sequence
    let mut after_text = String::new();
    for inline in &inlines[bold_index + 1..] {
        match inline {
            InlineContent::Text(t) => after_text.push_str(t),
            InlineContent::Bold(t) | InlineContent::Italic(t) => after_text.push_str(t),
            _ => {}
        }
    }

    for (idx, pattern) in patterns {
        if pattern.is_match(&after_text) {
            return Some(*idx);
        }
    }
    None
}

// --- Defined term validation + schedule collection (merged pass) ---
// Bold text marks definition sites only. References are plain text.
// We collect definitions from bold, then scan all text for usage.
// Schedule items are identified by phrase-matching within definition text.

#[derive(Debug)]
struct TermDefinition {
    term: String,
    location: Option<String>,
}

#[derive(Debug, PartialEq)]
enum TermKind {
    FormalDefinition,          // **Term** means ...
    InlineDefinition,          // ("**Term**")
    ScheduleDefinition(usize), // **Term** has the meaning given by the Schedule
    FieldLabel,                // **Label**: structural label, not a term
}

fn collect_and_validate_terms(doc: &mut Document, schedule_patterns: &[(usize, Regex)]) {
    let mut definitions: Vec<TermDefinition> = Vec::new();
    let mut schedule_items: Vec<ScheduleItem> = Vec::new();

    // Party roles are automatic definitions
    for party in &doc.meta.parties {
        definitions.push(TermDefinition {
            term: party.role.clone(),
            location: Some("front-matter".to_string()),
        });
    }

    // Short title is an automatic definition
    let doc_type = doc.meta.doc_type.as_deref().unwrap_or("Agreement");
    definitions.push(TermDefinition {
        term: doc_type.to_string(),
        location: Some("front-matter".to_string()),
    });

    // Collect all bold terms — definitions and schedule items in one pass
    if let Some(ref recitals) = doc.recitals {
        for element in &recitals.body {
            match element {
                BodyElement::Clause(clause) => {
                    collect_clause_terms(
                        clause,
                        &mut definitions,
                        &mut schedule_items,
                        schedule_patterns,
                    );
                }
                BodyElement::Prose(inlines) => {
                    collect_inline_terms(
                        inlines,
                        &mut definitions,
                        &mut schedule_items,
                        schedule_patterns,
                        Some("recitals"),
                    );
                }
            }
        }
    }
    for element in &doc.body {
        match element {
            BodyElement::Clause(clause) => {
                collect_clause_terms(
                    clause,
                    &mut definitions,
                    &mut schedule_items,
                    schedule_patterns,
                );
            }
            BodyElement::Prose(inlines) => {
                collect_inline_terms(
                    inlines,
                    &mut definitions,
                    &mut schedule_items,
                    schedule_patterns,
                    None,
                );
            }
        }
    }
    for addendum in &doc.addenda {
        collect_addendum_terms(
            addendum,
            &mut definitions,
            &mut schedule_items,
            schedule_patterns,
        );
    }

    doc.schedule_items = schedule_items;

    // Warn on declared schedules with no referencing terms
    let mut referenced_schedules = std::collections::HashSet::new();
    for item in &doc.schedule_items {
        referenced_schedules.insert(item.schedule_index);
    }
    for (idx, sched) in doc.meta.schedule.iter().enumerate() {
        if !referenced_schedules.contains(&idx) {
            doc.diagnostics.push(Diagnostic {
                level: DiagLevel::Warning,
                message: format!(
                    "Schedule '{}' is declared but no terms reference it",
                    sched.title
                ),
                location: Some("front-matter".to_string()),
            });
        }
    }

    // Build definition set (term → first location)
    let mut def_map: HashMap<String, String> = HashMap::new();
    for def in &definitions {
        let loc = def.location.clone().unwrap_or_default();
        def_map.entry(def.term.clone()).or_insert(loc);
    }

    // Collect all plain text from the document for usage scanning
    let mut all_text = String::new();
    if let Some(ref recitals) = doc.recitals {
        for element in &recitals.body {
            collect_element_text(element, &mut all_text);
        }
    }
    for element in &doc.body {
        collect_element_text(element, &mut all_text);
    }
    for addendum in &doc.addenda {
        collect_addendum_text(addendum, &mut all_text);
    }
    let text_lower = all_text.to_lowercase();

    // Warn on definitions never used in the document text (with fuzzy matching)
    // Schedule terms are exempt — they appear in the schedule table
    let schedule_terms: std::collections::HashSet<&str> = doc
        .schedule_items
        .iter()
        .map(|si| si.term.as_str())
        .collect();

    for (term, loc) in &def_map {
        if schedule_terms.contains(term.as_str()) {
            continue;
        }
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

fn collect_clause_terms(
    clause: &Clause,
    defs: &mut Vec<TermDefinition>,
    schedule_items: &mut Vec<ScheduleItem>,
    patterns: &[(usize, Regex)],
) {
    let clause_loc = clause.number.as_ref().map(|n| n.full_reference("clause"));

    for element in &clause.body {
        match element {
            ClauseBody::Content(content) => match content {
                ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                    collect_inline_terms(
                        inlines,
                        defs,
                        schedule_items,
                        patterns,
                        clause_loc.as_deref(),
                    );
                }
                _ => {}
            },
            ClauseBody::Children(kids) => {
                for child in kids {
                    collect_clause_terms(child, defs, schedule_items, patterns);
                }
            }
        }
    }
}

fn collect_addendum_terms(
    addendum: &Addendum,
    defs: &mut Vec<TermDefinition>,
    schedule_items: &mut Vec<ScheduleItem>,
    patterns: &[(usize, Regex)],
) {
    let heading = addendum.heading();
    let loc = Some(heading.as_str());
    for content in &addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) | AddendumContent::Heading(_, inlines) => {
                collect_inline_terms(inlines, defs, schedule_items, patterns, loc);
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    collect_clause_terms(clause, defs, schedule_items, patterns);
                }
            }
            AddendumContent::NumberedList(items) | AddendumContent::BulletList(items) => {
                for item_inlines in items {
                    collect_inline_terms(item_inlines, defs, schedule_items, patterns, loc);
                }
            }
            _ => {}
        }
    }
}

/// Collect bold terms: definitions and schedule items in one pass.
fn collect_inline_terms(
    inlines: &[InlineContent],
    defs: &mut Vec<TermDefinition>,
    schedule_items: &mut Vec<ScheduleItem>,
    patterns: &[(usize, Regex)],
    location: Option<&str>,
) {
    for (i, inline) in inlines.iter().enumerate() {
        if let InlineContent::Bold(term) = inline {
            let kind = classify_term(term, inlines, i, patterns);
            match kind {
                TermKind::FormalDefinition | TermKind::InlineDefinition => {
                    defs.push(TermDefinition {
                        term: term.clone(),
                        location: location.map(String::from),
                    });
                }
                TermKind::ScheduleDefinition(schedule_idx) => {
                    defs.push(TermDefinition {
                        term: term.clone(),
                        location: location.map(String::from),
                    });
                    schedule_items.push(ScheduleItem {
                        term: term.clone(),
                        schedule_index: schedule_idx,
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
    for element in &clause.body {
        match element {
            ClauseBody::Content(content) => match content {
                ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                    collect_inlines_text(inlines, out);
                }
                _ => {}
            },
            ClauseBody::Children(kids) => {
                for child in kids {
                    collect_clause_text(child, out);
                }
            }
        }
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
            AddendumContent::NumberedList(items) | AddendumContent::BulletList(items) => {
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
            InlineContent::CrossRef {
                display, resolved, ..
            } => {
                out.push_str(resolved.as_ref().unwrap_or(display));
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
    if s.ends_with("'s") || s.ends_with("s'") {
        s.truncate(s.len() - 2);
    }

    let lower = s.to_lowercase();
    let mut variants = vec![lower.clone()];

    // Apply suffix rules, each producing a variant
    let suffix_rules: &[(&str, &str)] = &[
        ("ies", "y"), // Authorities → authority
        ("ing", ""),  // Processing → process
        ("ed", ""),   // Processed → process
        ("es", "e"),  // Affiliates → affiliate
        ("es", ""),   // Breaches → breach
        ("s", ""),    // Members → member
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
fn classify_term(
    _term: &str,
    inlines: &[InlineContent],
    index: usize,
    schedule_patterns: &[(usize, Regex)],
) -> TermKind {
    // Check for inline definition pattern: ("**Term**") or (the "**Term**")
    if index > 0
        && let Some(InlineContent::Text(before)) = inlines.get(index - 1)
    {
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

    // Check for formal definition: **Term** means ...
    if let Some(InlineContent::Text(after)) = inlines.get(index + 1) {
        if FORMAL_DEF_RE.is_match(after) || FORMAL_DEF_ALT_RE.is_match(after) {
            // Check if this is a schedule definition (e.g. "has the meaning given by the Schedule")
            if let Some(schedule_idx) = check_schedule_phrase(inlines, index, schedule_patterns) {
                return TermKind::ScheduleDefinition(schedule_idx);
            }
            return TermKind::FormalDefinition;
        }
        // Check for field label pattern: **Label**: (bold followed by colon)
        // These are structural labels, not defined terms
        if after.starts_with(':') {
            return TermKind::FieldLabel;
        }
    }

    // Check for schedule phrase even without "means"/"has the meaning" prefix
    // e.g. "**Term** is set out in the Schedule."
    if let Some(schedule_idx) = check_schedule_phrase(inlines, index, schedule_patterns) {
        return TermKind::ScheduleDefinition(schedule_idx);
    }

    // Default: bold text in source is a definition (bold = definition sites only)
    TermKind::FormalDefinition
}

static GROUPED_DEF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(shall have the same meaning|have the meaning given|shall be construed)")
        .unwrap()
});

/// Check if any text in the inline sequence contains a phrase indicating
/// a grouped definition (e.g., "shall have the same meaning as in the GDPR").
fn inlines_contain_meaning_phrase(inlines: &[InlineContent]) -> bool {
    for inline in inlines {
        if let InlineContent::Text(t) = inline
            && GROUPED_DEF_RE.is_match(t)
        {
            return true;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_schedule_patterns(titles: &[&str]) -> Vec<(usize, Regex)> {
        let decls: Vec<ScheduleDecl> = titles
            .iter()
            .map(|t| ScheduleDecl {
                title: t.to_string(),
            })
            .collect();
        build_schedule_phrase_patterns(&decls)
    }

    #[test]
    fn schedule_phrase_given_by() {
        let patterns = make_schedule_patterns(&["Schedule"]);
        let inlines = vec![
            InlineContent::Bold("Objection Period".to_string()),
            InlineContent::Text(" has the meaning given by the Schedule.".to_string()),
        ];
        assert_eq!(check_schedule_phrase(&inlines, 0, &patterns), Some(0));
    }

    #[test]
    fn schedule_phrase_set_out_in() {
        let patterns = make_schedule_patterns(&["Schedule"]);
        let inlines = vec![
            InlineContent::Bold("Term".to_string()),
            InlineContent::Text(" is set out in the Schedule.".to_string()),
        ];
        assert_eq!(check_schedule_phrase(&inlines, 0, &patterns), Some(0));
    }

    #[test]
    fn schedule_phrase_case_insensitive() {
        let patterns = make_schedule_patterns(&["Schedule"]);
        let inlines = vec![
            InlineContent::Bold("Term".to_string()),
            InlineContent::Text(" has the meaning GIVEN BY THE SCHEDULE.".to_string()),
        ];
        assert_eq!(check_schedule_phrase(&inlines, 0, &patterns), Some(0));
    }

    #[test]
    fn schedule_phrase_no_match() {
        let patterns = make_schedule_patterns(&["Schedule"]);
        let inlines = vec![
            InlineContent::Bold("Term".to_string()),
            InlineContent::Text(" means something ordinary.".to_string()),
        ];
        assert_eq!(check_schedule_phrase(&inlines, 0, &patterns), None);
    }

    #[test]
    fn schedule_phrase_custom_title() {
        let patterns = make_schedule_patterns(&["Annexure"]);
        let inlines = vec![
            InlineContent::Bold("Rent".to_string()),
            InlineContent::Text(" has the meaning given by the Annexure.".to_string()),
        ];
        assert_eq!(check_schedule_phrase(&inlines, 0, &patterns), Some(0));
    }

    #[test]
    fn schedule_phrase_multiple_schedules() {
        let patterns = make_schedule_patterns(&["Schedule", "Payment Schedule"]);
        let inlines_1 = vec![
            InlineContent::Bold("Term A".to_string()),
            InlineContent::Text(" is specified in the Schedule.".to_string()),
        ];
        let inlines_2 = vec![
            InlineContent::Bold("Term B".to_string()),
            InlineContent::Text(" is specified in the Payment Schedule.".to_string()),
        ];
        assert_eq!(check_schedule_phrase(&inlines_1, 0, &patterns), Some(0));
        assert_eq!(check_schedule_phrase(&inlines_2, 0, &patterns), Some(1));
    }

    #[test]
    fn schedule_phrase_all_variants() {
        let patterns = make_schedule_patterns(&["Schedule"]);
        let phrases = [
            "given by the Schedule",
            "set out in the Schedule",
            "specified in the Schedule",
            "described in the Schedule",
            "defined in the Schedule",
            "provided in the Schedule",
            "contained in the Schedule",
            "stated in the Schedule",
            "referred to in the Schedule",
            "as per the Schedule",
            "in accordance with the Schedule",
            "pursuant to the Schedule",
            "detailed in the Schedule",
        ];
        for phrase in &phrases {
            let inlines = vec![
                InlineContent::Bold("Term".to_string()),
                InlineContent::Text(format!(" has the meaning {}.", phrase)),
            ];
            assert_eq!(
                check_schedule_phrase(&inlines, 0, &patterns),
                Some(0),
                "Failed to match phrase: {}",
                phrase,
            );
        }
    }

    #[test]
    fn classify_term_schedule_definition() {
        let patterns = make_schedule_patterns(&["Schedule"]);
        let inlines = vec![
            InlineContent::Bold("Objection Period".to_string()),
            InlineContent::Text(" has the meaning given by the Schedule.".to_string()),
        ];
        let kind = classify_term("Objection Period", &inlines, 0, &patterns);
        assert_eq!(kind, TermKind::ScheduleDefinition(0));
    }

    #[test]
    fn classify_term_formal_not_schedule() {
        let patterns = make_schedule_patterns(&["Schedule"]);
        let inlines = vec![
            InlineContent::Bold("Term".to_string()),
            InlineContent::Text(" means something.".to_string()),
        ];
        let kind = classify_term("Term", &inlines, 0, &patterns);
        assert_eq!(kind, TermKind::FormalDefinition);
    }

    #[test]
    fn integration_schedule_collection() {
        let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Seller
  - name: Bob
    role: Buyer
schedule:
  - title: Schedule
---

1. ## Definitions {#definitions}

    1. **Payment Amount** has the meaning given by the Schedule.

    2. **Delivery Date** is set out in the Schedule.

    3. **Warranty** means the manufacturer's warranty.

2. ## Obligations {#obligations}

    1. The Seller shall deliver the goods by the Delivery Date.
"#;
        let mut doc = crate::parse(input).unwrap();
        crate::resolve(&mut doc);

        assert_eq!(doc.schedule_items.len(), 2);
        assert_eq!(doc.schedule_items[0].term, "Payment Amount");
        assert_eq!(doc.schedule_items[0].schedule_index, 0);
        assert_eq!(doc.schedule_items[1].term, "Delivery Date");
        assert_eq!(doc.schedule_items[1].schedule_index, 0);
    }

    #[test]
    fn integration_unreferenced_schedule_warning() {
        let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Seller
schedule:
  - title: Schedule
  - title: Payment Schedule
---

1. ## Definitions {#definitions}

    1. **Amount** has the meaning given by the Schedule.
"#;
        let mut doc = crate::parse(input).unwrap();
        crate::resolve(&mut doc);

        // "Payment Schedule" is declared but no terms reference it
        let warnings: Vec<_> = doc
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("Payment Schedule"))
            .collect();
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]
                .message
                .contains("declared but no terms reference it")
        );
    }
}
