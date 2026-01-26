//! Provider list component for the add model dialog.
//!
//! Displays available model providers for selection.

use gpui::{div, prelude::*, px, rgb, Context, Hsla, MouseButton, Rgba, SharedString, Styled};

use crate::gui::app::ChatApp;
use crate::gui::components::scrollbar;

/// Selected background opacity (8% tint) for provider items.
const SELECTED_BG_OPACITY: f32 = 0.08;

/// Convert Rgba to Hsla with a specific opacity.
fn with_opacity(color: Rgba, opacity: f32) -> Hsla {
    let hsla: Hsla = color.into();
    hsla.opacity(opacity)
}

impl ChatApp {
    /// Get providers filtered by the current filter text.
    fn get_filtered_providers(
        &self,
        cx: &Context<Self>,
    ) -> Vec<stockpot_core::models::catalog::ProviderInfo> {
        let filter_text = self
            .add_model_provider_filter_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string().to_lowercase())
            .unwrap_or_default();

        self.add_model_providers
            .iter()
            .filter(|p| {
                if filter_text.is_empty() {
                    return true;
                }
                p.name.to_lowercase().contains(&filter_text)
                    || p.id.to_lowercase().contains(&filter_text)
            })
            .cloned()
            .collect()
    }

    /// Render the provider list in the add model dialog.
    pub(super) fn render_provider_list(&self, cx: &Context<Self>) -> gpui::AnyElement {
        // Show loading state
        if self.add_model_loading {
            return self.render_provider_loading_state();
        }

        // Show error state
        if let Some(error) = &self.add_model_error {
            return self.render_provider_error_state(error, cx);
        }

        // Show provider list
        self.render_provider_items(cx)
    }

    /// Render loading state for provider list.
    fn render_provider_loading_state(&self) -> gpui::AnyElement {
        let theme = self.theme.clone();

        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .child(div().text_size(px(20.)).child("⏳"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_muted)
                            .child("Loading providers..."),
                    ),
            )
            .into_any_element()
    }

    /// Render error state for provider list.
    fn render_provider_error_state(&self, error: &str, cx: &Context<Self>) -> gpui::AnyElement {
        let theme = self.theme.clone();

        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .p(px(16.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(0xff6b6b))
                    .child(error.to_string()),
            )
            .child(
                div()
                    .id("retry-fetch")
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .bg(theme.tool_card)
                    .text_color(theme.text)
                    .text_size(px(12.))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.8))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.fetch_providers(cx);
                        }),
                    )
                    .child("Retry"),
            )
            .into_any_element()
    }

    /// Render the scrollable list of provider items.
    fn render_provider_items(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .child(
                div()
                    .id("provider-list-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.add_model_providers_scroll_handle)
                    .children(
                        self.get_filtered_providers(cx)
                            .into_iter()
                            .map(|provider| self.render_provider_item(&provider, cx)),
                    ),
            )
            .child(scrollbar(
                self.add_model_providers_scroll_handle.clone(),
                self.add_model_providers_scrollbar_drag.clone(),
                theme.clone(),
            ))
            .into_any_element()
    }

    /// Render a single provider item.
    /// Uses subtle selection styling: thin border + 8% accent tint.
    fn render_provider_item(
        &self,
        provider: &stockpot_core::models::catalog::ProviderInfo,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let provider_id = provider.id.clone();
        let is_selected = self.add_model_selected_provider.as_ref() == Some(&provider_id);
        let name = if provider.name.is_empty() {
            provider.id.clone()
        } else {
            provider.name.clone()
        };
        let model_count = provider.models.len();

        // Subtle selection styling: thin border + 8% accent tint
        let (bg_color, left_border_color, text_color) = if is_selected {
            (
                with_opacity(theme.accent, SELECTED_BG_OPACITY),
                theme.accent,
                theme.accent,
            )
        } else {
            (
                Hsla::transparent_black(),
                theme.panel_background, // Invisible left border
                theme.text,
            )
        };

        div()
            .id(SharedString::from(format!("provider-{}", provider_id)))
            .px(px(16.))
            .py(px(10.))
            .cursor_pointer()
            .bg(bg_color)
            .text_color(text_color)
            .border_l_2() // Left accent border for selected state
            .border_color(left_border_color)
            .hover(move |s| {
                if is_selected {
                    s
                } else {
                    s.bg(with_opacity(theme.tool_card, 0.5))
                }
            })
            .border_b_1()
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.add_model_selected_provider = Some(provider_id.clone());
                    if let Some(p) = this
                        .add_model_providers
                        .iter()
                        .find(|p| p.id == provider_id)
                    {
                        this.add_model_models = p.models.values().cloned().collect();
                        this.add_model_models.sort_by(|a, b| a.id.cmp(&b.id));
                    }
                    if let Some(input) = &this.add_model_api_key_input_entity {
                        input.update(cx, |state, cx| state.set_value("", window, cx));
                    }
                    this.add_model_error = None;
                    cx.notify();
                }),
            )
            .child(self.render_provider_item_content(&name, model_count, is_selected))
    }

    /// Render the content for a provider item.
    /// Model count always uses muted text (subtle selection doesn't need white text).
    fn render_provider_item_content(
        &self,
        name: &str,
        model_count: usize,
        _is_selected: bool,
    ) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_size(px(13.)).child(name.to_string()))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme.text_muted)
                    .child(format!("{} models", model_count)),
            )
    }
}
