use docx_rs::{
    BreakType, Docx, Hyperlink, HyperlinkType, Paragraph, Run, Table as DocxTable, TableCell,
    TableRow, VertAlignType, WidthType,
};

use crate::model::*;
use crate::style::{DefinedTermStyle, StyleConfig};

pub fn render_inlines_paragraph(
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
        para = add_inline_run(para, inline, false, body_size, style, None);
    }
    para
}

pub fn add_inline_run(
    para: Paragraph,
    inline: &InlineContent,
    heading_bold: bool,
    size: usize,
    style: &StyleConfig,
    color: Option<&str>,
) -> Paragraph {
    // Helper: apply heading formatting (bold + optional color) to a run
    let apply_heading = |mut run: Run| -> Run {
        if heading_bold {
            run = run.bold();
        }
        if let Some(c) = color {
            run = run.color(c);
        }
        run
    };

    match inline {
        InlineContent::Text(t) => {
            let run = apply_heading(Run::new().add_text(t).size(size));
            para.add_run(run)
        }
        InlineContent::Bold(t) => {
            render_defined_term(para, t, size, color, &style.defined_term_style)
        }
        InlineContent::Italic(t) => para.add_run(Run::new().add_text(t).italic().size(size)),
        InlineContent::Superscript(t) => {
            let mut run = Run::new().add_text(t).size(size);
            run.run_property = run.run_property.vert_align(VertAlignType::SuperScript);
            let run = apply_heading(run);
            para.add_run(run)
        }
        InlineContent::CrossRef {
            display,
            anchor_id,
            resolved,
        } => {
            let text = resolved.as_ref().unwrap_or(display);
            let run = apply_heading(Run::new().add_text(text).size(size));
            let link = Hyperlink::new(bookmark_name(anchor_id), HyperlinkType::Anchor).add_run(run);
            para.add_hyperlink(link)
        }
        InlineContent::Link { text, .. } => {
            let run = apply_heading(Run::new().add_text(text).size(size));
            para.add_run(run)
        }
        InlineContent::SoftBreak => {
            let run = apply_heading(Run::new().add_text(" ").size(size));
            para.add_run(run)
        }
        InlineContent::LineBreak => para.add_run(Run::new().add_break(BreakType::TextWrapping)),
    }
}

/// Render a defined term according to the configured style.
pub fn render_defined_term(
    para: Paragraph,
    text: &str,
    size: usize,
    color: Option<&str>,
    term_style: &DefinedTermStyle,
) -> Paragraph {
    match term_style {
        DefinedTermStyle::Bold => {
            let mut run = Run::new().add_text(text).bold().size(size);
            if let Some(c) = color {
                run = run.color(c);
            }
            para.add_run(run)
        }
        DefinedTermStyle::Quoted => {
            let mut run = Run::new()
                .add_text(format!("\u{201c}{}\u{201d}", text))
                .size(size);
            if let Some(c) = color {
                run = run.color(c);
            }
            para.add_run(run)
        }
        DefinedTermStyle::BoldQuoted => {
            let mut run = Run::new()
                .add_text(format!("\u{201c}{}\u{201d}", text))
                .bold()
                .size(size);
            if let Some(c) = color {
                run = run.color(c);
            }
            para.add_run(run)
        }
    }
}

pub fn render_table(mut docx: Docx, table: &Table, style: &StyleConfig) -> Docx {
    let body_size = StyleConfig::pt_to_half_points(style.font_size);
    let mut rows = Vec::new();

    // Header row
    if !table.headers.is_empty() {
        let mut cells = Vec::new();
        for header_cell in &table.headers {
            let mut para = Paragraph::new();
            for inline in header_cell {
                para = add_inline_run(para, inline, true, body_size, style, None);
            }
            cells.push(TableCell::new().add_paragraph(para));
        }
        rows.push(TableRow::new(cells).cant_split());
    }

    // Data rows
    for row in &table.rows {
        let mut cells = Vec::new();
        for cell_content in row {
            let mut para = Paragraph::new();
            for inline in cell_content {
                para = add_inline_run(para, inline, false, body_size, style, None);
            }
            cells.push(TableCell::new().add_paragraph(para));
        }
        rows.push(TableRow::new(cells).cant_split());
    }

    if !rows.is_empty() {
        docx = docx.add_table(DocxTable::new(rows).width(5000, WidthType::Pct));
    }

    docx
}

/// Parse a template string with `**bold**` markers into a paragraph of Runs.
/// Bold markers represent defined terms and are rendered according to `term_style`.
pub fn render_template_paragraph(
    text: &str,
    size: usize,
    term_style: &DefinedTermStyle,
) -> Paragraph {
    let mut para = Paragraph::new();
    let mut remaining = text;

    while let Some(start) = remaining.find("**") {
        // Text before the bold marker
        if start > 0 {
            para = para.add_run(Run::new().add_text(&remaining[..start]).size(size));
        }

        // Find the closing **
        let after_open = &remaining[start + 2..];
        if let Some(end) = after_open.find("**") {
            let term_text = &after_open[..end];
            para = render_defined_term(para, term_text, size, None, term_style);
            remaining = &after_open[end + 2..];
        } else {
            // No closing **, treat rest as plain text
            para = para.add_run(Run::new().add_text(remaining).size(size));
            remaining = "";
        }
    }

    // Remaining plain text
    if !remaining.is_empty() {
        para = para.add_run(Run::new().add_text(remaining).size(size));
    }

    para
}

/// Remove empty parentheses left over when {specifier} is absent.
/// Handles `()`, `( )`, and surrounding whitespace collapse.
pub fn clean_empty_parens(text: &str) -> String {
    let result = text.replace("()", "").replace("( )", "");
    // Collapse any resulting double spaces
    let mut prev = String::new();
    let mut current = result;
    while current != prev {
        prev = current.clone();
        current = current.replace("  ", " ");
    }
    current.trim().to_string()
}

/// Convert a Lexicon anchor ID to a valid Word bookmark name.
/// Word bookmarks: must start with a letter, only `[A-Za-z0-9_]`, max 40 chars.
/// Prefix with `lx_` to avoid collision with Word's reserved `_`-prefixed bookmarks.
pub fn bookmark_name(anchor_id: &str) -> String {
    let mut name = String::with_capacity(anchor_id.len() + 3);
    name.push_str("lx_");
    for c in anchor_id.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            name.push('_');
        }
    }
    name.truncate(40);
    name
}

pub fn format_date_with_format(date_str: &str, fmt: &str) -> String {
    match chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
        Ok(date) => date.format(fmt).to_string().trim().to_string(),
        Err(_) => date_str.to_string(),
    }
}
