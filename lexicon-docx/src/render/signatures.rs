use docx_rs::{
    AlignmentType, BorderType, BreakType, Docx, Paragraph, Run, RunFonts,
    Table as DocxTable, TableCell, TableCellBorder, TableCellBorderPosition,
    TableCellBorders, TableCellMargins, TableRow, WidthType,
};

use crate::model::Party;
use crate::signatures::{expand_field_value, FieldType, SignatureBlock, SignatureField, Signatory};
use crate::style::{DefinedTermStyle, StyleConfig};

/// Render all signature blocks into the document.
pub fn render_signature_pages(
    mut docx: Docx,
    blocks: &[SignatureBlock],
    parties: &[Party],
    style: &StyleConfig,
) -> Docx {
    if blocks.is_empty() {
        return docx;
    }

    let body_half_pts = StyleConfig::pt_to_half_points(style.font_size);
    let label_half_pts = StyleConfig::pt_to_half_points(style.font_size - 2.0);

    // Page break before signature page
    docx = docx.add_paragraph(
        Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
    );

    // Optional heading
    if let Some(ref heading) = style.signatures.heading {
        let heading_size = StyleConfig::pt_to_half_points(style.heading1_size);
        docx = docx.add_paragraph(
            Paragraph::new()
                .align(AlignmentType::Center)
                .add_run(
                    Run::new()
                        .add_text(heading.to_uppercase())
                        .bold()
                        .size(heading_size)
                        .fonts(
                            RunFonts::new()
                                .ascii(&style.heading_font_family)
                                .hi_ansi(&style.heading_font_family),
                        ),
                ),
        );
        // Spacer after heading
        docx = docx.add_paragraph(Paragraph::new());
    }

    // Render each party's signature block
    for (i, (block, party)) in blocks.iter().zip(parties.iter()).enumerate() {
        if i > 0 {
            // Vertical spacing between blocks
            docx = docx.add_paragraph(Paragraph::new());
            docx = docx.add_paragraph(Paragraph::new());
        }

        // Intro paragraph with **bold** markers
        docx = docx.add_paragraph(render_intro_paragraph(
            &block.intro,
            body_half_pts,
            &style.defined_term_style,
        ));

        // Spacer between intro and table
        docx = docx.add_paragraph(Paragraph::new());

        // Build the signature table
        // Content columns = signatories + optional witness
        // Gap columns inserted between each content column
        let content_cols = block.signatories.len() + if block.witness { 1 } else { 0 };
        if content_cols == 0 {
            continue;
        }

        let gap_cols = if content_cols > 1 { content_cols - 1 } else { 0 };
        let gap_width: usize = 200; // narrow gap column (~4% of table width)
        let total_gap = gap_width * gap_cols;
        let col_width = (5000 - total_gap) / content_cols;

        // Build rows — each field definition becomes a row
        let max_fields = block.fields.len().max(if block.witness {
            block.witness_fields.len()
        } else {
            0
        });

        let mut rows: Vec<TableRow> = Vec::new();

        for field_idx in 0..max_fields {
            let mut cells: Vec<TableCell> = Vec::new();

            // Signatory columns with gap columns between them
            for (sig_idx, signatory) in block.signatories.iter().enumerate() {
                if sig_idx > 0 {
                    cells.push(empty_cell(gap_width));
                }
                let cell = if field_idx < block.fields.len() {
                    render_field_cell(
                        &block.fields[field_idx],
                        party,
                        signatory,
                        body_half_pts,
                        label_half_pts,
                        col_width,
                        style,
                    )
                } else {
                    empty_cell(col_width)
                };
                cells.push(cell);
            }

            // Witness column (with gap before it)
            if block.witness {
                cells.push(empty_cell(gap_width));
                let cell = if field_idx < block.witness_fields.len() {
                    render_field_cell(
                        &block.witness_fields[field_idx],
                        party,
                        &Signatory { title: None },
                        body_half_pts,
                        label_half_pts,
                        col_width,
                        style,
                    )
                } else {
                    empty_cell(col_width)
                };
                cells.push(cell);
            }

            rows.push(TableRow::new(cells));
        }

        let table = DocxTable::without_borders(rows)
            .width(5000, WidthType::Pct)
            .margins(TableCellMargins::new().margin(60, 120, 60, 0));
        docx = docx.add_table(table);
    }

    docx
}

