use lexicon_docx::model::*;
use lexicon_docx::style::{NumberingConvention, StyleConfig};

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

/// Helper to parse + resolve in one step (using Commonwealth convention by default).
fn parse_and_resolve(input: &str) -> Document {
    let mut doc = lexicon_docx::parse(input).unwrap();
    lexicon_docx::resolve(&mut doc, NumberingConvention::Commonwealth);
    doc
}

// ===========================================================================
// Front-matter parsing
// ===========================================================================

#[test]
fn frontmatter_basic_fields() {
    let doc = parse_and_resolve(MINIMAL);
    assert_eq!(doc.meta.title, "Test Agreement");
    assert_eq!(doc.meta.date, Some("2026-01-01".to_string()));
    assert_eq!(doc.meta.parties.len(), 2);
    assert_eq!(doc.meta.parties[0].name, Some("Alice".to_string()));
    assert_eq!(doc.meta.parties[0].role, "Buyer");
    assert_eq!(doc.meta.parties[1].name, Some("Bob".to_string()));
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
    assert_eq!(
        doc.meta.parties[0].specifier.as_deref(),
        Some("ACN 123 456 789")
    );
    assert_eq!(
        doc.meta.parties[0].entity_type.as_deref(),
        Some("au-company")
    );
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
    for (yaml_val, expected) in [
        ("draft", Status::Draft),
        ("final", Status::Final),
        ("executed", Status::Executed),
    ] {
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
    let errors: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("not a valid YYYY-MM-DD"))
        .collect();
    assert_eq!(errors.len(), 1);
}

#[test]
fn frontmatter_missing_parties() {
    let input = "---\ntitle: T\ndate: 2026-01-01\nparties: []\n---\n";
    let doc = parse_and_resolve(input);
    let errors: Vec<_> = doc
        .diagnostics
        .iter()
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
    assert!(matches!(sub.number, Some(ClauseNumber::SubClause(1, 1, 1))));

    let subsub = first_child_clause(sub).expect("Expected sub-sub-clause");
    assert!(matches!(
        subsub.number,
        Some(ClauseNumber::SubSubClause(1, 1, 1, 1))
    ));
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
fn clause_number_full_reference_commonwealth() {
    let c = NumberingConvention::Commonwealth;
    assert_eq!(
        ClauseNumber::TopLevel(3).full_reference("clause", c),
        "clause 3"
    );
    assert_eq!(
        ClauseNumber::Clause(2, 5).full_reference("clause", c),
        "clause 2.5"
    );
    assert_eq!(
        ClauseNumber::SubClause(1, 2, 3).full_reference("clause", c),
        "clause 1.2(c)"
    );
    assert_eq!(
        ClauseNumber::SubSubClause(1, 2, 1, 2).full_reference("clause", c),
        "clause 1.2(a)(ii)"
    );
}

#[test]
fn clause_number_full_reference_decimal() {
    let c = NumberingConvention::Decimal;
    assert_eq!(
        ClauseNumber::TopLevel(3).full_reference("clause", c),
        "clause 3"
    );
    assert_eq!(
        ClauseNumber::Clause(2, 5).full_reference("clause", c),
        "clause 2.5"
    );
    assert_eq!(
        ClauseNumber::SubClause(1, 2, 3).full_reference("clause", c),
        "clause 1.2.3"
    );
    assert_eq!(
        ClauseNumber::SubSubClause(1, 2, 1, 2).full_reference("clause", c),
        "clause 1.2.1.2"
    );
    assert_eq!(
        ClauseNumber::Paragraph(1, 2, 1, 2, 3).full_reference("clause", c),
        "clause 1.2.1.2.3"
    );
    assert_eq!(
        ClauseNumber::SubParagraph(1, 2, 1, 2, 3, 4).full_reference("clause", c),
        "clause 1.2.1.2.3.4"
    );
}

#[test]
fn clause_number_full_reference_us_traditional() {
    let c = NumberingConvention::UsTraditional;
    assert_eq!(
        ClauseNumber::TopLevel(3).full_reference("clause", c),
        "clause III"
    );
    assert_eq!(
        ClauseNumber::Clause(2, 5).full_reference("clause", c),
        "clause II.E"
    );
    assert_eq!(
        ClauseNumber::SubClause(1, 2, 3).full_reference("clause", c),
        "clause I.B.3"
    );
    assert_eq!(
        ClauseNumber::SubSubClause(1, 2, 1, 2).full_reference("clause", c),
        "clause I.B.1.b"
    );
    assert_eq!(
        ClauseNumber::Paragraph(1, 2, 1, 2, 3).full_reference("clause", c),
        "clause I.B.1.b(3)"
    );
    assert_eq!(
        ClauseNumber::SubParagraph(1, 2, 1, 2, 3, 4).full_reference("clause", c),
        "clause I.B.1.b(3)(d)"
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
    let has_bold = inlines
        .iter()
        .any(|i| matches!(i, InlineContent::Bold(t) if t == "Agreement"));
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
    let has_italic = inlines
        .iter()
        .any(|i| matches!(i, InlineContent::Italic(t) if t == "important"));
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
    let xref = inlines
        .iter()
        .find(|i| matches!(i, InlineContent::CrossRef { .. }));
    assert!(xref.is_some(), "Expected CrossRef in {:?}", inlines);

    if let Some(InlineContent::CrossRef {
        resolved,
        anchor_id,
        ..
    }) = xref
    {
        assert_eq!(anchor_id, "obligations");
        assert!(resolved.is_some(), "Cross-reference should be resolved");
        assert!(
            resolved.as_ref().unwrap().contains("2"),
            "Should reference clause 2"
        );
    }
}

#[test]
fn broken_cross_reference_produces_warning() {
    let input = format!(
        "{}\n1. ## Clause\n\n    1. See [clause X](#nonexistent).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("nonexistent"))
        .collect();
    assert!(
        !warnings.is_empty(),
        "Expected warning about broken cross-ref"
    );
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

    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Seller") && d.message.contains("never used"))
        .collect();
    assert!(
        !warnings.is_empty(),
        "Expected warning about unused 'Seller' role: {:?}",
        doc.diagnostics
    );
}

#[test]
fn defined_term_used_later_no_warning() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Service** means the hosted platform.\n\n1. ## Scope\n\n    1. The Service shall be available.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let term_warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Service"))
        .collect();
    assert!(
        term_warnings.is_empty(),
        "Should not warn about defined+used term: {:?}",
        term_warnings
    );
}

#[test]
fn party_role_not_flagged_as_undefined() {
    let input = format!(
        "{}\n1. ## Obligations\n\n    1. The **Buyer** shall pay.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let buyer_warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Buyer"))
        .collect();
    assert!(
        buyer_warnings.is_empty(),
        "Party role should not be flagged: {:?}",
        buyer_warnings
    );
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
    let amount = doc
        .schedule_items
        .iter()
        .find(|i| i.term == "Amount")
        .unwrap();
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
    let input = format!("{}# ADDENDUM\n\nContent here.\n", MINIMAL);
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
    assert!(
        !heading_text.contains("{#"),
        "Anchor should be stripped from heading text"
    );
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
    let has_table = doc.addenda[0]
        .content
        .iter()
        .any(|c| matches!(c, AddendumContent::Table(_)));
    assert!(
        has_table,
        "Expected a table in addendum content: {:?}",
        doc.addenda[0].content
    );
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
    lexicon_docx::resolve(&mut doc, NumberingConvention::Commonwealth);

    let style = StyleConfig::default();
    let bytes = lexicon_docx::render_docx(&doc, &style, None, &[]).unwrap();

    // DOCX files are ZIP archives starting with PK magic bytes
    assert!(bytes.len() > 100, "DOCX output should be non-trivial");
    assert_eq!(&bytes[0..2], b"PK", "DOCX should be a valid ZIP archive");
}

#[test]
fn process_convenience_function_works() {
    let input = format!("{}\n1. ## Clause\n\n    1. Text.\n", MINIMAL);
    let style = StyleConfig::default();
    let (bytes, diagnostics) = lexicon_docx::process(&input, &style, None, None).unwrap();

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
    let (bytes, _) = lexicon_docx::process(input, &style, None, None).unwrap();

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
    assert!(
        found_draft,
        "Draft watermark should be present in header XML"
    );
}

// ===========================================================================
// DOCX structural verification
// ===========================================================================

/// Helper: build a DOCX from input + style, return document.xml as a String.
fn build_and_read_document_xml(input: &str, style: &StyleConfig) -> String {
    let (bytes, _) = lexicon_docx::process(input, style, None, None).unwrap();
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut archive.by_name("word/document.xml").unwrap(), &mut xml)
        .unwrap();
    xml
}

