use docx_rs::*;

fn main() -> Result<(), DocxError> {
    let path = std::path::Path::new("/tmp/test_numbering.docx");
    let file = std::fs::File::create(path).unwrap();

    let mut abstract_num = AbstractNumbering::new(2);
    abstract_num.multi_level_type = Some("multilevel".to_string());
    let abstract_num = abstract_num
        .add_level(
            Level::new(0, Start::new(1), NumberFormat::new("decimal"),
                       LevelText::new("%1."), LevelJc::new("left"))
                .indent(Some(360), Some(SpecialIndentType::Hanging(360)), None, None)
        )
        .add_level(
            Level::new(1, Start::new(1), NumberFormat::new("decimal"),
                       LevelText::new("%1.%2"), LevelJc::new("left"))
                .indent(Some(1080), Some(SpecialIndentType::Hanging(360)), None, None)
                .level_restart(1)
        )
        .add_level(
            Level::new(2, Start::new(1), NumberFormat::new("lowerLetter"),
                       LevelText::new("(%3)"), LevelJc::new("left"))
                .indent(Some(1800), Some(SpecialIndentType::Hanging(360)), None, None)
                .level_restart(2)
        );

    Docx::new()
        .add_abstract_numbering(abstract_num)
        .add_numbering(Numbering::new(2, 2))
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("Top level clause one"))
                .numbering(NumberingId::new(2), IndentLevel::new(0)),
        )
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("Sub-clause one"))
                .numbering(NumberingId::new(2), IndentLevel::new(1)),
        )
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("Sub-clause two"))
                .numbering(NumberingId::new(2), IndentLevel::new(1)),
        )
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("Sub-sub-clause a"))
                .numbering(NumberingId::new(2), IndentLevel::new(2)),
        )
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("Top level clause two"))
                .numbering(NumberingId::new(2), IndentLevel::new(0)),
        )
        .add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text("Sub-clause one of two"))
                .numbering(NumberingId::new(2), IndentLevel::new(1)),
        )
        .build()
        .pack(file)?;

    println!("Written: /tmp/test_numbering.docx");
    Ok(())
}
