use std::path::{Path, PathBuf};

use crate::error::{LexiconError, Result};

/// Supported exhibit file types.
pub enum ExhibitFileType {
    Png,
    Jpeg,
    Pdf,
}

/// Loaded exhibit image: PNG bytes + pixel dimensions.
pub struct ExhibitImage {
    pub png_bytes: Vec<u8>,
    pub width_px: u32,
    pub height_px: u32,
}

/// Resolve an exhibit path relative to the input directory.
pub fn resolve_exhibit_path(path: &str, input_dir: Option<&Path>) -> Result<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Ok(p.to_path_buf());
    }
    match input_dir {
        Some(dir) => Ok(dir.join(p)),
        None => Err(LexiconError::Render(format!(
            "Exhibit path '{}' is relative but no input directory is available",
            path
        ))),
    }
}

/// Detect file type from extension.
pub fn detect_file_type(path: &Path) -> Result<ExhibitFileType> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "png" => Ok(ExhibitFileType::Png),
        "jpg" | "jpeg" => Ok(ExhibitFileType::Jpeg),
        "pdf" => Ok(ExhibitFileType::Pdf),
        _ => Err(LexiconError::Render(format!(
            "Unsupported exhibit file type '{}' for '{}'",
            ext,
            path.display()
        ))),
    }
}

/// Load an image file (PNG or JPEG), returning PNG bytes and dimensions.
pub fn load_image(path: &Path) -> Result<ExhibitImage> {
    let file_type = detect_file_type(path)?;

    let raw_bytes = std::fs::read(path).map_err(|e| {
        LexiconError::Render(format!("Failed to read exhibit file '{}': {}", path.display(), e))
    })?;

    match file_type {
        ExhibitFileType::Png => {
            let img = image::load_from_memory_with_format(&raw_bytes, image::ImageFormat::Png)
                .map_err(|e| {
                    LexiconError::Render(format!(
                        "Failed to decode PNG '{}': {}",
                        path.display(),
                        e
                    ))
                })?;
            Ok(ExhibitImage {
                png_bytes: raw_bytes,
                width_px: img.width(),
                height_px: img.height(),
            })
        }
        ExhibitFileType::Jpeg => {
            let img =
                image::load_from_memory_with_format(&raw_bytes, image::ImageFormat::Jpeg)
                    .map_err(|e| {
                        LexiconError::Render(format!(
                            "Failed to decode JPEG '{}': {}",
                            path.display(),
                            e
                        ))
                    })?;
            // Convert to PNG for docx-rs
            let mut png_buf = std::io::Cursor::new(Vec::new());
            img.write_to(&mut png_buf, image::ImageFormat::Png)
                .map_err(|e| {
                    LexiconError::Render(format!(
                        "Failed to convert JPEG to PNG for '{}': {}",
                        path.display(),
                        e
                    ))
                })?;
            Ok(ExhibitImage {
                png_bytes: png_buf.into_inner(),
                width_px: img.width(),
                height_px: img.height(),
            })
        }
        ExhibitFileType::Pdf => Err(LexiconError::Render(
            "PDF loading should use render_pdf_pages, not load_image".to_string(),
        )),
    }
}