#[test]
fn docx_contains_clause_text() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Service** means the hosted platform.\n",
        MINIMAL
    );
    let xml = build_and_read_document_xml(&input, &StyleConfig::default());
    assert!(
        xml.contains("Service"),
        "document.xml should contain clause text"
    );
    assert!(
        xml.contains("hosted platform"),
        "document.xml should contain definition text"
    );
}

#[test]
fn docx_contains_title() {
    let xml = build_and_read_document_xml(MINIMAL, &StyleConfig::default());
    assert!(
        xml.contains("Test Agreement"),
        "document.xml should contain the document title"
    );
}

#[test]
fn docx_cover_page_present_by_default() {
    let input = format!("{}\n1. ## Clause\n\n    1. Text.\n", MINIMAL);
    let xml = build_and_read_document_xml(&input, &StyleConfig::default());
    // Cover page renders party names and "between" label
    assert!(
        xml.contains("Alice"),
        "Cover page should contain party name"
    );
    assert!(xml.contains("Bob"), "Cover page should contain party name");
}

#[test]
fn docx_cover_page_absent_when_disabled() {
    let input = format!("{}\n1. ## Clause\n\n    1. Text.\n", MINIMAL);
    let mut style = StyleConfig::default();
    style.cover.enabled = false;
    let xml = build_and_read_document_xml(&input, &style);
    // Title should still appear (inline title), but no "between" label
    assert!(
        xml.contains("Test Agreement"),
        "Title should still appear inline"
    );
    assert!(
        !xml.contains(&style.cover.between_label),
        "Between label should not appear when cover is disabled"
    );
}

#[test]
fn docx_toc_contains_heading_text() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. Text.\n\n1. ## Obligations\n\n    1. Text.\n",
        MINIMAL
    );
    let xml = build_and_read_document_xml(&input, &StyleConfig::default());
    // TOC items should contain the heading text
    assert!(
        xml.contains("Definitions"),
        "TOC should include 'Definitions' heading"
    );
    assert!(
        xml.contains("Obligations"),
        "TOC should include 'Obligations' heading"
    );
}

#[test]
fn docx_toc_absent_when_disabled() {
    let input = format!("{}\n1. ## Definitions\n\n    1. Text.\n", MINIMAL);
    let mut style = StyleConfig::default();
    style.toc.enabled = false;
    let xml = build_and_read_document_xml(&input, &style);
    // When TOC is disabled, there should be no TOC instruction field
    assert!(
        !xml.contains("w:instrText"),
        "No TOC field instruction when TOC is disabled"
    );
}

