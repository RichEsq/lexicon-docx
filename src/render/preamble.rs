use docx_rs::{Docx, Paragraph, Run};

use crate::model::Document;
use crate::render::common::{
    clean_empty_parens, format_date_with_format, render_defined_term, render_template_paragraph,
};
use crate::style::{PreambleStyle, StyleConfig};

pub fn render_preamble(mut docx: Docx, doc: &Document, style: &StyleConfig) -> Docx {
    let meta = &doc.meta;
    let body_half_pts = StyleConfig::pt_to_half_points(style.font_size);
    let doc_type = meta.doc_type.as_deref().unwrap_or("Agreement");
    let formatted_date = format_date_with_format(&meta.date, &style.date_format);

    match style.preamble.style {
        PreambleStyle::Simple => {
            let term_style = &style.defined_term_style;

            // Opening line: This [title] ([type]) is dated [date] between
            let between_word = if meta.parties.len() == 1 {
                "by"
            } else {
                "between"
            };
            let mut opening = Paragraph::new();
            opening = opening.add_run(
                Run::new()
                    .add_text(format!("This {} (", &meta.title))
                    .size(body_half_pts),
            );
            opening = render_defined_term(opening, doc_type, body_half_pts, None, term_style);
            opening = opening.add_run(
                Run::new()
                    .add_text(format!(") is dated {} {}", &formatted_date, between_word))
                    .size(body_half_pts),
            );
            docx = docx.add_paragraph(opening);

            // Spacer
            docx = docx.add_paragraph(Paragraph::new());

            // Parties
            let party_count = meta.parties.len();
            for (i, party) in meta.parties.iter().enumerate() {
                let mut para = Paragraph::new();
                para = para.add_run(Run::new().add_text(&party.name).size(body_half_pts));
                if let Some(ref spec) = party.specifier {
                    para = para.add_run(
                        Run::new()
                            .add_text(format!(" ({})", spec))
                            .size(body_half_pts),
                    );
                }
                para = para.add_run(Run::new().add_text(" (").size(body_half_pts));
                para = render_defined_term(para, &party.role, body_half_pts, None, term_style);
                para = para.add_run(Run::new().add_text(")").size(body_half_pts));

                // "; and" suffix on all but the last party
                if i < party_count - 1 {
                    para = para.add_run(Run::new().add_text("; and").size(body_half_pts));
                }

                docx = docx.add_paragraph(para);
            }

            // Spacer after parties
            docx = docx.add_paragraph(Paragraph::new());
        }
        PreambleStyle::Prose => {
            let term_style = &style.defined_term_style;

            // Single paragraph: This [title] ([type]) is entered into as of [date]
            // between [party1] and [party2].
            let mut para = Paragraph::new();
            para = para.add_run(
                Run::new()
                    .add_text(format!("This {} (", &meta.title))
                    .size(body_half_pts),
            );
            para = render_defined_term(para, doc_type, body_half_pts, None, term_style);
            para = para.add_run(
                Run::new()
                    .add_text(format!(
                        ") is entered into as of {} {} ",
                        &formatted_date,
                        if meta.parties.len() == 1 {
                            "by"
                        } else {
                            "between"
                        }
                    ))
                    .size(body_half_pts),
            );

            // Parties
            let party_count = meta.parties.len();
            for (i, party) in meta.parties.iter().enumerate() {
                para = para.add_run(Run::new().add_text(&party.name).size(body_half_pts));
                if let Some(ref spec) = party.specifier {
                    para = para.add_run(
                        Run::new()
                            .add_text(format!(" ({})", spec))
                            .size(body_half_pts),
                    );
                }
                para = para.add_run(Run::new().add_text(" (").size(body_half_pts));
                para = render_defined_term(para, &party.role, body_half_pts, None, term_style);
                para = para.add_run(Run::new().add_text(")").size(body_half_pts));

                if party_count > 2 && i < party_count - 1 {
                    // Comma-separated for 3+ parties
                    if i < party_count - 2 {
                        para = para.add_run(Run::new().add_text(", ").size(body_half_pts));
                    } else {
                        para = para.add_run(Run::new().add_text(" and ").size(body_half_pts));
                    }
                } else if party_count == 2 && i == 0 {
                    para = para.add_run(Run::new().add_text(" and ").size(body_half_pts));
                }
            }

            // Closing period
            para = para.add_run(Run::new().add_text(".").size(body_half_pts));
            docx = docx.add_paragraph(para);

            // Spacer after preamble
            docx = docx.add_paragraph(Paragraph::new());
        }
        PreambleStyle::Custom => {
            let preamble = &style.preamble;

            // Expand the opening template
            let expanded_template = preamble
                .template
                .replace("{title}", &meta.title)
                .replace("{type}", doc_type)
                .replace("{date}", &formatted_date);

            // Render template lines as paragraphs
            for line in expanded_template.split("\\n") {
                let cleaned = clean_empty_parens(line);
                docx = docx.add_paragraph(render_template_paragraph(
                    &cleaned,
                    body_half_pts,
                    &style.defined_term_style,
                ));
            }

            // Spacer before parties
            docx = docx.add_paragraph(Paragraph::new());

            // Render each party
            let party_count = meta.parties.len();
            for (i, party) in meta.parties.iter().enumerate() {
                let expanded_party = preamble
                    .party_template
                    .replace("{name}", &party.name)
                    .replace("{specifier}", party.specifier.as_deref().unwrap_or(""))
                    .replace("{role}", &party.role);
                let cleaned = clean_empty_parens(&expanded_party);

                // Append separator to all but the last party
                let line = if i < party_count - 1 {
                    format!("{}{}", cleaned, &preamble.party_separator)
                } else {
                    cleaned
                };

                docx = docx.add_paragraph(render_template_paragraph(
                    &line,
                    body_half_pts,
                    &style.defined_term_style,
                ));
            }

            // Spacer after preamble
            docx = docx.add_paragraph(Paragraph::new());
        }
    }

    docx
}
