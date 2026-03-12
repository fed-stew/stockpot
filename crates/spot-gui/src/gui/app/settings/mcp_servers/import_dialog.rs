//! MCP Import Dialog component.
//!
//! Renders the modal dialog for importing MCP server configurations from JSON.

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::gui::app::ChatApp;
use crate::gui::theme::Theme;

/// Renders the MCP import dialog overlay.
pub fn render_import_dialog(
    theme: &Theme,
    cx: &Context<ChatApp>,
    show: bool,
    import_json: &str,
    import_error: Option<&String>,
) -> impl IntoElement {
    div().when(show, |d| {
        d.absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(rgba(0x000000aa))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .child(render_dialog_content(theme, cx, import_json, import_error))
    })
}

/// Renders the dialog content card.
fn render_dialog_content(
    theme: &Theme,
    cx: &Context<ChatApp>,
    import_json: &str,
    import_error: Option<&String>,
) -> impl IntoElement {
    let theme = theme.clone();
    let import_json_owned = import_json.to_string();

    div()
        .w(px(600.))
        .max_h(px(500.))
        .bg(theme.panel_background)
        .border_1()
        .border_color(theme.border)
        .rounded(px(12.))
        .flex()
        .flex_col()
        .overflow_hidden()
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .child(render_dialog_header(&theme, cx))
        .child(render_dialog_body(
            &theme,
            cx,
            &import_json_owned,
            import_error,
        ))
}

/// Renders the dialog header with title and close button.
fn render_dialog_header(theme: &Theme, cx: &Context<ChatApp>) -> impl IntoElement {
    let theme = theme.clone();

    div()
        .flex()
        .items_center()
        .justify_between()
        .px(px(20.))
        .py(px(14.))
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .text_size(px(15.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text)
                .child("📋 Import MCP Config from JSON"),
        )
        .child(
            div()
                .id("close-mcp-import")
                .px(px(8.))
                .py(px(4.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|s| s.bg(theme.tool_card))
                .text_color(theme.text_muted)
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.show_mcp_import_dialog = false;
                        this.mcp_import_json.clear();
                        this.mcp_import_error = None;
                        cx.notify();
                    }),
                )
                .child("✕"),
        )
}

/// Renders the dialog body with JSON preview and action buttons.
fn render_dialog_body(
    theme: &Theme,
    cx: &Context<ChatApp>,
    import_json: &str,
    import_error: Option<&String>,
) -> impl IntoElement {
    let theme = theme.clone();
    let has_content = !import_json.is_empty();

    div()
        .flex_1()
        .p(px(20.))
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.text_muted)
                .child("Paste your MCP config JSON (Claude Desktop format):"),
        )
        .child(render_json_preview(&theme, import_json))
        .when_some(import_error, |d, err| {
            d.child(render_error_message(&theme, err))
        })
        .child(render_action_buttons(&theme, cx, has_content))
}

/// Renders the JSON preview area.
fn render_json_preview(theme: &Theme, import_json: &str) -> impl IntoElement {
    let theme = theme.clone();
    let has_content = !import_json.is_empty();
    let display_text = if has_content {
        SharedString::from(import_json.to_string())
    } else {
        SharedString::from(
            r#"{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["@playwright/mcp@latest"]
    }
  }
}"#,
        )
    };

    div()
        .id("mcp-json-preview")
        .flex_1()
        .min_h(px(150.))
        .p(px(12.))
        .rounded(px(8.))
        .bg(theme.background)
        .border_1()
        .border_color(theme.border)
        .overflow_y_scroll()
        .scrollbar_width(px(6.))
        .child(
            div()
                .text_size(px(12.))
                .font_family("monospace")
                .text_color(if has_content {
                    theme.text
                } else {
                    theme.text_muted
                })
                .child(display_text),
        )
}

/// Renders the error message if present.
fn render_error_message(_theme: &Theme, error: &str) -> impl IntoElement {
    div()
        .px(px(12.))
        .py(px(8.))
        .rounded(px(6.))
        .bg(rgba(0xff6b6b22))
        .text_size(px(12.))
        .text_color(rgb(0xff6b6b))
        .child(error.to_string())
}

/// Renders the action buttons row.
fn render_action_buttons(
    theme: &Theme,
    cx: &Context<ChatApp>,
    has_content: bool,
) -> impl IntoElement {
    let theme = theme.clone();

    div()
        .flex()
        .gap(px(8.))
        .child(render_paste_button(&theme, cx))
        .child(render_clear_button(&theme, cx))
        .child(div().flex_1()) // spacer
        .when(has_content, |d| d.child(render_import_button(&theme, cx)))
}

/// Renders the "Paste from Clipboard" button.
fn render_paste_button(theme: &Theme, cx: &Context<ChatApp>) -> impl IntoElement {
    let theme = theme.clone();

    div()
        .id("paste-mcp-json")
        .px(px(16.))
        .py(px(10.))
        .rounded(px(6.))
        .bg(theme.tool_card)
        .text_color(theme.text)
        .text_size(px(13.))
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    this.mcp_import_json = text.to_string();
                    this.mcp_import_error = None;
                    cx.notify();
                }
            }),
        )
        .child("📋 Paste from Clipboard")
}

/// Renders the "Clear" button.
fn render_clear_button(theme: &Theme, cx: &Context<ChatApp>) -> impl IntoElement {
    let theme = theme.clone();

    div()
        .id("clear-mcp-json")
        .px(px(16.))
        .py(px(10.))
        .rounded(px(6.))
        .bg(theme.background)
        .text_color(theme.text_muted)
        .text_size(px(13.))
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.mcp_import_json.clear();
                this.mcp_import_error = None;
                cx.notify();
            }),
        )
        .child("Clear")
}

/// Renders the "Import Servers" button.
fn render_import_button(theme: &Theme, cx: &Context<ChatApp>) -> impl IntoElement {
    let theme = theme.clone();

    div()
        .id("import-mcp-btn")
        .px(px(20.))
        .py(px(10.))
        .rounded(px(6.))
        .bg(theme.accent)
        .text_color(rgb(0xffffff))
        .text_size(px(13.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.do_mcp_import(cx);
            }),
        )
        .child("Import Servers")
}
