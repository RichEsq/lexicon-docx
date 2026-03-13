use std::collections::HashMap;

use docx_rs::{
    AlignmentType, BreakType, Docx, IndentLevel, LevelOverride, NumberingId, Numbering, Paragraph,
    Run,
};

use crate::model::*;
use crate::render::common::{add_inline_run, bookmark_name, render_inlines_paragraph, render_table};
use crate::render::numbering::{ABSTRACT_NUM_ID, SIMPLE_LIST_ABSTRACT_NUM_ID};
use crate::style::StyleConfig;

pub fn render_addendum(
    mut docx: Docx,
    addendum: &Addendum,
    style: &StyleConfig,
    next_num_id: &mut usize,
    bookmark_ids: &HashMap<String, usize>,
) -> Docx {
    let heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
    let body_size = StyleConfig::pt_to_half_points(style.font_size);

    // Page break before addendum
    docx = docx.add_paragraph(
        Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
    );

    // Addendum heading (auto-numbered)
    let heading_text = addendum.heading();
    let mut heading_para = Paragraph::new()
        .align(AlignmentType::Center)
        .add_run(
            Run::new()
                .add_text(&heading_text)
                .bold()
                .size(heading_size),
        );

    // Place bookmark on addendum heading
    if let Some(ref anchor_id) = addendum.anchor {
        if let Some(&bm_id) = bookmark_ids.get(anchor_id.as_str()) {
            heading_para = heading_para
                .add_bookmark_start(bm_id, bookmark_name(anchor_id))
                .add_bookmark_end(bm_id);
        }
    }

    docx = docx.add_paragraph(heading_para);

    docx = docx.add_paragraph(Paragraph::new());

    // Addendum content
    for content in &addendum.content {
        match content {
            AddendumContent::Paragraph(inlines) => {
                docx = docx.add_paragraph(render_inlines_paragraph(inlines, 0, style));
            }
            AddendumContent::Heading(level, inlines) => {
                let size = match level {
                    2 => StyleConfig::pt_to_half_points(style.heading1_size),
                    _ => StyleConfig::pt_to_half_points(style.heading2_size),
                };
                let mut para = Paragraph::new().keep_next(true);
                for inline in inlines {
                    para = add_inline_run(para, inline, true, size, style, None);
                }
                docx = docx.add_paragraph(para);
                docx = docx.add_paragraph(Paragraph::new());
            }
            AddendumContent::ClauseList(clauses) => {
                // Create a new numbering instance for this addendum's clauses
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
                    docx = super::docx::render_clause(docx, clause, style, num_id, style.body_align_first_level, bookmark_ids);
                }
            }
            AddendumContent::Table(table) => {
                docx = render_table(docx, table, style);
            }
            AddendumContent::NumberedList(items) => {
                let num_id = *next_num_id;
                *next_num_id += 1;
                docx = docx.add_numbering(
                    Numbering::new(num_id, SIMPLE_LIST_ABSTRACT_NUM_ID)
                        .add_override(LevelOverride::new(0).start(1)),
                );
                for item in items {
                    let mut para = Paragraph::new()
                        .numbering(NumberingId::new(num_id), IndentLevel::new(0));
                    for inline in item {
                        para = add_inline_run(para, inline, false, body_size, style, None);
                    }
                    docx = docx.add_paragraph(para);
                }
            }
            AddendumContent::BulletList(items) => {
                let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
                for item in items {
                    let mut para = Paragraph::new()
                        .indent(Some(step), None, None, None);
                    // Bullet character
                    para = para.add_run(Run::new().add_text("• \t").size(body_size));
                    for inline in item {
                        para = add_inline_run(para, inline, false, body_size, style, None);
                    }
                    docx = docx.add_paragraph(para);
                }
            }
        }
    }

    docx
}
