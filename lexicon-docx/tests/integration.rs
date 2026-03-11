use lexicon_docx::model::*;
use lexicon_docx::style::StyleConfig;

// ---------------------------------------------------------------------------
// Helper: minimal valid front-matter
// ---------------------------------------------------------------------------

/// Smallest valid Lexicon document — use as a base and extend for specific tests.
const MINIMAL: &str = r#"---
title: Test Agreement
date: 2026-01-01
parties:
  - name: Alice
    role: Buyer
  - name: Bob
    role: Seller
---
"#;

/// Helper to parse + resolve in one step.
fn parse_and_resolve(input: &str) -> Document {
    let mut doc = lexicon_docx::parse(input).unwrap();
    lexicon_docx::resolve(&mut doc);
    doc
}

// ===========================================================================
// Front-matter parsing
// ===========================================================================

#[test]
fn frontmatter_basic_fields() {
    let doc = parse_and_resolve(MINIMAL);
    assert_eq!(doc.meta.title, "Test Agreement");
    assert_eq!(doc.meta.date, "2026-01-01");
    assert_eq!(doc.meta.parties.len(), 2);
    assert_eq!(doc.meta.parties[0].name, "Alice");
    assert_eq!(doc.meta.parties[0].role, "Buyer");
    assert_eq!(doc.meta.parties[1].name, "Bob");
    assert_eq!(doc.meta.parties[1].role, "Seller");
}

#[test]
fn frontmatter_optional_fields() {
    let input = r#"---
title: Service Agreement
type: Deed
date: 2026-06-15
ref: SA-2026-001
author: Legal Team
status: draft
version: 2.1
parties:
  - name: Acme Corp
    role: Provider
    specifier: ACN 123 456 789
    entity_type: au-company
schedule:
  - title: Schedule
exhibits:
  - title: Terms of Service
---
"#;
    let doc = parse_and_resolve(input);
    assert_eq!(doc.meta.doc_type.as_deref(), Some("Deed"));
    assert_eq!(doc.meta.ref_.as_deref(), Some("SA-2026-001"));
    assert_eq!(doc.meta.author.as_deref(), Some("Legal Team"));
    assert_eq!(doc.meta.status, Some(Status::Draft));
    assert_eq!(doc.meta.version.as_deref(), Some("2.1"));
    assert_eq!(doc.meta.parties[0].specifier.as_deref(), Some("ACN 123 456 789"));
    assert_eq!(doc.meta.parties[0].entity_type.as_deref(), Some("au-company"));
    assert_eq!(doc.meta.schedule.len(), 1);
    assert_eq!(doc.meta.exhibits.len(), 1);
    assert_eq!(doc.meta.exhibits[0].title, "Terms of Service");
}

#[test]
fn frontmatter_version_as_integer() {
    let input = r#"---
title: Test
date: 2026-01-01
version: 3
parties:
  - name: A
    role: R
---
"#;
    let doc = parse_and_resolve(input);
    assert_eq!(doc.meta.version.as_deref(), Some("3"));
}

#[test]
fn frontmatter_version_as_decimal() {
    let input = r#"---
title: Test
date: 2026-01-01
version: 1.4
parties:
  - name: A
    role: R
---
"#;
    let doc = parse_and_resolve(input);
    assert_eq!(doc.meta.version.as_deref(), Some("1.4"));
}

#[test]
fn frontmatter_status_variants() {
    for (yaml_val, expected) in [("draft", Status::Draft), ("final", Status::Final), ("executed", Status::Executed)] {
        let input = format!(
            "---\ntitle: T\ndate: 2026-01-01\nstatus: {}\nparties:\n  - name: A\n    role: R\n---\n",
            yaml_val
        );
        let doc = parse_and_resolve(&input);
        assert_eq!(doc.meta.status, Some(expected));
    }
}

// ---------------------------------------------------------------------------
// Front-matter validation errors
// ---------------------------------------------------------------------------

#[test]
fn frontmatter_missing_delimiters() {
    let input = "title: No Delimiters\n";
    let result = lexicon_docx::parse(input);
    assert!(result.is_err());
}

#[test]
fn frontmatter_invalid_date_produces_diagnostic() {
    let input = r#"---
title: Test
date: not-a-date
parties:
  - name: A
    role: R
---
"#;
    let doc = parse_and_resolve(input);
    let errors: Vec<_> = doc.diagnostics.iter()
        .filter(|d| d.message.contains("not a valid YYYY-MM-DD"))
        .collect();
    assert_eq!(errors.len(), 1);
}

