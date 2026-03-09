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
