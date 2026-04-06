use docx_rs::{
    AbstractNumbering, Level, LevelJc, LevelText, NumberFormat, SpecialIndentType, Start,
};

use crate::model::ClauseLevel;
use crate::style::{NumberingConvention, StyleConfig};

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

struct LevelDef {
    format: &'static str,
    text: &'static str,
}

fn level_defs(convention: NumberingConvention) -> [LevelDef; 6] {
    match convention {
        NumberingConvention::Commonwealth => [
            LevelDef {
                format: "decimal",
                text: "%1.",
            },
            LevelDef {
                format: "decimal",
                text: "%1.%2",
            },
            LevelDef {
                format: "lowerLetter",
                text: "(%3)",
            },
            LevelDef {
                format: "lowerRoman",
                text: "(%4)",
            },
            LevelDef {
                format: "upperLetter",
                text: "(%5)",
            },
            LevelDef {
                format: "upperRoman",
                text: "(%6)",
            },
        ],
        NumberingConvention::Decimal => [
            LevelDef {
                format: "decimal",
                text: "%1.",
            },
            LevelDef {
                format: "decimal",
                text: "%1.%2",
            },
            LevelDef {
                format: "decimal",
                text: "%1.%2.%3",
            },
            LevelDef {
                format: "decimal",
                text: "%1.%2.%3.%4",
            },
            LevelDef {
                format: "decimal",
                text: "%1.%2.%3.%4.%5",
            },
            LevelDef {
                format: "decimal",
                text: "%1.%2.%3.%4.%5.%6",
            },
        ],
        NumberingConvention::UsTraditional => [
            LevelDef {
                format: "upperRoman",
                text: "%1.",
            },
            LevelDef {
                format: "upperLetter",
                text: "%2.",
            },
            LevelDef {
                format: "decimal",
                text: "%3.",
            },
            LevelDef {
                format: "lowerLetter",
                text: "%4.",
            },
            LevelDef {
                format: "decimal",
                text: "(%5)",
            },
            LevelDef {
                format: "lowerLetter",
                text: "(%6)",
            },
        ],
    }
}

fn create_clause_numbering_with(style: &StyleConfig, id: usize, align: bool) -> AbstractNumbering {
    let step = StyleConfig::cm_to_twips(style.indent_per_level_cm);
    let hanging = StyleConfig::cm_to_twips(style.hanging_indent_cm);
    let defs = level_defs(style.numbering_convention);

    let mut numbering = AbstractNumbering::new(id);
    numbering.multi_level_type = Some("multilevel".to_string());

    for (i, def) in defs.iter().enumerate() {
        let num_steps = if align {
            match i {
                0 | 1 => 0,
                n => n - 1,
            }
        } else {
            i
        };
        let indent = num_steps as i32 * step + hanging;

        numbering = numbering.add_level(
            Level::new(
                i,
                Start::new(1),
                NumberFormat::new(def.format),
                LevelText::new(def.text),
                LevelJc::new("left"),
            )
            .indent(
                Some(indent),
                Some(SpecialIndentType::Hanging(hanging)),
                None,
                None,
            ),
        );
    }

    numbering
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
