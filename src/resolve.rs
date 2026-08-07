use std::collections::HashMap;

use regex::Regex;
use std::sync::LazyLock;

use crate::error::{Diagnostic, SourcePos};
use crate::model::*;
use crate::style::NumberingConvention;

static FORMAL_DEF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\s*means[\s:,]").unwrap());

static FORMAL_DEF_ALT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(has the meaning|shall have the same meaning|have the same meaning)")
        .unwrap()
});

/// A registered anchor target: the reference text it resolves to, where it
/// was declared, and whether any cross-reference points at it.
struct AnchorInfo {
    reference: String,
    pos: Option<SourcePos>,
    used: bool,
}

pub fn resolve(doc: &mut Document, convention: NumberingConvention) {
    // Number recitals and body clauses
    if let Some(ref mut recitals) = doc.recitals {
        assign_clause_numbers(&mut recitals.body);
    }
    assign_clause_numbers(&mut doc.body);

    // Build anchor → reference text map (from both recitals and body)
    let mut anchor_map: HashMap<String, AnchorInfo> = HashMap::new();
    if let Some(ref recitals) = doc.recitals {
        collect_body_anchors(
            &recitals.body,
            &mut anchor_map,
            "Recital",
            convention,
            &mut doc.diagnostics,
        );
    }
    collect_body_anchors(
        &doc.body,
        &mut anchor_map,
        "clause",
        convention,
        &mut doc.diagnostics,
    );

    // Register addendum heading anchors
    for addendum in &doc.addenda {
        if let Some(ref anchor_id) = addendum.anchor {
            register_anchor(
                &mut anchor_map,
                anchor_id,
                format!("Addendum {}", addendum.number),
                addendum.source_pos,
                &mut doc.diagnostics,
            );
        }
    }

    // Resolve cross-references and validate
    if let Some(ref mut recitals) = doc.recitals {
        resolve_cross_refs(
            &mut recitals.body,
            &mut anchor_map,
            &mut doc.diagnostics,
            convention,
        );
    }
    resolve_cross_refs(
        &mut doc.body,
        &mut anchor_map,
        &mut doc.diagnostics,
        convention,
    );
    for addendum in &mut doc.addenda {
        resolve_addendum_cross_refs(addendum, &mut anchor_map, &mut doc.diagnostics, convention);
    }

    // Report anchors that no cross-reference points at (drafting cruft)
    let mut unused: Vec<(&String, &AnchorInfo)> =
        anchor_map.iter().filter(|(_, a)| !a.used).collect();
    unused.sort_by_key(|(id, a)| (a.pos.map_or(0, |p| p.line), id.as_str()));
    for (id, info) in unused {
        doc.diagnostics.push(
            Diagnostic::info(
                "unused-anchor",
                format!("Anchor '#{}' is never referenced by a cross-reference", id),
            )
            .at(info.reference.clone())
            .at_pos(info.pos),
        );
    }

    // Build schedule matching context from front-matter
    let schedule_ctx = ScheduleContext::new(&doc.meta.schedule);

    // Collect schedule items and validate defined terms (single pass)
    collect_and_validate_terms(doc, &schedule_ctx, convention);
}

