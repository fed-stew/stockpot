use gpui::{div, prelude::*, Context, Entity, Focusable, IntoElement, Render, SharedString, Window};

use crate::gui::{theme::Theme, GlobalLanguageRegistry};
use zed_theme::ActiveTheme as _;

pub struct ZedMarkdownText {
    theme: Theme,
    markdown: Entity<markdown::Markdown>,
}

impl ZedMarkdownText {
    pub fn new(cx: &mut Context<Self>, content: impl Into<SharedString>, theme: Theme) -> Self {
        let source = content.into();
        let language_registry = cx
            .try_global::<GlobalLanguageRegistry>()
            .map(|registry| registry.0.clone());
        let markdown_entity =
            cx.new(|cx| markdown::Markdown::new(source, language_registry, None, cx));
        Self {
            theme,
            markdown: markdown_entity,
        }
    }

    pub fn set_content(&mut self, content: impl Into<SharedString>, cx: &mut Context<Self>) {
        let source = content.into();
        self.markdown.update(cx, |md, cx| {
            md.reset(source, cx);
        });
        cx.notify();
    }
}

impl Focusable for ZedMarkdownText {
    fn focus_handle(&self, cx: &gpui::App) -> gpui::FocusHandle {
        self.markdown.read(cx).focus_handle(cx)
    }
}

impl Render for ZedMarkdownText {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut style = markdown::MarkdownStyle::default();
        style.base_text_style = window.text_style();
        style.syntax = cx.theme().syntax().clone();
        style.selection_background_color = cx.theme().colors().element_selection_background;

        div()
            .child(markdown::MarkdownElement::new(self.markdown.clone(), style))
    }
}
