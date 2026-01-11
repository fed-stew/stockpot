use gpui::{
    div, list, prelude::*, px, AnyElement, App, Context, ElementId, Entity, IntoElement,
    MouseButton, SharedString, StatefulInteractiveElement, Styled,
};
use gpui_component::text::TextView;

use super::ChatApp;
use crate::gui::components::{collapsible_display, list_scrollbar, CollapsibleProps};
use crate::gui::state::{
    AgentContentItem, MessageRole, MessageSection, ThinkingSection, ToolCallSection,
};

impl ChatApp {
    fn markdown_text_view(&self, element_id: &SharedString, content: &str) -> TextView {
        if let Some(state) = self.text_view_cache.borrow().get(element_id.as_ref()) {
            TextView::new(state)
        } else {
            TextView::markdown(ElementId::Name(element_id.clone()), content.to_string())
        }
    }

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
                                view.update(cx, |app, cx| {
                                    let Some(msg) = app.conversation.messages.get(idx) else {
                                        return div().into_any_element();
                                    };

                                    let is_user = msg.role == MessageRole::User;
                                    let bubble_bg = if is_user {
                                        theme.user_bubble
                                    } else {
                                        theme.assistant_bubble
                                    };
                                    let msg_id = msg.id.clone();
                                    let content_elements: Vec<gpui::AnyElement> = app
                                        .render_message_content(
                                            &msg.sections,
                                            &msg.content,
                                            &msg_id,
                                            &theme,
                                            &view,
                                            cx,
                                        );

                                    // ALWAYS use stable UUID-based IDs.
                                    // This prevents element ID changes when streaming finishes,
                                    // which would cause GPUI to treat it as a new element.
                                    let element_id = SharedString::from(format!("msg-{}", msg_id));

                                    div()
                                        .id(element_id)
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
                                                .children(content_elements),
                                        )
                                        .into_any_element()
                                })
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
        msg_id: &str,
        theme: &crate::gui::theme::Theme,
        view: &Entity<ChatApp>,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        // If we have sections, render them
        if !sections.is_empty() {
            sections
                .iter()
                .enumerate()
                .map(|(sec_idx, section)| {
                    self.render_section(section, sec_idx, msg_id, theme, view, cx)
                })
                .collect()
        } else {
            // Legacy: render content directly as markdown
            // Always use stable UUID-based ID to prevent re-renders
            let element_id = SharedString::from(format!("msg-{}-content", msg_id));
            let owned_content = content.to_string();
            let text_view = self.markdown_text_view(&element_id, &owned_content);
            vec![div()
                .id(element_id.clone())
                .w_full()
                .overflow_x_hidden()
                .child(text_view.selectable(true))
                .into_any_element()]
        }
    }

    /// Render a single message section.
    fn render_section(
        &self,
        section: &MessageSection,
        sec_idx: usize,
        msg_id: &str,
        theme: &crate::gui::theme::Theme,
        view: &Entity<ChatApp>,
        cx: &mut App,
    ) -> AnyElement {
        match section {
            MessageSection::Text(text) => {
                // Text sections render as markdown
                // Always use stable UUID-based ID to prevent re-renders
                let element_id = SharedString::from(format!("msg-{}-sec-{}", msg_id, sec_idx));
                let text_view = self.markdown_text_view(&element_id, text);
                div()
                    .id(element_id.clone())
                    .w_full()
                    .overflow_x_hidden()
                    .child(text_view.selectable(true))
                    .into_any_element()
            }
            MessageSection::NestedAgent(agent_section) => {
                // Nested agent sections render as collapsible with click handler
                // agent_section.id is already a stable UUID
                self.render_agent_section_clickable(agent_section, theme, view, cx)
            }
            MessageSection::Thinking(thinking_section) => {
                // Thinking sections render as collapsible containers
                self.render_thinking_section(thinking_section, msg_id, theme, view, cx)
            }
            MessageSection::ToolCall(tool_section) => {
                // Tool call sections render as styled inline elements
                self.render_tool_call_section(tool_section, msg_id, theme)
            }
        }
    }

    /// Render a thinking section as a collapsible container with click handler.
    /// Shows "Thinking" + preview when collapsed, full markdown content when expanded.
    fn render_thinking_section(
        &self,
        thinking: &ThinkingSection,
        _msg_id: &str,
        theme: &crate::gui::theme::Theme,
        view: &Entity<ChatApp>,
        _cx: &mut App,
    ) -> AnyElement {
        let is_collapsed = thinking.is_collapsed;
        let stable_id = &thinking.id;

        // Build title: "Thinking" + preview when collapsed
        let title = if is_collapsed {
            let preview = thinking.preview();
            if preview.is_empty() {
                "Thinking".to_string()
            } else {
                format!("Thinking: {}", preview)
            }
        } else {
            "Thinking".to_string()
        };

        let props = CollapsibleProps::with_theme(theme)
            .id(format!("thinking-{}", stable_id))
            .title(title)
            .collapsed(is_collapsed)
            .loading(false);

        // LAZY EVALUATION: Only render content when section is expanded!
        let content = if is_collapsed {
            // Fast path: empty placeholder when collapsed
            div().into_any_element()
        } else {
            // Full markdown content when expanded
            let element_id = SharedString::from(format!("thinking-{}-content", stable_id));
            let text_view = self.markdown_text_view(&element_id, &thinking.content);
            div()
                .id(element_id.clone())
                .w_full()
                .overflow_x_hidden()
                .child(text_view.selectable(true))
                .into_any_element()
        };

        // Create the collapsible in display-only mode
        let collapsible_element = collapsible_display(props, content);

        // Clone for click handler closure
        let section_id = thinking.id.clone();
        let view = view.clone();

        div()
            .id(SharedString::from(format!(
                "thinking-{}-container",
                stable_id
            )))
            .w_full()
            .my(px(8.)) // Vertical margin for visual separation
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                view.update(cx, |app, cx: &mut Context<ChatApp>| {
                    app.conversation.toggle_thinking_collapsed(&section_id);
                    cx.notify();
                });
            })
            .child(collapsible_element)
            .into_any_element()
    }

    /// Render a tool call section as a styled inline element.
    /// Format: • Verb subject ✓
    fn render_tool_call_section(
        &self,
        tool_section: &ToolCallSection,
        msg_id: &str,
        theme: &crate::gui::theme::Theme,
    ) -> AnyElement {
        let element_id = SharedString::from(format!("tool-{}-{}", msg_id, tool_section.id));

        // Status indicator at the end
        let status = if tool_section.is_running {
            None // No indicator while running, could add spinner later
        } else if tool_section.succeeded == Some(true) {
            Some(("✓", theme.success))
        } else {
            Some(("✗", theme.error))
        };

        div()
            .id(element_id)
            .flex()
            .items_center()
            .gap(px(6.))
            .py(px(3.))
            .my(px(2.))
            .text_size(px(13.))
            // Muted bullet
            .child(div().text_color(theme.tool_bullet).child("•"))
            // Semi-bold colored verb (matches markdown bold styling)
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.tool_verb)
                    .child(tool_section.info.verb.clone()),
            )
            // Subject (if any)
            .when(!tool_section.info.subject.is_empty(), |el| {
                el.child(
                    div()
                        .text_color(theme.text)
                        .child(tool_section.info.subject.clone()),
                )
            })
            // Status indicator at end
            .when_some(status, |el, (icon, color)| {
                el.child(div().text_color(color).child(icon))
            })
            .into_any_element()
    }

    /// Render a nested agent section as a collapsible container with click handler.
    /// The click handler toggles the collapsed state via the Entity<ChatApp> reference.
    fn render_agent_section_clickable(
        &self,
        agent_section: &crate::gui::state::AgentSection,
        theme: &crate::gui::theme::Theme,
        view: &Entity<ChatApp>,
        _cx: &mut App,
    ) -> AnyElement {
        let is_collapsed = agent_section.is_collapsed;

        // Use agent_section.id (a stable UUID) for element IDs.
        // This prevents flickering during streaming - the ID doesn't change
        // even as content updates every ~8ms.
        let stable_id = &agent_section.id;
        let props = CollapsibleProps::with_theme(theme)
            .id(format!("agent-{}", stable_id))
            .title(agent_section.display_name.clone())
            .collapsed(is_collapsed)
            .loading(!agent_section.is_complete);

        // LAZY EVALUATION: Only render content when section is expanded!
        // This is critical for performance - markdown parsing is expensive and
        // was causing 5+ second delays when toggling sections with large content.
        let content = if is_collapsed {
            // Fast path: empty placeholder when collapsed (content won't be shown anyway)
            div().into_any_element()
        } else {
            // Render each content item appropriately
            let children: Vec<AnyElement> = agent_section
                .items
                .iter()
                .enumerate()
                .map(|(idx, item)| match item {
                    AgentContentItem::Text(text) => {
                        let element_id =
                            SharedString::from(format!("agent-{}-text-{}", stable_id, idx));
                        let text_view = self.markdown_text_view(&element_id, text);
                        div()
                            .id(element_id.clone())
                            .w_full()
                            .overflow_x_hidden()
                            .child(text_view.selectable(true))
                            .into_any_element()
                    }
                    AgentContentItem::ToolCall {
                        id,
                        info,
                        is_running,
                        succeeded,
                    } => {
                        // Render tool call with same styling as top-level tool calls
                        let element_id = SharedString::from(format!("nested-tool-{}", id));
                        let status = if *is_running {
                            None
                        } else if *succeeded == Some(true) {
                            Some(("✓", theme.success))
                        } else {
                            Some(("✗", theme.error))
                        };

                        div()
                            .id(element_id)
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .py(px(3.))
                            .my(px(2.))
                            .text_size(px(13.))
                            // Muted bullet
                            .child(div().text_color(theme.tool_bullet).child("•"))
                            // Semi-bold colored verb
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.tool_verb)
                                    .child(info.verb.clone()),
                            )
                            // Subject (if any)
                            .when(!info.subject.is_empty(), |el| {
                                el.child(div().text_color(theme.text).child(info.subject.clone()))
                            })
                            // Status indicator at end
                            .when_some(status, |el, (icon, color)| {
                                el.child(div().text_color(color).child(icon))
                            })
                            .into_any_element()
                    }
                    AgentContentItem::Thinking {
                        id,
                        content,
                        is_complete,
                    } => {
                        // Render thinking section with same styling as top-level thinking
                        let element_id = SharedString::from(format!("nested-thinking-{}", id));

                        // Get preview (first line, max 50 chars)
                        let preview = if content.is_empty() {
                            String::new()
                        } else {
                            let first_line = content.lines().next().unwrap_or("");
                            let truncated: String = first_line.chars().take(50).collect();
                            if truncated.len() < first_line.len() || content.contains('\n') {
                                format!("{}...", truncated)
                            } else {
                                truncated
                            }
                        };

                        div()
                            .id(element_id)
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .py(px(3.))
                            .my(px(2.))
                            .text_size(px(13.))
                            // Muted bullet
                            .child(div().text_color(theme.tool_bullet).child("•"))
                            // Semi-bold colored "Thinking"
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.tool_verb)
                                    .child("Thinking"),
                            )
                            // Preview text
                            .when(!preview.is_empty(), |el| {
                                el.child(div().text_color(theme.text_muted).child(preview))
                            })
                            // Completion indicator
                            .when(*is_complete, |el| {
                                el.child(div().text_color(theme.success).child("✓"))
                            })
                            .into_any_element()
                    }
                })
                .collect();

            div()
                .w_full()
                .overflow_x_hidden()
                .children(children)
                .into_any_element()
        };

        // Create the collapsible in display-only mode (we handle clicks on the container)
        let collapsible_element = collapsible_display(props, content);

        // Clone section_id and view for the click handler closure
        let section_id = agent_section.id.clone();
        let view = view.clone();

        div()
            .id(SharedString::from(format!("agent-{}-container", stable_id)))
            .w_full()
            .my(px(8.)) // Vertical margin for visual separation
            .cursor_pointer()
            .on_mouse_down(MouseButton::Left, move |_event, _window, cx| {
                view.update(cx, |app, cx: &mut Context<ChatApp>| {
                    app.conversation.toggle_section_collapsed(&section_id);
                    cx.notify();
                });
            })
            .child(collapsible_element)
            .into_any_element()
    }
}
