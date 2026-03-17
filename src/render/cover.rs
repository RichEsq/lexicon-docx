use docx_rs::{AlignmentType, Docx, Paragraph, Run, RunFonts};

use crate::model::Document;
use crate::render::common::format_date_with_format;
use crate::style::{PartyFormat, StyleConfig};

pub fn render_cover_page(mut docx: Docx, doc: &Document, style: &StyleConfig) -> Docx {
    let meta = &doc.meta;
    let cover = &style.cover;
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
                {
                    let mut run = Run::new()
                        .add_text(&meta.title)
                        .bold()
                        .size(StyleConfig::pt_to_half_points(style.title_size))
                        .fonts(
                            RunFonts::new()
                                .ascii(&style.heading_font_family)
                                .hi_ansi(&style.heading_font_family),
                        );
                    if let Some(ref color) = style.brand_color_hex() {
                        run = run.color(color);
                    }
                    run
                },
            ),
    );

    // Spacer
    docx = docx.add_paragraph(Paragraph::new());

    // Status + Version line
    if cover.show_status && (meta.status.is_some() || meta.version.is_some()) {
        let mut parts = Vec::new();
        if let Some(ref status) = meta.status {
            parts.push(status.to_string());
        }
        if let Some(ref version) = meta.version {
            parts.push(format!("Version {}", version));
        }
        if !parts.is_empty() {
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
    }

    // Date
    let formatted_date = format_date_with_format(&meta.date, &style.date_format);
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
    if cover.show_ref
        && let Some(ref ref_) = meta.ref_
    {
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
    if cover.show_author
        && let Some(ref author) = meta.author
    {
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
                    .add_text(&cover.between_label)
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

        match cover.party_format {
            PartyFormat::NameSpecRole => {
                if let Some(ref spec) = party.specifier {
                    para = para.add_run(
                        Run::new()
                            .add_text(format!(" {}", spec))
                            .size(body_half_pts),
                    );
                }
                docx = docx.add_paragraph(para);
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .align(AlignmentType::Center)
                        .add_run(
                            Run::new()
                                .add_text(format!("(\"{}\")", party.role))
                                .italic()
                                .size(body_half_pts),
                        ),
                );
            }
            PartyFormat::NameRole => {
                docx = docx.add_paragraph(para);
                docx = docx.add_paragraph(
                    Paragraph::new()
                        .align(AlignmentType::Center)
                        .add_run(
                            Run::new()
                                .add_text(format!("(\"{}\")", party.role))
                                .italic()
                                .size(body_half_pts),
                        ),
                );
            }
            PartyFormat::NameOnly => {
                docx = docx.add_paragraph(para);
            }
        }

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

pub fn render_inline_title(mut docx: Docx, doc: &Document, style: &StyleConfig) -> Docx {
    let meta = &doc.meta;

    // Title
    let mut title_run = Run::new()
        .add_text(&meta.title)
        .bold()
        .size(StyleConfig::pt_to_half_points(style.title_size))
        .fonts(
            RunFonts::new()
                .ascii(&style.heading_font_family)
                .hi_ansi(&style.heading_font_family),
        );
    if let Some(ref color) = style.brand_color_hex() {
        title_run = title_run.color(color);
    }
    docx = docx.add_paragraph(
        Paragraph::new()
            .align(AlignmentType::Center)
            .add_run(title_run),
    );

    docx
}