/// Insert an anchor into the map, warning if the id is already taken.
fn register_anchor(
    map: &mut HashMap<String, AnchorInfo>,
    anchor_id: &str,
    reference: String,
    pos: Option<SourcePos>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(existing) = map.get(anchor_id) {
        diagnostics.push(
            Diagnostic::warning(
                "duplicate-anchor",
                format!(
                    "Anchor '#{}' is declared more than once (first declared at {}); cross-references will resolve to the first declaration",
                    anchor_id, existing.reference
                ),
            )
            .at(reference)
            .at_pos(pos),
        );
    } else {
        map.insert(
            anchor_id.to_string(),
            AnchorInfo {
                reference,
                pos,
                used: false,
            },
        );
    }
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
    let mut i = 0u32;

    for element in &mut parent.body {
        if let ClauseBody::Children(kids) = element {
            for child in kids.iter_mut() {
                let idx = i + 1;
                let number = match child.level {
                    ClauseLevel::TopLevel => ClauseNumber::TopLevel(idx),
                    ClauseLevel::Clause => ClauseNumber::Clause(top, idx),
                    ClauseLevel::SubClause => {
                        let clause_num = match &parent_number {
                            Some(ClauseNumber::Clause(_, c)) => *c,
                            _ => 0,
                        };
                        ClauseNumber::SubClause(top, clause_num, idx)
                    }
                    ClauseLevel::SubSubClause => {
                        let (clause_num, sub_idx) = match &parent_number {
                            Some(ClauseNumber::SubClause(_, c, s)) => (*c, *s),
                            _ => (0, 1),
                        };
                        ClauseNumber::SubSubClause(top, clause_num, sub_idx, idx)
                    }
                    ClauseLevel::Paragraph => {
                        let (clause_num, sub_idx, subsub_idx) = match &parent_number {
                            Some(ClauseNumber::SubSubClause(_, c, s, ss)) => (*c, *s, *ss),
                            _ => (0, 1, 1),
                        };
                        ClauseNumber::Paragraph(top, clause_num, sub_idx, subsub_idx, idx)
                    }
                    ClauseLevel::SubParagraph => {
                        let (clause_num, sub_idx, subsub_idx, para_idx) = match &parent_number {
                            Some(ClauseNumber::Paragraph(_, c, s, ss, p)) => (*c, *s, *ss, *p),
                            _ => (0, 1, 1, 1),
                        };
                        ClauseNumber::SubParagraph(
                            top, clause_num, sub_idx, subsub_idx, para_idx, idx,
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

fn collect_body_anchors(
    body: &[BodyElement],
    map: &mut HashMap<String, AnchorInfo>,
    prefix: &str,
    convention: NumberingConvention,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for element in body {
        if let BodyElement::Clause(clause) = element {
            collect_anchors(clause, map, prefix, convention, diagnostics);
        }
    }
}

fn collect_anchors(
    clause: &Clause,
    map: &mut HashMap<String, AnchorInfo>,
    prefix: &str,
    convention: NumberingConvention,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let (Some(anchor), Some(number)) = (&clause.anchor, &clause.number) {
        register_anchor(
            map,
            anchor,
            number.full_reference(prefix, convention),
            clause.source_pos,
            diagnostics,
        );
    }
    for element in &clause.body {
        if let ClauseBody::Children(kids) = element {
            for child in kids {
                collect_anchors(child, map, prefix, convention, diagnostics);
            }
        }
    }
}

// --- Cross-reference resolution ---

fn resolve_cross_refs(
    body: &mut [BodyElement],
    anchor_map: &mut HashMap<String, AnchorInfo>,
    diagnostics: &mut Vec<Diagnostic>,
    convention: NumberingConvention,
) {
    for element in body.iter_mut() {
        match element {
            BodyElement::Clause(clause) => {
                resolve_clause_cross_refs(clause, anchor_map, diagnostics, convention);
            }
            BodyElement::Prose(inlines) => {
                resolve_inlines_cross_refs(inlines, anchor_map, diagnostics, None, None);
            }
            BodyElement::BulletList(items) => {
                for item_inlines in items.iter_mut() {
                    resolve_inlines_cross_refs(item_inlines, anchor_map, diagnostics, None, None);
                }
            }
        }
    }
}

fn resolve_clause_cross_refs(
    clause: &mut Clause,
    anchor_map: &mut HashMap<String, AnchorInfo>,
    diagnostics: &mut Vec<Diagnostic>,
    convention: NumberingConvention,
) {
    let clause_loc = clause
        .number
        .as_ref()
        .map(|n| n.full_reference("clause", convention));
    let clause_pos = clause.source_pos;

    if let Some(ref mut heading) = clause.heading {
        resolve_inlines_cross_refs(
            &mut heading.text,
            anchor_map,
            diagnostics,
            clause_loc.as_deref(),
            clause_pos,
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
                        clause_pos,
                    );
                }
                ClauseContent::BulletList(items) => {
                    for item_inlines in items {
                        resolve_inlines_cross_refs(
                            item_inlines,
                            anchor_map,
                            diagnostics,
                            clause_loc.as_deref(),
                            clause_pos,
                        );
                    }
                }
                ClauseContent::Table(_) => {}
            },
            ClauseBody::Children(kids) => {
                for child in kids {
                    resolve_clause_cross_refs(child, anchor_map, diagnostics, convention);
                }
            }
        }
    }
}

fn resolve_inlines_cross_refs(
    inlines: &mut [InlineContent],
    anchor_map: &mut HashMap<String, AnchorInfo>,
    diagnostics: &mut Vec<Diagnostic>,
    location: Option<&str>,
    pos: Option<SourcePos>,
) {
    for inline in inlines.iter_mut() {
        if let InlineContent::CrossRef {
            anchor_id,
            resolved,
            display,
        } = inline
        {
            if let Some(info) = anchor_map.get_mut(anchor_id.as_str()) {
                *resolved = Some(info.reference.clone());
                info.used = true;
            } else {
                diagnostics.push(
                    Diagnostic::warning(
                        "broken-cross-ref",
                        format!(
                            "Cross-reference '{}' (#{}) points to non-existent anchor",
                            display, anchor_id
                        ),
                    )
                    .at_opt(location)
                    .at_pos(pos),
                );
            }
        }
    }
}

fn resolve_addendum_cross_refs(
    addendum: &mut Addendum,
    anchor_map: &mut HashMap<String, AnchorInfo>,
    diagnostics: &mut Vec<Diagnostic>,
    convention: NumberingConvention,
) {
    let loc = addendum.heading();
    let pos = addendum.source_pos;
    for content in &mut addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) => {
                resolve_inlines_cross_refs(inlines, anchor_map, diagnostics, Some(&loc), pos);
            }
            AddendumContent::Heading(_, inlines) => {
                resolve_inlines_cross_refs(inlines, anchor_map, diagnostics, Some(&loc), pos);
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    resolve_clause_cross_refs(clause, anchor_map, diagnostics, convention);
                }
            }
            AddendumContent::NumberedList(items) | AddendumContent::BulletList(items) => {
                for item_inlines in items {
                    resolve_inlines_cross_refs(
                        item_inlines,
                        anchor_map,
                        diagnostics,
                        Some(&loc),
                        pos,
                    );
                }
            }
            _ => {}
        }
    }
}

