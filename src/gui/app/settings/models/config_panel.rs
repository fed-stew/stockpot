//! Model configuration panel for the add model dialog.
//!
//! Displays API key input and model list for the selected provider.

use gpui::{div, prelude::*, px, rgb, Context, MouseButton, SharedString, Styled};

use crate::gui::app::ChatApp;
use crate::gui::components::scrollbar;

impl ChatApp {
    /// Render the model configuration panel.
    pub(super) fn render_model_config_panel(&self, cx: &Context<Self>) -> gpui::AnyElement {
        // Show placeholder if no provider selected
        let Some(provider_id) = &self.add_model_selected_provider else {
            return self.render_no_provider_selected();
        };

        let provider = self
            .add_model_providers
            .iter()
            .find(|p| &p.id == provider_id);
        let env_var = provider
            .and_then(|p| p.env.first())
            .map(|s| s.as_str())
            .unwrap_or("API_KEY");

        let has_existing_key = self.db.has_api_key(env_var) || std::env::var(env_var).is_ok();
        let has_key_input = self
            .add_model_api_key_input_entity
            .as_ref()
            .map(|e| !e.read(cx).value().is_empty())
            .unwrap_or(false);
        let can_add_models = has_existing_key || has_key_input;

        let provider_id = provider_id.clone();
        let env_var = env_var.to_string();

        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_api_key_section(&env_var, has_existing_key, cx))
            .child(self.render_models_header())
            .child(self.render_models_list_panel(&provider_id, &env_var, can_add_models, cx))
            .into_any_element()
    }

    /// Render placeholder when no provider is selected.
    fn render_no_provider_selected(&self) -> gpui::AnyElement {
        let theme = self.theme.clone();

        div()
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(theme.text_muted)
                    .child("← Select a provider"),
            )
            .into_any_element()
    }

    /// Render the API key input section.
    fn render_api_key_section(
        &self,
        env_var: &str,
        has_existing_key: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .px(px(16.))
            .py(px(12.))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(self.render_api_key_header(env_var, has_existing_key))
            .child(self.render_api_key_input(cx))
    }

    /// Render the API key section header.
    fn render_api_key_header(&self, env_var: &str, has_existing_key: bool) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.text_muted)
                    .child(format!("API Key ({})", env_var)),
            )
            .when(has_existing_key, |d| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(0x4ade80))
                        .child("✓ Key configured"),
                )
            })
    }

    /// Render the API key input field with paste button.
    fn render_api_key_input(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_h(px(44.))
                    .when_some(self.add_model_api_key_input_entity.as_ref(), |d, input| {
                        d.child(gpui_component::input::Input::new(input).flex_1())
                    }),
            )
            .child(self.render_paste_button(cx))
    }

    /// Render the paste button for API key.
    fn render_paste_button(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .id("paste-api-key")
            .px(px(12.))
            .py(px(8.))
            .rounded(px(6.))
            .bg(theme.tool_card)
            .text_color(theme.text)
            .text_size(px(12.))
            .cursor_pointer()
            .hover(|s| s.opacity(0.8))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                        if let Some(input) = &this.add_model_api_key_input_entity {
                            input.update(cx, |state, cx| {
                                state.set_value(text.to_string(), window, cx);
                            });
                        }
                        cx.notify();
                    }
                }),
            )
            .child("Paste")
    }

    /// Render the models section header.
    fn render_models_header(&self) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .px(px(16.))
            .py(px(8.))
            .border_b_1()
            .border_color(theme.border)
            .text_size(px(12.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.text_muted)
            .child(format!("Models ({})", self.add_model_models.len()))
    }

    /// Render the scrollable models list.
    fn render_models_list_panel(
        &self,
        provider_id: &str,
        env_var: &str,
        can_add_models: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .child(
                div()
                    .id("models-list-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.add_model_models_scroll_handle)
                    .children(self.add_model_models.iter().map(|model| {
                        self.render_model_list_item(model, provider_id, env_var, can_add_models, cx)
                    })),
            )
            .child(scrollbar(
                self.add_model_models_scroll_handle.clone(),
                self.add_model_models_scrollbar_drag.clone(),
                theme.clone(),
            ))
    }

    /// Render a single model item in the list.
    fn render_model_list_item(
        &self,
        model: &crate::models::catalog::ModelInfo,
        provider_id: &str,
        env_var: &str,
        can_add: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let model_id = model.id.clone();
        let model_name = model.name.clone().unwrap_or_else(|| model.id.clone());
        let provider_id = provider_id.to_string();
        let env_var = env_var.to_string();

        let ctx_info = model
            .context_length
            .map(|c| format!("{}k", c / 1000))
            .unwrap_or_default();

        div()
            .id(SharedString::from(format!("model-{}", model_id)))
            .px(px(16.))
            .py(px(10.))
            .border_b_1()
            .border_color(theme.border)
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_model_item_info(&model_name, &ctx_info))
            .child(self.render_model_add_button(&model_id, &provider_id, &env_var, can_add, cx))
    }

    /// Render the model info (name and context length).
    fn render_model_item_info(&self, model_name: &str, ctx_info: &str) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(theme.text)
                    .child(model_name.to_string()),
            )
            .when(!ctx_info.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.text_muted)
                        .child(ctx_info.to_string()),
                )
            })
    }

    /// Render the add button for a model.
    fn render_model_add_button(
        &self,
        model_id: &str,
        provider_id: &str,
        env_var: &str,
        can_add: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = self.theme.clone();
        let model_id = model_id.to_string();
        let provider_id = provider_id.to_string();
        let env_var = env_var.to_string();

        div()
            .id(SharedString::from(format!("add-model-{}", model_id)))
            .px(px(10.))
            .py(px(6.))
            .rounded(px(6.))
            .bg(if can_add {
                theme.accent
            } else {
                theme.tool_card
            })
            .text_color(if can_add {
                rgb(0xffffff)
            } else {
                theme.text_muted
            })
            .text_size(px(12.))
            .cursor(if can_add {
                gpui::CursorStyle::PointingHand
            } else {
                gpui::CursorStyle::Arrow
            })
            .when(can_add, |d| d.hover(|s| s.opacity(0.8)))
            .when(can_add, |d| {
                d.on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.add_single_model(&provider_id, &model_id, &env_var, cx);
                    }),
                )
            })
            .child("+")
    }
}
