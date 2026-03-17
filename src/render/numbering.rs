use docx_rs::{
    AbstractNumbering, Level, LevelJc, LevelText, NumberFormat, SpecialIndentType, Start,
};

use crate::model::ClauseLevel;
use crate::style::StyleConfig;

// Word numbering engine IDs (start at 2 to avoid docx-rs default abstractNum at ID 1)
pub const ABSTRACT_NUM_ID: usize = 2;
pub const BODY_NUMBERING_ID: usize = 2;
// Simple numbered list (for addendum prose lists)
pub const SIMPLE_LIST_ABSTRACT_NUM_ID: usize = 3;
// Recitals use a separate abstract numbering (may have different align_first_level)
pub const RECITAL_ABSTRACT_NUM_ID: usize = 4;
pub const RECITAL_NUMBERING_ID: usize = 4;

pub fn create_clause_numbering(style: &StyleConfig) -> AbstractNumbering {
    create_clause_numbering_with(style, ABSTRACT_NUM_ID, style.body_align_first_level)
}

pub fn create_recital_numbering(style: &StyleConfig) -> AbstractNumbering {
    create_clause_numbering_with(
        style,
        RECITAL_ABSTRACT_NUM_ID,
        style.recitals_align_first_level,
    )
}

fn create_clause_numbering_with(style: &StyleConfig, id: usize, align: bool) -> AbstractNumbering {
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    let hanging = StyleConfig::cm_to_twips(style.hanging_indent_cm);

    let level_indent = |level: usize| -> i32 {
        let num_steps = if align {
            match level {
                0 | 1 => 0,
                n => n - 1,
            }
        } else {
            level
        };
        num_steps as i32 * step + hanging
    };

    let mut numbering = AbstractNumbering::new(id);
    numbering.multi_level_type = Some("multilevel".to_string());
    let level0 = Level::new(
        0,
        Start::new(1),
        NumberFormat::new("decimal"),
        LevelText::new("%1."),
        LevelJc::new("left"),
    )
    .indent(
        Some(level_indent(0)),
        Some(SpecialIndentType::Hanging(hanging)),
        None,
        None,
    );

    numbering
        // Level 0: TopLevel — "1."
        .add_level(level0)
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
                None,
                None,
            ),
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
                None,
                None,
            ),
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
                None,
                None,
            ),
        )
        // Level 4: Paragraph — "(A)"
        .add_level(
            Level::new(
                4,
                Start::new(1),
                NumberFormat::new("upperLetter"),
                LevelText::new("(%5)"),
                LevelJc::new("left"),
            )
            .indent(
                Some(level_indent(4)),
                Some(SpecialIndentType::Hanging(hanging)),
                None,
                None,
            ),
        )
        // Level 5: SubParagraph — "(I)"
        .add_level(
            Level::new(
                5,
                Start::new(1),
                NumberFormat::new("upperRoman"),
                LevelText::new("(%6)"),
                LevelJc::new("left"),
            )
            .indent(
                Some(level_indent(5)),
                Some(SpecialIndentType::Hanging(hanging)),
                None,
                None,
            ),
        )
}

pub fn create_simple_list_numbering(style: &StyleConfig) -> AbstractNumbering {
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    let hanging = StyleConfig::cm_to_twips(style.hanging_indent_cm);

    let mut numbering = AbstractNumbering::new(SIMPLE_LIST_ABSTRACT_NUM_ID);
    numbering.multi_level_type = Some("singleLevel".to_string());
    numbering.add_level(
        Level::new(
            0,
            Start::new(1),
            NumberFormat::new("decimal"),
            LevelText::new("%1."),
            LevelJc::new("left"),
        )
        .indent(
            Some(step + hanging),
            Some(SpecialIndentType::Hanging(hanging)),
            None,
            None,
        ),
    )
}

pub fn numbering_level_for(level: ClauseLevel) -> usize {
    match level {
        ClauseLevel::TopLevel => 0,
        ClauseLevel::Clause => 1,
        ClauseLevel::SubClause => 2,
        ClauseLevel::SubSubClause => 3,
        ClauseLevel::Paragraph => 4,
        ClauseLevel::SubParagraph => 5,
    }
}

pub fn indent_for_level(level: ClauseLevel, style: &StyleConfig, align_first_level: bool) -> i32 {
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    if align_first_level {
        match level {
            ClauseLevel::TopLevel => 0,
            ClauseLevel::Clause => 0,
            ClauseLevel::SubClause => step,
            ClauseLevel::SubSubClause => step * 2,
            ClauseLevel::Paragraph => step * 3,
            ClauseLevel::SubParagraph => step * 4,
        }
    } else {
        match level {
            ClauseLevel::TopLevel => 0,
            ClauseLevel::Clause => step,
            ClauseLevel::SubClause => step * 2,
            ClauseLevel::SubSubClause => step * 3,
            ClauseLevel::Paragraph => step * 4,
            ClauseLevel::SubParagraph => step * 5,
        }
    }
}

pub fn outline_level_for(level: ClauseLevel) -> usize {
    match level {
        ClauseLevel::TopLevel => 0,
        ClauseLevel::Clause => 1,
        ClauseLevel::SubClause => 2,
        ClauseLevel::SubSubClause => 3,
        ClauseLevel::Paragraph => 4,
        ClauseLevel::SubParagraph => 5,
    }
}
