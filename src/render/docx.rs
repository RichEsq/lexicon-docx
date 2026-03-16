use std::collections::HashMap;
use std::path::Path;

use docx_rs::{
    BreakType, Docx, Footer, Header, IndentLevel, LineSpacing, LineSpacingType, NumberingId,
    NumPages, Numbering, PageMargin, PageNum, Paragraph, Pic, Run, RunFonts, RunProperty, Style,
    StyleType, Tab, TabValueType, TableOfContents, TableOfContentsItem,
};

use crate::error::{LexiconError, Result};
use crate::model::*;
use crate::render::addendum::render_addendum;
use crate::render::common::{add_inline_run, bookmark_name, render_inlines_paragraph, render_table};
use crate::render::cover::{render_cover_page, render_inline_title};
use crate::render::exhibit::{self as exhibit_loader, PdfRenderer};
use crate::render::numbering::{
    create_clause_numbering, create_recital_numbering, create_simple_list_numbering,
    indent_for_level, numbering_level_for, outline_level_for, ABSTRACT_NUM_ID,
    BODY_NUMBERING_ID, RECITAL_ABSTRACT_NUM_ID, RECITAL_NUMBERING_ID,
};
use crate::render::preamble::render_preamble;
use crate::render::schedule::render_schedules;
use crate::render::signatures as sig_renderer;
use crate::signatures::SignatureBlock;
use crate::style::{SchedulePosition, StyleConfig};

