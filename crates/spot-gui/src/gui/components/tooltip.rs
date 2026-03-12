//! Simple tooltip component

use gpui::{div, prelude::*, px, rgb, Styled};
use gpui_component::scroll::ScrollableElement;
use gpui_component::text::markdown;

/// A simple tooltip view that displays text
pub struct SimpleTooltip {
    text: String,
}

impl SimpleTooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    /// Create a tooltip builder function for use with .tooltip()
    pub fn text(
        text: impl Into<String> + Clone + 'static,
    ) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView {
        let text = text.into();
        move |_window, cx| cx.new(|_| SimpleTooltip::new(text.clone())).into()
    }
}

impl Render for SimpleTooltip {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(4.))
            .bg(rgb(0x1a1a1a))
            .border_1()
            .border_color(rgb(0x333333))
            .text_size(px(11.))
            .text_color(rgb(0xcccccc))
            .child(self.text.clone())
    }
}

/// A tooltip view that renders markdown content
pub struct MarkdownTooltip {
    content: String,
}

impl MarkdownTooltip {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
        }
    }

    /// Create a tooltip builder function for use with .tooltip()
    pub fn markdown(
        content: impl Into<String> + Clone + 'static,
    ) -> impl Fn(&mut gpui::Window, &mut gpui::App) -> gpui::AnyView {
        let content = content.into();
        move |_window, cx| cx.new(|_| MarkdownTooltip::new(content.clone())).into()
    }
}

impl Render for MarkdownTooltip {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .max_w(px(400.))
            .max_h(px(300.))
            .overflow_y_scrollbar()
            .p(px(12.))
            .rounded(px(6.))
            .bg(rgb(0x1a1a1a))
            .border_1()
            .border_color(rgb(0x333333))
            .text_size(px(12.))
            .text_color(rgb(0xcccccc))
            .child(markdown(&self.content))
    }
}
