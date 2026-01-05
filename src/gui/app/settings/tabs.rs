//! Main settings tabs rendering
//!
//! Contains the SettingsTab enum and the main render_settings() entry point
//! that dispatches to individual tab renderers.

use gpui::{
    anchored, deferred, div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString,
    StatefulInteractiveElement, Styled,
};

use crate::gui::app::ChatApp;
use crate::gui::components::scrollbar;

/// The available settings tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    PinnedAgents,
    McpServers,
    Models,
    General,
}

impl SettingsTab {
    fn label(self) -> &'static str {
        match self {
            Self::PinnedAgents => "Pinned Agents",
            Self::McpServers => "MCP Servers",
            Self::Models => "Models",
            Self::General => "General",
        }
    }
}

impl ChatApp {
    /// Renders the main settings panel with tab navigation
    pub(crate) fn render_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_settings;
        let tab = self.settings_tab;

        div().when(show, |d| {
            d.absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(theme.background)
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if this.show_default_model_dropdown {
                            this.show_default_model_dropdown = false;
                            cx.notify();
                        }
                    }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .size_full()
                        .child(
                            // Header
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(20.))
                                .py(px(14.))
                                .border_b_1()
                                .border_color(theme.border)
                                .bg(theme.panel_background)
                                .child(
                                    div()
                                        .text_size(px(16.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.text)
                                        .child("Settings"),
                                )
                                .child(
                                    div()
                                        .id("close-settings")
                                        .px(px(10.))
                                        .py(px(6.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.tool_card))
                                        .text_color(theme.text_muted)
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.show_settings = false;
                                                this.show_default_model_dropdown = false;
                                                this.default_model_dropdown_bounds = None;
                                                cx.notify();
                                            }),
                                        )
                                        .child("✕"),
                                ),
                        )
                        .child(
                            // Tabs
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .px(px(20.))
                                .py(px(10.))
                                .border_b_1()
                                .border_color(theme.border)
                                .bg(theme.panel_background)
                                .children(
                                    [
                                        SettingsTab::PinnedAgents,
                                        SettingsTab::McpServers,
                                        SettingsTab::Models,
                                        SettingsTab::General,
                                    ]
                                    .into_iter()
                                    .map(|t| {
                                        let is_selected = t == tab;
                                        div()
                                            .id(SharedString::from(format!("settings-tab-{:?}", t)))
                                            .px(px(12.))
                                            .py(px(7.))
                                            .rounded(px(999.))
                                            .bg(if is_selected {
                                                theme.accent
                                            } else {
                                                theme.tool_card
                                            })
                                            .text_color(if is_selected {
                                                rgb(0xffffff)
                                            } else {
                                                theme.text
                                            })
                                            .text_size(px(12.))
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.9))
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.settings_tab = t;
                                                    cx.notify();
                                                }),
                                            )
                                            .child(t.label())
                                    }),
                                ),
                        )
                        .child(
                            // Content
                            div()
                                .id("settings-content-wrap")
                                .flex()
                                .flex_1()
                                .min_h(px(0.))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .id("settings-content-scroll")
                                        .flex_1()
                                        .min_h(px(0.))
                                        .overflow_y_scroll()
                                        .track_scroll(&self.settings_scroll_handle)
                                        .on_scroll_wheel(cx.listener(|_, _, _, cx| {
                                            cx.notify();
                                        }))
                                        .px(px(20.))
                                        .py(px(18.))
                                        .child(
                                            div()
                                                .when(tab == SettingsTab::PinnedAgents, |d| {
                                                    d.child(self.render_settings_pinned_agents(cx))
                                                })
                                                .when(tab == SettingsTab::McpServers, |d| {
                                                    d.child(self.render_settings_mcp_servers(cx))
                                                })
                                                .when(tab == SettingsTab::Models, |d| {
                                                    d.child(self.render_settings_models(cx))
                                                })
                                                .when(tab == SettingsTab::General, |d| {
                                                    d.child(self.render_settings_general(cx))
                                                }),
                                        ),
                                )
                                .child(scrollbar(
                                    self.settings_scroll_handle.clone(),
                                    self.settings_scrollbar_drag.clone(),
                                    theme.clone(),
                                )),
                        ),
                )
        })
    }
}