pub fn render_docx(doc: &Document, style: &StyleConfig, input_dir: Option<&Path>, signature_blocks: &[SignatureBlock], pdf_renderer: PdfRenderer) -> Result<Vec<u8>> {
    let mut docx = Docx::new();

    // Page setup
    docx = docx
        .page_size(style.page_width_twips(), style.page_height_twips())
        .page_margin(
            PageMargin::new()
                .top(StyleConfig::cm_to_twips(style.margin_top_cm))
                .bottom(StyleConfig::cm_to_twips(style.margin_bottom_cm))
                .left(StyleConfig::cm_to_twips(style.margin_left_cm))
                .right(StyleConfig::cm_to_twips(style.margin_right_cm)),
        );

    // Default font and line spacing
    let line_spacing_val = (style.line_spacing * 240.0) as i32;
    docx = docx
        .default_fonts(RunFonts::new().ascii(&style.font_family).hi_ansi(&style.font_family))
        .default_size(StyleConfig::pt_to_half_points(style.font_size))
        .default_line_spacing(
            LineSpacing::new()
                .line_rule(LineSpacingType::Auto)
                .line(line_spacing_val),
        );

    // Register clause numbering definitions
    // Recitals get a separate abstract numbering (may have different align_first_level)
    docx = docx
        .add_abstract_numbering(create_clause_numbering(style))
        .add_numbering(Numbering::new(BODY_NUMBERING_ID, ABSTRACT_NUM_ID))
        .add_abstract_numbering(create_recital_numbering(style))
        .add_numbering(Numbering::new(RECITAL_NUMBERING_ID, RECITAL_ABSTRACT_NUM_ID))
        .add_abstract_numbering(create_simple_list_numbering(style));

    // Build bookmark ID map: anchor_id → unique integer ID for Word bookmarks
    let bookmark_ids = build_bookmark_map(doc);

    // Register heading styles so the TOC field can find them.
    // Brand colour and spacing are set on the style (not as direct formatting)
    // so that Word's TOC regeneration doesn't carry the colour into TOC entries.
    let heading_spacing = LineSpacing::new()
        .before(StyleConfig::pt_to_twips(style.heading_space_before_pt))
        .after(StyleConfig::pt_to_twips(style.heading_space_after_pt));
    for i in 1..=3 {
        let mut heading_style = Style::new(format!("Heading{}", i), StyleType::Paragraph)
            .name(format!("heading {}", i))
            .line_spacing(heading_spacing.clone());
        if i == 1 {
            if let Some(ref color) = style.brand_color_hex() {
                heading_style = heading_style.color(color);
            }
        }
        docx = docx.add_style(heading_style);
    }

    // Footer
    docx = render_footer(docx, doc, style);

    if style.cover.enabled {
        // Empty first-page header/footer so cover page is clean
        docx = docx.first_header(Header::new());
        docx = docx.first_footer(Footer::new());

        // Cover page
        docx = render_cover_page(docx, doc, style);

        // Page break after cover
        docx = docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
        );
    } else {
        // Inline title at top of first page
        docx = render_inline_title(docx, doc, style);
        docx = docx.add_paragraph(Paragraph::new());
    }

    let has_schedule_items = !doc.schedule_items.is_empty();
    let schedule_after_toc = matches!(style.schedule_position, SchedulePosition::AfterToc)
        && has_schedule_items;

    if style.toc.enabled {
        // TOC heading — same visual style as section headings (Heading1)
        // but without the Heading1 style so it doesn't appear in its own TOC
        let toc_heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
        let mut toc_heading_run = Run::new()
            .add_text(style.toc.heading.to_uppercase())
            .bold()
            .size(toc_heading_size)
            .fonts(
                RunFonts::new()
                    .ascii(&style.heading_font_family)
                    .hi_ansi(&style.heading_font_family),
            );
        if let Some(ref color) = style.brand_color_hex() {
            toc_heading_run = toc_heading_run.color(color);
        }
        docx = docx.add_paragraph(
            Paragraph::new()
                .line_spacing(
                    LineSpacing::new()
                        .before(StyleConfig::pt_to_twips(style.heading_space_before_pt))
                        .after(StyleConfig::pt_to_twips(style.heading_space_after_pt)),
                )
                .add_run(toc_heading_run),
        );

        // Build TOC items manually from our Document IR rather than using
        // docx-rs .auto(), which double-escapes apostrophes and other XML
        // entities in cached TOC text. The field is still marked dirty="true"
        // so Word regenerates on open, but the cached items provide a
        // readable fallback if the user declines the update prompt.
        let mut toc = TableOfContents::new()
            .heading_styles_range(1, 3)
            .dirty();
        for (text, level) in collect_toc_entries(doc) {
            toc = toc.add_item(
                TableOfContentsItem::new()
                    .text(&text)
                    .level(level),
            );
        }
        docx = docx.add_table_of_contents(toc);

        // Page break after TOC (skip if schedule follows — it has its own leading page break)
        if !schedule_after_toc {
            docx = docx.add_paragraph(
                Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
            );
        }
    }

    // Schedule before body (if configured)
    if schedule_after_toc {
        docx = render_schedules(docx, &doc.meta.schedule, &doc.schedule_items, style);

        // Page break before body
        docx = docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
        );
    }

    // Parties preamble (before body, after cover/TOC/schedule)
    if style.preamble.enabled {
        docx = render_preamble(docx, doc, style);
    }

    // Recitals section (after preamble, before body clauses)
    if let Some(ref recitals) = doc.recitals {
        docx = render_section_heading(docx, &recitals.heading, style);
        for element in &recitals.body {
            match element {
                BodyElement::Prose(inlines) => {
                    docx = docx.add_paragraph(render_inlines_paragraph(inlines, 0, style));
                }
                BodyElement::Clause(clause) => {
                    docx = render_clause(docx, clause, style, RECITAL_NUMBERING_ID, style.recitals_align_first_level, &bookmark_ids);
                }
            }
        }
    }

    // Body heading (rendered when recitals are present)
    if let Some(ref heading) = doc.body_heading {
        docx = render_section_heading(docx, heading, style);
    }

    // Body clauses
    for element in &doc.body {
        match element {
            BodyElement::Prose(inlines) => {
                docx = docx.add_paragraph(render_inlines_paragraph(inlines, 0, style));
            }
            BodyElement::Clause(clause) => {
                docx = render_clause(docx, clause, style, BODY_NUMBERING_ID, style.body_align_first_level, &bookmark_ids);
            }
        }
    }

    // Signature pages (after body clauses, before addenda)
    if style.signatures.enabled && !signature_blocks.is_empty() {
        docx = sig_renderer::render_signature_pages(docx, signature_blocks, &doc.meta.parties, style);
    }

    // Addenda — each ClauseList/NumberedList gets its own numbering instance
    // Start after the abstract numbering IDs we've registered
    let mut next_num_id: usize = RECITAL_ABSTRACT_NUM_ID + 1;
    for addendum in &doc.addenda {
        docx = render_addendum(docx, addendum, style, &mut next_num_id, &bookmark_ids);
    }

    // Exhibits — placeholder pages or imported images/PDFs
    for (i, exhibit) in doc.meta.exhibits.iter().enumerate() {
        docx = render_exhibit(docx, exhibit, i + 1, style, input_dir, pdf_renderer)?;
    }

    // Schedule at end (if configured, this is the default)
    if matches!(style.schedule_position, SchedulePosition::End) && has_schedule_items {
        docx = render_schedules(docx, &doc.meta.schedule, &doc.schedule_items, style);
    }

    // Build
    let buf = Vec::new();
    let mut cursor = std::io::Cursor::new(buf);
    docx.build().pack(&mut cursor).map_err(|e| {
        LexiconError::Render(format!("Failed to build DOCX: {}", e))
    })?;

    Ok(cursor.into_inner())
}

