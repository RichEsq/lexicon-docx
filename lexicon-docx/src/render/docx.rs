use docx_rs::{
    AbstractNumbering, AlignmentType, BreakType, Docx, Footer, Header, IndentLevel,
    Level, LevelJc, LevelOverride, LevelText, LineSpacing, LineSpacingType, NumberFormat,
    NumberingId, NumPages, Numbering, PageMargin, PageNum, Paragraph, Run, RunFonts,
    SpecialIndentType, Start, Tab, TabValueType, Table as DocxTable, TableCell,
    TableOfContents, TableRow, VertAlignType, WidthType,
};

use crate::error::{LexiconError, Result};
use crate::model::*;
use crate::style::StyleConfig;

// Word numbering engine IDs (start at 2 to avoid docx-rs default abstractNum at ID 1)
const ABSTRACT_NUM_ID: usize = 2;
const BODY_NUMBERING_ID: usize = 2;

pub fn render_docx(doc: &Document, style: &StyleConfig) -> Result<Vec<u8>> {
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
        .add_numbering(Numbering::new(BODY_NUMBERING_ID, ABSTRACT_NUM_ID));

    // Footer: ref on left, page numbers on right
    let footer_size = StyleConfig::pt_to_half_points(style.font_size - 2.0);
    let mut default_footer = Footer::new();
    let right_tab_pos = (style.page_width_twips() as i32
        - StyleConfig::cm_to_twips(style.margin_left_cm)
        - StyleConfig::cm_to_twips(style.margin_right_cm)) as usize;
    let mut footer_para = Paragraph::new()
        .add_tab(Tab::new().val(TabValueType::Right).pos(right_tab_pos));
    if let Some(ref ref_) = doc.meta.ref_ {
        footer_para = footer_para.add_run(
            Run::new()
                .add_text(format!("Ref: {}", ref_))
                .size(footer_size)
                .italic(),
        );
    }
    footer_para = footer_para
        .add_run(Run::new().add_tab())
        .add_run(Run::new().add_text("Page ").size(footer_size))
        .add_page_num(PageNum::new())
        .add_run(Run::new().add_text(" of ").size(footer_size))
        .add_num_pages(NumPages::new());
    default_footer = default_footer.add_paragraph(footer_para);
    docx = docx.footer(default_footer);

    if doc.meta.cover_page {
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
    }

    if doc.meta.toc {
        // Table of contents
        let toc = TableOfContents::new()
            .heading_styles_range(1, 3)
            .auto();
        docx = docx.add_table_of_contents(toc);

        // Page break after TOC
        docx = docx.add_paragraph(
            Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
        );
    }

    // Prose before first clause (e.g., recitals)
    // Then clauses
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

    // Annexures — each ClauseList gets its own numbering instance
    let mut next_num_id: usize = BODY_NUMBERING_ID + 1;
    for annexure in &doc.annexures {
        docx = render_annexure(docx, annexure, style, &mut next_num_id);
    }

    // Schedule annexure
    if !doc.schedule_items.is_empty() {
        docx = render_schedule(docx, &doc.schedule_items, style);
    }

    // Build
    let buf = Vec::new();
    let mut cursor = std::io::Cursor::new(buf);
    docx.build().pack(&mut cursor).map_err(|e| {
        LexiconError::Render(format!("Failed to build DOCX: {}", e))
    })?;

    Ok(cursor.into_inner())
}

