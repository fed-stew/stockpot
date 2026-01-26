use std::io::Cursor;
use std::sync::Arc;

use gpui::{
    div, img, prelude::*, px, rgb, rgba, Context, ImageSource, MouseButton, RenderImage,
    SharedString, Styled,
};
use gpui_component::input::Input;
use image::codecs::png::PngDecoder;
use image::{DynamicImage, Frame};

use super::{ChatApp, PendingAttachment, MAX_ATTACHMENTS};
use crate::gui::components::current_spinner_frame;
use crate::gui::theme::Theme;

/// Size of the attachment preview container
const PREVIEW_SIZE: f32 = 120.0;
/// Border radius for preview cards
const BORDER_RADIUS: f32 = 12.0;
/// Size of the remove button
const REMOVE_BUTTON_SIZE: f32 = 24.0;

impl ChatApp {
    pub(super) fn render_input(&self, cx: &Context<Self>) -> impl IntoElement {
        let is_generating = self.is_generating;
        let theme = self.theme.clone();
        let attachments = self.pending_attachments.clone();
        let attachment_count = attachments.len();

        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(16.))
            .border_t_1()
            .border_color(self.theme.border)
            .bg(self.theme.panel_background)
            // Attachment previews row (only shown if there are attachments)
            .when(!attachments.is_empty(), |this| {
                this.child(self.render_attachments_preview(&attachments, &theme, cx))
            })
            // Input row (text input + send button)
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(12.))
                    .child(Input::new(&self.input_state).cleanable(true).flex_1())
                    .child(
                        div()
                            .id("send-btn")
                            .px(px(16.))
                            .py(px(10.))
                            .rounded(px(8.))
                            .bg(if is_generating {
                                theme.text_muted
                            } else {
                                theme.accent
                            })
                            .text_color(rgb(0xffffff))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    if !this.is_generating {
                                        this.send_message(window, cx);
                                    }
                                }),
                            )
                            .child(if is_generating {
                                current_spinner_frame()
                            } else if attachment_count > 0 {
                                "Send 📎→"
                            } else {
                                "Send →"
                            }),
                    ),
            )
    }

    /// Render the attachment preview row
    fn render_attachments_preview(
        &self,
        attachments: &[PendingAttachment],
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
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
                    .children(
                        attachments.iter().enumerate().map(|(index, att)| {
                            self.render_single_attachment(att, index, theme, cx)
                        }),
                    ),
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
    }

    /// Render a single attachment preview card
    fn render_single_attachment(
        &self,
        attachment: &PendingAttachment,
        index: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        match attachment {
            PendingAttachment::Image(img) => self
                .render_image_attachment(img, index, theme, cx)
                .into_any_element(),
            PendingAttachment::File(file) => self
                .render_file_attachment(file, index, theme, cx)
                .into_any_element(),
            PendingAttachment::Pdf(pdf) => self
                .render_pdf_attachment(pdf, index, theme, cx)
                .into_any_element(),
        }
    }

    /// Render an image attachment preview
    fn render_image_attachment(
        &self,
        image: &super::PendingImage,
        index: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let image_source = png_to_image_source(&image.thumbnail_data);
        let has_image = image_source.is_some();
        let error_color = theme.error;

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
            // Remove button
            .child(
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.remove_attachment(index, cx);
                        }),
                    ),
            )
    }

    /// Render a file attachment preview
    fn render_file_attachment(
        &self,
        file: &super::PendingFile,
        index: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let icon = get_file_icon(&file.extension);
        let filename = truncate_filename(&file.filename, 12);
        let error_color = theme.error;

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
            .child(
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.remove_attachment(index, cx);
                        }),
                    ),
            )
    }

    /// Render a PDF attachment preview
    fn render_pdf_attachment(
        &self,
        pdf: &super::PendingPdf,
        index: usize,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
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
        let error_color = theme.error;

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
            .child(
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
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.remove_attachment(index, cx);
                        }),
                    ),
            )
    }
}

/// Convert PNG bytes to a gpui ImageSource
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
