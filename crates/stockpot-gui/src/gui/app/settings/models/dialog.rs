//! Add model dialog component.
//!
//! Modal dialog for adding API key based models.

use gpui::{div, prelude::*, px, rgba, Context, MouseButton, Styled};
use gpui_component::input::Input;
use gpui_component::Sizable;

use crate::gui::app::ChatApp;

impl ChatApp {
    /// Render the add model dialog overlay.
    pub(crate) fn render_add_model_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        let show = self.show_add_model_dialog;

        div().when(show, |d| {
            d.absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x000000aa))
                .occlude()
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(self.render_dialog_container(cx))
        })
    }

    /// Render the dialog container with header and content.
    fn render_dialog_container(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .w(px(700.))
            .h(px(500.))
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
            .child(self.render_dialog_header(cx))
            .child(self.render_dialog_content(cx))
    }

    /// Render the dialog header with title and close button.
    fn render_dialog_header(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

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
                    .child("Add API Key based Models"),
            )
            .child(self.render_dialog_close_button(cx))
    }

    /// Render the close button for the dialog.
    fn render_dialog_close_button(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .id("close-add-model")
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
                cx.listener(|this, _, window, cx| {
                    this.show_add_model_dialog = false;
                    this.add_model_selected_provider = None;
                    this.add_model_selected_model = None;
                    this.add_model_models.clear();
                    if let Some(input) = &this.add_model_api_key_input_entity {
                        input.update(cx, |state, cx| state.set_value("", window, cx));
                    }
                    this.add_model_error = None;
                    cx.notify();
                }),
            )
            .child("✕")
    }

    /// Render the main dialog content with provider list and config panel.
    fn render_dialog_content(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .overflow_hidden()
            .child(self.render_provider_sidebar(cx))
            .child(self.render_config_panel_container(cx))
    }

    /// Render the provider sidebar on the left.
    fn render_provider_sidebar(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .w(px(250.))
            .min_h(px(0.))
            .border_r_1()
            .border_color(theme.border)
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(theme.border)
                    .child(self.render_provider_filter_input(cx)),
            )
            .child(self.render_provider_list(cx))
    }

    /// Render the filter input for provider list.
    fn render_provider_filter_input(&self, _cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();

        div()
            .id("provider-filter-container")
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("🔍"),
            )
            .child(
                div()
                    .flex_1()
                    .when_some(self.add_model_provider_filter_input.as_ref(), |d, input| {
                        d.child(Input::new(input).small())
                    }),
            )
    }

    /// Render the config panel container on the right.
    fn render_config_panel_container(&self, cx: &Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(self.render_model_config_panel(cx))
    }
}
