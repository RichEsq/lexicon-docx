use serde::Deserialize;
use std::path::Path;

use crate::error::{LexiconError, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StyleConfig {
    pub font_family: String,
    pub font_size: f32,
    pub heading_font_family: String,
    pub heading1_size: f32,
    pub heading2_size: f32,
    pub line_spacing: f32,
    pub margin_top_cm: f32,
    pub margin_bottom_cm: f32,
    pub margin_left_cm: f32,
    pub margin_right_cm: f32,
    pub page_size: PageSize,
    pub indent_per_level_cm: f32,
    pub hanging_indent_cm: f32,
    pub align_first_level: bool,
    pub brand_color: Option<String>,
    pub cover: CoverConfig,
    pub toc: TocConfig,
    pub footer: FooterConfig,
    pub schedule_position: SchedulePosition,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CoverConfig {
    pub enabled: bool,
    pub title_size: f32,
    pub date_format: String,
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
            title_size: 20.0,
            date_format: "%e %B %Y".to_string(),
            between_label: "BETWEEN".to_string(),
            party_format: PartyFormat::default(),
            show_ref: true,
            show_author: true,
            show_status: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartyFormat {
    NameSpecRole,
    NameRole,
    NameOnly,
}

impl Default for PartyFormat {
    fn default() -> Self {
        PartyFormat::NameSpecRole
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TocConfig {
    pub enabled: bool,
}

impl Default for TocConfig {
    fn default() -> Self {
        TocConfig { enabled: true }
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulePosition {
    End,
    AfterToc,
}

impl Default for SchedulePosition {
    fn default() -> Self {
        SchedulePosition::End
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageSize {
    A4,
    Letter,
}

impl Default for PageSize {
    fn default() -> Self {
        PageSize::A4
    }
}

impl Default for StyleConfig {
    fn default() -> Self {
        StyleConfig {
            font_family: "Times New Roman".to_string(),
            font_size: 12.0,
            heading_font_family: "Times New Roman".to_string(),
            heading1_size: 14.0,
            heading2_size: 12.0,
            line_spacing: 1.5,
            margin_top_cm: 2.54,
            margin_bottom_cm: 2.54,
            margin_left_cm: 2.54,
            margin_right_cm: 2.54,
            page_size: PageSize::A4,
            indent_per_level_cm: 1.27,
            hanging_indent_cm: 1.27,
            align_first_level: false,
            brand_color: None,
            cover: CoverConfig::default(),
            toc: TocConfig::default(),
            footer: FooterConfig::default(),
            schedule_position: SchedulePosition::default(),
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
