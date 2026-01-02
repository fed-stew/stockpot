use gpui::{div, prelude::*, px, Context, SharedString, StatefulInteractiveElement, Styled};

use super::ChatApp;
use crate::gui::components::scrollbar;
use crate::gui::state::MessageRole;

impl ChatApp {
    pub(super) fn render_messages(&self, cx: &Context<Self>) -> impl IntoElement {
        let messages = self.conversation.messages.clone();
        let theme = self.theme.clone();
        let has_messages = !messages.is_empty();
        let message_texts = self.message_texts.clone();

        div()
            .id("messages-container")
            .flex()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .child(
                div()
                    .id("messages-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .track_scroll(&self.messages_scroll_handle)
                    .on_scroll_wheel(cx.listener(|_, _, _, cx| {
                        cx.notify();
                    }))
                    .p(px(16.))
                    .when(!has_messages, |d| {
                        d.flex().items_center().justify_center().child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(12.))
                                .child(div().text_size(px(56.)).child("🍲"))
                                .child(
                                    div()
                                        .text_size(px(20.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(theme.text)
                                        .child("Welcome to Stockpot"),
                                )
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .text_color(theme.text_muted)
                                        .child("Your AI-powered coding assistant"),
                                )
                                .child(
                                    div()
                                        .mt(px(16.))
                                        .text_size(px(13.))
                                        .text_color(theme.text_muted)
                                        .child("Type a message below to get started"),
                                )
                                .child(
                                    div()
                                        .mt(px(8.))
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child("📁 Drag and drop files here to share them"),
                                ),
                        )
                    })
                    .when(has_messages, |d| {
                        d.child(div().flex().flex_col().gap(px(16.)).children(
                            messages.into_iter().enumerate().map(|(idx, msg)| {
                                let is_user = msg.role == MessageRole::User;
                                let bubble_bg = if is_user {
                                    theme.user_bubble
                                } else {
                                    theme.assistant_bubble
                                };
                                let text_entity = message_texts.get(&msg.id).cloned();
                                let has_entity = text_entity.is_some();
                                let msg_content = msg.content.clone();
                                let is_streaming = msg.is_streaming;

                                div()
                                    .id(SharedString::from(format!("msg-{}", idx)))
                                    .flex()
                                    .flex_col()
                                    .when(is_user, |d| d.items_end())
                                    .when(!is_user, |d| d.items_start())
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(theme.text_muted)
                                            .mb(px(4.))
                                            .child(if is_user { "You" } else { "Assistant" }),
                                    )
                                    .child(
                                        div()
                                            .max_w(px(700.))
                                            .min_w(px(100.))
                                            .p(px(12.))
                                            .rounded(px(8.))
                                            .bg(bubble_bg)
                                            .text_color(theme.text)
                                            .when_some(text_entity, |d, entity| d.child(entity))
                                            .when(!has_entity, |d| {
                                                // Fallback to plain text if no entity exists
                                                d.child(msg_content)
                                            })
                                            .when(is_streaming, |d: gpui::Div| {
                                                d.child(
                                                    div()
                                                        .ml(px(2.))
                                                        .text_color(theme.accent)
                                                        .child("▋"),
                                                )
                                            }),
                                    )
                            }),
                        ))
                    }),
            )
            .child(scrollbar(
                self.messages_scroll_handle.clone(),
                self.messages_scrollbar_drag.clone(),
                theme.clone(),
            ))
    }
}
