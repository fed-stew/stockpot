//! Import section component for MCP servers settings.
//!
//! Renders the top "Import from JSON" button and description.

use gpui::{div, prelude::*, px, rgb, Context, MouseButton, Styled};

use crate::gui::app::ChatApp;
use crate::gui::theme::Theme;

/// Renders the import section with the "Import from JSON" button.
pub fn render_import_section(theme: &Theme, cx: &Context<ChatApp>) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .mb(px(16.))
        .pb(px(16.))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .id("import-mcp-json")
                .px(px(16.))
                .py(px(10.))
                .rounded(px(8.))
                .bg(theme.accent)
                .text_color(rgb(0xffffff))
                .text_size(px(13.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .cursor_pointer()
                .hover(|s| s.opacity(0.9))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.show_mcp_import_dialog = true;
                        this.mcp_import_json.clear();
                        this.mcp_import_error = None;
                        cx.notify();
                    }),
                )
                .child("📋 Import from JSON"),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.text_muted)
                .child("Paste Claude Desktop / standard MCP config format"),
        )
}