/// Render PDF pages to PNG images via pdftoppm.
/// Returns one ExhibitImage per page.
pub fn render_pdf_pages(path: &Path) -> Result<Vec<ExhibitImage>> {
    let temp_dir = tempfile::tempdir().map_err(|e| {
        LexiconError::Render(format!("Failed to create temp directory: {}", e))
    })?;

    let output_prefix = temp_dir.path().join("page");

    let output = std::process::Command::new("pdftoppm")
        .args(["-png", "-r", "200"])
        .arg(path)
        .arg(&output_prefix)
        .output()
        .map_err(|e| {
            LexiconError::Render(format!(
                "Failed to run pdftoppm (is poppler-utils installed?): {}",
                e
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(LexiconError::Render(format!(
            "pdftoppm failed for '{}': {}",
            path.display(),
            stderr
        )));
    }

    // Collect output files (page-01.png, page-02.png, ...)
    let mut entries: Vec<PathBuf> = std::fs::read_dir(temp_dir.path())
        .map_err(|e| LexiconError::Render(format!("Failed to read temp directory: {}", e)))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();

    entries.sort();

    if entries.is_empty() {
        return Err(LexiconError::Render(format!(
            "pdftoppm produced no output for '{}'",
            path.display()
        )));
    }

    let mut pages = Vec::new();
    for entry in &entries {
        let bytes = std::fs::read(entry).map_err(|e| {
            LexiconError::Render(format!("Failed to read rendered PDF page: {}", e))
        })?;
        let img = image::load_from_memory(&bytes).map_err(|e| {
            LexiconError::Render(format!("Failed to decode rendered PDF page: {}", e))
        })?;
        pages.push(ExhibitImage {
            png_bytes: bytes,
            width_px: img.width(),
            height_px: img.height(),
        });
    }

    Ok(pages)
}

/// Load exhibit content: either a single image or multiple pages from a PDF.
pub fn load_exhibit(path: &str, input_dir: Option<&Path>) -> Result<Vec<ExhibitImage>> {
    let resolved = resolve_exhibit_path(path, input_dir)?;

    if !resolved.exists() {
        return Err(LexiconError::Render(format!(
            "Exhibit file not found: '{}'",
            resolved.display()
        )));
    }

    let file_type = detect_file_type(&resolved)?;

    match file_type {
        ExhibitFileType::Png | ExhibitFileType::Jpeg => {
            Ok(vec![load_image(&resolved)?])
        }
        ExhibitFileType::Pdf => render_pdf_pages(&resolved),
    }
}

/// Calculate display size in EMU to fit within page content area, preserving aspect ratio.
/// Returns (width_emu, height_emu).
pub fn fit_to_page(
    img_w_px: u32,
    img_h_px: u32,
    max_w_emu: u32,
    max_h_emu: u32,
) -> (u32, u32) {
    const EMU_PER_PX: u32 = 9525;

    let img_w_emu = img_w_px as u64 * EMU_PER_PX as u64;
    let img_h_emu = img_h_px as u64 * EMU_PER_PX as u64;

    if img_w_emu <= max_w_emu as u64 && img_h_emu <= max_h_emu as u64 {
        return (img_w_emu as u32, img_h_emu as u32);
    }

    let scale_w = max_w_emu as f64 / img_w_emu as f64;
    let scale_h = max_h_emu as f64 / img_h_emu as f64;
    let scale = scale_w.min(scale_h);

    (
        (img_w_emu as f64 * scale) as u32,
        (img_h_emu as f64 * scale) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fit_to_page_no_scaling_needed() {
        // Small image that fits
        let (w, h) = fit_to_page(100, 100, 5_000_000, 5_000_000);
        assert_eq!(w, 100 * 9525);
        assert_eq!(h, 100 * 9525);
    }

    #[test]
    fn test_fit_to_page_scale_by_width() {
        // Wide image: 2000px wide, 100px tall; max area 9_525_000 EMU wide
        let (w, h) = fit_to_page(2000, 100, 9_525_000, 9_525_000);
        // 2000px = 19_050_000 EMU, needs to scale to 9_525_000 = 0.5x
        assert_eq!(w, 9_525_000);
        assert_eq!(h, 952_500 / 2); // 100 * 9525 * 0.5
    }

    #[test]
    fn test_fit_to_page_scale_by_height() {
        // Tall image: 100px wide, 2000px tall; max area 9_525_000 EMU tall
        let (w, h) = fit_to_page(100, 2000, 9_525_000, 9_525_000);
        assert_eq!(h, 9_525_000);
        assert_eq!(w, 952_500 / 2);
    }

    #[test]
    fn test_fit_to_page_preserves_aspect_ratio() {
        let (w, h) = fit_to_page(1600, 1200, 5_000_000, 5_000_000);
        let original_ratio = 1600.0 / 1200.0;
        let fitted_ratio = w as f64 / h as f64;
        assert!((original_ratio - fitted_ratio).abs() < 0.01);
    }

    #[test]
    fn test_detect_file_type() {
        assert!(matches!(detect_file_type(Path::new("foo.png")), Ok(ExhibitFileType::Png)));
        assert!(matches!(detect_file_type(Path::new("foo.jpg")), Ok(ExhibitFileType::Jpeg)));
        assert!(matches!(detect_file_type(Path::new("foo.jpeg")), Ok(ExhibitFileType::Jpeg)));
        assert!(matches!(detect_file_type(Path::new("foo.pdf")), Ok(ExhibitFileType::Pdf)));
        assert!(matches!(detect_file_type(Path::new("foo.docx")), Err(_)));
    }

    #[test]
    fn test_resolve_exhibit_path_absolute() {
        let result = resolve_exhibit_path("/tmp/test.png", None).unwrap();
        assert_eq!(result, PathBuf::from("/tmp/test.png"));
    }

    #[test]
    fn test_resolve_exhibit_path_relative() {
        let result = resolve_exhibit_path("images/test.png", Some(Path::new("/docs"))).unwrap();
        assert_eq!(result, PathBuf::from("/docs/images/test.png"));
    }

    #[test]
    fn test_resolve_exhibit_path_relative_no_dir() {
        let result = resolve_exhibit_path("test.png", None);
        assert!(result.is_err());
    }
}