fn render_footer(mut docx: Docx, doc: &Document, style: &StyleConfig) -> Docx {
    let footer_size = StyleConfig::pt_to_half_points(style.font_size - 2.0);
    let mut default_footer = Footer::new();
    let has_ref = style.footer.show_ref && doc.meta.ref_.is_some();
    let has_version = style.footer.show_version && doc.meta.version.is_some();
    let has_page = style.footer.show_page_number;
    let has_left = has_ref || has_version;

    let right_tab_pos = (style.page_width_twips() as i32
        - StyleConfig::cm_to_twips(style.margin_left_cm)
        - StyleConfig::cm_to_twips(style.margin_right_cm)) as usize;
    let mut footer_para = Paragraph::new();

    // Right tab when we have content on both sides, or page number alone (right-aligned)
    if (has_left && has_page) || (!has_left && has_page) {
        footer_para = footer_para
            .add_tab(Tab::new().val(TabValueType::Right).pos(right_tab_pos));
    }

    // Left side: ref and/or version
    if has_ref {
        if let Some(ref ref_) = doc.meta.ref_ {
            footer_para = footer_para.add_run(
                Run::new()
                    .add_text(format!("Ref: {}", ref_))
                    .size(footer_size),
            );
        }
    }
    if has_version {
        if let Some(ref version) = doc.meta.version {
            if has_ref {
                footer_para = footer_para.add_run(
                    Run::new().add_text(" ").size(footer_size),
                );
            }
            footer_para = footer_para.add_run(
                Run::new()
                    .add_text(format!("v{}", version))
                    .size(footer_size),
            );
        }
    }

    if has_page {
        footer_para = footer_para.add_run(Run::new().add_tab());
        footer_para = footer_para
            .add_run(Run::new().add_text("Page ").size(footer_size))
            .add_page_num(PageNum::new())
            .add_run(Run::new().add_text(" of ").size(footer_size))
            .add_num_pages(NumPages::new());
    }

    default_footer = default_footer.add_paragraph(footer_para);
    docx = docx.footer(default_footer);
    docx
}

fn render_section_heading(mut docx: Docx, text: &str, style: &StyleConfig) -> Docx {
    let heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
    let heading_run = Run::new()
        .add_text(text.to_uppercase())
        .bold()
        .size(heading_size)
        .fonts(
            RunFonts::new()
                .ascii(&style.heading_font_family)
                .hi_ansi(&style.heading_font_family),
        );
    docx = docx.add_paragraph(
        Paragraph::new()
            .style("Heading1")
            .add_run(heading_run),
    );
    docx
}