fn render_clause(mut docx: Docx, clause: &Clause, style: &StyleConfig, numbering_id: usize) -> Docx {
    let indent = indent_for_level(clause.level, style);
    let hanging = StyleConfig::cm_to_twips(style.hanging_indent_cm);
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    let level_idx = numbering_level_for(clause.level);
    let has_number = clause.number.is_some();

    // If this clause has a heading, render it as a heading paragraph with native numbering
    if let Some(ref heading) = clause.heading {
        let heading_size = match heading.level {
            2 => StyleConfig::pt_to_half_points(style.heading1_size),
            _ => StyleConfig::pt_to_half_points(style.heading2_size),
        };

        let mut para = Paragraph::new()
            .numbering(NumberingId::new(numbering_id), IndentLevel::new(level_idx))
            .outline_lvl(outline_level_for(clause.level))
            .keep_next(true);

        // Heading inline content — Word generates the number
        for inline in &heading.text {
            para = add_inline_run(para, inline, true, heading_size, style);
        }

        docx = docx.add_paragraph(para);
    }

    // Render clause content paragraphs
    let mut first_content = true;
    for content in &clause.content {
        match content {
            ClauseContent::Paragraph(inlines) => {
                let body_size = StyleConfig::pt_to_half_points(style.font_size);

                let mut para = if clause.heading.is_none() && first_content && has_number {
                    // First content paragraph of a non-headed clause: attach numbering
                    first_content = false;
                    Paragraph::new()
                        .numbering(NumberingId::new(numbering_id), IndentLevel::new(level_idx))
                } else {
                    // Continuation paragraph — align to text position past the number
                    Paragraph::new().indent(Some(indent + hanging), None, None, None)
                };

                for inline in inlines {
                    para = add_inline_run(para, inline, false, body_size, style);
                }

                docx = docx.add_paragraph(para);
            }
            ClauseContent::Blockquote(inlines) => {
                let body_size = StyleConfig::pt_to_half_points(style.font_size);
                let bq_indent = indent + hanging + step;
                let mut para = Paragraph::new()
                    .indent(Some(bq_indent), None, None, None);

                for inline in inlines {
                    para = add_inline_run(para, inline, false, body_size, style);
                }

                docx = docx.add_paragraph(para);
            }
            ClauseContent::Table(table) => {
                docx = render_table(docx, table, style);
            }
        }
    }

    // Render children
    for child in &clause.children {
        docx = render_clause(docx, child, style, numbering_id);
    }

    docx
}

fn render_inlines_paragraph(
    inlines: &[InlineContent],
    indent: i32,
    style: &StyleConfig,
) -> Paragraph {
    let body_size = StyleConfig::pt_to_half_points(style.font_size);
    let mut para = Paragraph::new();
    if indent > 0 {
        para = para.indent(Some(indent), None, None, None);
    }
    for inline in inlines {
        para = add_inline_run(para, inline, false, body_size, style);
    }
    para
}

fn add_inline_run(
    para: Paragraph,
    inline: &InlineContent,
    heading_bold: bool,
    size: usize,
    _style: &StyleConfig,
) -> Paragraph {
    match inline {
        InlineContent::Text(t) => {
            let mut run = Run::new().add_text(t).size(size);
            if heading_bold {
                run = run.bold();
            }
            para.add_run(run)
        }
        InlineContent::Bold(t) => {
            para.add_run(Run::new().add_text(t).bold().size(size))
        }
        InlineContent::Italic(t) => {
            para.add_run(Run::new().add_text(t).italic().size(size))
        }
        InlineContent::Superscript(t) => {
            let mut run = Run::new().add_text(t).size(size);
            run.run_property = run.run_property.vert_align(VertAlignType::SuperScript);
            if heading_bold {
                run = run.bold();
            }
            para.add_run(run)
        }
        InlineContent::CrossRef {
            display,
            resolved,
            ..
        } => {
            let text = resolved.as_ref().unwrap_or(display);
            let mut run = Run::new().add_text(text).size(size);
            if heading_bold {
                run = run.bold();
            }
            para.add_run(run)
        }
        InlineContent::ScheduleRef {
            display,
            resolved_value,
            ..
        } => {
            let mut run = Run::new().size(size);
            if heading_bold {
                run = run.bold();
            }
            match resolved_value {
                Some(val) if !val.is_empty() => {
                    run = run.add_text(format!("{} ({})", display, val));
                }
                Some(_) => {
                    // Empty value — show blank line for completion
                    run = run.add_text(format!("{} (____________)", display));
                }
                None => {
                    run = run.add_text(display);
                }
            }
            para.add_run(run)
        }
        InlineContent::Link { text, .. } => {
            let mut run = Run::new().add_text(text).size(size);
            if heading_bold {
                run = run.bold();
            }
            para.add_run(run)
        }
        InlineContent::SoftBreak => {
            let mut run = Run::new().add_text(" ").size(size);
            if heading_bold {
                run = run.bold();
            }
            para.add_run(run)
        }
        InlineContent::LineBreak => {
            para.add_run(Run::new().add_break(BreakType::TextWrapping))
        }
    }
}