#[test]
fn frontmatter_missing_parties() {
    let input = "---\ntitle: T\ndate: 2026-01-01\nparties: []\n---\n";
    let doc = parse_and_resolve(input);
    let errors: Vec<_> = doc.diagnostics.iter()
        .filter(|d| d.message.contains("No parties"))
        .collect();
    assert_eq!(errors.len(), 1);
}

// ===========================================================================
// Clause parsing and numbering
// ===========================================================================

#[test]
fn single_top_level_clause() {
    let input = format!("{}\n1. ## Definitions\n\n    1. Some text.\n", MINIMAL);
    let doc = parse_and_resolve(&input);

    assert_eq!(doc.body.len(), 1);
    let clause = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        other => panic!("Expected Clause, got {:?}", other),
    };
    assert_eq!(clause.level, ClauseLevel::TopLevel);
    assert!(clause.heading.is_some());
    assert_eq!(clause.number.as_ref().unwrap().to_string(), "1.");
}

#[test]
fn nested_clause_numbering() {
    let input = format!(
        "{}\n1. ## First\n\n    1. Clause text.\n\n        1. Sub-clause text.\n\n            1. Sub-sub-clause text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    // Walk the clause tree to verify numbering at each level
    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    assert!(matches!(top.number, Some(ClauseNumber::TopLevel(1))));

    // Find the first child
    let child = first_child_clause(top).expect("Expected child clause");
    assert!(matches!(child.number, Some(ClauseNumber::Clause(1, 1))));

    let sub = first_child_clause(child).expect("Expected sub-clause");
    assert!(matches!(sub.number, Some(ClauseNumber::SubClause(1, 1, 'a'))));

    let subsub = first_child_clause(sub).expect("Expected sub-sub-clause");
    match &subsub.number {
        Some(ClauseNumber::SubSubClause(1, 1, 'a', r)) => assert_eq!(r, "i"),
        other => panic!("Expected SubSubClause(1,1,a,i), got {:?}", other),
    }
}

#[test]
fn multiple_top_level_clauses_numbered_sequentially() {
    let input = format!(
        "{}\n1. ## First\n\n    1. Text.\n\n1. ## Second\n\n    1. Text.\n\n1. ## Third\n\n    1. Text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    assert_eq!(doc.body.len(), 3);
    for (i, element) in doc.body.iter().enumerate() {
        let clause = match element {
            BodyElement::Clause(c) => c,
            _ => panic!("Expected clause at index {}", i),
        };
        assert!(matches!(clause.number, Some(ClauseNumber::TopLevel(n)) if n == (i as u32 + 1)));
    }
}

#[test]
fn clause_number_full_reference() {
    assert_eq!(ClauseNumber::TopLevel(3).full_reference(), "clause 3");
    assert_eq!(ClauseNumber::Clause(2, 5).full_reference(), "clause 2.5");
    assert_eq!(ClauseNumber::SubClause(1, 2, 'c').full_reference(), "clause 1.2(c)");
    assert_eq!(
        ClauseNumber::SubSubClause(1, 2, 'a', "ii".to_string()).full_reference(),
        "clause 1.2(a)(ii)"
    );
}

// ===========================================================================
// Inline content
// ===========================================================================

#[test]
fn bold_text_parsed_as_defined_term() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Agreement** means this agreement.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let inlines = first_clause_paragraph_inlines(&doc);
    let has_bold = inlines.iter().any(|i| matches!(i, InlineContent::Bold(t) if t == "Agreement"));
    assert!(has_bold, "Expected Bold('Agreement') in {:?}", inlines);
}

#[test]
fn italic_text_preserved() {
    let input = format!(
        "{}\n1. ## Notes\n\n    1. This is *important* text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let inlines = first_clause_paragraph_inlines(&doc);
    let has_italic = inlines.iter().any(|i| matches!(i, InlineContent::Italic(t) if t == "important"));
    assert!(has_italic, "Expected Italic('important') in {:?}", inlines);
}

// ===========================================================================
// Cross-references
// ===========================================================================

#[test]
fn cross_reference_resolves() {
    let input = format!(
        "{}\n1. ## Definitions {{#definitions}}\n\n    1. See [clause X](#obligations).\n\n1. ## Obligations {{#obligations}}\n\n    1. Obligation text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let inlines = first_clause_paragraph_inlines(&doc);
    let xref = inlines.iter().find(|i| matches!(i, InlineContent::CrossRef { .. }));
    assert!(xref.is_some(), "Expected CrossRef in {:?}", inlines);

    if let Some(InlineContent::CrossRef { resolved, anchor_id, .. }) = xref {
        assert_eq!(anchor_id, "obligations");
        assert!(resolved.is_some(), "Cross-reference should be resolved");
        assert!(resolved.as_ref().unwrap().contains("2"), "Should reference clause 2");
    }
}

#[test]
fn broken_cross_reference_produces_warning() {
    let input = format!(
        "{}\n1. ## Clause\n\n    1. See [clause X](#nonexistent).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let warnings: Vec<_> = doc.diagnostics.iter()
        .filter(|d| d.message.contains("nonexistent"))
        .collect();
    assert!(!warnings.is_empty(), "Expected warning about broken cross-ref");
}

// ===========================================================================
// Defined terms
// ===========================================================================

#[test]
fn defined_but_unused_term_produces_warning() {
    // Party roles are auto-defined. If a role never appears in the body text
    // (not even as bold), it's flagged as "defined but never used".
    // Here, "Seller" is a party role that doesn't appear in the body.
    let input = format!(
        "{}\n1. ## Obligations\n\n    1. The Buyer shall pay on time.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let warnings: Vec<_> = doc.diagnostics.iter()
        .filter(|d| d.message.contains("Seller") && d.message.contains("never used"))
        .collect();
    assert!(!warnings.is_empty(), "Expected warning about unused 'Seller' role: {:?}", doc.diagnostics);
}

#[test]
fn defined_term_used_later_no_warning() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Service** means the hosted platform.\n\n1. ## Scope\n\n    1. The **Service** shall be available.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let term_warnings: Vec<_> = doc.diagnostics.iter()
        .filter(|d| d.message.contains("Service"))
        .collect();
    assert!(term_warnings.is_empty(), "Should not warn about defined+used term: {:?}", term_warnings);
}

#[test]
fn party_role_not_flagged_as_undefined() {
    let input = format!(
        "{}\n1. ## Obligations\n\n    1. The **Buyer** shall pay.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let buyer_warnings: Vec<_> = doc.diagnostics.iter()
        .filter(|d| d.message.contains("Buyer"))
        .collect();
    assert!(buyer_warnings.is_empty(), "Party role should not be flagged: {:?}", buyer_warnings);
}

// ===========================================================================
// Schedule items
// ===========================================================================

#[test]
fn schedule_items_detected_from_phrases() {
    let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: A
    role: R
schedule:
  - title: Schedule
---

1. ## Definitions

    1. **Payment Amount** has the meaning given by the Schedule.

    1. **Delivery Date** is set out in the Schedule.

    1. **Warranty Period** means 12 months.
"#;
    let doc = parse_and_resolve(input);

    assert_eq!(doc.schedule_items.len(), 2);
    let terms: Vec<_> = doc.schedule_items.iter().map(|i| i.term.as_str()).collect();
    assert!(terms.contains(&"Payment Amount"));
    assert!(terms.contains(&"Delivery Date"));
    // Warranty Period should NOT be a schedule item
    assert!(!terms.contains(&"Warranty Period"));
}

#[test]
fn multiple_schedules_items_assigned_correctly() {
    let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: A
    role: R
schedule:
  - title: Schedule
  - title: Payment Schedule
---

1. ## Definitions

    1. **Amount** has the meaning given by the Schedule.

    1. **Fee** is set out in the Payment Schedule.
"#;
    let doc = parse_and_resolve(input);

    assert_eq!(doc.schedule_items.len(), 2);
    let amount = doc.schedule_items.iter().find(|i| i.term == "Amount").unwrap();
    let fee = doc.schedule_items.iter().find(|i| i.term == "Fee").unwrap();
    assert_eq!(amount.schedule_index, 0);
    assert_eq!(fee.schedule_index, 1);
}

// ===========================================================================
// Addenda
// ===========================================================================

#[test]
fn addendum_parsed_and_numbered() {
    let input = format!(
        "{}# ADDENDUM - Processing Details\n\nSome addendum text.\n\n# ADDENDUM - Security Measures\n\nSecurity text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    assert_eq!(doc.addenda.len(), 2);
    assert_eq!(doc.addenda[0].number, 1);
    assert_eq!(doc.addenda[0].title, "Processing Details");
    assert_eq!(doc.addenda[0].heading(), "ADDENDUM 1 - Processing Details");
    assert_eq!(doc.addenda[1].number, 2);
    assert_eq!(doc.addenda[1].title, "Security Measures");
}

#[test]
fn addendum_without_title() {
    let input = format!(
        "{}# ADDENDUM\n\nContent here.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    assert_eq!(doc.addenda.len(), 1);
    assert_eq!(doc.addenda[0].title, "");
    assert_eq!(doc.addenda[0].heading(), "ADDENDUM 1");
}

// ===========================================================================
// Prose (non-clause body text)
// ===========================================================================

#[test]
fn prose_before_first_clause() {
    let input = format!(
        "{}This agreement is entered into on the date above.\n\n1. ## Definitions\n\n    1. **Term** means something.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    assert!(doc.body.len() >= 2);
    assert!(matches!(&doc.body[0], BodyElement::Prose(_)));
    assert!(matches!(&doc.body[1], BodyElement::Clause(_)));
}

// ===========================================================================
// Anchors
// ===========================================================================

#[test]
fn anchor_stripped_from_heading_text() {
    let input = format!(
        "{}\n1. ## Definitions {{#definitions}}\n\n    1. Text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let clause = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected clause"),
    };
    let heading = clause.heading.as_ref().unwrap();
    let heading_text: String = heading.text.iter().map(|i| i.as_plain_text()).collect();
    assert!(!heading_text.contains("{#"), "Anchor should be stripped from heading text");
    assert_eq!(clause.anchor.as_deref(), Some("definitions"));
}

// ===========================================================================
// Tables in clauses
// ===========================================================================

#[test]
fn table_in_addendum_parsed() {
    // Tables in addenda are easier to test since they don't need list nesting.
    let input = format!(
        "{}# ADDENDUM - Data\n\nSome introductory text.\n\n| Header A | Header B |\n|----------|----------|\n| Cell 1   | Cell 2   |\n| Cell 3   | Cell 4   |\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    assert_eq!(doc.addenda.len(), 1);
    let has_table = doc.addenda[0].content.iter().any(|c| matches!(c, AddendumContent::Table(_)));
    assert!(has_table, "Expected a table in addendum content: {:?}", doc.addenda[0].content);
}

// ===========================================================================
// Full pipeline: parse → resolve → render
// ===========================================================================

#[test]
fn full_pipeline_produces_docx_bytes() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Agreement** means this agreement.\n\n1. ## Obligations\n\n    1. The **Buyer** shall pay.\n",
        MINIMAL
    );
    let mut doc = lexicon_docx::parse(&input).unwrap();
    lexicon_docx::resolve(&mut doc);

    let style = StyleConfig::default();
    let bytes = lexicon_docx::render_docx(
        &doc, &style, None, &[], lexicon_docx::PdfRenderer::Auto,
    ).unwrap();

    // DOCX files are ZIP archives starting with PK magic bytes
    assert!(bytes.len() > 100, "DOCX output should be non-trivial");
    assert_eq!(&bytes[0..2], b"PK", "DOCX should be a valid ZIP archive");
}

#[test]
fn process_convenience_function_works() {
    let input = format!(
        "{}\n1. ## Clause\n\n    1. Text.\n",
        MINIMAL
    );
    let style = StyleConfig::default();
    let (bytes, diagnostics) = lexicon_docx::process(
        &input, &style, None, None, lexicon_docx::PdfRenderer::Auto,
    ).unwrap();

    assert!(bytes.len() > 100);
    assert_eq!(&bytes[0..2], b"PK");
    // Diagnostics should be a vec (may have warnings but no hard errors)
    let _ = diagnostics;
}

#[test]
fn draft_status_injects_watermark() {
    let input = r#"---
title: Draft Contract
date: 2026-01-01
status: draft
parties:
  - name: A
    role: R
---

1. ## Clause

    1. Text.
"#;
    let style = StyleConfig::default();
    let (bytes, _) = lexicon_docx::process(
        input, &style, None, None, lexicon_docx::PdfRenderer::Auto,
    ).unwrap();

    // The watermark is injected as VML XML containing "DRAFT" inside the DOCX ZIP
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();

    let mut found_draft = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        if file.name().contains("header") {
            let mut contents = String::new();
            std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
            if contents.contains("DRAFT") {
                found_draft = true;
                break;
            }
        }
    }
    assert!(found_draft, "Draft watermark should be present in header XML");
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Walk into the first clause's first child, recursively.
fn first_child_clause(clause: &Clause) -> Option<&Clause> {
    for element in &clause.body {
        if let ClauseBody::Children(children) = element {
            return children.first();
        }
    }
    None
}

/// Get the inline content from the first paragraph of the first clause's first child.
fn first_clause_paragraph_inlines(doc: &Document) -> &[InlineContent] {
    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    // First child clause's first paragraph
    let child = first_child_clause(top).expect("Expected child clause");
    for element in &child.body {
        if let ClauseBody::Content(ClauseContent::Paragraph(inlines)) = element {
            return inlines;
        }
    }
    panic!("No paragraph found in first child clause");
}

// ===========================================================================
// Recitals / Background
// ===========================================================================

#[test]
fn recitals_basic_parsing() {
    let input = format!(
        "{}# Background\n\n1. First recital.\n\n2. Second recital.\n\n# Operative Provisions\n\n1. ## Obligations\n\n    1. The Buyer shall pay.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    // Recitals parsed
    let recitals = doc.recitals.as_ref().expect("Expected recitals");
    assert_eq!(recitals.heading, "Background");

    // Two recital clauses with letters A, B
    let clauses: Vec<_> = recitals.body.iter().filter_map(|e| {
        if let BodyElement::Clause(c) = e { Some(c) } else { None }
    }).collect();
    assert_eq!(clauses.len(), 2);
    assert!(matches!(clauses[0].number, Some(ClauseNumber::RecitalTopLevel('A'))));
    assert!(matches!(clauses[1].number, Some(ClauseNumber::RecitalTopLevel('B'))));

    // Body heading captured
    assert_eq!(doc.body_heading.as_deref(), Some("Operative Provisions"));

    // Body clause still parsed
    assert!(!doc.body.is_empty());
}

#[test]
fn recitals_heading_case_insensitive() {
    let input = format!(
        "{}# RECITALS\n\nSome prose.\n\n# Terms\n\n1. ## Clause One\n\n    1. Text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let recitals = doc.recitals.as_ref().expect("Expected recitals");
    assert_eq!(recitals.heading, "RECITALS");
    assert_eq!(doc.body_heading.as_deref(), Some("Terms"));
}

#[test]
fn recitals_prose_content() {
    let input = format!(
        "{}# Background\n\nWHEREAS the parties wish to agree.\n\n# Operative Provisions\n\n1. ## Clause\n\n    1. Text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let recitals = doc.recitals.as_ref().unwrap();
    let prose_count = recitals.body.iter().filter(|e| matches!(e, BodyElement::Prose(_))).count();
    assert_eq!(prose_count, 1);
}

#[test]
fn recitals_cross_reference() {
    let input = format!(
        "{}# Background\n\n1. The background to this agreement. {{#bg}}\n\n# Operative Provisions\n\n1. ## Clause\n\n    1. See [Recital A](#bg).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    // Check the cross-reference resolved
    if let Some(BodyElement::Clause(clause)) = doc.body.first() {
        let child = first_child_clause(clause).unwrap();
        let has_resolved = child.body.iter().any(|e| {
            if let ClauseBody::Content(ClauseContent::Paragraph(inlines)) = e {
                inlines.iter().any(|i| matches!(i, InlineContent::CrossRef { resolved: Some(r), .. } if r == "Recital A"))
            } else {
                false
            }
        });
        assert!(has_resolved, "Cross-reference to recital should resolve to 'Recital A'");
    }
}

#[test]
fn recitals_no_body_heading_warning() {
    let input = format!(
        "{}# Background\n\n1. A recital.\n\n1. ## Clause\n\n    1. Text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(doc.body_heading.is_none());
    let has_warning = doc.diagnostics.iter().any(|d| d.message.contains("no body heading"));
    assert!(has_warning, "Should warn about missing body heading when recitals present");
}

#[test]
fn no_recitals_backward_compatible() {
    let input = format!(
        "{}1. ## Clause One\n\n    1. Text here.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(doc.recitals.is_none());
    assert!(doc.body_heading.is_none());
    assert!(!doc.body.is_empty());
}

#[test]
fn recitals_defined_terms_validated() {
    let input = format!(
        "{}# Background\n\n1. The **Principal Agreement** means the main contract.\n\n# Operative Provisions\n\n1. ## Clause\n\n    1. Under the Principal Agreement, the parties agree.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    // "Principal Agreement" should not produce an unused-term warning
    let unused_warnings: Vec<_> = doc.diagnostics.iter()
        .filter(|d| d.message.contains("Principal Agreement") && d.message.contains("never used"))
        .collect();
    assert!(unused_warnings.is_empty(), "Principal Agreement should be found in body text");
}

