use std::path::Path;

use docx_rs::{
    BreakType, Docx, Footer, Header, IndentLevel, LineSpacing, LineSpacingType, NumberingId,
    NumPages, Numbering, PageMargin, PageNum, Paragraph, Pic, Run, RunFonts, RunProperty, Style,
    StyleType, Tab, TabValueType, TableOfContents,
};

use crate::error::{LexiconError, Result};
use crate::model::*;
use crate::render::addendum::render_addendum;
use crate::render::common::{add_inline_run, render_inlines_paragraph, render_table};
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
    docx = docx
        .add_abstract_numbering(create_clause_numbering(style))
        .add_numbering(Numbering::new(BODY_NUMBERING_ID, ABSTRACT_NUM_ID))
        .add_abstract_numbering(create_simple_list_numbering(style))
        .add_abstract_numbering(create_recital_numbering(style))
        .add_numbering(Numbering::new(RECITAL_NUMBERING_ID, RECITAL_ABSTRACT_NUM_ID));

    // Register heading styles so the TOC field can find them
    for i in 1..=3 {
        docx = docx.add_style(
            Style::new(format!("Heading{}", i), StyleType::Paragraph)
                .name(format!("heading {}", i)),
        );
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
        // TOC heading
        let toc_heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
        let mut toc_heading_run = Run::new()
            .add_text(&style.toc.heading)
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
                .add_run(toc_heading_run),
        );
        docx = docx.add_paragraph(Paragraph::new());

        // Table of contents
        let toc = TableOfContents::new()
            .heading_styles_range(1, 3)
            .auto();
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
                    docx = render_clause(docx, clause, style, RECITAL_NUMBERING_ID);
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
                docx = render_clause(docx, clause, style, BODY_NUMBERING_ID);
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
        docx = render_addendum(docx, addendum, style, &mut next_num_id);
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
    let mut heading_run = Run::new()
        .add_text(text.to_uppercase())
        .bold()
        .size(heading_size)
        .fonts(
            RunFonts::new()
                .ascii(&style.heading_font_family)
                .hi_ansi(&style.heading_font_family),
        );
    if let Some(ref color) = style.brand_color_hex() {
        heading_run = heading_run.color(color);
    }
    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(
        Paragraph::new()
            .style("Heading1")
            .add_run(heading_run),
    );
    docx = docx.add_paragraph(Paragraph::new());
    docx
}

pub fn render_clause(mut docx: Docx, clause: &Clause, style: &StyleConfig, numbering_id: usize) -> Docx {
    let indent = indent_for_level(clause.level, style);
    let hanging = StyleConfig::cm_to_twips(style.hanging_indent_cm);
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    let level_idx = numbering_level_for(clause.level);

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
            .run_property({
                let mut rp = RunProperty::new().bold().size(heading_size);
                if heading.level == 2 {
                    if let Some(ref color) = style.brand_color_hex() {
                        rp = rp.color(color);
                    }
                }
                rp
            });

        // Heading inline content — Word generates the number
        let heading_color = if heading.level == 2 { style.brand_color_hex() } else { None };
        for inline in &heading.text {
            para = add_inline_run(para, inline, true, heading_size, style, heading_color.as_deref());
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
                    docx = render_clause(docx, child, style, numbering_id);
                }
            }
        }
    }

    docx
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

    // Exhibit heading — centred title
    let heading_text = format!("EXHIBIT {} - {}", number, exhibit.title);
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(docx_rs::AlignmentType::Center)
            .add_run(
                Run::new()
                    .add_text(&heading_text)
                    .bold()
                    .size(heading_size),
            ),
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
