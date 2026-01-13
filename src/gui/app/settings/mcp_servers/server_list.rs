//! MCP Server list panel component.
//!
//! Renders the left panel showing all defined MCP servers with
//! enable/disable and delete controls.

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::gui::app::ChatApp;
use crate::gui::theme::Theme;
use crate::mcp::McpConfig;

/// Server info tuple: (name, enabled, description, command_preview)
pub type ServerInfo = (String, bool, Option<String>, String);

/// Loads and returns sorted server info from the MCP config.
pub fn load_servers() -> Vec<ServerInfo> {
    let mcp_config = McpConfig::load_or_default();
    let mut servers: Vec<ServerInfo> = mcp_config
        .servers
        .iter()
        .map(|(name, entry)| {
            let cmd_preview = format!("{} {}", entry.command, entry.args.join(" "));
            (
                name.clone(),
                entry.enabled,
                entry.description.clone(),
                cmd_preview,
            )
        })
        .collect();
    servers.sort_by(|a, b| a.0.cmp(&b.0));
    servers
}

/// Renders the left panel with the MCP servers list.
pub fn render_server_list(
    theme: &Theme,
    cx: &Context<ChatApp>,
    servers: &[ServerInfo],
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w(px(380.))
        .min_h(px(0.))
        .pr(px(20.))
        .border_r_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(12.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.text)
                        .child("🔌 MCP Servers"),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.text_muted)
                        .child(format!("{} defined", servers.len())),
                ),
        )
        .child(render_servers_scroll_area(theme, cx, servers))
}

/// Renders the scrollable server list area.
fn render_servers_scroll_area(
    theme: &Theme,
    cx: &Context<ChatApp>,
    servers: &[ServerInfo],
) -> impl IntoElement {
    let theme = theme.clone();

    div()
        .id("mcp-servers-list")
        .flex_1()
        .min_h(px(0.))
        .max_h(px(400.))
        .overflow_y_scroll()
        .scrollbar_width(px(8.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .when(servers.is_empty(), |d| {
            d.child(
                div()
                    .px(px(16.))
                    .py(px(24.))
                    .rounded(px(8.))
                    .bg(theme.tool_card)
                    .text_size(px(13.))
                    .text_color(theme.text_muted)
                    .text_center()
                    .child("No MCP servers defined.\nClick 'Import from JSON' to add servers."),
            )
        })
        .children(servers.iter().map(|(name, enabled, desc, cmd_preview)| {
            render_server_card(&theme, cx, name, *enabled, desc.clone(), cmd_preview)
        }))
}

/// Renders a single server card with controls.
fn render_server_card(
    theme: &Theme,
    cx: &Context<ChatApp>,
    name: &str,
    enabled: bool,
    description: Option<String>,
    cmd_preview: &str,
) -> impl IntoElement {
    let theme = theme.clone();
    let server_name = name.to_string();
    let server_name_toggle = name.to_string();
    let server_name_del = name.to_string();
    let name_display = name.to_string();
    let cmd = cmd_preview.to_string();

    div()
        .id(SharedString::from(format!("mcp-server-{}", server_name)))
        .p(px(12.))
        .rounded(px(8.))
        .bg(theme.tool_card)
        .border_l_2()
        .border_color(if enabled { rgb(0x4ade80) } else { theme.border })
        .child(render_server_header(
            &theme,
            cx,
            &name_display,
            enabled,
            server_name_toggle,
            server_name_del,
        ))
        .when_some(description, |d, desc| {
            d.child(
                div()
                    .mt(px(6.))
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child(desc),
            )
        })
        .child(
            div()
                .mt(px(8.))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .bg(theme.background)
                .text_size(px(11.))
                .text_color(theme.text_muted)
                .overflow_hidden()
                .child(truncate_text(&cmd, 50)),
        )
}

/// Renders the header row of a server card with name, status, and controls.
fn render_server_header(
    theme: &Theme,
    cx: &Context<ChatApp>,
    name: &str,
    enabled: bool,
    server_name_toggle: String,
    server_name_del: String,
) -> impl IntoElement {
    let theme = theme.clone();
    let name_owned = name.to_string();

    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.text)
                        .child(name_owned),
                )
                .child(
                    div()
                        .px(px(6.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .text_size(px(11.))
                        .bg(if enabled {
                            rgba(0x4ade8033)
                        } else {
                            theme.background
                        })
                        .text_color(if enabled {
                            rgb(0x4ade80)
                        } else {
                            theme.text_muted
                        })
                        .child(if enabled { "enabled" } else { "disabled" }),
                ),
        )
        .child(render_server_controls(
            &theme,
            cx,
            enabled,
            server_name_toggle,
            server_name_del,
        ))
}

/// Renders the toggle and delete buttons for a server.
fn render_server_controls(
    theme: &Theme,
    cx: &Context<ChatApp>,
    enabled: bool,
    server_name_toggle: String,
    server_name_del: String,
) -> impl IntoElement {
    let theme = theme.clone();
    let toggle_name = server_name_toggle.clone();
    let del_name = server_name_del.clone();

    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .child(
            div()
                .id(SharedString::from(format!("toggle-mcp-{}", toggle_name)))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .text_size(px(11.))
                .text_color(theme.text_muted)
                .cursor_pointer()
                .hover(|s| s.bg(theme.background).text_color(theme.text))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |_this, _, _, cx| {
                        let mut config = McpConfig::load_or_default();
                        if let Some(entry) = config.servers.get_mut(&server_name_toggle) {
                            entry.enabled = !entry.enabled;
                            let _ = config.save_default();
                        }
                        cx.notify();
                    }),
                )
                .child(if enabled { "disable" } else { "enable" }),
        )
        .child(
            div()
                .id(SharedString::from(format!("delete-mcp-{}", del_name)))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .text_size(px(11.))
                .text_color(theme.text_muted)
                .cursor_pointer()
                .hover(|s| s.bg(rgba(0xff6b6b22)).text_color(rgb(0xff6b6b)))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |_this, _, _, cx| {
                        let mut config = McpConfig::load_or_default();
                        config.remove_server(&server_name_del);
                        let _ = config.save_default();
                        cx.notify();
                    }),
                )
                .child("×"),
        )
}

/// Truncates text to a maximum length with ellipsis.
fn truncate_text(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