// --- Schedule phrase pattern building ---

const SCHEDULE_PHRASE_TEMPLATES: [&str; 13] = [
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

/// Matches a schedule-referencing phrase with an arbitrary Title-Case
/// schedule-like title (containing "Schedule", "Annexure", or "Appendix"),
/// used to detect references to schedules not declared in front-matter.
static GENERIC_SCHEDULE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:given by|set out in|specified in|described in|defined in|provided in|contained in|stated in|referred to in|as per|in accordance with|pursuant to|detailed in) the ((?:[A-Z][A-Za-z]*\s+)*(?:Schedules?|Annexures?|Appendix(?:es)?|Appendices)\b(?:\s+(?:[A-Z][A-Za-z]*|[0-9]+)\b)*)",
    )
    .unwrap()
});

/// A schedule mention immediately followed by "to the X" / "of the X" is a
/// schedule OF another instrument ("the Schedule to the Corporations Act"),
/// not one of this contract's schedules.
static STATUTE_FOLLOW_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:to|of)\s+the\s+[A-Z0-9]").unwrap());

/// A captured schedule-like title that names a statute is a statutory
/// reference, not a contract schedule ("GST Act Schedule 2").
static STATUTE_TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:Act|Acts|Regulation|Regulations|Code|Statute|Ordinance|Directive)\b").unwrap()
});