fn render_table(mut docx: Docx, table: &Table, style: &StyleConfig) -> Docx {
    let body_size = StyleConfig::pt_to_half_points(style.font_size);
    let mut rows = Vec::new();

    // Header row
    if !table.headers.is_empty() {
        let mut cells = Vec::new();
        for header_cell in &table.headers {
            let mut para = Paragraph::new();
            for inline in header_cell {
                para = add_inline_run(para, inline, true, body_size, style);
            }
            cells.push(TableCell::new().add_paragraph(para));
        }
        rows.push(TableRow::new(cells));
    }

    // Data rows
    for row in &table.rows {
        let mut cells = Vec::new();
        for cell_content in row {
            let mut para = Paragraph::new();
            for inline in cell_content {
                para = add_inline_run(para, inline, false, body_size, style);
            }
            cells.push(TableCell::new().add_paragraph(para));
        }
        rows.push(TableRow::new(cells));
    }

    if !rows.is_empty() {
        docx = docx.add_table(DocxTable::new(rows).width(5000, WidthType::Pct));
    }

    docx
}

fn render_annexure(
    mut docx: Docx,
    annexure: &Annexure,
    style: &StyleConfig,
    next_num_id: &mut usize,
) -> Docx {
    let heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
    let body_size = StyleConfig::pt_to_half_points(style.font_size);

    // Page break before annexure
    docx = docx.add_paragraph(
        Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
    );

    // Annexure heading
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(
                Run::new()
                    .add_text(&annexure.heading)
                    .bold()
                    .size(heading_size),
            ),
    );

    docx = docx.add_paragraph(Paragraph::new());

    // Annexure content
    for content in &annexure.content {
        match content {
            AnnexureContent::Paragraph(inlines) => {
                docx = docx.add_paragraph(render_inlines_paragraph(inlines, 0, style));
            }
            AnnexureContent::Heading(level, inlines) => {
                let size = match level {
                    2 => StyleConfig::pt_to_half_points(style.heading1_size),
                    _ => StyleConfig::pt_to_half_points(style.heading2_size),
                };
                let mut para = Paragraph::new().keep_next(true);
                for inline in inlines {
                    para = add_inline_run(para, inline, true, size, style);
                }
                docx = docx.add_paragraph(para);
                docx = docx.add_paragraph(Paragraph::new());
            }
            AnnexureContent::ClauseList(clauses) => {
                // Create a new numbering instance for this annexure's clauses
                let num_id = *next_num_id;
                *next_num_id += 1;
                docx = docx.add_numbering(
                    Numbering::new(num_id, ABSTRACT_NUM_ID)
                        .add_override(LevelOverride::new(0).start(1))
                        .add_override(LevelOverride::new(1).start(1))
                        .add_override(LevelOverride::new(2).start(1))
                        .add_override(LevelOverride::new(3).start(1)),
                );
                for clause in clauses {
                    docx = render_clause(docx, clause, style, num_id);
                }
            }
            AnnexureContent::Table(table) => {
                docx = render_table(docx, table, style);
            }
            AnnexureContent::NumberedList(items) => {
                let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
                for (i, item) in items.iter().enumerate() {
                    let mut para = Paragraph::new()
                        .indent(Some(step), None, None, None);
                    para = para.add_run(
                        Run::new()
                            .add_text(format!("{}.\t", i + 1))
                            .size(body_size),
                    );
                    for inline in item {
                        para = add_inline_run(para, inline, false, body_size, style);
                    }
                    docx = docx.add_paragraph(para);
                }
            }
            AnnexureContent::BulletList(items) => {
                let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
                for item in items {
                    let mut para = Paragraph::new()
                        .indent(Some(step), None, None, None);
                    // Bullet character
                    para = para.add_run(Run::new().add_text("• \t").size(body_size));
                    for inline in item {
                        para = add_inline_run(para, inline, false, body_size, style);
                    }
                    docx = docx.add_paragraph(para);
                }
            }
        }
    }

    docx
}

fn render_schedule(
    mut docx: Docx,
    items: &[ScheduleItem],
    style: &StyleConfig,
) -> Docx {
    let heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
    let body_size = StyleConfig::pt_to_half_points(style.font_size);

    // Page break before schedule
    docx = docx.add_paragraph(
        Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
    );

    // Schedule heading
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(
                Run::new()
                    .add_text("SCHEDULE")
                    .bold()
                    .size(heading_size),
            ),
    );

    docx = docx.add_paragraph(Paragraph::new());

    // Schedule table: Item | Value
    let mut rows = Vec::new();

    // Header row
    rows.push(TableRow::new(vec![
        TableCell::new().add_paragraph(
            Paragraph::new().add_run(
                Run::new().add_text("Item").bold().size(body_size),
            ),
        ),
        TableCell::new().add_paragraph(
            Paragraph::new().add_run(
                Run::new().add_text("Value").bold().size(body_size),
            ),
        ),
    ]));

    // Data rows
    for item in items {
        let value_text = match &item.value {
            Some(v) if !v.is_empty() => v.clone(),
            _ => "____________".to_string(),
        };
        rows.push(TableRow::new(vec![
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(
                    Run::new().add_text(&item.description).size(body_size),
                ),
            ),
            TableCell::new().add_paragraph(
                Paragraph::new().add_run(
                    Run::new().add_text(&value_text).size(body_size),
                ),
            ),
        ]));
    }

    docx = docx.add_table(DocxTable::new(rows).width(5000, WidthType::Pct));

    docx
}

