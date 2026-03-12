//! Attachment preview components for displaying pending images and files
//!
//! Renders a row of attachment thumbnails with remove buttons.

use std::io::Cursor;
use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, rgb, rgba, ImageSource, MouseButton, RenderImage, SharedString,
    Styled,
};
use image::codecs::png::PngDecoder;
use image::{DynamicImage, Frame};

use crate::gui::app::{PendingAttachment, PendingFile, PendingImage, PendingPdf, MAX_ATTACHMENTS};
use crate::gui::theme::Theme;

/// Size of the attachment preview container
const PREVIEW_SIZE: f32 = 120.0;
/// Border radius for preview cards
const BORDER_RADIUS: f32 = 12.0;
/// Size of the remove button
const REMOVE_BUTTON_SIZE: f32 = 24.0;

/// Convert PNG bytes to a gpui ImageSource
///
/// Decodes PNG, converts RGBA to BGRA (gpui's internal format), and wraps in RenderImage.
fn png_to_image_source(png_bytes: &[u8]) -> Option<ImageSource> {
    let cursor = Cursor::new(png_bytes);
    let decoder = PngDecoder::new(cursor).ok()?;
    let mut data = DynamicImage::from_decoder(decoder).ok()?.into_rgba8();

    // Convert from RGBA to BGRA (gpui uses BGRA internally)
    for pixel in data.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let frame = Frame::new(data);
    let render_image = RenderImage::new(vec![frame]);
    Some(ImageSource::Render(Arc::new(render_image)))
}

/// Render a single image attachment preview
pub fn render_image_preview<F>(
    image: &PendingImage,
    index: usize,
    theme: &Theme,
    on_remove: F,
) -> impl IntoElement
where
    F: Fn(usize) + 'static,
{
    let image_source = png_to_image_source(&image.thumbnail_data);
    let has_image = image_source.is_some();

    div()
        .id(SharedString::from(format!("attachment-{}", index)))
        .relative()
        .w(px(PREVIEW_SIZE))
        .h(px(PREVIEW_SIZE))
        .rounded(px(BORDER_RADIUS))
        .bg(theme.panel_background)
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_center()
        // Image thumbnail or placeholder
        .child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .when_some(image_source, |el, src| {
                    el.child(
                        img(src)
                            .max_w(px(PREVIEW_SIZE - 8.0))
                            .max_h(px(PREVIEW_SIZE - 8.0)),
                    )
                })
                .when(!has_image, |el| {
                    el.child(div().text_size(px(40.0)).child("🖼️"))
                }),
        )
        // Remove button (always visible in corner)
        .child(render_remove_button(index, theme, on_remove))
}

/// Render a single file attachment preview (placeholder icon)
pub fn render_file_preview<F>(
    file: &PendingFile,
    index: usize,
    theme: &Theme,
    on_remove: F,
) -> impl IntoElement
where
    F: Fn(usize) + 'static,
{
    let icon = get_file_icon(&file.extension);
    let filename = truncate_filename(&file.filename, 12);

    div()
        .id(SharedString::from(format!("attachment-{}", index)))
        .relative()
        .w(px(PREVIEW_SIZE))
        .h(px(PREVIEW_SIZE))
        .rounded(px(BORDER_RADIUS))
        .bg(theme.panel_background)
        .border_1()
        .border_color(theme.border)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        // File icon
        .child(div().text_size(px(40.0)).child(icon))
        // Filename (truncated)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme.text_muted)
                .max_w(px(PREVIEW_SIZE - 16.0))
                .overflow_hidden()
                .text_ellipsis()
                .child(filename),
        )
        // Remove button
        .child(render_remove_button(index, theme, on_remove))
}