/// Schedule matching context built from the front-matter declarations.
struct ScheduleContext {
    /// One compiled regex per declared schedule, with all phrases as alternations.
    patterns: Vec<(usize, Regex)>,
    /// Declared titles, lower-cased, for undeclared-reference detection.
    declared_lower: Vec<String>,
}

impl ScheduleContext {
    fn new(schedules: &[ScheduleDecl]) -> Self {
        let patterns = schedules
            .iter()
            .enumerate()
            .map(|(idx, sched)| {
                let escaped_title = regex::escape(&sched.title);
                let alternations: Vec<String> = SCHEDULE_PHRASE_TEMPLATES
                    .iter()
                    .map(|t| t.replace("{title}", &escaped_title))
                    .collect();
                let pattern = format!(r"(?i)({})", alternations.join("|"));
                (idx, Regex::new(&pattern).unwrap())
            })
            .collect();
        let declared_lower = schedules.iter().map(|s| s.title.to_lowercase()).collect();
        ScheduleContext {
            patterns,
            declared_lower,
        }
    }

    /// Check whether a captured schedule-like title matches a declared one.
    /// Prefix matches in either direction are accepted so that over- or
    /// under-capture of surrounding Title-Case words doesn't cause false
    /// positives (e.g. captured "Schedule The" vs declared "Schedule").
    fn is_declared(&self, captured: &str) -> bool {
        let cap = captured.trim().to_lowercase();
        // A capture that begins with a declared title is accepted so that
        // over-capture of trailing Title-Case words doesn't cause false
        // positives. The reverse is deliberately NOT accepted: a bare
        // "the Schedule" when only "Schedule of Particulars" is declared
        // should warn — the phrase won't produce a schedule item, so
        // silence would hide a real problem.
        self.declared_lower
            .iter()
            .any(|d| cap == *d || cap.starts_with(&format!("{} ", d)))
    }
}