pub fn render_clause(mut docx: Docx, clause: &Clause, style: &StyleConfig, numbering_id: usize, align_first_level: bool, bookmark_ids: &HashMap<String, usize>) -> Docx {
    let indent = indent_for_level(clause.level, style, align_first_level);
    let hanging = StyleConfig::cm_to_twips(style.hanging_indent_cm);
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    let level_idx = numbering_level_for(clause.level);

    // Track whether a bookmark has been placed for this clause's anchor
    let mut bookmark_placed = false;

    // If this clause has a heading, render it as a heading paragraph with native numbering
    if let Some(ref heading) = clause.heading {
        let heading_size = match heading.level {
            2 => StyleConfig::pt_to_half_points(style.heading1_size),
            _ => StyleConfig::pt_to_half_points(style.heading2_size),
        };

        let outline_lvl = outline_level_for(clause.level);
        let heading_style = format!("Heading{}", outline_lvl + 1);

        let mut para = Paragraph::new()
            .style(&heading_style)
            .numbering(NumberingId::new(numbering_id), IndentLevel::new(level_idx))
            .outline_lvl(outline_lvl)
            .keep_next(true)
            .run_property(RunProperty::new().bold().size(heading_size));

        // Place bookmark on the heading paragraph
        if let Some(ref anchor_id) = clause.anchor {
            if let Some(&bm_id) = bookmark_ids.get(anchor_id.as_str()) {
                para = para
                    .add_bookmark_start(bm_id, bookmark_name(anchor_id))
                    .add_bookmark_end(bm_id);
                bookmark_placed = true;
            }
        }

        // Heading inline content — Word generates the number.
        // Brand colour comes from the Heading style, not direct run formatting,
        // so that TOC entries don't inherit it.
        for inline in &heading.text {
            para = add_inline_run(para, inline, true, heading_size, style, None);
        }

        docx = docx.add_paragraph(para);
    }

    // Render clause body elements in source order (content and children interleaved)
    let mut first_content = true;
    for element in &clause.body {
        match element {
            ClauseBody::Content(content) => {
                match content {
                    ClauseContent::Paragraph(inlines) => {
                        let body_size = StyleConfig::pt_to_half_points(style.font_size);

                        let mut para = if clause.heading.is_none() && first_content {
                            // First content paragraph of a non-headed clause: attach numbering.
                            first_content = false;
                            Paragraph::new()
                                .numbering(NumberingId::new(numbering_id), IndentLevel::new(level_idx))
                        } else {
                            // Continuation paragraph — align to text position past the number
                            Paragraph::new().indent(Some(indent + hanging), None, None, None)
                        };

                        // Place bookmark on first content paragraph if not already on heading
                        if !bookmark_placed {
                            if let Some(ref anchor_id) = clause.anchor {
                                if let Some(&bm_id) = bookmark_ids.get(anchor_id.as_str()) {
                                    para = para
                                        .add_bookmark_start(bm_id, bookmark_name(anchor_id))
                                        .add_bookmark_end(bm_id);
                                    bookmark_placed = true;
                                }
                            }
                        }

                        for inline in inlines {
                            para = add_inline_run(para, inline, false, body_size, style, None);
                        }

                        docx = docx.add_paragraph(para);
                    }
                    ClauseContent::Blockquote(inlines) => {
                        let body_size = StyleConfig::pt_to_half_points(style.font_size);
                        let bq_indent = indent + hanging + step;
                        let mut para = Paragraph::new()
                            .indent(Some(bq_indent), None, None, None);

                        for inline in inlines {
                            para = add_inline_run(para, inline, false, body_size, style, None);
                        }

                        docx = docx.add_paragraph(para);
                    }
                    ClauseContent::Table(table) => {
                        docx = render_table(docx, table, style);
                    }
                }
            }
            ClauseBody::Children(children) => {
                for child in children {
                    docx = render_clause(docx, child, style, numbering_id, align_first_level, bookmark_ids);
                }
            }
        }
    }

    docx
}

/// Build a map from anchor IDs to unique bookmark integer IDs.
fn build_bookmark_map(doc: &Document) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let mut next_id: usize = 1;

    // Collect anchors from recitals
    if let Some(ref recitals) = doc.recitals {
        collect_clause_anchors_from_body(&recitals.body, &mut map, &mut next_id);
    }

    // Collect anchors from body clauses
    collect_clause_anchors_from_body(&doc.body, &mut map, &mut next_id);

    // Collect anchors from addenda
    for addendum in &doc.addenda {
        if let Some(ref anchor_id) = addendum.anchor {
            map.insert(anchor_id.clone(), next_id);
            next_id += 1;
        }
        for content in &addendum.content {
            if let AddendumContent::ClauseList(clauses) = content {
                for clause in clauses {
                    collect_clause_anchors(clause, &mut map, &mut next_id);
                }
            }
        }
    }

    map
}

fn collect_clause_anchors_from_body(body: &[BodyElement], map: &mut HashMap<String, usize>, next_id: &mut usize) {
    for element in body {
        if let BodyElement::Clause(clause) = element {
            collect_clause_anchors(clause, map, next_id);
        }
    }
}

fn collect_clause_anchors(clause: &Clause, map: &mut HashMap<String, usize>, next_id: &mut usize) {
    if let Some(ref anchor_id) = clause.anchor {
        map.insert(anchor_id.clone(), *next_id);
        *next_id += 1;
    }
    for element in &clause.body {
        if let ClauseBody::Children(children) = element {
            for child in children {
                collect_clause_anchors(child, map, next_id);
            }
        }
    }
}

