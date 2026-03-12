use gpui::{
    anchored, deferred, div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled,
};

use super::ChatApp;

impl ChatApp {
    pub(super) fn render_agent_dropdown_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_agent_dropdown;
        let current_agent = self.current_agent.clone();

        div().when(show && self.agent_dropdown_bounds.is_some(), |d| {
            let bounds = self.agent_dropdown_bounds.unwrap();
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
                            this.show_agent_dropdown = false;
                            cx.notify();
                        }),
                    )
                    .child(deferred(
                        anchored().position(position).child(
                            div()
                                .id("agent-dropdown-list")
                                .w(bounds.size.width.max(px(260.)))
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
                                .children(self.available_agents.iter().map(|(name, display)| {
                                    let is_selected = name == &current_agent;
                                    let agent_name = name.clone();

                                    div()
                                        .id(SharedString::from(format!("agent-dd-{}", name)))
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
                                        .text_size(px(16.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.tool_card))
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.set_current_agent(&agent_name);
                                                this.show_agent_dropdown = false;
                                                cx.notify();
                                            }),
                                        )
                                        .child(display.clone())
                                })),
                        ),
                    )),
            )
        })
    }
}
