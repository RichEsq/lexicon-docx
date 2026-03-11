use docx_rs::{
    AbstractNumbering, Level, LevelJc, LevelText, NumberFormat, RunFonts, SpecialIndentType, Start,
};

use crate::model::ClauseLevel;
use crate::style::StyleConfig;

// Word numbering engine IDs (start at 2 to avoid docx-rs default abstractNum at ID 1)
pub const ABSTRACT_NUM_ID: usize = 2;
pub const BODY_NUMBERING_ID: usize = 2;
// Simple numbered list (for addendum prose lists)
pub const SIMPLE_LIST_ABSTRACT_NUM_ID: usize = 3;
// Recital numbering (A, B, C at top level)
pub const RECITAL_ABSTRACT_NUM_ID: usize = 4;
pub const RECITAL_NUMBERING_ID: usize = 4;

pub fn create_clause_numbering(style: &StyleConfig) -> AbstractNumbering {
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
    let mut level0 = Level::new(
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
    );
    if let Some(ref color) = style.brand_color_hex() {
        level0 = level0.color(color);
    }

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

pub fn create_recital_numbering(style: &StyleConfig) -> AbstractNumbering {
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

    let mut numbering = AbstractNumbering::new(RECITAL_ABSTRACT_NUM_ID);
    numbering.multi_level_type = Some("multilevel".to_string());
    numbering
        // Level 0: RecitalTopLevel — "(A)"
        .add_level(
            Level::new(
                0,
                Start::new(1),
                NumberFormat::new("upperLetter"),
                LevelText::new("(%1)"),
                LevelJc::new("left"),
            )
            .indent(Some(level_indent(0)), Some(SpecialIndentType::Hanging(hanging)), None, None)
        )
        // Level 1: RecitalClause — "A.1"
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
        // Level 2: RecitalSubClause — "(a)"
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
        // Level 3: RecitalSubSubClause — "(i)"
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
        .indent(Some(step + hanging), Some(SpecialIndentType::Hanging(hanging)), None, None),
    )
}

pub fn numbering_level_for(level: ClauseLevel) -> usize {
    match level {
        ClauseLevel::TopLevel => 0,
        ClauseLevel::Clause => 1,
        ClauseLevel::SubClause => 2,
        ClauseLevel::SubSubClause => 3,
    }
}

pub fn indent_for_level(level: ClauseLevel, style: &StyleConfig) -> i32 {
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

pub fn outline_level_for(level: ClauseLevel) -> usize {
    match level {
        ClauseLevel::TopLevel => 0,
        ClauseLevel::Clause => 1,
        ClauseLevel::SubClause => 2,
        ClauseLevel::SubSubClause => 3,
    }
}
