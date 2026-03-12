//! Image processing utilities for attachment handling
//!
//! Provides functions to:
//! - Load images from files or raw bytes
//! - Convert any supported format to PNG
//! - Resize large images to fit within constraints
//! - Generate thumbnails for preview

use std::io::Cursor;
use std::path::Path;

use image::{imageops::FilterType, DynamicImage, ImageFormat, ImageReader};

use crate::gui::app::{PendingImage, MAX_IMAGE_DIMENSION, THUMBNAIL_SIZE};

/// Supported image extensions (lowercase)
const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "ico",
];

/// Check if a file path points to a supported image format
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Process an image from a file path
///
/// Loads the image, converts to PNG, resizes if needed, and generates thumbnail.
pub fn process_image_from_path(path: &Path) -> anyhow::Result<PendingImage> {
    let img = ImageReader::open(path)?.with_guessed_format()?.decode()?;

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string();

    process_image_internal(img, Some(path.to_path_buf()), filename)
}

/// Process an image from raw bytes (e.g., from clipboard)
///
/// Detects format, converts to PNG, resizes if needed, and generates thumbnail.
pub fn process_image_from_bytes(
    data: &[u8],
    filename: Option<String>,
) -> anyhow::Result<PendingImage> {
    let img = ImageReader::new(Cursor::new(data))
        .with_guessed_format()?
        .decode()?;

    let filename = filename.unwrap_or_else(|| "pasted_image.png".to_string());

    process_image_internal(img, None, filename)
}

/// Internal processing: resize, convert to PNG, generate thumbnail
fn process_image_internal(
    img: DynamicImage,
    original_path: Option<std::path::PathBuf>,
    filename: String,
) -> anyhow::Result<PendingImage> {
    // Resize if needed (keep aspect ratio, fit within MAX_IMAGE_DIMENSION)
    let processed_img = resize_to_fit(img, MAX_IMAGE_DIMENSION);
    let (width, height) = (processed_img.width(), processed_img.height());

    // Encode processed image as PNG
    let processed_data = encode_as_png(&processed_img)?;

    // Generate thumbnail
    let thumbnail_data = generate_thumbnail(&processed_img, THUMBNAIL_SIZE)?;

    Ok(PendingImage {
        original_path,
        thumbnail_data,
        processed_data,
        filename,
        width,
        height,
    })
}

/// Resize image if either dimension exceeds max_pixels
///
/// Maintains aspect ratio using high-quality Lanczos3 filter.
fn resize_to_fit(img: DynamicImage, max_pixels: u32) -> DynamicImage {
    let (w, h) = (img.width(), img.height());

    if w <= max_pixels && h <= max_pixels {
        return img;
    }

    // Calculate new dimensions maintaining aspect ratio
    let ratio = (max_pixels as f64) / (w.max(h) as f64);
    let new_w = ((w as f64) * ratio).round() as u32;
    let new_h = ((h as f64) * ratio).round() as u32;

    img.resize(new_w, new_h, FilterType::Lanczos3)
}

/// Generate a square thumbnail that fits within max_size
///
/// Maintains aspect ratio - image will fit inside max_size x max_size box.
fn generate_thumbnail(img: &DynamicImage, max_size: u32) -> anyhow::Result<Vec<u8>> {
    let thumbnail = img.thumbnail(max_size, max_size);
    encode_as_png(&thumbnail)
}

/// Encode a DynamicImage as PNG bytes
fn encode_as_png(img: &DynamicImage) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    img.write_to(&mut cursor, ImageFormat::Png)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_file() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.JPEG")));
        assert!(is_image_file(Path::new("image.png")));
        assert!(is_image_file(Path::new("animation.gif")));
        assert!(is_image_file(Path::new("photo.webp")));
        assert!(!is_image_file(Path::new("document.pdf")));
        assert!(!is_image_file(Path::new("code.rs")));
    }

    #[test]
    fn test_resize_small_image_unchanged() {
        // Create a small test image (100x100)
        let img = DynamicImage::new_rgb8(100, 100);
        let result = resize_to_fit(img, 1000);
        assert_eq!(result.width(), 100);
        assert_eq!(result.height(), 100);
    }

    #[test]
    fn test_resize_large_image() {
        // Create a large test image (2000x1000)
        let img = DynamicImage::new_rgb8(2000, 1000);
        let result = resize_to_fit(img, 1000);
        assert_eq!(result.width(), 1000);
        assert_eq!(result.height(), 500);
    }

    #[test]
    fn test_resize_tall_image() {
        // Create a tall test image (500x1500)
        let img = DynamicImage::new_rgb8(500, 1500);
        let result = resize_to_fit(img, 1000);
        assert!(result.width() <= 1000);
        assert!(result.height() <= 1000);
        // Should be approximately 333x1000 (aspect ratio preserved)
    }
}