fn create_clause_numbering(style: &StyleConfig) -> AbstractNumbering {
    let h1_size = StyleConfig::pt_to_half_points(style.heading1_size);
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    let hanging = StyleConfig::cm_to_twips(style.hanging_indent_cm);
    let align = style.align_first_level;

    let level_indent = |level: usize| -> i32 {
        let num_steps = if align {
            match level { 0 | 1 => 0, n => n - 1 }
        } else {
            level
        };
        num_steps as i32 * step + hanging
    };

    let mut numbering = AbstractNumbering::new(ABSTRACT_NUM_ID);
    numbering.multi_level_type = Some("multilevel".to_string());
    numbering
        // Level 0: TopLevel — "1."
        .add_level(
            Level::new(
                0,
                Start::new(1),
                NumberFormat::new("decimal"),
                LevelText::new("%1."),
                LevelJc::new("left"),
            )
            .indent(Some(level_indent(0)), Some(SpecialIndentType::Hanging(hanging)), None, None)
            .bold()
            .size(h1_size)
            .fonts(
                RunFonts::new()
                    .ascii(&style.heading_font_family)
                    .hi_ansi(&style.heading_font_family),
            )
        )
        // Level 1: Clause — "1.1"
        .add_level(
            Level::new(
                1,
                Start::new(1),
                NumberFormat::new("decimal"),
                LevelText::new("%1.%2"),
                LevelJc::new("left"),
            )
            .indent(
                Some(level_indent(1)),
                Some(SpecialIndentType::Hanging(hanging)),
                None, None,
            )
        )
        // Level 2: SubClause — "(a)"
        .add_level(
            Level::new(
                2,
                Start::new(1),
                NumberFormat::new("lowerLetter"),
                LevelText::new("(%3)"),
                LevelJc::new("left"),
            )
            .indent(
                Some(level_indent(2)),
                Some(SpecialIndentType::Hanging(hanging)),
                None, None,
            )
        )
        // Level 3: SubSubClause — "(i)"
        .add_level(
            Level::new(
                3,
                Start::new(1),
                NumberFormat::new("lowerRoman"),
                LevelText::new("(%4)"),
                LevelJc::new("left"),
            )
            .indent(
                Some(level_indent(3)),
                Some(SpecialIndentType::Hanging(hanging)),
                None, None,
            )
        )
}

fn numbering_level_for(level: ClauseLevel) -> usize {
    match level {
        ClauseLevel::TopLevel => 0,
        ClauseLevel::Clause => 1,
        ClauseLevel::SubClause => 2,
        ClauseLevel::SubSubClause => 3,
    }
}

fn indent_for_level(level: ClauseLevel, style: &StyleConfig) -> i32 {
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    if style.align_first_level {
        match level {
            ClauseLevel::TopLevel => 0,
            ClauseLevel::Clause => 0,
            ClauseLevel::SubClause => step,
            ClauseLevel::SubSubClause => step * 2,
        }
    } else {
        match level {
            ClauseLevel::TopLevel => 0,
            ClauseLevel::Clause => step,
            ClauseLevel::SubClause => step * 2,
            ClauseLevel::SubSubClause => step * 3,
        }
    }
}

fn outline_level_for(level: ClauseLevel) -> usize {
    match level {
        ClauseLevel::TopLevel => 0,
        ClauseLevel::Clause => 1,
        ClauseLevel::SubClause => 2,
        ClauseLevel::SubSubClause => 3,
    }
}

// --- Inline title (no cover page) ---

