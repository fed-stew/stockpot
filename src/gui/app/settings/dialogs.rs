//! Shared dialog components for settings
//!
//! Contains OAuth status display and API keys management dialog.

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::gui::app::ChatApp;

impl ChatApp {
    pub(crate) fn render_oauth_status(
        &self,
        provider: &'static str,
        display_name: &'static str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        use crate::auth::TokenStorage;

        let theme = self.theme.clone();
        let storage = TokenStorage::new(&self.db);
        let is_authenticated = storage.is_authenticated(provider).unwrap_or(false);

        div()
            .id(SharedString::from(format!("oauth-{}", provider)))
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(10.))
            .rounded(px(8.))
            .bg(theme.tool_card)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(theme.text)
                            .child(display_name),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(if is_authenticated {
                                rgb(0x4ade80)
                            } else {
                                theme.text_muted
                            })
                            .child(if is_authenticated {
                                "✓ Connected"
                            } else {
                                "Not connected"
                            }),
                    ),
            )
            .child(
                div()
                    .id(SharedString::from(format!("oauth-btn-{}", provider)))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(if is_authenticated {
                        theme.background
                    } else {
                        theme.accent
                    })
                    .text_color(if is_authenticated {
                        theme.text
                    } else {
                        rgb(0xffffff)
                    })
                    .text_size(px(12.))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.start_oauth_flow(provider, cx);
                        }),
                    )
                    .child(if is_authenticated {
                        "Reconnect"
                    } else {
                        "Connect"
                    }),
            )
    }

    pub(crate) fn render_api_keys_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_api_keys_dialog;

        div().when(show, |d| {
            d.child(
                div()
                    .id("api-keys-backdrop")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(rgba(0x000000aa))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.show_api_keys_dialog = false;
                            this.api_key_new_name.clear();
                            this.api_key_new_value.clear();
                            cx.notify();
                        }),
                    )
                    .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .w(px(450.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        .max_h(px(500.))
                        .bg(theme.panel_background)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(12.))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
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
                                        .child("🔑 API Keys"),
                                )
                                .child(
                                    div()
                                        .id("close-api-keys")
                                        .px(px(8.))
                                        .py(px(4.))
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
                                                this.show_api_keys_dialog = false;
                                                this.api_key_new_name.clear();
                                                this.api_key_new_value.clear();
                                                cx.notify();
                                            }),
                                        )
                                        .child("✕"),
                                ),
                        )
                        .child(
                            div()
                                .id("api-keys-content")
                                .flex_1()
                                .overflow_y_scroll().scrollbar_width(px(8.))
                                .p(px(20.))
                                .flex()
                                .flex_col()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child(format!(
                                            "Stored Keys ({})",
                                            self.api_keys_list.len()
                                        )),
                                )
                                .when(self.api_keys_list.is_empty(), |d| {
                                    d.child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.text_muted)
                                            .py(px(8.))
                                            .child("No API keys stored yet."),
                                    )
                                })
                                .children(self.api_keys_list.iter().map(|key_name| {
                                    let name = key_name.clone();
                                    let name_for_delete = key_name.clone();

                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .px(px(12.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .bg(theme.tool_card)
                                        .child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(8.))
                                                .child(
                                                    div()
                                                        .text_size(px(13.))
                                                        .text_color(theme.text)
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(11.))
                                                        .text_color(theme.text_muted)
                                                        .child("••••••••"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id(SharedString::from(format!(
                                                    "delete-key-{}",
                                                    name_for_delete
                                                )))
                                                .px(px(8.))
                                                .py(px(4.))
                                                .rounded(px(4.))
                                                .text_size(px(11.))
                                                .text_color(rgb(0xff6b6b))
                                                .cursor_pointer()
                                                .hover(|s| s.bg(theme.background))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, _, cx| {
                                                        let _ = this
                                                            .db
                                                            .delete_api_key(&name_for_delete);
                                                        this.refresh_api_keys_list();
                                                        cx.notify();
                                                    }),
                                                )
                                                .child("Delete"),
                                        )
                                }))
                                .child(
                                    div()
                                        .mt(px(8.))
                                        .pt(px(12.))
                                        .border_t_1()
                                        .border_color(theme.border)
                                        .flex()
                                        .flex_col()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.text_muted)
                                                .child("Add New Key"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(8.))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.background)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .child(
                                                            div()
                                                                .text_size(px(12.))
                                                                .text_color(if self
                                                                    .api_key_new_name
                                                                    .is_empty()
                                                                {
                                                                    theme.text_muted
                                                                } else {
                                                                    theme.text
                                                                })
                                                                .child(if self
                                                                    .api_key_new_name
                                                                    .is_empty()
                                                                {
                                                                    SharedString::from(
                                                                        "Name (e.g., OPENAI_API_KEY)",
                                                                    )
                                                                } else {
                                                                    SharedString::from(
                                                                        self.api_key_new_name
                                                                            .clone(),
                                                                    )
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id("paste-key-name")
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.tool_card)
                                                        .text_size(px(11.))
                                                        .text_color(theme.text)
                                                        .cursor_pointer()
                                                        .hover(|s| s.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                if let Some(text) = cx
                                                                    .read_from_clipboard()
                                                                    .and_then(|i| i.text())
                                                                {
                                                                    this.api_key_new_name =
                                                                        text.to_string();
                                                                    cx.notify();
                                                                }
                                                            }),
                                                        )
                                                        .child("Paste"),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .gap(px(8.))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.background)
                                                        .border_1()
                                                        .border_color(theme.border)
                                                        .child(
                                                            div()
                                                                .text_size(px(12.))
                                                                .text_color(if self
                                                                    .api_key_new_value
                                                                    .is_empty()
                                                                {
                                                                    theme.text_muted
                                                                } else {
                                                                    theme.text
                                                                })
                                                                .child(if self
                                                                    .api_key_new_value
                                                                    .is_empty()
                                                                {
                                                                    SharedString::from(
                                                                        "API Key value",
                                                                    )
                                                                } else {
                                                                    SharedString::from(
                                                                        "•".repeat(
                                                                            self.api_key_new_value
                                                                                .len()
                                                                                .min(20),
                                                                        ),
                                                                    )
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .id("paste-key-value")
                                                        .px(px(10.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.tool_card)
                                                        .text_size(px(11.))
                                                        .text_color(theme.text)
                                                        .cursor_pointer()
                                                        .hover(|s| s.opacity(0.8))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                if let Some(text) = cx
                                                                    .read_from_clipboard()
                                                                    .and_then(|i| i.text())
                                                                {
                                                                    this.api_key_new_value =
                                                                        text.to_string();
                                                                    cx.notify();
                                                                }
                                                            }),
                                                        )
                                                        .child("Paste"),
                                                ),
                                        )
                                        .when(
                                            !self.api_key_new_name.is_empty()
                                                && !self.api_key_new_value.is_empty(),
                                            |d| {
                                                d.child(
                                                    div()
                                                        .id("save-new-key")
                                                        .px(px(12.))
                                                        .py(px(8.))
                                                        .rounded(px(6.))
                                                        .bg(theme.accent)
                                                        .text_color(rgb(0xffffff))
                                                        .text_size(px(12.))
                                                        .cursor_pointer()
                                                        .hover(|s| s.opacity(0.9))
                                                        .on_mouse_up(
                                                            MouseButton::Left,
                                                            cx.listener(|this, _, _, cx| {
                                                                let name = this
                                                                    .api_key_new_name
                                                                    .clone();
                                                                let value = this
                                                                    .api_key_new_value
                                                                    .clone();
                                                                let _ = this
                                                                    .db
                                                                    .save_api_key(&name, &value);
                                                                this.api_key_new_name.clear();
                                                                this.api_key_new_value.clear();
                                                                this.refresh_api_keys_list();
                                                                cx.notify();
                                                            }),
                                                        )
                                                        .child("Save Key"),
                                                )
                                            },
                                        ),
                        ),
                        ),
                )
            )
        })
    }
}
