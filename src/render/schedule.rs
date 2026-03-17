use docx_rs::{
    BreakType, Docx, Paragraph, Run, RunFonts, Table as DocxTable, TableCell, TableRow, WidthType,
};

use crate::model::{ScheduleDecl, ScheduleItem};
use crate::style::{ScheduleOrder, StyleConfig};

pub fn render_schedules(
    mut docx: Docx,
    schedule_configs: &[ScheduleDecl],
    items: &[ScheduleItem],
    style: &StyleConfig,
) -> Docx {
    let heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
    let body_size = StyleConfig::pt_to_half_points(style.font_size);

    for (idx, sched) in schedule_configs.iter().enumerate() {
        let mut sched_items: Vec<&ScheduleItem> = items
            .iter()
            .filter(|item| item.schedule_index == idx)
            .collect();

        if sched_items.is_empty() {
            continue;
        }

        if matches!(style.schedule_order, ScheduleOrder::Alphabetical) {
            sched_items.sort_by(|a, b| a.term.to_lowercase().cmp(&b.term.to_lowercase()));
        }

        // Page break before schedule
        docx = docx.add_paragraph(Paragraph::new().add_run(Run::new().add_break(BreakType::Page)));

        // Schedule heading — styled as Heading1 (same as section headings)
        let heading_run = Run::new()
            .add_text(sched.title.to_uppercase())
            .bold()
            .size(heading_size)
            .fonts(
                RunFonts::new()
                    .ascii(&style.heading_font_family)
                    .hi_ansi(&style.heading_font_family),
            );
        docx = docx.add_paragraph(Paragraph::new().style("Heading1").add_run(heading_run));

        // Schedule table: Item | Particulars
        let mut rows = Vec::new();

        // Header row
        rows.push(
            TableRow::new(vec![
                TableCell::new().add_paragraph(
                    Paragraph::new().add_run(Run::new().add_text("Item").bold().size(body_size)),
                ),
                TableCell::new().add_paragraph(
                    Paragraph::new()
                        .add_run(Run::new().add_text("Particulars").bold().size(body_size)),
                ),
            ])
            .cant_split(),
        );

        // Data rows
        for item in &sched_items {
            rows.push(
                TableRow::new(vec![
                    TableCell::new().add_paragraph(
                        Paragraph::new().add_run(Run::new().add_text(&item.term).size(body_size)),
                    ),
                    TableCell::new().add_paragraph(Paragraph::new()),
                ])
                .cant_split(),
            );
        }

        docx = docx.add_table(DocxTable::new(rows).width(5000, WidthType::Pct));
    }

    docx
}
