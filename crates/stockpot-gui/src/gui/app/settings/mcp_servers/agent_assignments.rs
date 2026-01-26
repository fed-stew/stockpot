//! Agent MCP assignments panel component.
//!
//! Renders the right panel for assigning MCP servers to agents.
//! Uses subtle selection styling: thin border + 8% accent tint.

use std::collections::HashMap;

use gpui::{
    div, prelude::*, px, rgb, rgba, Context, Hsla, MouseButton, Rgba, SharedString, Styled,
};

use crate::gui::app::ChatApp;
use crate::gui::theme::Theme;
use stockpot_core::agents::AgentInfo;
use stockpot_core::config::Settings;

use super::server_list::ServerInfo;

/// Selected/attached background opacity (8% tint).
const SELECTED_BG_OPACITY: f32 = 0.08;

/// Convert Rgba to Hsla with a specific opacity.
fn with_opacity(color: Rgba, opacity: f32) -> Hsla {
    let hsla: Hsla = color.into();
    hsla.opacity(opacity)
}

/// Renders the right panel for agent-to-MCP assignments.
pub fn render_agent_assignments(
    theme: &Theme,
    cx: &Context<ChatApp>,
    agents: &[AgentInfo],
    selected_agent: &str,
    servers: &[ServerInfo],
    agent_mcps: &[String],
    all_attachments: &HashMap<String, Vec<String>>,
) -> impl IntoElement {
    let theme = theme.clone();
    let selected_agent_owned = selected_agent.to_string();

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .pl(px(20.))
        .child(
            div()
                .text_size(px(14.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.text)
                .mb(px(12.))
                .child("🤖 Agent → MCP Attachments"),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.text_muted)
                .mb(px(12.))
                .child("Select an agent, then check which MCPs it should use."),
        )
        .child(
            div()
                .flex()
                .gap(px(16.))
                .flex_1()
                .min_h(px(0.))
                .child(render_agent_list(
                    &theme,
                    cx,
                    agents,
                    &selected_agent_owned,
                    all_attachments,
                ))
                .child(render_mcp_checkboxes(
                    &theme,
                    cx,
                    servers,
                    &selected_agent_owned,
                    agent_mcps,
                )),
        )
}

/// Renders the agent list (left side of right panel).
fn render_agent_list(
    theme: &Theme,
    cx: &Context<ChatApp>,
    agents: &[AgentInfo],
    selected_agent: &str,
    all_attachments: &HashMap<String, Vec<String>>,
) -> impl IntoElement {
    let theme = theme.clone();

    div()
        .w(px(200.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.text_muted)
                .mb(px(4.))
                .child("Agents"),
        )
        .child(
            div()
                .id("mcp-agents-scroll")
                .flex_1()
                .min_h(px(0.))
                .max_h(px(300.))
                .overflow_y_scroll()
                .scrollbar_width(px(6.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(agents.iter().map(|info| {
                    render_agent_item(&theme, cx, info, selected_agent, all_attachments)
                })),
        )
}

/// Renders a single agent item in the list.
/// Uses subtle selection styling: thin border + 8% accent tint.
fn render_agent_item(
    theme: &Theme,
    cx: &Context<ChatApp>,
    info: &AgentInfo,
    selected_agent: &str,
    all_attachments: &HashMap<String, Vec<String>>,
) -> impl IntoElement {
    let is_selected = info.name == selected_agent;
    let agent_mcp_list = all_attachments.get(&info.name);
    let mcp_count = agent_mcp_list.map(|m| m.len()).unwrap_or(0);
    let agent_name = info.name.clone();
    let display_name = info.display_name.clone();
    let theme = theme.clone();

    // Subtle selection styling: thin border + 8% accent tint
    let (bg_color, border_color, text_color) = if is_selected {
        (
            with_opacity(theme.accent, SELECTED_BG_OPACITY),
            theme.accent,
            theme.accent,
        )
    } else {
        (
            with_opacity(theme.tool_card, 1.0),
            theme.tool_card, // Invisible border (same as bg)
            theme.text,
        )
    };

    div()
        .id(SharedString::from(format!("mcp-agent-{}", agent_name)))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(6.))
        .border_1()
        .border_color(border_color)
        .bg(bg_color)
        .text_color(text_color)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.mcp_settings_selected_agent = agent_name.clone();
                cx.notify();
            }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_size(px(13.)).child(display_name))
                .when(mcp_count > 0, |d| {
                    d.child(
                        div()
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(10.))
                            .bg(if is_selected {
                                rgba(0x0078d433) // Slightly more visible on selected
                            } else {
                                theme.background
                            })
                            .text_size(px(11.))
                            .text_color(if is_selected {
                                theme.accent
                            } else {
                                theme.text_muted
                            })
                            .child(format!("{}", mcp_count)),
                    )
                }),
        )
}