/// Render a single field as a table cell.
fn render_field_cell(
    field: &SignatureField,
    party: &Party,
    signatory: &Signatory,
    body_size: usize,
    label_size: usize,
    col_width: usize,
    _style: &StyleConfig,
) -> TableCell {
    let mut cell = TableCell::new().width(col_width, WidthType::Pct);

    match field.field_type {
        FieldType::Line => {
            // Signature line — empty cell with only a bottom border
            let para = Paragraph::new().add_run(
                Run::new().add_text("").size(body_size),
            );
            cell = cell.add_paragraph(para);
            cell = cell.set_borders(
                TableCellBorders::with_empty().set(
                    TableCellBorder::new(TableCellBorderPosition::Bottom)
                        .border_type(BorderType::Single)
                        .size(4)
                        .color("000000"),
                ),
            );

            // Label below the line (if any)
            if let Some(ref label) = field.label {
                cell = cell.add_paragraph(
                    Paragraph::new().add_run(
                        Run::new()
                            .add_text(label)
                            .size(label_size)
                            .color("666666"),
                    ),
                );
            }
        }
        FieldType::Blank => {
            // Label with optional pre-filled value
            let display_value = field
                .value
                .as_ref()
                .map(|v| expand_field_value(v, party, signatory))
                .unwrap_or_default();

            if let Some(ref label) = field.label {
                if display_value.is_empty() {
                    // Label only — for handwriting
                    let para = Paragraph::new().add_run(
                        Run::new()
                            .add_text(format!("{}:", label))
                            .size(body_size),
                    );
                    cell = cell.add_paragraph(para);
                } else {
                    // Label: Value
                    let para = Paragraph::new()
                        .add_run(
                            Run::new()
                                .add_text(format!("{}: ", label))
                                .size(label_size)
                                .color("666666"),
                        )
                        .add_run(
                            Run::new()
                                .add_text(&display_value)
                                .size(body_size),
                        );
                    cell = cell.add_paragraph(para);
                }
            } else if !display_value.is_empty() {
                let para = Paragraph::new().add_run(
                    Run::new().add_text(&display_value).size(body_size),
                );
                cell = cell.add_paragraph(para);
            } else {
                cell = cell.add_paragraph(Paragraph::new());
            }
        }
    }

    cell
}

fn empty_cell(col_width: usize) -> TableCell {
    TableCell::new()
        .width(col_width, WidthType::Pct)
        .add_paragraph(Paragraph::new())
}

/// Parse a template string with `**bold**` markers into a paragraph.
/// Replicates the logic from render_template_paragraph in docx.rs.
fn render_intro_paragraph(
    text: &str,
    size: usize,
    term_style: &DefinedTermStyle,
) -> Paragraph {
    let mut para = Paragraph::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("**") {
        if start > 0 {
            para = para.add_run(
                Run::new().add_text(&remaining[..start]).size(size),
            );
        }

        let after_open = &remaining[start + 2..];
        if let Some(end) = after_open.find("**") {
            let term_text = &after_open[..end];
            para = match term_style {
                DefinedTermStyle::Bold => {
                    para.add_run(Run::new().add_text(term_text).bold().size(size))
                }
                DefinedTermStyle::Quoted => para.add_run(
                    Run::new()
                        .add_text(format!("\u{201C}{}\u{201D}", term_text))
                        .size(size),
                ),
                DefinedTermStyle::BoldQuoted => para.add_run(
                    Run::new()
                        .add_text(format!("\u{201C}{}\u{201D}", term_text))
                        .bold()
                        .size(size),
                ),
            };
            remaining = &after_open[end + 2..];
        } else {
            para = para.add_run(Run::new().add_text(remaining).size(size));
            remaining = "";
        }
    }

    if !remaining.is_empty() {
        para = para.add_run(Run::new().add_text(remaining).size(size));
    }

    para
}