#[test]
fn docx_cross_reference_creates_bookmark() {
    let input = format!(
        "{}\n1. ## Terms {{#terms}}\n\n    1. See [clause 2](#obligations).\n\n1. ## Obligations {{#obligations}}\n\n    1. The Buyer shall pay.\n",
        MINIMAL
    );
    let xml = build_and_read_document_xml(&input, &StyleConfig::default());
    // Bookmarks for anchors
    assert!(
        xml.contains("w:bookmarkStart"),
        "Should contain bookmark start elements"
    );
    assert!(
        xml.contains("lx_obligations"),
        "Should contain bookmark for #obligations anchor"
    );
}

#[test]
fn docx_cross_reference_creates_hyperlink() {
    let input = format!(
        "{}\n1. ## Terms {{#terms}}\n\n    1. See [clause 2](#obligations).\n\n1. ## Obligations {{#obligations}}\n\n    1. The Buyer shall pay.\n",
        MINIMAL
    );
    let xml = build_and_read_document_xml(&input, &StyleConfig::default());
    // Internal hyperlink to the bookmark
    assert!(
        xml.contains("w:hyperlink") && xml.contains("lx_obligations"),
        "Should contain hyperlink to bookmark"
    );
}

#[test]
fn docx_numbering_references_present() {
    let input = format!("{}\n1. ## Clause\n\n    1. Text.\n", MINIMAL);
    let xml = build_and_read_document_xml(&input, &StyleConfig::default());
    // Clauses should reference a numbering ID
    assert!(
        xml.contains("w:numId"),
        "Clause paragraphs should reference Word numbering"
    );
}

#[test]
fn docx_schedule_renders_at_end_by_default() {
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

    1. **Payment** has the meaning given by the Schedule.

1. ## Body

    1. Body clause content here.
"#;
    let xml = build_and_read_document_xml(input, &StyleConfig::default());
    // Schedule heading should appear
    assert!(
        xml.contains("SCHEDULE"),
        "Schedule heading should appear in output"
    );
    // "Payment" should appear as a schedule item
    assert!(
        xml.contains("Payment"),
        "Schedule item 'Payment' should appear"
    );
    // Schedule table (with "Particulars" header) should come after body text.
    // We check for the table header rather than "SCHEDULE" because the heading
    // also appears in the TOC (before the body).
    let body_pos = xml
        .find("Body clause content here")
        .expect("Body text should appear in document.xml");
    let schedule_table_pos = xml
        .find("Particulars")
        .expect("Schedule table header should appear in document.xml");
    assert!(
        schedule_table_pos > body_pos,
        "Schedule table should appear after body clauses (default: end)"
    );
}

#[test]
fn docx_addendum_renders_with_heading() {
    let input = format!(
        "{}# ADDENDUM - Processing Details\n\nAddendum content here.\n",
        MINIMAL
    );
    let xml = build_and_read_document_xml(&input, &StyleConfig::default());
    // Heading is fully uppercased by the renderer
    assert!(
        xml.contains("ADDENDUM 1"),
        "Addendum heading should appear with number"
    );
    assert!(
        xml.contains("PROCESSING DETAILS"),
        "Addendum title should appear (uppercased)"
    );
    assert!(
        xml.contains("Addendum content here"),
        "Addendum body should appear"
    );
}

#[test]
fn docx_addendum_subheading_uses_native_spacing_not_blank_paragraph() {
    // An addendum sub-heading (## Item) should carry Word-native heading spacing
    // (space before to separate it from preceding content, space after to bind it
    // to its own content) instead of a trailing blank paragraph, which produced a
    // large, lopsided gap. See render/addendum.rs.
    let input = format!(
        "{}# ADDENDUM - Intellectual Property\n\n## Item 1 — Trade marks\n\nNone.\n",
        MINIMAL
    );
    let mut style = StyleConfig::default();
    // Isolate the addendum sub-heading: the TOC heading also carries heading
    // spacing, and the cover page emits its own blank layout paragraphs.
    style.toc.enabled = false;
    style.cover.enabled = false;
    let xml = build_and_read_document_xml(&input, &style);

    let before = StyleConfig::pt_to_twips(style.heading_space_before);
    let after = StyleConfig::pt_to_twips(style.heading_space_after);
    let spacing = format!("w:before=\"{}\" w:after=\"{}\"", before, after);
    assert!(
        xml.contains(&spacing),
        "Addendum sub-heading should carry Word-native heading spacing ({spacing})"
    );

    // Regression: no blank spacer paragraph should sit between the sub-heading and
    // its content. Checked region-locally so unrelated blank paragraphs elsewhere
    // (e.g. signature blocks) don't mask the result.
    let heading_pos = xml
        .find("Item 1 — Trade marks")
        .expect("sub-heading present");
    let content_pos = xml.find("None.").expect("content present");
    let between = &xml[heading_pos..content_pos];
    assert!(
        !between.contains("<w:pPr><w:rPr /></w:pPr></w:p>"),
        "No blank paragraph should sit between the addendum sub-heading and its content"
    );
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
// Defined term plural matching (spec 4.4.3)
// ===========================================================================

#[test]
fn plural_s_suffix_no_warning() {
    // "Member" defined, "Members" used — "member" is a substring of "members"
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Member** means any person who has joined.\n\n1. ## Scope\n\n    1. All Members must comply with the rules.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Member") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "'Member' should be found as substring of 'Members': {:?}",
        warnings
    );
}

