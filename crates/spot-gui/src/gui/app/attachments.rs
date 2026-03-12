//! Attachment types for pending files and images in the chat input

use std::path::PathBuf;

/// Maximum number of attachments allowed
pub const MAX_ATTACHMENTS: usize = 10;

/// Thumbnail size for preview (square container)
pub const THUMBNAIL_SIZE: u32 = 120;

/// Maximum image dimension before resizing
pub const MAX_IMAGE_DIMENSION: u32 = 1000;

/// A pending attachment waiting to be sent
#[derive(Clone)]
pub enum PendingAttachment {
    /// An image attachment (will be sent as base64 PNG)
    Image(PendingImage),
    /// A non-image file attachment
    File(PendingFile),
    /// A PDF attachment (converted to images or text at send time)
    Pdf(PendingPdf),
}

/// A processed image ready for preview and sending
#[derive(Clone)]
pub struct PendingImage {
    /// Original file path (None if from clipboard paste)
    pub original_path: Option<PathBuf>,
    /// PNG bytes for the 120x120 thumbnail preview
    pub thumbnail_data: Vec<u8>,
    /// PNG bytes for sending (resized to ≤1000px)
    pub processed_data: Vec<u8>,
    /// Original filename for display
    pub filename: String,
    /// Processed image width
    pub width: u32,
    /// Processed image height
    pub height: u32,
}

/// A non-image file attachment
#[derive(Clone)]
pub struct PendingFile {
    /// Full path to the file
    pub path: PathBuf,
    /// Filename for display
    pub filename: String,
    /// File extension (lowercase, without dot)
    pub extension: String,
}

/// A PDF file attachment (processed lazily at send time)
#[derive(Clone)]
pub struct PendingPdf {
    /// Full path to the PDF file
    pub path: PathBuf,
    /// Filename for display
    pub filename: String,
    /// Number of pages in the PDF
    pub page_count: u32,
    /// PNG bytes for the first page thumbnail preview
    pub thumbnail_data: Option<Vec<u8>>,
}

impl PendingAttachment {
    /// Get display name for the attachment
    pub fn display_name(&self) -> &str {
        match self {
            PendingAttachment::Image(img) => &img.filename,
            PendingAttachment::File(file) => &file.filename,
            PendingAttachment::Pdf(pdf) => &pdf.filename,
        }
    }

    /// Check if this is an image attachment
    pub fn is_image(&self) -> bool {
        matches!(self, PendingAttachment::Image(_))
    }

    /// Check if this is a PDF attachment
    pub fn is_pdf(&self) -> bool {
        matches!(self, PendingAttachment::Pdf(_))
    }
}
