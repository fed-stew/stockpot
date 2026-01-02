use gpui::{
    anchored, deferred, div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled,
};

use super::ChatApp;
use crate::config::Settings;

impl ChatApp {
    pub(super) fn render_model_dropdown_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_model_dropdown;
        let current_agent = self.current_agent.clone();
        let (effective_model, is_pinned) = self.current_effective_model();
        let default_model = self.current_model.clone();

        div().when(show && self.model_dropdown_bounds.is_some(), |d| {
            let bounds = self.model_dropdown_bounds.unwrap();
            let position = gpui::Point::new(
                bounds.origin.x,
                bounds.origin.y + bounds.size.height + px(4.),
            );

            d.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(rgba(0x00000000))
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.show_model_dropdown = false;
                            cx.notify();
                        }),
                    )
                    .child(deferred(
                        anchored().position(position).child(
                            div()
                                .id("model-dropdown-list")
                                .w(bounds.size.width.max(px(300.)))
                                .max_h(px(300.))
                                .overflow_y_scroll()
                                .scrollbar_width(px(8.))
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .rounded(px(8.))
                                .bg(theme.panel_background)
                                .border_1()
                                .border_color(theme.border)
                                .shadow_lg()
                                .occlude()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                    cx.stop_propagation();
                                })
                                .child({
                                    let is_selected = !is_pinned;
                                    let agent_name = current_agent.clone();
                                    let default_model_label = format!(
                                        "Default ({})",
                                        Self::truncate_model_name(&default_model)
                                    );

                                    div()
                                        .id("model-dd-default")
                                        .px(px(12.))
                                        .py(px(8.))
                                        .bg(if is_selected {
                                            theme.accent
                                        } else {
                                            theme.panel_background
                                        })
                                        .text_color(if is_selected {
                                            rgb(0xffffff)
                                        } else {
                                            theme.text
                                        })
                                        .text_size(px(13.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.tool_card))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                let settings = Settings::new(&this.db);
                                                if let Err(e) = settings
                                                    .clear_agent_pinned_model(&agent_name)
                                                {
                                                    tracing::warn!(
                                                        "Failed to clear pinned model for {}: {}",
                                                        agent_name,
                                                        e
                                                    );
                                                }
                                                this.show_model_dropdown = false;
                                                cx.notify();
                                            }),
                                        )
                                        .child(default_model_label)
                                })
                                .children(self.available_models.iter().map(|model| {
                                    let is_selected = is_pinned && model == &effective_model;
                                    let model_clone = model.clone();
                                    let agent_name = current_agent.clone();

                                    div()
                                        .id(SharedString::from(format!("model-dd-{}", model)))
                                        .px(px(12.))
                                        .py(px(8.))
                                        .bg(if is_selected {
                                            theme.accent
                                        } else {
                                            theme.panel_background
                                        })
                                        .text_color(if is_selected {
                                            rgb(0xffffff)
                                        } else {
                                            theme.text
                                        })
                                        .text_size(px(13.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.tool_card))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                let settings = Settings::new(&this.db);
                                                if let Err(e) = settings.set_agent_pinned_model(
                                                    &agent_name,
                                                    &model_clone,
                                                ) {
                                                    tracing::warn!(
                                                        "Failed to pin model for {}: {}",
                                                        agent_name,
                                                        e
                                                    );
                                                }
                                                this.show_model_dropdown = false;
                                                cx.notify();
                                            }),
                                        )
                                        .child(Self::truncate_model_name(model))
                                })),
                        ),
                    )),
            )
        })
    }
}