#[test]
fn plural_s_substring_match() {
    // "Agreement" is a substring of "Agreements" — simple plurals work via substring matching
    let input = format!(
        "{}\n1. ## Scope\n\n    1. All prior Agreements are superseded.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Agreement") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "'Agreement' should be found as substring of 'Agreements': {:?}",
        warnings
    );
}

#[test]
fn plural_ies_suffix_no_warning() {
    // "Authority" → "Authorities" matched via forward plural generation
    // (consonant + y → ies, per spec 4.4.3)
    let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Authority
  - name: Bob
    role: Buyer
---

1. ## Scope

    1. All relevant Authorities must approve the transaction.
"#;
    let doc = parse_and_resolve(input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Authority") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "'Authorities' should match 'Authority' via ies plural rule: {:?}",
        warnings
    );
}

#[test]
fn plural_es_suffix_no_warning() {
    // "Business" → "Businesses" (es suffix)
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Business** means the company's operations.\n\n1. ## Scope\n\n    1. The Buyer shall manage all Businesses.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Business") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "'Businesses' should match 'Business' via es suffix: {:?}",
        warnings
    );
}

#[test]
fn possessive_form_no_warning() {
    // "Employer" used as "Employer's" — possessive should still match
    let input = r#"---
title: Test
date: 2026-01-01
parties:
  - name: Alice
    role: Employer
  - name: Bob
    role: Employee
---

1. ## Scope

    1. The Employee shall follow the Employer's instructions.
"#;
    let doc = parse_and_resolve(input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Employer") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "Possessive form should match: {:?}",
        warnings
    );
}

// ===========================================================================
// Defined term longest match (spec 4.4.2)
// ===========================================================================

#[test]
fn longest_match_both_terms_found() {
    // Both "Merchant" and "Merchant Data" defined; text uses both.
    // Neither should produce an "unused" warning.
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Merchant** means the retailer.\n\n    1. **Merchant Data** means data belonging to the Merchant.\n\n1. ## Scope\n\n    1. The Buyer shall protect all Merchant Data received from the Merchant.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let merchant_warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Merchant") && d.message.contains("never used"))
        .collect();
    assert!(
        merchant_warnings.is_empty(),
        "Both 'Merchant' and 'Merchant Data' should be found in text: {:?}",
        merchant_warnings
    );
}

#[test]
fn prefix_term_not_consumed_by_longer_term() {
    // "Merchant Data" appears but "Merchant" alone does not (outside of "Merchant Data").
    // The substring matching means "merchant" IS found inside "merchant data", so no warning.
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Merchant** means the retailer.\n\n    1. **Merchant Data** means data belonging to the Merchant.\n\n1. ## Scope\n\n    1. The Buyer shall protect all Merchant Data.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    // With substring matching, "merchant" appears inside "merchant data" — no warning
    let merchant_warning: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("'Merchant'") && d.message.contains("never used"))
        .collect();
    assert!(
        merchant_warning.is_empty(),
        "'Merchant' substring is present within 'Merchant Data': {:?}",
        merchant_warning
    );
}

// ===========================================================================
// Inline definitions (spec 4.3)
// ===========================================================================

#[test]
fn inline_definition_parenthetical() {
    // ("**Term**") pattern
    let input = format!(
        "{}\n1. ## Scope\n\n    1. The Buyer agrees to pay (the \"**Payment**\") to the Seller.\n\n1. ## Terms\n\n    1. The Payment shall be made in full.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Payment") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "Inline definition should be recognised and 'Payment' found in usage: {:?}",
        warnings
    );
}

#[test]
fn inline_definition_without_the() {
    // ("**Term**") without "the"
    let input = format!(
        "{}\n1. ## Scope\n\n    1. The parties to this agreement (\"**Parties**\") agree as follows.\n\n1. ## Terms\n\n    1. The Parties shall cooperate.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Parties") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "Inline definition without 'the' should be recognised: {:?}",
        warnings
    );
}

// ===========================================================================
// Multiple siblings at same level
// ===========================================================================