/// Collect TOC entries from the Document IR with their heading level (1-3).
/// Used to build cached TOC items manually, avoiding docx-rs's .auto()
/// which double-escapes XML entities like apostrophes.
fn collect_toc_entries(doc: &Document) -> Vec<(String, usize)> {
    let mut entries = Vec::new();

    // Section headings use Heading1 (level 1)
    if let Some(ref recitals) = doc.recitals {
        entries.push((recitals.heading.to_uppercase(), 1));
    }
    if let Some(ref heading) = doc.body_heading {
        entries.push((heading.to_uppercase(), 1));
    }

    // Clause headings from recitals
    if let Some(ref recitals) = doc.recitals {
        collect_clause_toc_entries(&recitals.body, &mut entries);
    }

    // Clause headings from body
    collect_clause_toc_entries(&doc.body, &mut entries);

    // Addendum headings (Heading1)
    for addendum in &doc.addenda {
        entries.push((addendum.heading().to_uppercase(), 1));
    }

    // Exhibit headings (Heading1)
    for (i, exhibit) in doc.meta.exhibits.iter().enumerate() {
        let text = format!("EXHIBIT {} - {}", i + 1, exhibit.title).to_uppercase();
        entries.push((text, 1));
    }

    entries
}

fn collect_clause_toc_entries(body: &[BodyElement], entries: &mut Vec<(String, usize)>) {
    for element in body {
        if let BodyElement::Clause(clause) = element {
            collect_clause_heading_entries(clause, entries);
        }
    }
}

fn collect_clause_heading_entries(clause: &Clause, entries: &mut Vec<(String, usize)>) {
    if let Some(ref heading) = clause.heading {
        let outline = outline_level_for(clause.level);
        let heading_level = outline + 1; // Heading1 = level 1, etc.
        if heading_level <= 3 {
            let text: String = heading.text.iter().map(|i| i.as_plain_text()).collect();
            entries.push((text, heading_level));
        }
    }
    for element in &clause.body {
        if let ClauseBody::Children(children) = element {
            for child in children {
                collect_clause_heading_entries(child, entries);
            }
        }
    }
}

fn render_exhibit(
    mut docx: Docx,
    exhibit: &Exhibit,
    number: usize,
    style: &StyleConfig,
    input_dir: Option<&Path>,
    pdf_renderer: PdfRenderer,
) -> Result<Docx> {
    let heading_size = StyleConfig::pt_to_half_points(style.heading1_size);

    // Page break before exhibit
    docx = docx.add_paragraph(
        Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
    );

    // Exhibit heading — styled as Heading1 (same as section headings)
    let heading_text = format!("EXHIBIT {} - {}", number, exhibit.title).to_uppercase();
    let heading_run = Run::new()
        .add_text(&heading_text)
        .bold()
        .size(heading_size)
        .fonts(
            RunFonts::new()
                .ascii(&style.heading_font_family)
                .hi_ansi(&style.heading_font_family),
        );
    docx = docx.add_paragraph(
        Paragraph::new()
            .style("Heading1")
            .add_run(heading_run),
    );

    // If path is set, load and embed the file; otherwise leave as placeholder
    if let Some(ref path) = exhibit.path {
        let images = exhibit_loader::load_exhibit(path, input_dir, pdf_renderer)?;

        // Calculate content area in EMU (1 twip = 635 EMU)
        let margin_left = StyleConfig::cm_to_twips(style.margin_left_cm) as u32;
        let margin_right = StyleConfig::cm_to_twips(style.margin_right_cm) as u32;
        let margin_top = StyleConfig::cm_to_twips(style.margin_top_cm) as u32;
        let margin_bottom = StyleConfig::cm_to_twips(style.margin_bottom_cm) as u32;
        let max_w_emu = (style.page_width_twips() - margin_left - margin_right) * 635;
        let max_h_emu = (style.page_height_twips() - margin_top - margin_bottom) * 635;

        // Blank line after heading
        docx = docx.add_paragraph(Paragraph::new());

        for (i, img) in images.iter().enumerate() {
            // Page break between multi-page images (e.g. PDF pages), not before the first
            if i > 0 {
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
                );
            }

            let (fit_w, fit_h) =
                exhibit_loader::fit_to_page(img.width_px, img.height_px, max_w_emu, max_h_emu);

            let pic = Pic::new_with_dimensions(img.png_bytes.clone(), img.width_px, img.height_px)
                .size(fit_w, fit_h);

            docx = docx.add_paragraph(
                Paragraph::new()
                    .align(docx_rs::AlignmentType::Center)
                    .add_run(Run::new().add_image(pic)),
            );
        }
    }

    Ok(docx)
}
