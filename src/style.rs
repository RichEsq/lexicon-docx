use serde::Deserialize;
use std::path::Path;

use crate::error::{LexiconError, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StyleConfig {
    pub font_family: String,
    pub font_size: f32,
    pub heading_font_family: String,
    pub title_size: f32,
    pub heading1_size: f32,
    pub heading2_size: f32,
    pub heading_space_before: f32,
    pub heading_space_after: f32,
    pub paragraph_space_before: f32,
    pub paragraph_space_after: f32,
    pub line_spacing: f32,
    pub margin_top_cm: f32,
    pub margin_bottom_cm: f32,
    pub margin_left_cm: f32,
    pub margin_right_cm: f32,
    pub page_size: PageSize,
    pub indent_per_level_cm: f32,
    pub hanging_indent_cm: f32,
    pub body_align_first_level: bool,
    pub recitals_align_first_level: bool,
    pub brand_color: Option<String>,
    pub date_format: String,
    pub defined_term_style: DefinedTermStyle,
    pub cover: CoverConfig,
    pub toc: TocConfig,
    pub footer: FooterConfig,
    pub preamble: PreambleConfig,
    pub schedule_position: SchedulePosition,
    pub schedule_order: ScheduleOrder,
    pub signatures: SignaturesConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SignaturesConfig {
    pub enabled: bool,
    pub heading: Option<String>,
    pub default_template: Option<String>,
    pub separate_pages: bool,
    #[serde(default)]
    pub party: std::collections::HashMap<String, SignaturesPartyOverride>,
}

impl Default for SignaturesConfig {
    fn default() -> Self {
        SignaturesConfig {
            enabled: true,
            heading: None,
            default_template: None,
            separate_pages: false,
            party: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignaturesPartyOverride {
    pub template: Option<String>,
    pub signatories: Option<Vec<SignatoryOverride>>,
    pub witness: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignatoryOverride {
    pub title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CoverConfig {
    pub enabled: bool,
    pub between_label: String,
    pub party_format: PartyFormat,
    pub show_ref: bool,
    pub show_author: bool,
    pub show_status: bool,
}

impl Default for CoverConfig {
    fn default() -> Self {
        CoverConfig {
            enabled: true,
            between_label: "BETWEEN".to_string(),
            party_format: PartyFormat::default(),
            show_ref: true,
            show_author: true,
            show_status: true,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyFormat {
    #[default]
    NameSpecRole,
    NameRole,
    NameOnly,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TocConfig {
    pub enabled: bool,
    pub heading: String,
}

impl Default for TocConfig {
    fn default() -> Self {
        TocConfig {
            enabled: true,
            heading: "Contents".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FooterConfig {
    pub show_ref: bool,
    pub show_page_number: bool,
    pub show_version: bool,
}

impl Default for FooterConfig {
    fn default() -> Self {
        FooterConfig {
            show_ref: true,
            show_page_number: true,
            show_version: false,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulePosition {
    #[default]
    End,
    AfterToc,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleOrder {
    #[default]
    Document,
    Alphabetical,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreambleStyle {
    #[default]
    Simple,
    Prose,
    Custom,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PreambleConfig {
    pub enabled: bool,
    pub style: PreambleStyle,
    pub template: String,
    pub party_template: String,
    pub party_separator: String,
}

impl Default for PreambleConfig {
    fn default() -> Self {
        PreambleConfig {
            enabled: false,
            style: PreambleStyle::Simple,
            template: "This {title} (**{type}**) is dated {date} between".to_string(),
            party_template: "{name} ({specifier}) (**{role}**)".to_string(),
            party_separator: "; and".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageSize {
    #[default]
    A4,
    Letter,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinedTermStyle {
    #[default]
    Bold,
    Quoted,
    BoldQuoted,
}

impl Default for StyleConfig {
    fn default() -> Self {
        StyleConfig {
            font_family: "Times New Roman".to_string(),
            font_size: 12.0,
            heading_font_family: "Times New Roman".to_string(),
            title_size: 20.0,
            heading1_size: 14.0,
            heading2_size: 12.0,
            heading_space_before: 18.0,
            heading_space_after: 12.0,
            paragraph_space_before: 0.0,
            paragraph_space_after: 6.0,
            line_spacing: 1.5,
            margin_top_cm: 2.54,
            margin_bottom_cm: 2.54,
            margin_left_cm: 2.54,
            margin_right_cm: 2.54,
            page_size: PageSize::A4,
            indent_per_level_cm: 1.27,
            hanging_indent_cm: 1.27,
            body_align_first_level: false,
            recitals_align_first_level: false,
            brand_color: None,
            date_format: "%e %B %Y".to_string(),
            defined_term_style: DefinedTermStyle::default(),
            cover: CoverConfig::default(),
            toc: TocConfig::default(),
            footer: FooterConfig::default(),
            preamble: PreambleConfig::default(),
            schedule_position: SchedulePosition::default(),
            schedule_order: ScheduleOrder::default(),
            signatures: SignaturesConfig::default(),
        }
    }
}

impl StyleConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: StyleConfig = toml::from_str(&content).map_err(|e| {
            LexiconError::Render(format!("Invalid style config: {}", e))
        })?;
        Ok(config)
    }

    /// Convert cm to twips (twentieths of a point). 1 cm = 567 twips.
    pub fn cm_to_twips(cm: f32) -> i32 {
        (cm * 567.0) as i32
    }

    /// Convert pt to half-points (docx-rs uses half-points for font size).
    pub fn pt_to_half_points(pt: f32) -> usize {
        (pt * 2.0) as usize
    }

    /// Convert pt to twentieths of a point (used for paragraph spacing before/after).
    pub fn pt_to_twips(pt: f32) -> u32 {
        (pt * 20.0) as u32
    }

    /// Return the brand color as a 6-char hex string (no #), or None.
    pub fn brand_color_hex(&self) -> Option<String> {
        self.brand_color.as_ref().map(|c| c.trim_start_matches('#').to_uppercase())
    }

    pub fn page_width_twips(&self) -> u32 {
        match self.page_size {
            PageSize::A4 => 11906,    // 210mm
            PageSize::Letter => 12240, // 8.5in
        }
    }

    pub fn page_height_twips(&self) -> u32 {
        match self.page_size {
            PageSize::A4 => 16838,    // 297mm
            PageSize::Letter => 15840, // 11in
        }
    }
}