/// Renders the MCP checkboxes for the selected agent.
fn render_mcp_checkboxes(
    theme: &Theme,
    cx: &Context<ChatApp>,
    servers: &[ServerInfo],
    selected_agent: &str,
    agent_mcps: &[String],
) -> impl IntoElement {
    let theme = theme.clone();
    let selected_agent_owned = selected_agent.to_string();

    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.text_muted)
                .mb(px(4.))
                .child(format!("MCPs for {}", selected_agent_owned)),
        )
        .when(servers.is_empty(), |d| {
            d.child(
                div()
                    .py(px(16.))
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("No MCPs available yet."),
            )
        })
        .child(
            div()
                .id("mcp-checkboxes-scroll")
                .flex_1()
                .min_h(px(0.))
                .max_h(px(300.))
                .overflow_y_scroll()
                .scrollbar_width(px(6.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(servers.iter().filter(|(_, enabled, _, _)| *enabled).map(
                    |(name, _, _, _)| {
                        render_mcp_checkbox(&theme, cx, name, &selected_agent_owned, agent_mcps)
                    },
                )),
        )
}

/// Renders a single MCP checkbox item.
/// Uses subtle selection styling: thin border + 8% accent tint when attached.
fn render_mcp_checkbox(
    theme: &Theme,
    cx: &Context<ChatApp>,
    mcp_name: &str,
    selected_agent: &str,
    agent_mcps: &[String],
) -> impl IntoElement {
    let mcp_name_owned = mcp_name.to_string();
    let mcp_name_display = mcp_name.to_string();
    let is_attached = agent_mcps.contains(&mcp_name_owned);
    let selected_agent_owned = selected_agent.to_string();
    let theme = theme.clone();

    // Subtle selection styling for attached state
    let (bg_color, border_color, text_color) = if is_attached {
        (
            with_opacity(theme.accent, SELECTED_BG_OPACITY),
            theme.accent,
            theme.accent,
        )
    } else {
        (
            with_opacity(theme.tool_card, 1.0),
            theme.tool_card, // Invisible border
            theme.text,
        )
    };

    div()
        .id(SharedString::from(format!("attach-mcp-{}", mcp_name_owned)))
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(8.))
        .rounded(px(6.))
        .border_1()
        .border_color(border_color)
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                let settings = Settings::new(&this.db);
                if is_attached {
                    let _ = settings.remove_agent_mcp(&selected_agent_owned, &mcp_name_owned);
                } else {
                    let _ = settings.add_agent_mcp(&selected_agent_owned, &mcp_name_owned);
                }
                cx.notify();
            }),
        )
        // Checkbox indicator
        .child(
            div()
                .w(px(18.))
                .h(px(18.))
                .rounded(px(4.))
                .border_2()
                .border_color(if is_attached {
                    theme.accent
                } else {
                    theme.border
                })
                .bg(if is_attached {
                    theme.accent
                } else {
                    theme.background
                })
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .when(is_attached, |d| d.child("✓")),
        )
        // MCP name label
        .child(
            div()
                .text_size(px(13.))
                .text_color(text_color)
                .child(mcp_name_display),
        )
}
