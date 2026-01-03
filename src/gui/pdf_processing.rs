//! PDF processing utilities for attachment handling
//!
//! Provides functions to:
//! - Detect PDF files
//! - Get PDF preview (page count, thumbnail of first page)
//! - Render PDF pages to images (Image mode)
//! - Extract text from PDF pages (Text Extract mode)
//!
//! Note: Full PDF support requires mupdf which is only available on Unix.
//! On Windows, PDF files are detected but processing returns errors.

use std::path::Path;

#[cfg(unix)]
use image::{DynamicImage, RgbaImage};
#[cfg(unix)]
use mupdf::{Colorspace, Document, Matrix};

#[cfg(unix)]
use crate::gui::app::PendingImage;
#[cfg(unix)]
use crate::gui::image_processing::process_image_from_bytes;

/// Maximum number of PDF pages to process
pub const MAX_PDF_PAGES: usize = 20;

/// Maximum characters to extract in text mode (~25k tokens)
pub const MAX_PDF_TEXT_CHARS: usize = 100_000;

/// DPI for rendering PDF pages (150 is good balance of quality/size)
#[cfg(unix)]
const RENDER_DPI: f32 = 150.0;

/// PDF preview information
pub struct PdfPreview {
    pub page_count: u32,
    pub thumbnail_data: Vec<u8>,
}

/// Check if a file path points to a PDF
pub fn is_pdf_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("pdf"))
        .unwrap_or(false)
}

// ============================================================================
// Unix implementation (with mupdf)
// ============================================================================

#[cfg(unix)]
pub fn get_pdf_preview(path: &Path) -> anyhow::Result<PdfPreview> {
    let document = Document::open(
        path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path encoding"))?,
    )?;

    let page_count = document.page_count()? as u32;

    // Render first page as thumbnail (at lower DPI for speed)
    let thumbnail_data = if page_count > 0 {
        let page = document.load_page(0)?;
        let matrix = Matrix::new_scale(1.0, 1.0); // 72 DPI for thumbnail
        let pixmap = page.to_pixmap(&matrix, &Colorspace::device_rgb(), true, false)?;

        let width = pixmap.width();
        let height = pixmap.height();
        let samples = pixmap.samples();
        let n = pixmap.n() as usize;
        let stride = pixmap.stride() as usize;

        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height as usize {
            let row_start = y * stride;
            for x in 0..width as usize {
                let pixel_start = row_start + x * n;
                if n >= 3 && pixel_start + 2 < samples.len() {
                    rgba_data.push(samples[pixel_start]);
                    rgba_data.push(samples[pixel_start + 1]);
                    rgba_data.push(samples[pixel_start + 2]);
                    rgba_data.push(if n >= 4 && pixel_start + 3 < samples.len() {
                        samples[pixel_start + 3]
                    } else {
                        255
                    });
                } else if n == 1 && pixel_start < samples.len() {
                    let g = samples[pixel_start];
                    rgba_data.extend_from_slice(&[g, g, g, 255]);
                } else {
                    rgba_data.extend_from_slice(&[255, 255, 255, 255]);
                }
            }
        }

        let img = RgbaImage::from_raw(width, height, rgba_data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from pixmap"))?;

        let thumbnail = DynamicImage::ImageRgba8(img).thumbnail(120, 120);

        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);
        thumbnail.write_to(&mut cursor, image::ImageFormat::Png)?;
        buffer
    } else {
        Vec::new()
    };

    Ok(PdfPreview {
        page_count,
        thumbnail_data,
    })
}