#[test]
fn multiple_sub_clauses_lettered_correctly() {
    let input = format!(
        "{}\n1. ## Termination\n\n    1. A party may terminate if:\n\n        1. the other party breaches; or\n\n        2. the other party is insolvent; or\n\n        3. mutual agreement.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let child = first_child_clause(top).expect("Expected child clause");
    let sub_clauses = collect_children(child);

    assert_eq!(sub_clauses.len(), 3, "Expected 3 sub-clauses");
    assert!(matches!(
        sub_clauses[0].number,
        Some(ClauseNumber::SubClause(1, 1, 1))
    ));
    assert!(matches!(
        sub_clauses[1].number,
        Some(ClauseNumber::SubClause(1, 1, 2))
    ));
    assert!(matches!(
        sub_clauses[2].number,
        Some(ClauseNumber::SubClause(1, 1, 3))
    ));
}

#[test]
fn multiple_clause_level_siblings() {
    let input = format!(
        "{}\n1. ## Obligations\n\n    1. First obligation.\n\n    2. Second obligation.\n\n    3. Third obligation.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let children = collect_children(top);

    assert_eq!(children.len(), 3, "Expected 3 clause-level children");
    assert!(matches!(
        children[0].number,
        Some(ClauseNumber::Clause(1, 1))
    ));
    assert!(matches!(
        children[1].number,
        Some(ClauseNumber::Clause(1, 2))
    ));
    assert!(matches!(
        children[2].number,
        Some(ClauseNumber::Clause(1, 3))
    ));
}

#[test]
fn sub_clause_numbering_resets_per_parent() {
    // Two clause-level siblings each with sub-clauses — lettering should restart
    let input = format!(
        "{}\n1. ## Terms\n\n    1. First clause:\n\n        1. sub a;\n\n        2. sub b.\n\n    2. Second clause:\n\n        1. sub a;\n\n        2. sub b;\n\n        3. sub c.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let clauses = collect_children(top);
    assert_eq!(clauses.len(), 2);

    let subs_1 = collect_children(clauses[0]);
    assert_eq!(subs_1.len(), 2);
    assert!(matches!(
        subs_1[0].number,
        Some(ClauseNumber::SubClause(1, 1, 1))
    ));
    assert!(matches!(
        subs_1[1].number,
        Some(ClauseNumber::SubClause(1, 1, 2))
    ));

    let subs_2 = collect_children(clauses[1]);
    assert_eq!(subs_2.len(), 3);
    assert!(matches!(
        subs_2[0].number,
        Some(ClauseNumber::SubClause(1, 2, 1))
    ));
    assert!(matches!(
        subs_2[1].number,
        Some(ClauseNumber::SubClause(1, 2, 2))
    ));
    assert!(matches!(
        subs_2[2].number,
        Some(ClauseNumber::SubClause(1, 2, 3))
    ));
}

// ===========================================================================
// Continuation paragraphs in clauses
// ===========================================================================

#[test]
fn continuation_paragraph_after_sub_clauses() {
    let input = format!(
        "{}\n1. ## Terms\n\n    1. The Buyer must:\n\n        1. deliver the goods; and\n\n        2. pay the invoice.\n\n       Nothing in this clause limits liability.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let child = first_child_clause(top).expect("Expected child clause");

    // Should have content (intro paragraph), children (sub-clauses), and content (continuation)
    let mut has_children = false;
    let mut content_count = 0;
    for element in &child.body {
        match element {
            ClauseBody::Content(_) => content_count += 1,
            ClauseBody::Children(_) => has_children = true,
        }
    }
    assert!(has_children, "Should have children (sub-clauses)");
    assert!(
        content_count >= 2,
        "Should have at least 2 content blocks (intro + continuation), got {}",
        content_count
    );
}

#[test]
fn multiple_paragraphs_in_clause() {
    let input = format!(
        "{}\n1. ## Terms\n\n    1. First paragraph of the clause.\n\n       Second paragraph of the clause.\n\n       Third paragraph of the clause.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let child = first_child_clause(top).expect("Expected child clause");

    let para_count = child
        .body
        .iter()
        .filter(|e| matches!(e, ClauseBody::Content(ClauseContent::Paragraph(_))))
        .count();
    assert_eq!(para_count, 3, "Expected 3 paragraphs in the clause");
}

// ===========================================================================
// Tables inside clauses
// ===========================================================================

#[test]
fn table_inside_clause() {
    let input = format!(
        "{}\n1. ## Fees\n\n    1. The following fees apply:\n\n       | Service | Rate |\n       |---------|------|\n       | Basic   | $100 |\n       | Premium | $200 |\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let child = first_child_clause(top).expect("Expected child clause");

    let has_table = child
        .body
        .iter()
        .any(|e| matches!(e, ClauseBody::Content(ClauseContent::Table(_))));
    assert!(has_table, "Expected a table inside the clause body");
}

#[test]
fn table_inside_clause_has_correct_structure() {
    let input = format!(
        "{}\n1. ## Fees\n\n    1. Rates:\n\n       | Item | Cost |\n       |------|------|\n       | A    | $10  |\n       | B    | $20  |\n       | C    | $30  |\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let child = first_child_clause(top).unwrap();

    let table = child.body.iter().find_map(|e| {
        if let ClauseBody::Content(ClauseContent::Table(t)) = e {
            Some(t)
        } else {
            None
        }
    });
    let table = table.expect("Expected table in clause");
    assert_eq!(table.headers.len(), 2, "Expected 2 header columns");
    assert_eq!(table.rows.len(), 3, "Expected 3 data rows");
}

// ===========================================================================
// Blockquotes inside clauses
// ===========================================================================

#[test]
fn blockquote_inside_clause() {
    let input = format!(
        "{}\n1. ## Fees\n\n    1. The fee is calculated as follows:\n\n       > (2 x A) - C\n       >\n       > Where A = monthly rent\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let child = first_child_clause(top).expect("Expected child clause");

    let has_blockquote = child
        .body
        .iter()
        .any(|e| matches!(e, ClauseBody::Content(ClauseContent::Blockquote(_))));
    assert!(has_blockquote, "Expected a blockquote inside the clause");
}

#[test]
fn blockquote_content_extracted() {
    let input = format!(
        "{}\n1. ## Formula\n\n    1. Calculate as:\n\n       > Total = X + Y\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let top = match &doc.body[0] {
        BodyElement::Clause(c) => c,
        _ => panic!("Expected top-level clause"),
    };
    let child = first_child_clause(top).unwrap();

    let bq_text: String = child
        .body
        .iter()
        .filter_map(|e| {
            if let ClauseBody::Content(ClauseContent::Blockquote(inlines)) = e {
                Some(
                    inlines
                        .iter()
                        .map(|i| i.as_plain_text())
                        .collect::<String>(),
                )
            } else {
                None
            }
        })
        .collect();
    assert!(
        bq_text.contains("Total = X + Y"),
        "Blockquote should contain formula text, got: {:?}",
        bq_text
    );
}

// ===========================================================================
// doc_type as auto-defined term
// ===========================================================================

#[test]
fn doc_type_as_defined_term_no_warning() {
    let input = r#"---
title: Deed of Release
type: Deed
date: 2026-01-01
parties:
  - name: Alice
    role: Buyer
  - name: Bob
    role: Seller
---

1. ## Scope

    1. This Deed is entered into by the Buyer and the Seller.
"#;
    let doc = parse_and_resolve(input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Deed") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "'Deed' from type field should be auto-defined and found in text: {:?}",
        warnings
    );
}

#[test]
fn default_agreement_type_auto_defined() {
    // When type is omitted, "Agreement" is the default auto-defined term
    let input = format!(
        "{}\n1. ## Scope\n\n    1. This Agreement is binding on both the Buyer and the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Agreement") && d.message.contains("never used"))
        .collect();
    assert!(
        warnings.is_empty(),
        "Default 'Agreement' type should be auto-defined: {:?}",
        warnings
    );
}

#[test]
fn unused_doc_type_produces_warning() {
    // If the doc type is never used in the body, it should warn
    let input = r#"---
title: Release
type: Deed
date: 2026-01-01
parties:
  - name: Alice
    role: Buyer
  - name: Bob
    role: Seller
---

1. ## Scope

    1. The Buyer shall pay the Seller.
"#;
    let doc = parse_and_resolve(input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Deed") && d.message.contains("never used"))
        .collect();
    assert!(
        !warnings.is_empty(),
        "'Deed' type is defined but never used — should warn: {:?}",
        doc.diagnostics
    );
}

// ===========================================================================
// Helpers
// ===========================================================================

/// Collect all direct child clauses from a parent clause.
fn collect_children(clause: &Clause) -> Vec<&Clause> {
    let mut children = Vec::new();
    for element in &clause.body {
        if let ClauseBody::Children(kids) = element {
            children.extend(kids.iter());
        }
    }
    children
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
    let clauses: Vec<_> = recitals
        .body
        .iter()
        .filter_map(|e| {
            if let BodyElement::Clause(c) = e {
                Some(c)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(clauses.len(), 2);
    assert!(matches!(clauses[0].number, Some(ClauseNumber::TopLevel(1))));
    assert!(matches!(clauses[1].number, Some(ClauseNumber::TopLevel(2))));

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
    let prose_count = recitals
        .body
        .iter()
        .filter(|e| matches!(e, BodyElement::Prose(_)))
        .count();
    assert_eq!(prose_count, 1);
}

#[test]
fn recitals_cross_reference() {
    let input = format!(
        "{}# Background\n\n1. The background to this agreement. {{#bg}}\n\n# Operative Provisions\n\n1. ## Clause\n\n    1. See [Recital 1](#bg).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    // Check the cross-reference resolved
    if let Some(BodyElement::Clause(clause)) = doc.body.first() {
        let child = first_child_clause(clause).unwrap();
        let has_resolved = child.body.iter().any(|e| {
            if let ClauseBody::Content(ClauseContent::Paragraph(inlines)) = e {
                inlines.iter().any(|i| matches!(i, InlineContent::CrossRef { resolved: Some(r), .. } if r == "Recital 1"))
            } else {
                false
            }
        });
        assert!(
            has_resolved,
            "Cross-reference to recital should resolve to 'Recital 1'"
        );
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
    let has_warning = doc
        .diagnostics
        .iter()
        .any(|d| d.message.contains("no body heading"));
    assert!(
        has_warning,
        "Should warn about missing body heading when recitals present"
    );
}

#[test]
fn no_recitals_backward_compatible() {
    let input = format!("{}1. ## Clause One\n\n    1. Text here.\n", MINIMAL);
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
    let unused_warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Principal Agreement") && d.message.contains("never used"))
        .collect();
    assert!(
        unused_warnings.is_empty(),
        "Principal Agreement should be found in body text"
    );
}

// ===========================================================================
// Bullet point fallback (spec 3.10)
// ===========================================================================

#[test]
fn bullet_at_top_of_body_captured_with_warning() {
    let input = format!(
        "{}* first bullet\n* second bullet\n\n1. ## Clause One\n\n    1. Text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    // First body element should be the bullet list, second the clause.
    assert!(matches!(doc.body[0], BodyElement::BulletList(_)));
    if let BodyElement::BulletList(items) = &doc.body[0] {
        assert_eq!(items.len(), 2);
    }
    assert!(matches!(doc.body[1], BodyElement::Clause(_)));

    // Warning emitted with body location.
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Bullet point"))
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].location.as_deref(), Some("document body"));

    // Bullet list does not consume a clause number — the following clause is 1.
    if let BodyElement::Clause(c) = &doc.body[1] {
        assert!(matches!(c.number, Some(ClauseNumber::TopLevel(1))));
    }
}

#[test]
fn bullet_inside_clause_captured_as_clause_content() {
    let input = format!(
        "{}1. ## Definitions\n\n    1. The list of accepted methods includes:\n\n        * email\n        * post\n        * fax\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    // Find the bullet list inside the clause hierarchy.
    let mut found = false;
    if let BodyElement::Clause(top) = &doc.body[0] {
        for element in &top.body {
            if let ClauseBody::Children(kids) = element {
                for kid in kids {
                    for inner in &kid.body {
                        if let ClauseBody::Content(ClauseContent::BulletList(items)) = inner {
                            assert_eq!(items.len(), 3);
                            found = true;
                        }
                    }
                }
            }
        }
    }
    assert!(
        found,
        "Expected a ClauseContent::BulletList inside the clause body"
    );

    // Warning emitted with a clause-context location.
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Bullet point inside clause body"))
        .collect();
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0]
            .location
            .as_deref()
            .map(|s| s.starts_with("clause"))
            .unwrap_or(false)
    );
}

#[test]
fn bullet_in_recitals_captured_with_warning() {
    let input = format!(
        "{}# Background\n\n* this bullet is in recitals\n\n# Operative Provisions\n\n1. ## Clause\n\n    1. Some text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    let recitals = doc.recitals.as_ref().expect("recitals should be present");
    assert!(
        recitals
            .body
            .iter()
            .any(|e| matches!(e, BodyElement::BulletList(_)))
    );

    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Bullet point in recitals"))
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].location.as_deref(), Some("recitals"));
}

#[test]
fn bullet_inside_clause_does_not_break_sibling_numbering() {
    let input = format!(
        "{}1. ## First Clause\n\n    1. Intro text:\n\n        * a bullet\n        * another bullet\n\n    1. Following sub-clause text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);

    // The clause numbering for the second sub-clause should still be 1.2.
    if let BodyElement::Clause(top) = &doc.body[0] {
        let mut subs = Vec::new();
        for element in &top.body {
            if let ClauseBody::Children(kids) = element {
                for kid in kids {
                    subs.push(kid);
                }
            }
        }
        assert_eq!(subs.len(), 2, "Two sub-clauses expected");
        assert!(matches!(subs[0].number, Some(ClauseNumber::Clause(1, 1))));
        assert!(matches!(subs[1].number, Some(ClauseNumber::Clause(1, 2))));
    } else {
        panic!("Expected a clause at body[0]");
    }
}

#[test]
fn bullet_in_addenda_still_works_as_addendum_content() {
    let input = format!(
        "{}1. ## Body Clause\n\n    1. Some text.\n\n# Addendum 1 - Notes\n\n* alpha\n* beta\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert_eq!(doc.addenda.len(), 1);
    let has_bullet = doc.addenda[0]
        .content
        .iter()
        .any(|c| matches!(c, AddendumContent::BulletList(_)));
    assert!(has_bullet, "Addendum should still hold its bullet list");

    // No "Bullet point" warning should be emitted for addendum bullets.
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Bullet point"))
        .collect();
    assert!(
        warnings.is_empty(),
        "Bullets inside addenda should not warn: {:?}",
        warnings
    );
}

#[test]
fn bullet_with_dash_marker_handled_the_same_as_asterisk() {
    let input = format!(
        "{}- dash bullet one\n- dash bullet two\n\n1. ## Clause\n\n    1. Text.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(matches!(doc.body[0], BodyElement::BulletList(_)));
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.message.contains("Bullet point"))
        .collect();
    assert_eq!(warnings.len(), 1);
}

// ===========================================================================
// Linter rules (resolve-level drafting checks)
// ===========================================================================

#[test]
fn duplicate_anchor_produces_warning() {
    let input = format!(
        "{}\n1. ## Definitions {{#defs}}\n\n    1. Text.\n\n1. ## Scope {{#defs}}\n\n    1. More text referencing [clause 1](#defs).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.code == "duplicate-anchor")
        .collect();
    assert_eq!(warnings.len(), 1, "diagnostics: {:?}", doc.diagnostics);
    assert!(warnings[0].message.contains("#defs"));
    assert!(warnings[0].line.is_some());
}

#[test]
fn duplicate_definition_produces_warning() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Service** means the hosted platform.\n\n1. ## Scope\n\n    1. The **Service** shall be available at all times.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.code == "duplicate-definition")
        .collect();
    assert_eq!(warnings.len(), 1, "diagnostics: {:?}", doc.diagnostics);
    assert!(warnings[0].message.contains("Service"));
    // The bolded second occurrence counts as an appearance, so no
    // unused-term warning should accompany the duplicate.
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.code == "unused-term" && d.message.contains("Service"))
    );
}

#[test]
fn role_redefined_in_body_not_flagged_as_duplicate() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Buyer** means the party purchasing the goods.\n\n1. ## Scope\n\n    1. The Buyer shall pay.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.code == "duplicate-definition"),
        "front-matter roles may be formally re-defined in the body: {:?}",
        doc.diagnostics
    );
}

#[test]
fn undeclared_schedule_reference_produces_warning() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Fee** has the meaning given by the Payment Schedule.\n\n    2. The Buyer pays the Fee to the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warnings: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.code == "undeclared-schedule")
        .collect();
    assert_eq!(warnings.len(), 1, "diagnostics: {:?}", doc.diagnostics);
    assert!(warnings[0].message.contains("Payment Schedule"));
}

#[test]
fn declared_schedule_reference_produces_no_warning() {
    let input = "---\ntitle: Test\ndate: 2026-01-01\nparties:\n  - name: Alice\n    role: Buyer\n  - name: Bob\n    role: Seller\nschedule:\n  - title: Payment Schedule\n---\n\n1. ## Definitions\n\n    1. **Fee** has the meaning given by the Payment Schedule.\n\n    2. The Buyer pays the Fee to the Seller.\n";
    let doc = parse_and_resolve(input);
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.code == "undeclared-schedule"),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn unused_anchor_produces_info() {
    let input = format!(
        "{}\n1. ## Definitions {{#defs}}\n\n    1. Text about the Buyer, the Seller, and this Test Agreement.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let infos: Vec<_> = doc
        .diagnostics
        .iter()
        .filter(|d| d.code == "unused-anchor")
        .collect();
    assert_eq!(infos.len(), 1, "diagnostics: {:?}", doc.diagnostics);
    assert!(matches!(
        infos[0].level,
        lexicon_docx::error::DiagLevel::Info
    ));
}

#[test]
fn referenced_anchor_not_flagged_unused() {
    let input = format!(
        "{}\n1. ## Definitions {{#defs}}\n\n    1. See [clause 1](#defs).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        !doc.diagnostics.iter().any(|d| d.code == "unused-anchor"),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn broken_cross_ref_has_line_number() {
    let input = format!(
        "{}\n1. ## Scope\n\n    1. See [clause 9](#nowhere).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    let warning = doc
        .diagnostics
        .iter()
        .find(|d| d.code == "broken-cross-ref")
        .expect("expected broken-cross-ref warning");
    // MINIMAL has 8 front-matter lines; the clause item is 2 lines below the
    // heading list item which starts on line 10.
    assert!(warning.line.is_some());
    assert!(warning.line.unwrap() > 8, "line: {:?}", warning.line);
}

#[test]
fn unused_term_flagged_even_though_definition_mentions_it() {
    // The bold definition site itself must not count as a usage.
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Gadget** means a device. The Buyer and Seller sign this Test Agreement.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        doc.diagnostics
            .iter()
            .any(|d| d.code == "unused-term" && d.message.contains("Gadget")),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

// ===========================================================================
// Red-team regression tests
// ===========================================================================

#[test]
fn bom_prefixed_document_parses() {
    let input = format!(
        "\u{feff}{}\n1. ## Obligations\n\n    1. The Buyer pays the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert_eq!(doc.meta.title, "Test Agreement");
}

#[test]
fn statutory_schedule_reference_not_a_schedule_item() {
    let input = "---\ntitle: Test\ndate: 2026-01-01\nparties:\n  - name: A\n    role: Buyer\n  - name: B\n    role: Seller\nschedule:\n  - title: Schedule\n---\n\n1. ## Definitions\n\n    1. **Approved Form** means the form described in the Schedule to the Corporations Act 2001. The Buyer gives the Seller the Approved Form.\n\n    2. **Fee** is set out in the Schedule.\n";
    let doc = parse_and_resolve(input);
    // "Schedule to the Corporations Act" must not create a schedule item...
    assert!(
        !doc.schedule_items.iter().any(|i| i.term == "Approved Form"),
        "statutory reference misclassified: {:?}",
        doc.schedule_items
    );
    // ...while the genuine schedule reference still does.
    assert!(doc.schedule_items.iter().any(|i| i.term == "Fee"));
    // And no undeclared-schedule noise for the statutory reference.
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.code == "undeclared-schedule"),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn statutory_act_schedule_title_not_flagged_undeclared() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Levy** means the amount payable in accordance with the GST Act Schedule 2 provisions. The Buyer pays the Levy to the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.code == "undeclared-schedule"),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn plural_schedules_reference_flagged_undeclared() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Outgoings** means the amounts set out in the Schedules. The Buyer pays the Outgoings to the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        doc.diagnostics
            .iter()
            .any(|d| d.code == "undeclared-schedule"),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn short_declared_title_reference_warns_instead_of_silence() {
    // Declared "Schedule of Particulars"; text says just "the Schedule" —
    // no schedule item can be generated, so the linter must warn.
    let input = "---\ntitle: Test\ndate: 2026-01-01\nparties:\n  - name: A\n    role: Buyer\n  - name: B\n    role: Seller\nschedule:\n  - title: Schedule of Particulars\n---\n\n1. ## Definitions\n\n    1. **Duty** means the duty set out in the Schedule. The Buyer pays the Duty to the Seller.\n";
    let doc = parse_and_resolve(input);
    assert!(
        doc.diagnostics
            .iter()
            .any(|d| d.code == "undeclared-schedule"),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn colon_inside_bold_is_field_label() {
    let input = format!(
        "{}\n1. ## Details\n\n    1. **Position:** Senior Engineer, reporting to the Buyer and the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.message.contains("Position")),
        "field label flagged: {:?}",
        doc.diagnostics
    );
}

#[test]
fn implicit_agreement_doc_type_not_flagged() {
    // No `type:` in front-matter — the implicit default "Agreement" must not
    // produce an unused-term warning the drafter never wrote.
    let input = format!(
        "{}\n1. ## Obligations\n\n    1. The Buyer pays the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.message.contains("'Agreement'")),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn lowercase_usage_does_not_count_case_sensitive() {
    // Spec 4.4.1: lowercase "company" does not reference defined "Company".
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Company** means Acme Pty Ltd. The Buyer notifies the Seller and the company representative.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        doc.diagnostics
            .iter()
            .any(|d| d.code == "unused-term" && d.message.contains("Company")),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn substring_usage_does_not_count_word_boundary() {
    // "Act" inside "Contract" must not count as a use of defined "Act".
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Act** means the legislation described below. This Contract binds the Buyer and the Seller.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        doc.diagnostics
            .iter()
            .any(|d| d.code == "unused-term" && d.message.contains("'Act'")),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}

#[test]
fn addendum_redefinition_not_flagged_duplicate() {
    let input = format!(
        "{}\n1. ## Definitions\n\n    1. **Personal Data** means data about a person. The Buyer gives Personal Data to the Seller.\n\n# ADDENDUM - EU Terms\n\nFor the purposes of this Addendum, **Personal Data** means personal data as defined in the GDPR.\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        !doc.diagnostics
            .iter()
            .any(|d| d.code == "duplicate-definition"),
        "scoped addendum redefinition flagged: {:?}",
        doc.diagnostics
    );
}

#[test]
fn multiple_anchors_on_one_clause_warn() {
    let input = format!(
        "{}\n1. ## Payment\n\n    1. Invoices monthly. {{#invoice}}\n\n       Payment in 30 days. {{#payment-due}}\n\n    2. See [clause 1.1](#payment-due) and [clause 1.1](#invoice).\n",
        MINIMAL
    );
    let doc = parse_and_resolve(&input);
    assert!(
        doc.diagnostics.iter().any(|d| d.code == "multiple-anchors"),
        "diagnostics: {:?}",
        doc.diagnostics
    );
}