/// Concatenate all text content after the bold term in this inline sequence.
fn text_after_bold(inlines: &[InlineContent], bold_index: usize) -> String {
    let mut after_text = String::new();
    for inline in &inlines[bold_index + 1..] {
        match inline {
            InlineContent::Text(t) => after_text.push_str(t),
            InlineContent::Bold(t) | InlineContent::Italic(t) => after_text.push_str(t),
            _ => {}
        }
    }
    after_text
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

    let after_text = text_after_bold(inlines, bold_index);

    for (idx, pattern) in patterns {
        for m in pattern.find_iter(&after_text) {
            // "the Schedule to the Corporations Act" is a statutory
            // reference, not this contract's schedule.
            if !STATUTE_FOLLOW_RE.is_match(&after_text[m.end()..]) {
                return Some(*idx);
            }
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
    pos: Option<SourcePos>,
    /// True for terms defined implicitly by front-matter (party roles, short
    /// title) rather than bold text in the document body.
    from_front_matter: bool,
    /// True for definition sites inside an addendum — a term redefined there
    /// ("for the purposes of this Addendum, **X** means ...") is scoped
    /// drafting, not a duplicate of the main-body definition.
    in_addendum: bool,
}

#[derive(Debug, PartialEq)]
enum TermKind {
    FormalDefinition,          // **Term** means ...
    InlineDefinition,          // ("**Term**")
    ScheduleDefinition(usize), // **Term** has the meaning given by the Schedule
    FieldLabel,                // **Label**: structural label, not a term
}

fn collect_and_validate_terms(
    doc: &mut Document,
    schedule_ctx: &ScheduleContext,
    convention: NumberingConvention,
) {
    let mut definitions: Vec<TermDefinition> = Vec::new();
    let mut schedule_items: Vec<ScheduleItem> = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    // Party roles are automatic definitions
    for party in &doc.meta.parties {
        definitions.push(TermDefinition {
            term: party.role.clone(),
            location: Some("front-matter".to_string()),
            pos: None,
            from_front_matter: true,
            in_addendum: false,
        });
    }

    // An explicitly declared short title is an automatic definition. The
    // implicit default ("Agreement") is not — warning about a term the
    // drafter never wrote would be noise.
    if let Some(ref doc_type) = doc.meta.doc_type {
        definitions.push(TermDefinition {
            term: doc_type.clone(),
            location: Some("front-matter".to_string()),
            pos: None,
            from_front_matter: true,
            in_addendum: false,
        });
    }

    // Collect all bold terms — definitions and schedule items in one pass
    if let Some(ref recitals) = doc.recitals {
        for element in &recitals.body {
            match element {
                BodyElement::Clause(clause) => {
                    collect_clause_terms(
                        clause,
                        &mut definitions,
                        &mut schedule_items,
                        schedule_ctx,
                        convention,
                        false,
                        &mut diagnostics,
                    );
                }
                BodyElement::Prose(inlines) => {
                    collect_inline_terms(
                        inlines,
                        &mut definitions,
                        &mut schedule_items,
                        schedule_ctx,
                        Some("recitals"),
                        None,
                        false,
                        &mut diagnostics,
                    );
                }
                BodyElement::BulletList(items) => {
                    for item_inlines in items {
                        collect_inline_terms(
                            item_inlines,
                            &mut definitions,
                            &mut schedule_items,
                            schedule_ctx,
                            Some("recitals"),
                            None,
                            false,
                            &mut diagnostics,
                        );
                    }
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
                    schedule_ctx,
                    convention,
                    false,
                    &mut diagnostics,
                );
            }
            BodyElement::Prose(inlines) => {
                collect_inline_terms(
                    inlines,
                    &mut definitions,
                    &mut schedule_items,
                    schedule_ctx,
                    None,
                    None,
                    false,
                    &mut diagnostics,
                );
            }
            BodyElement::BulletList(items) => {
                for item_inlines in items {
                    collect_inline_terms(
                        item_inlines,
                        &mut definitions,
                        &mut schedule_items,
                        schedule_ctx,
                        None,
                        None,
                        false,
                        &mut diagnostics,
                    );
                }
            }
        }
    }
    for addendum in &doc.addenda {
        collect_addendum_terms(
            addendum,
            &mut definitions,
            &mut schedule_items,
            schedule_ctx,
            convention,
            &mut diagnostics,
        );
    }

    doc.diagnostics.extend(diagnostics);
    doc.schedule_items = schedule_items;

    // Warn on declared schedules with no referencing terms
    let mut referenced_schedules = std::collections::HashSet::new();
    for item in &doc.schedule_items {
        referenced_schedules.insert(item.schedule_index);
    }
    for (idx, sched) in doc.meta.schedule.iter().enumerate() {
        if !referenced_schedules.contains(&idx) {
            doc.diagnostics.push(
                Diagnostic::warning(
                    "unreferenced-schedule",
                    format!(
                        "Schedule '{}' is declared but no terms reference it",
                        sched.title
                    ),
                )
                .at("front-matter"),
            );
        }
    }

    // Warn on terms defined more than once within the same scope.
    // Front-matter auto-definitions (party roles, short title) are exempt —
    // formally re-defining a role in the body is common practice — and a
    // main-body term redefined in an addendum ("for the purposes of this
    // Addendum, **X** means ...") is scoped drafting, not a duplicate, so
    // duplicates are only counted among body sites or among addendum sites.
    let mut scoped_defs: HashMap<(&str, bool), Vec<&TermDefinition>> = HashMap::new();
    let mut scope_order: Vec<(&str, bool)> = Vec::new();
    for def in definitions.iter().filter(|d| !d.from_front_matter) {
        let key = (def.term.as_str(), def.in_addendum);
        let entry = scoped_defs.entry(key).or_default();
        if entry.is_empty() {
            scope_order.push(key);
        }
        entry.push(def);
    }
    for key in scope_order {
        let defs = &scoped_defs[&key];
        if defs.len() > 1 {
            let first_loc = defs[0]
                .location
                .clone()
                .unwrap_or_else(|| "unknown location".to_string());
            let second = defs[1];
            doc.diagnostics.push(
                Diagnostic::warning(
                    "duplicate-definition",
                    format!(
                        "'{}' is bold-defined {} times (first at {}). If a later occurrence is a reference rather than a new definition, use plain text — bold marks definition sites",
                        key.0,
                        defs.len(),
                        first_loc
                    ),
                )
                .at_opt(second.location.clone())
                .at_pos(second.pos),
            );
        }
    }

    // Build definition set (term → first location + line) and per-term
    // definition site counts (front-matter and body sites alike)
    let mut def_map: HashMap<String, (String, Option<SourcePos>)> = HashMap::new();
    let mut def_site_counts: HashMap<&str, usize> = HashMap::new();
    for def in &definitions {
        let loc = def.location.clone().unwrap_or_default();
        def_map.entry(def.term.clone()).or_insert((loc, def.pos));
        *def_site_counts.entry(def.term.as_str()).or_insert(0) += 1;
    }

    // Collect all plain text from the document for usage scanning. Bold text
    // is excluded: bold marks definition sites, not references, so a term's
    // own definition must not count as a usage of it.
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

    // Warn on definitions never used in the document text. Matching is
    // word-boundary and case-sensitive (spec 4.4.1: lowercase "agreement"
    // does not reference defined "Agreement"; substring hits like "Act"
    // inside "Contract" must not count). All terms' variants are compiled
    // into one RegexSet so the document text is scanned once.
    // Schedule terms are exempt — they appear in the schedule table. A term
    // with two or more definition sites also counts as used: it appears in
    // the document more than once, and the duplicate-definition warning
    // already covers the questionable markup.
    let schedule_terms: std::collections::HashSet<&str> = doc
        .schedule_items
        .iter()
        .map(|si| si.term.as_str())
        .collect();

    let candidates: Vec<(&String, &(String, Option<SourcePos>))> = def_map
        .iter()
        .filter(|(term, _)| !schedule_terms.contains(term.as_str()))
        .filter(|(term, _)| def_site_counts.get(term.as_str()).copied().unwrap_or(0) < 2)
        .collect();

    let mut patterns: Vec<String> = Vec::new();
    let mut owners: Vec<usize> = Vec::new();
    for (i, (term, _)) in candidates.iter().enumerate() {
        for variant in term_variants(term) {
            if variant.is_empty() {
                continue;
            }
            patterns.push(format!(r"\b{}\b", regex::escape(&variant)));
            owners.push(i);
        }
    }
    let mut used = vec![false; candidates.len()];
    if let Ok(set) = regex::RegexSet::new(&patterns) {
        for m in set.matches(&all_text) {
            used[owners[m]] = true;
        }
    }

    let mut unused: Vec<(&String, &(String, Option<SourcePos>))> = candidates
        .iter()
        .enumerate()
        .filter(|(i, _)| !used[*i])
        .map(|(_, c)| *c)
        .collect();
    unused.sort_by_key(|(term, (_, pos))| (pos.map_or(0, |p| p.line), term.as_str()));
    for (term, (loc, pos)) in unused {
        doc.diagnostics.push(
            Diagnostic::warning(
                "unused-term",
                format!("'{}' is defined but never used in the document", term),
            )
            .at(loc.clone())
            .at_pos(*pos),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_clause_terms(
    clause: &Clause,
    defs: &mut Vec<TermDefinition>,
    schedule_items: &mut Vec<ScheduleItem>,
    schedule_ctx: &ScheduleContext,
    convention: NumberingConvention,
    in_addendum: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let clause_loc = clause
        .number
        .as_ref()
        .map(|n| n.full_reference("clause", convention));

    for element in &clause.body {
        match element {
            ClauseBody::Content(content) => match content {
                ClauseContent::Paragraph(inlines) | ClauseContent::Blockquote(inlines) => {
                    collect_inline_terms(
                        inlines,
                        defs,
                        schedule_items,
                        schedule_ctx,
                        clause_loc.as_deref(),
                        clause.source_pos,
                        in_addendum,
                        diagnostics,
                    );
                }
                ClauseContent::BulletList(items) => {
                    for item_inlines in items {
                        collect_inline_terms(
                            item_inlines,
                            defs,
                            schedule_items,
                            schedule_ctx,
                            clause_loc.as_deref(),
                            clause.source_pos,
                            in_addendum,
                            diagnostics,
                        );
                    }
                }
                ClauseContent::Table(_) => {}
            },
            ClauseBody::Children(kids) => {
                for child in kids {
                    collect_clause_terms(
                        child,
                        defs,
                        schedule_items,
                        schedule_ctx,
                        convention,
                        in_addendum,
                        diagnostics,
                    );
                }
            }
        }
    }
}

fn collect_addendum_terms(
    addendum: &Addendum,
    defs: &mut Vec<TermDefinition>,
    schedule_items: &mut Vec<ScheduleItem>,
    schedule_ctx: &ScheduleContext,
    convention: NumberingConvention,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let heading = addendum.heading();
    let loc = Some(heading.as_str());
    let pos = addendum.source_pos;
    for content in &addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) | AddendumContent::Heading(_, inlines) => {
                collect_inline_terms(
                    inlines,
                    defs,
                    schedule_items,
                    schedule_ctx,
                    loc,
                    pos,
                    true,
                    diagnostics,
                );
            }
            AddendumContent::ClauseList(clauses) => {
                for clause in clauses {
                    collect_clause_terms(
                        clause,
                        defs,
                        schedule_items,
                        schedule_ctx,
                        convention,
                        true,
                        diagnostics,
                    );
                }
            }
            AddendumContent::NumberedList(items) | AddendumContent::BulletList(items) => {
                for item_inlines in items {
                    collect_inline_terms(
                        item_inlines,
                        defs,
                        schedule_items,
                        schedule_ctx,
                        loc,
                        pos,
                        true,
                        diagnostics,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Collect bold terms: definitions and schedule items in one pass.
/// Also warns when a definition references a schedule-like title that is not
/// declared in front-matter (spec 10.2.4).
#[allow(clippy::too_many_arguments)]
fn collect_inline_terms(
    inlines: &[InlineContent],
    defs: &mut Vec<TermDefinition>,
    schedule_items: &mut Vec<ScheduleItem>,
    schedule_ctx: &ScheduleContext,
    location: Option<&str>,
    pos: Option<SourcePos>,
    in_addendum: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (i, inline) in inlines.iter().enumerate() {
        if let InlineContent::Bold(term) = inline {
            let kind = classify_term(term, inlines, i, &schedule_ctx.patterns);
            match kind {
                TermKind::FormalDefinition | TermKind::InlineDefinition => {
                    defs.push(TermDefinition {
                        term: term.clone(),
                        location: location.map(String::from),
                        pos,
                        from_front_matter: false,
                        in_addendum,
                    });
                    // The definition didn't match a declared schedule — check
                    // whether it references a schedule-like title that was
                    // never declared in front-matter. Statutory references
                    // ("the Schedule to the Corporations Act", "the GST Act
                    // Schedule 2") are everyday drafting and are skipped.
                    let after = text_after_bold(inlines, i);
                    for caps in GENERIC_SCHEDULE_RE.captures_iter(&after) {
                        let capture = caps.get(1).unwrap();
                        let title = capture.as_str().trim();
                        if title.is_empty()
                            || STATUTE_TITLE_RE.is_match(title)
                            || STATUTE_FOLLOW_RE.is_match(&after[capture.end()..])
                        {
                            continue;
                        }
                        if !schedule_ctx.is_declared(title) {
                            diagnostics.push(
                                Diagnostic::warning(
                                    "undeclared-schedule",
                                    format!(
                                        "'{}' appears to reference schedule '{}', which is not declared in front-matter",
                                        term, title
                                    ),
                                )
                                .at_opt(location)
                                .at_pos(pos),
                            );
                            break;
                        }
                    }
                }
                TermKind::ScheduleDefinition(schedule_idx) => {
                    defs.push(TermDefinition {
                        term: term.clone(),
                        location: location.map(String::from),
                        pos,
                        from_front_matter: false,
                        in_addendum,
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
        BodyElement::BulletList(items) => {
            for item in items {
                collect_inlines_text(item, out);
            }
        }
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
                ClauseContent::BulletList(items) => {
                    for item in items {
                        collect_inlines_text(item, out);
                    }
                }
                ClauseContent::Table(_) => {}
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
            // Bold marks definition sites, not references — excluded so a
            // term's own definition doesn't count as a usage of it.
            InlineContent::Bold(_) => {}
            InlineContent::Italic(t) => {
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

    // Case is preserved: matching is case-sensitive per spec 4.4.1 ("the
    // word 'agreement' in lowercase does not reference" defined "Agreement").
    let base = s;
    let mut variants = vec![base.clone()];

    // Apply suffix rules, each producing a variant
    let suffix_rules: &[(&str, &str)] = &[
        ("ies", "y"), // Authorities → Authority
        ("ing", ""),  // Processing → Process
        ("ed", ""),   // Processed → Process
        ("es", "e"),  // Affiliates → Affiliate
        ("es", ""),   // Breaches → Breach
        ("s", ""),    // Members → Member
    ];

    for &(suffix, replacement) in suffix_rules {
        if base.ends_with(suffix) && base.len() > suffix.len() + 2 {
            let stem = &base[..base.len() - suffix.len()];
            let variant = format!("{}{}", stem, replacement);
            if variant != base {
                variants.push(variant);
            }
        }
    }

    // Generate forward plural forms (spec 4.4.3) so the defined term
    // matches its plural in the document text.
    let last_word = base.rsplit_once(' ').map_or(base.as_str(), |(_, w)| w);
    if last_word.len() > 2 {
        let plural = if last_word.ends_with('y')
            && !last_word.ends_with("ay")
            && !last_word.ends_with("ey")
            && !last_word.ends_with("oy")
            && !last_word.ends_with("uy")
        {
            // consonant + y → ies (Party → Parties, Authority → Authorities)
            format!("{}ies", &base[..base.len() - 1])
        } else if last_word.ends_with('s')
            || last_word.ends_with('x')
            || last_word.ends_with('z')
            || last_word.ends_with("sh")
            || last_word.ends_with("ch")
        {
            // sibilant endings → +es (Business → Businesses)
            format!("{}es", base)
        } else {
            // default → +s (Agreement → Agreements)
            format!("{}s", base)
        };
        if plural != base {
            variants.push(plural);
        }
    }

    variants
}

/// Classify a bold term based on what follows/precedes it in the inline sequence.
/// In the source, bold marks definition sites only. This classifies the type of
/// definition, or identifies field labels (structural bold, not a term).
fn classify_term(
    term: &str,
    inlines: &[InlineContent],
    index: usize,
    schedule_patterns: &[(usize, Regex)],
) -> TermKind {
    // Field label with the colon inside the bold: **Label:** value
    if term.trim_end().ends_with(':') {
        return TermKind::FieldLabel;
    }

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
        ScheduleContext::new(&decls).patterns
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
        crate::resolve(&mut doc, NumberingConvention::Commonwealth);

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
        crate::resolve(&mut doc, NumberingConvention::Commonwealth);

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