#[cfg(unix)]
pub fn render_pdf_to_images(path: &Path, _max_dimension: u32) -> anyhow::Result<Vec<PendingImage>> {
    let document = Document::open(
        path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path encoding"))?,
    )?;

    let page_count = document.page_count()?;
    let pages_to_render = page_count.min(MAX_PDF_PAGES as i32);

    let filename_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("pdf");

    let mut images = Vec::new();

    let scale = RENDER_DPI / 72.0;
    let matrix = Matrix::new_scale(scale, scale);

    for i in 0..pages_to_render {
        let page = document.load_page(i)?;
        let pixmap = page.to_pixmap(&matrix, &Colorspace::device_rgb(), true, false)?;

        let width = pixmap.width();
        let height = pixmap.height();
        let samples = pixmap.samples();
        let n = pixmap.n() as usize;
        let stride = pixmap.stride() as usize;

        let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height as usize {
            let row_start = y * stride;
            for x in 0..width as usize {
                let pixel_start = row_start + x * n;
                if n >= 3 && pixel_start + 2 < samples.len() {
                    rgba_data.push(samples[pixel_start]);
                    rgba_data.push(samples[pixel_start + 1]);
                    rgba_data.push(samples[pixel_start + 2]);
                    rgba_data.push(if n >= 4 && pixel_start + 3 < samples.len() {
                        samples[pixel_start + 3]
                    } else {
                        255
                    });
                } else if n == 1 && pixel_start < samples.len() {
                    let g = samples[pixel_start];
                    rgba_data.extend_from_slice(&[g, g, g, 255]);
                } else {
                    rgba_data.extend_from_slice(&[255, 255, 255, 255]);
                }
            }
        }

        let img = RgbaImage::from_raw(width, height, rgba_data)
            .ok_or_else(|| anyhow::anyhow!("Failed to create image from pixmap"))?;

        let mut png_buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut png_buffer);
        DynamicImage::ImageRgba8(img).write_to(&mut cursor, image::ImageFormat::Png)?;

        let page_filename = format!("{}_page_{}.png", filename_stem, i + 1);
        let pending = process_image_from_bytes(&png_buffer, Some(page_filename))?;
        images.push(pending);
    }

    Ok(images)
}

#[cfg(unix)]
pub fn extract_pdf_text(path: &Path) -> anyhow::Result<String> {
    let document = Document::open(
        path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid path encoding"))?,
    )?;

    let page_count = document.page_count()?;
    let mut full_text = String::new();
    let mut total_chars = 0;

    for i in 0..page_count {
        if total_chars >= MAX_PDF_TEXT_CHARS {
            full_text.push_str("\n\n[... text truncated due to length limit ...]");
            break;
        }

        let page = document.load_page(i)?;
        let text = page.to_text()?;

        if i > 0 {
            full_text.push_str("\n\n");
        }
        full_text.push_str(&format!("--- Page {} ---\n", i + 1));
        full_text.push_str(&text);

        total_chars = full_text.len();
    }

    if full_text.len() > MAX_PDF_TEXT_CHARS {
        full_text.truncate(MAX_PDF_TEXT_CHARS);
        full_text.push_str("\n\n[... text truncated due to length limit ...]");
    }

    Ok(full_text)
}

// ============================================================================
// Windows stubs (no mupdf support)
// ============================================================================

#[cfg(windows)]
use crate::gui::app::PendingImage;

#[cfg(windows)]
pub fn get_pdf_preview(_path: &Path) -> anyhow::Result<PdfPreview> {
    anyhow::bail!("PDF processing is not supported on Windows")
}

#[cfg(windows)]
pub fn render_pdf_to_images(
    _path: &Path,
    _max_dimension: u32,
) -> anyhow::Result<Vec<PendingImage>> {
    anyhow::bail!("PDF processing is not supported on Windows")
}

#[cfg(windows)]
pub fn extract_pdf_text(_path: &Path) -> anyhow::Result<String> {
    anyhow::bail!("PDF processing is not supported on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pdf_file() {
        assert!(is_pdf_file(Path::new("document.pdf")));
        assert!(is_pdf_file(Path::new("document.PDF")));
        assert!(is_pdf_file(Path::new("/path/to/file.pdf")));
        assert!(!is_pdf_file(Path::new("image.png")));
        assert!(!is_pdf_file(Path::new("document.txt")));
        assert!(!is_pdf_file(Path::new("no_extension")));
    }
}