/// Render a single PDF attachment preview (thumbnail or icon + page count)
pub fn render_pdf_preview<F>(
    pdf: &PendingPdf,
    index: usize,
    theme: &Theme,
    on_remove: F,
) -> impl IntoElement
where
    F: Fn(usize) + 'static,
{
    let image_source = pdf
        .thumbnail_data
        .as_ref()
        .and_then(|data| png_to_image_source(data));
    let has_thumbnail = image_source.is_some();
    let filename = truncate_filename(&pdf.filename, 12);
    let page_info = format!(
        "{} page{}",
        pdf.page_count,
        if pdf.page_count == 1 { "" } else { "s" }
    );

    div()
        .id(SharedString::from(format!("attachment-{}", index)))
        .relative()
        .w(px(PREVIEW_SIZE))
        .h(px(PREVIEW_SIZE))
        .rounded(px(BORDER_RADIUS))
        .bg(theme.panel_background)
        .border_1()
        .border_color(theme.border)
        .overflow_hidden()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        // Thumbnail or PDF icon
        .child(
            div()
                .flex_1()
                .w_full()
                .flex()
                .items_center()
                .justify_center()
                .when_some(image_source, |el, src| {
                    el.child(
                        img(src)
                            .max_w(px(PREVIEW_SIZE - 8.0))
                            .max_h(px(PREVIEW_SIZE - 40.0)),
                    )
                })
                .when(!has_thumbnail, |el| {
                    el.child(div().text_size(px(40.0)).child("📕"))
                }),
        )
        // Bottom info bar (filename + page count)
        .child(
            div()
                .w_full()
                .px(px(6.0))
                .py(px(4.0))
                .bg(rgba(0x00000040))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgb(0xffffff))
                        .max_w(px(PREVIEW_SIZE - 16.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(filename),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(rgba(0xffffffaa))
                        .child(page_info),
                ),
        )
        // Remove button
        .child(render_remove_button(index, theme, on_remove))
}

/// Render the remove (×) button in top-right corner
fn render_remove_button<F>(index: usize, theme: &Theme, on_remove: F) -> impl IntoElement
where
    F: Fn(usize) + 'static,
{
    let error_color = theme.error;

    div()
        .id(SharedString::from(format!("remove-{}", index)))
        .absolute()
        .top(px(4.0))
        .right(px(4.0))
        .w(px(REMOVE_BUTTON_SIZE))
        .h(px(REMOVE_BUTTON_SIZE))
        .rounded_full()
        .bg(rgba(0x00000080))
        .hover(|s| s.bg(error_color))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            div()
                .text_size(px(14.0))
                .text_color(rgb(0xffffff))
                .child("×"),
        )
        .on_mouse_down(MouseButton::Left, move |_, _, _| {
            on_remove(index);
        })
}

/// Render a single attachment (dispatches to image, file, or PDF)
pub fn render_attachment_preview<F>(
    attachment: &PendingAttachment,
    index: usize,
    theme: &Theme,
    on_remove: F,
) -> impl IntoElement
where
    F: Fn(usize) + Clone + 'static,
{
    match attachment {
        PendingAttachment::Image(img) => {
            render_image_preview(img, index, theme, on_remove).into_any_element()
        }
        PendingAttachment::File(file) => {
            render_file_preview(file, index, theme, on_remove).into_any_element()
        }
        PendingAttachment::Pdf(pdf) => {
            render_pdf_preview(pdf, index, theme, on_remove).into_any_element()
        }
    }
}

/// Render the entire attachment preview row
pub fn render_attachments_row<F>(
    attachments: &[PendingAttachment],
    theme: &Theme,
    on_remove: F,
) -> impl IntoElement
where
    F: Fn(usize) + Clone + 'static,
{
    if attachments.is_empty() {
        return div().into_any_element();
    }

    let count = attachments.len();
    let warning_color = theme.warning;
    let muted_color = theme.text_muted;

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .child(
            // Scrollable row of previews
            div()
                .w_full()
                .flex()
                .flex_row()
                .gap(px(12.0))
                .overflow_x_hidden()
                .pb(px(8.0))
                .children(attachments.iter().enumerate().map(|(i, att)| {
                    let on_remove = on_remove.clone();
                    render_attachment_preview(att, i, theme, on_remove)
                })),
        )
        .child(
            // Attachment count indicator
            div()
                .text_size(px(11.0))
                .text_color(if count >= MAX_ATTACHMENTS {
                    warning_color
                } else {
                    muted_color
                })
                .child(format!("{}/{} attachments", count, MAX_ATTACHMENTS)),
        )
        .into_any_element()
}

/// Get an emoji icon for a file based on its extension
fn get_file_icon(extension: &str) -> &'static str {
    match extension.to_lowercase().as_str() {
        "pdf" => "📕",
        "doc" | "docx" => "📘",
        "xls" | "xlsx" => "📗",
        "ppt" | "pptx" => "📙",
        "txt" | "md" => "📄",
        "zip" | "tar" | "gz" | "rar" | "7z" => "📦",
        "rs" | "py" | "js" | "ts" | "go" | "c" | "cpp" | "h" => "💻",
        "json" | "yaml" | "yml" | "toml" | "xml" => "⚙️",
        "html" | "css" => "🌐",
        "mp3" | "wav" | "flac" | "ogg" => "🎵",
        "mp4" | "mov" | "avi" | "mkv" => "🎬",
        _ => "📄",
    }
}

/// Truncate filename for display
fn truncate_filename(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len.saturating_sub(3)])
    }
}
