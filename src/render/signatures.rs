use docx_rs::{
    AlignmentType, BorderType, BreakType, Docx, Paragraph, Run, RunFonts,
    Table as DocxTable, TableCell, TableCellBorder, TableCellBorderPosition,
    TableCellBorders, TableCellContent, TableCellMargins, TableRow, TableRowChild, WidthType,
};

use crate::model::Party;
use crate::signatures::{expand_field_value, FieldType, Layout, SignatureBlock, SignatureField, Signatory};
use crate::style::{DefinedTermStyle, StyleConfig};

// Height of the writing space in "long" layout (in half-points).
// Line fields (signatures) are taller than blank fields (names/dates).
const LONG_LINE_HEIGHT_HALF_PTS: usize = 56; // 28pt
const LONG_BLANK_HEIGHT_HALF_PTS: usize = 32; // 16pt

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
            if style.signatures.separate_pages {
                docx = docx.add_paragraph(
                    Paragraph::new().add_run(Run::new().add_break(BreakType::Page)),
                );
            } else {
                // Vertical spacing between blocks
                docx = docx.add_paragraph(Paragraph::new());
                docx = docx.add_paragraph(Paragraph::new());
            }
        }

        // Intro paragraph with **bold** markers (keep with table)
        docx = docx.add_paragraph(render_intro_paragraph(
            &block.intro,
            body_half_pts,
            &style.defined_term_style,
        ).keep_next(true));

        // Spacer between intro and table (keep with table)
        docx = docx.add_paragraph(Paragraph::new().keep_next(true));

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

        // Build rows — each field definition becomes a row (short) or two rows (long)
        let max_fields = block.fields.len().max(if block.witness {
            block.witness_fields.len()
        } else {
            0
        });

        let mut rows = match block.layout {
            Layout::Short => build_short_rows(
                block, party, max_fields, col_width, gap_width,
                body_half_pts, label_half_pts, style,
            ),
            Layout::Long => build_long_rows(
                block, party, max_fields, col_width, gap_width,
                body_half_pts, label_half_pts,
            ),
        };

        // Set keep_next on all cell paragraphs so the table stays on one page
        set_keep_next_on_rows(&mut rows);

        // Single-column layouts match the width of one column in a two-column layout
        let table_width = if content_cols == 1 { 2400 } else { 5000 };
        let table = DocxTable::without_borders(rows)
            .width(table_width, WidthType::Pct)
            .margins(TableCellMargins::new().margin(60, 120, 60, 0));
        docx = docx.add_table(table);
    }

    docx
}

