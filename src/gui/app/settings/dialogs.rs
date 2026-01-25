//! Shared dialog components for settings
//!
//! Contains OAuth status display and API keys management dialog.

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::gui::app::ChatApp;

/// Find the last valid UTF-8 char boundary at or before max_bytes
fn safe_truncate_index(s: &str, max_bytes: usize) -> usize {
    if s.len() <= max_bytes {
        return s.len();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

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
                                                .text_size(px(12.))
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
                                                        .text_size(px(12.))
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
                                                        .text_size(px(12.))
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

    // =========================================================================
    // Key Pool Dialog (Multi-key management for API key rotation)
    // =========================================================================

    /// Render the key pool management dialog.
    ///
    /// This dialog allows managing multiple API keys for a single provider,
    /// with priority ordering and activation status.
    pub(crate) fn render_key_pool_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let show = self.show_key_pool_dialog;

        // Use correct pattern: apply absolute positioning directly to d
        // (not wrapped in d.child() which causes positioning bugs)
        div().when(show, |d| {
            d.absolute()
                .inset_0()
                .size_full()
                .bg(rgba(0x000000aa))
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.close_key_pool_dialog(cx);
                    }),
                )
                .flex()
                .items_center()
                .justify_center()
                .child(self.render_key_pool_dialog_inner(cx))
        })
    }

    /// Render the inner dialog container for key pool management.
    fn render_key_pool_dialog_inner(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let title = self
            .key_pool_provider_display
            .clone()
            .unwrap_or_else(|| "API Keys".to_string());

        div()
            .w(px(500.))
            .max_h(px(600.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .bg(theme.panel_background)
            .border_1()
            .border_color(theme.border)
            .rounded(px(12.))
            .shadow_lg()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Header
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
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child(format!("🔑 {} Keys", title)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.text_muted)
                                    .child(format!("({} configured)", self.key_pool_keys.len())),
                            ),
                    )
                    .child(
                        div()
                            .id("close-key-pool")
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.tool_card))
                            .text_color(theme.text_muted)
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.close_key_pool_dialog(cx);
                                }),
                            )
                            .child("✕"),
                    ),
            )
            // Key list
            .child(self.render_key_pool_list(cx))
            // Add new key section
            .child(self.render_key_pool_add_section(cx))
    }

    /// Render the scrollable list of pool keys.
    fn render_key_pool_list(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .id("key-pool-list")
            .flex_1()
            .overflow_y_scroll()
            .scrollbar_width(px(8.))
            .p(px(16.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .when(self.key_pool_keys.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(13.))
                        .text_color(theme.text_muted)
                        .py(px(20.))
                        .text_center()
                        .child("No API keys configured yet.\nAdd your first key below."),
                )
            })
            .children(
                self.key_pool_keys
                    .iter()
                    .enumerate()
                    .map(|(idx, key)| self.render_pool_key_item(key, idx, cx)),
            )
    }

    /// Render a single key item in the pool list.
    fn render_pool_key_item(
        &self,
        key: &crate::db::PoolKey,
        idx: usize,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let key_id = key.id;
        let is_active = key.is_active;
        let is_first = idx == 0;
        let is_last = idx == self.key_pool_keys.len() - 1;
        let error_count = key.error_count;

        // Mask the key (show first 8 and last 4 chars, with UTF-8 safe truncation)
        let masked_key = if key.api_key.len() > 16 {
            let start_end = safe_truncate_index(&key.api_key, 8);
            // Find a safe start point for the suffix
            let suffix_start = key.api_key.len().saturating_sub(4);
            let safe_suffix_start = {
                let mut start = suffix_start;
                while start < key.api_key.len() && !key.api_key.is_char_boundary(start) {
                    start += 1;
                }
                start
            };
            format!(
                "{}...{}",
                &key.api_key[..start_end],
                &key.api_key[safe_suffix_start..]
            )
        } else {
            "••••••••".to_string()
        };

        let label = key
            .label
            .clone()
            .unwrap_or_else(|| format!("Key #{}", idx + 1));

        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(10.))
            .rounded(px(8.))
            .bg(if is_active {
                theme.tool_card
            } else {
                theme.background
            })
            .border_1()
            .border_color(if is_active {
                theme.border
            } else {
                rgba(0xff6b6b44)
            })
            // Priority badge
            .child(
                div()
                    .w(px(24.))
                    .h(px(24.))
                    .rounded_full()
                    .bg(if is_first {
                        theme.accent
                    } else {
                        theme.tool_card
                    })
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(if is_first {
                        rgb(0xffffff)
                    } else {
                        theme.text_muted
                    })
                    .child(format!("{}", idx + 1)),
            )
            // Key info
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(if is_active {
                                        theme.text
                                    } else {
                                        theme.text_muted
                                    })
                                    .child(label),
                            )
                            .when(!is_active, |d| {
                                d.child(
                                    div()
                                        .px(px(6.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .bg(rgba(0xff6b6b22))
                                        .text_size(px(10.))
                                        .text_color(rgb(0xff6b6b))
                                        .child("Disabled"),
                                )
                            })
                            .when(error_count > 0, |d| {
                                d.child(
                                    div()
                                        .px(px(6.))
                                        .py(px(1.))
                                        .rounded(px(4.))
                                        .bg(rgba(0xffa50022))
                                        .text_size(px(10.))
                                        .text_color(rgb(0xffa500))
                                        .child(format!("{}⚠", error_count)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.text_muted)
                            .font_family("monospace")
                            .child(masked_key),
                    ),
            )
            // Action buttons
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    // Move up button
                    .when(!is_first, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("key-up-{}", key_id)))
                                .px(px(6.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.background))
                                .text_size(px(12.))
                                .text_color(theme.text_muted)
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.move_key_up(key_id, cx);
                                    }),
                                )
                                .child("↑"),
                        )
                    })
                    // Move down button
                    .when(!is_last, |d| {
                        d.child(
                            div()
                                .id(SharedString::from(format!("key-down-{}", key_id)))
                                .px(px(6.))
                                .py(px(4.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(|s| s.bg(theme.background))
                                .text_size(px(12.))
                                .text_color(theme.text_muted)
                                .on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.move_key_down(key_id, cx);
                                    }),
                                )
                                .child("↓"),
                        )
                    })
                    // Toggle active button
                    .child(
                        div()
                            .id(SharedString::from(format!("key-toggle-{}", key_id)))
                            .px(px(6.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.background))
                            .text_size(px(12.))
                            .text_color(if is_active {
                                theme.text_muted
                            } else {
                                theme.success
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.toggle_pool_key_active(key_id, is_active, cx);
                                }),
                            )
                            .child(if is_active { "Disable" } else { "Enable" }),
                    )
                    // Delete button
                    .child(
                        div()
                            .id(SharedString::from(format!("key-delete-{}", key_id)))
                            .px(px(6.))
                            .py(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgba(0xff6b6b22)))
                            .text_size(px(12.))
                            .text_color(rgb(0xff6b6b))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.delete_pool_key(key_id, cx);
                                }),
                            )
                            .child("Delete"),
                    ),
            )
    }

    /// Render the add new key section.
    fn render_key_pool_add_section(&self, cx: &Context<Self>) -> impl IntoElement {
        use gpui_component::input::Input;

        let theme = self.theme.clone();

        div()
            .px(px(16.))
            .py(px(12.))
            .border_t_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("Add New Key"),
            )
            // Label input
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .w(px(50.))
                            .child("Label:"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .when_some(self.key_pool_new_label_input.clone(), |d, input| {
                                d.child(Input::new(&input))
                            }),
                    ),
            )
            // API key input
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .w(px(50.))
                            .child("Key:"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .when_some(self.key_pool_new_key_input.clone(), |d, input| {
                                d.child(Input::new(&input))
                            }),
                    ),
            )
            // Add button
            .child(
                div().flex().justify_end().child(
                    div()
                        .id("add-pool-key")
                        .px(px(14.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .bg(theme.accent)
                        .text_color(rgb(0xffffff))
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover(|s| s.opacity(0.9))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                cx.stop_propagation();
                                this.add_key_to_pool(window, cx);
                            }),
                        )
                        .child("Add Key"),
                ),
            )
    }
}