fn render_inline_title(mut docx: Docx, doc: &Document, style: &StyleConfig) -> Docx {
    let meta = &doc.meta;
    let body_half_pts = StyleConfig::pt_to_half_points(style.font_size);

    // Title
    docx = docx.add_paragraph(
        Paragraph::new()
            .add_run(
                Run::new()
                    .add_text(&meta.title)
                    .bold()
                    .size(StyleConfig::pt_to_half_points(style.heading1_size))
                    .fonts(
                        RunFonts::new()
                            .ascii(&style.heading_font_family)
                            .hi_ansi(&style.heading_font_family),
                    ),
            ),
    );

    // Status + Version line
    if meta.status.is_some() || meta.version.is_some() {
        let mut parts = Vec::new();
        if let Some(ref status) = meta.status {
            parts.push(status.to_string());
        }
        if let Some(version) = meta.version {
            parts.push(format!("Version {}", version));
        }
        docx = docx.add_paragraph(
            Paragraph::new()
                .add_run(
                    Run::new()
                        .add_text(parts.join(" — "))
                        .size(body_half_pts),
                ),
        );
    }

    // Date
    let formatted_date = format_date(&meta.date);
    docx = docx.add_paragraph(
        Paragraph::new()
            .add_run(
                Run::new()
                    .add_text(&formatted_date)
                    .size(body_half_pts),
            ),
    );

    // Spacer before content
    docx = docx.add_paragraph(Paragraph::new());

    docx
}

// --- Cover page ---

fn render_cover_page(mut docx: Docx, doc: &Document, style: &StyleConfig) -> Docx {
    let meta = &doc.meta;
    let heading_half_pts = StyleConfig::pt_to_half_points(style.heading1_size);
    let body_half_pts = StyleConfig::pt_to_half_points(style.font_size);

    // Spacer
    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(Paragraph::new());

    // Title
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(
                Run::new()
                    .add_text(&meta.title)
                    .bold()
                    .size(StyleConfig::pt_to_half_points(20.0))
                    .fonts(
                        RunFonts::new()
                            .ascii(&style.heading_font_family)
                            .hi_ansi(&style.heading_font_family),
                    ),
            ),
    );

    // Spacer
    docx = docx.add_paragraph(Paragraph::new());

    // Status + Version line
    if meta.status.is_some() || meta.version.is_some() {
        let mut parts = Vec::new();
        if let Some(ref status) = meta.status {
            parts.push(status.to_string());
        }
        if let Some(version) = meta.version {
            parts.push(format!("Version {}", version));
        }
        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(
                    Run::new()
                        .add_text(parts.join(" — "))
                        .size(body_half_pts),
                ),
        );
    }

    // Date
    let formatted_date = format_date(&meta.date);
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(
                Run::new()
                    .add_text(&formatted_date)
                    .size(body_half_pts),
            ),
    );

    // Spacer
    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(Paragraph::new());

    // Ref
    if let Some(ref ref_) = meta.ref_ {
        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(
                    Run::new()
                        .add_text(format!("Ref: {}", ref_))
                        .size(body_half_pts)
                        .italic(),
                ),
        );
    }

    // Author
    if let Some(ref author) = meta.author {
        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(
                    Run::new()
                        .add_text(author.as_str())
                        .size(body_half_pts)
                        .italic(),
                ),
        );
    }

    // Spacer before parties
    docx = docx.add_paragraph(Paragraph::new());
    docx = docx.add_paragraph(Paragraph::new());

    // "Between" heading
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(
                Run::new()
                    .add_text("BETWEEN")
                    .bold()
                    .size(heading_half_pts),
            ),
    );

    docx = docx.add_paragraph(Paragraph::new());

    // Parties
    for (i, party) in meta.parties.iter().enumerate() {
        let mut para = Paragraph::new().align(AlignmentType::Center);

        para = para.add_run(
            Run::new()
                .add_text(&party.name)
                .bold()
                .size(body_half_pts),
        );

        if let Some(ref spec) = party.specifier {
            para = para.add_run(
                Run::new()
                    .add_text(format!(" ({})", spec))
                    .size(body_half_pts),
            );
        }

        docx = docx.add_paragraph(para);

        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(
                    Run::new()
                        .add_text(format!("(the \"{}\")", party.role))
                        .italic()
                        .size(body_half_pts),
                ),
        );

        if i < meta.parties.len() - 1 {
            docx = docx.add_paragraph(Paragraph::new());
            docx = docx.add_paragraph(
                Paragraph::new()
                    .align(AlignmentType::Center)
                    .add_run(
                        Run::new()
                            .add_text("and")
                            .size(body_half_pts),
                    ),
            );
            docx = docx.add_paragraph(Paragraph::new());
        }
    }

    docx
}

fn format_date(date_str: &str) -> String {
    match chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) => date.format("%e %B %Y").to_string().trim().to_string(),
        Err(_) => date_str.to_string(),
    }
}