/// Build rows for "short" layout — one row per field (current behaviour).
fn build_short_rows(
    block: &SignatureBlock,
    party: &Party,
    max_fields: usize,
    col_width: usize,
    gap_width: usize,
    body_half_pts: usize,
    label_half_pts: usize,
    style: &StyleConfig,
) -> Vec<TableRow> {
    let mut rows = Vec::new();

    for field_idx in 0..max_fields {
        let mut cells: Vec<TableCell> = Vec::new();

        for (sig_idx, signatory) in block.signatories.iter().enumerate() {
            if sig_idx > 0 {
                cells.push(empty_cell(gap_width));
            }
            let cell = if field_idx < block.fields.len() {
                render_short_field_cell(
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

        if block.witness {
            cells.push(empty_cell(gap_width));
            let cell = if field_idx < block.witness_fields.len() {
                render_short_field_cell(
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

        rows.push(TableRow::new(cells).cant_split());
    }

    rows
}

/// Build rows for "long" layout — each field produces two rows (space + label).
fn build_long_rows(
    block: &SignatureBlock,
    party: &Party,
    max_fields: usize,
    col_width: usize,
    gap_width: usize,
    body_half_pts: usize,
    label_half_pts: usize,
) -> Vec<TableRow> {
    let mut rows = Vec::new();

    for field_idx in 0..max_fields {
        // Row 1: writing space cells (with bottom border)
        let mut space_cells: Vec<TableCell> = Vec::new();
        // Row 2: label cells (small grey caption)
        let mut label_cells: Vec<TableCell> = Vec::new();

        for (sig_idx, signatory) in block.signatories.iter().enumerate() {
            if sig_idx > 0 {
                space_cells.push(empty_cell(gap_width));
                label_cells.push(empty_cell(gap_width));
            }
            if field_idx < block.fields.len() {
                let field = &block.fields[field_idx];
                space_cells.push(render_long_space_cell(
                    field, party, signatory, body_half_pts, col_width,
                ));
                label_cells.push(render_long_label_cell(
                    field, party, signatory, label_half_pts, col_width,
                ));
            } else {
                space_cells.push(empty_cell(col_width));
                label_cells.push(empty_cell(col_width));
            }
        }

        if block.witness {
            space_cells.push(empty_cell(gap_width));
            label_cells.push(empty_cell(gap_width));
            if field_idx < block.witness_fields.len() {
                let field = &block.witness_fields[field_idx];
                let witness_sig = Signatory { title: None };
                space_cells.push(render_long_space_cell(
                    field, party, &witness_sig, body_half_pts, col_width,
                ));
                label_cells.push(render_long_label_cell(
                    field, party, &witness_sig, label_half_pts, col_width,
                ));
            } else {
                space_cells.push(empty_cell(col_width));
                label_cells.push(empty_cell(col_width));
            }
        }

        rows.push(TableRow::new(space_cells).cant_split());
        rows.push(TableRow::new(label_cells).cant_split());
    }

    rows
}

/// Render a writing-space cell for "long" layout.
/// Cell has a bottom border and height controlled by field type.
fn render_long_space_cell(
    field: &SignatureField,
    party: &Party,
    signatory: &Signatory,
    body_half_pts: usize,
    col_width: usize,
) -> TableCell {
    let height = match field.field_type {
        FieldType::Line => LONG_LINE_HEIGHT_HALF_PTS,
        FieldType::Blank => LONG_BLANK_HEIGHT_HALF_PTS,
    };

    let mut cell = TableCell::new().width(col_width, WidthType::Pct);

    // If there's a pre-filled value, render it; otherwise use a sized NBSP for height
    let display_value = field
        .value
        .as_ref()
        .map(|v| expand_field_value(v, party, signatory))
        .unwrap_or_default();

    if display_value.is_empty() {
        let para = Paragraph::new().add_run(
            Run::new().add_text("\u{00A0}").size(height),
        );
        cell = cell.add_paragraph(para);
    } else {
        let para = Paragraph::new().add_run(
            Run::new().add_text(&display_value).size(body_half_pts),
        );
        cell = cell.add_paragraph(para);
    }

    // Bottom border for the writing line
    cell = cell.set_borders(
        TableCellBorders::with_empty().set(
            TableCellBorder::new(TableCellBorderPosition::Bottom)
                .border_type(BorderType::Single)
                .size(4)
                .color("000000"),
        ),
    );

    cell
}

/// Render a label cell for "long" layout.
/// Small grey caption text, no borders.
fn render_long_label_cell(
    field: &SignatureField,
    party: &Party,
    signatory: &Signatory,
    label_half_pts: usize,
    col_width: usize,
) -> TableCell {
    let mut cell = TableCell::new().width(col_width, WidthType::Pct);

    if let Some(ref label) = field.label {
        let expanded = expand_field_value(label, party, signatory);
        cell = cell.add_paragraph(
            Paragraph::new().add_run(
                Run::new()
                    .add_text(&expanded)
                    .size(label_half_pts)
                    .color("666666"),
            ),
        );
    } else {
        cell = cell.add_paragraph(Paragraph::new());
    }

    cell
}

/// Render a single field as a table cell for "short" layout.
fn render_short_field_cell(
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
/// Bold in signature intros is always rendered as bold (not as defined term style),
/// since this is emphasis, not a term definition.
fn render_intro_paragraph(
    text: &str,
    size: usize,
    _term_style: &DefinedTermStyle,
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
            para = para.add_run(Run::new().add_text(term_text).bold().size(size));
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

/// Set keep_next on all paragraphs inside table rows so Word keeps the
/// entire signature table on one page.
fn set_keep_next_on_rows(rows: &mut [TableRow]) {
    for row in rows.iter_mut() {
        for cell_child in row.cells.iter_mut() {
            let TableRowChild::TableCell(cell) = cell_child;
            for content in cell.children.iter_mut() {
                if let TableCellContent::Paragraph(para) = content {
                    para.property.keep_next = Some(true);
                }
            }
        }
    }
}
