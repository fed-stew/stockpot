use gpui::{
    div, list, prelude::*, px, AnyElement, App, Context, Entity, IntoElement, SharedString,
    StatefulInteractiveElement, Styled,
};
use gpui_component::text::markdown;

use super::ChatApp;
use crate::gui::components::{
    collapsible_display, current_spinner_frame, list_scrollbar, CollapsibleProps,
};
use crate::gui::state::{MessageRole, MessageSection};

impl ChatApp {
    pub(super) fn render_messages(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let has_messages = !self.conversation.messages.is_empty();

        div()
            .id("messages-container")
            .flex()
            .flex_row() // Makes children sit side by side (list + scrollbar)
            .flex_1()
            .w_full()
            .min_h(px(0.))
            .overflow_hidden()
            .child(
                div()
                    .id("messages-scroll")
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))
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
                        // Use GPUI's virtualized list - only renders visible messages!
                        let view = cx.entity().clone();
                        let theme = theme.clone();

                        d.overflow_y_scroll().child(
                            list(self.messages_list_state.clone(), move |idx, _window, cx| {
                                // Read FRESH data from the entity each time!
                                // This fixes the stale closure capture bug where streaming
                                // updates weren't visible because messages was cloned at render time.
                                let app = view.read(cx);
                                let Some(msg) = app.conversation.messages.get(idx) else {
                                    return div().into_any_element();
                                };

                                let is_user = msg.role == MessageRole::User;
                                let bubble_bg = if is_user {
                                    theme.user_bubble
                                } else {
                                    theme.assistant_bubble
                                };
                                let is_streaming = msg.is_streaming;

                                let content_elements: Vec<gpui::AnyElement> = app
                                    .render_message_content(
                                        &msg.sections,
                                        &msg.content,
                                        idx,
                                        &theme,
                                        cx,
                                    );

                                div()
                                    .id(SharedString::from(format!("msg-{}", idx)))
                                    .flex()
                                    .flex_col()
                                    .w_full()
                                    .pb(px(16.)) // Gap between messages
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
                                            .p(px(12.))
                                            .rounded(px(8.))
                                            .bg(bubble_bg)
                                            .text_color(theme.text)
                                            .overflow_hidden()
                                            .min_w_0()
                                            .when(is_user, |d| d.max_w(px(600.)))
                                            .when(!is_user, |d| d.w_full().min_w_0())
                                            .children(content_elements)
                                            .when(is_streaming, |d| {
                                                d.child(
                                                    div()
                                                        .ml(px(2.))
                                                        .text_color(theme.accent)
                                                        .child(current_spinner_frame()),
                                                )
                                            }),
                                    )
                                    .into_any_element()
                            })
                            .size_full(),
                        )
                    }),
            )
            .child(list_scrollbar(
                self.messages_list_state.clone(),
                self.conversation.messages.len(),
                self.messages_list_scrollbar_drag.clone(),
                theme.clone(),
            ))
    }

    /// Render the content of a message, handling sections or falling back to raw content.
    ///
    /// When a message has structured sections, each section is rendered appropriately:
    /// - Text sections render as markdown
    /// - NestedAgent sections render as collapsible containers
    ///
    /// If no sections exist (legacy messages), the raw content is rendered as markdown.
    ///
    /// This variant accepts Entity<ChatApp> and &App for use within virtualized list callbacks.
    pub(super) fn render_message_content(
        &self,
        sections: &[MessageSection],
        content: &str,
        msg_idx: usize,
        theme: &crate::gui::theme::Theme,
        _cx: &App,
    ) -> Vec<AnyElement> {
        // If we have sections, render them
        if !sections.is_empty() {
            sections
                .iter()
                .enumerate()
                .map(|(sec_idx, section)| self.render_section(section, msg_idx, sec_idx, theme))
                .collect()
        } else {
            // Legacy: render content directly as markdown
            // Clone to owned String for markdown renderer's 'static requirement
            let owned_content = content.to_string();
            vec![div()
                .w_full()
                .overflow_x_hidden()
                .child(markdown(&owned_content).selectable(true))
                .into_any_element()]
        }
    }

    /// Render a single message section.
    fn render_section(
        &self,
        section: &MessageSection,
        msg_idx: usize,
        sec_idx: usize,
        theme: &crate::gui::theme::Theme,
    ) -> AnyElement {
        match section {
            MessageSection::Text(text) => {
                // Text sections render as markdown
                div()
                    .id(SharedString::from(format!(
                        "msg-{}-sec-{}",
                        msg_idx, sec_idx
                    )))
                    .w_full()
                    .overflow_x_hidden()
                    .child(markdown(text).selectable(true))
                    .into_any_element()
            }
            MessageSection::NestedAgent(agent_section) => {
                // Nested agent sections render as collapsible
                self.render_agent_section_static(agent_section, msg_idx, sec_idx, theme)
            }
        }
    }

    /// Render a nested agent section as a collapsible container (static, no click handler).
    /// Click handlers for collapsibles inside virtualized lists need special handling.
    fn render_agent_section_static(
        &self,
        agent_section: &crate::gui::state::AgentSection,
        msg_idx: usize,
        sec_idx: usize,
        theme: &crate::gui::theme::Theme,
    ) -> AnyElement {
        let is_collapsed = agent_section.is_collapsed;

        // Build collapsible props from theme and agent section state
        let props = CollapsibleProps::with_theme(theme)
            .id(format!("msg-{}-agent-{}", msg_idx, sec_idx))
            .title(agent_section.display_name.clone())
            .icon("🤖")
            .collapsed(is_collapsed)
            .loading(!agent_section.is_complete);

        // LAZY EVALUATION: Only parse markdown when section is expanded!
        // This is critical for performance - markdown parsing is expensive and
        // was causing 5+ second delays when toggling sections with large content.
        let content = if is_collapsed {
            // Fast path: empty placeholder when collapsed (content won't be shown anyway)
            div().into_any_element()
        } else {
            // Slow path: only parse markdown when actually visible
            div()
                .w_full()
                .overflow_x_hidden()
                .child(markdown(&agent_section.content).selectable(true))
                .into_any_element()
        };

        // Create the collapsible in display-only mode
        // NOTE: Click handlers for toggling are not supported in virtualized list items
        // The collapsible will display properly but won't toggle on click
        // This is a trade-off for virtualization performance
        let collapsible_element = collapsible_display(props, content);

        div()
            .id(SharedString::from(format!(
                "msg-{}-sec-{}-container",
                msg_idx, sec_idx
            )))
            .w_full()
            .my(px(8.)) // Vertical margin for visual separation
            .child(collapsible_element)
            .into_any_element()
    }
}
