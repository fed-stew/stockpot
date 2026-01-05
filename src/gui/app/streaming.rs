//! Message handling and streaming for ChatApp
//!
//! This module handles incoming messages and streaming responses:
//! - `start_message_listener()` - Set up the message bus listener
//! - `scroll_messages_to_bottom()` - Auto-scroll chat view
//! - `handle_message()` - Process incoming Message events
//! - `toggle_agent_section()` - Toggle collapsible agent sections

use gpui::{AsyncApp, Context, WeakEntity};

use crate::messaging::{AgentEvent, Message, ToolStatus};

use super::ChatApp;

impl ChatApp {
    pub(super) fn start_message_listener(&self, cx: &mut Context<Self>) {
        let mut receiver = self.message_bus.subscribe();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            while let Ok(msg) = receiver.recv().await {
                let result = this.update(cx, |app, cx| {
                    app.handle_message(msg, cx);
                });
                if result.is_err() {
                    break; // Entity dropped
                }
            }
        })
        .detach();
    }

    /// Scroll the messages view to the bottom
    pub(super) fn scroll_messages_to_bottom(&self) {
        let max = self.messages_scroll_handle.max_offset().height;
        if max > gpui::px(0.) {
            self.messages_scroll_handle
                .set_offset(gpui::point(gpui::px(0.), -max));
        }
    }

    /// Handle incoming messages from the agent
    pub(super) fn handle_message(&mut self, msg: Message, cx: &mut Context<Self>) {
        match &msg {
            Message::TextDelta(delta) => {
                // Check if this delta is from a nested agent
                if let Some(agent_name) = &delta.agent_name {
                    // Route to the nested agent's section
                    if let Some(section_id) = self.active_section_ids.get(agent_name) {
                        self.conversation
                            .append_to_nested_agent(section_id, &delta.text);
                    } else {
                        // Fallback: append to main content if section not found
                        self.conversation.append_to_current(&delta.text);
                    }
                } else {
                    // No agent attribution - append to current (handles main agent)
                    self.conversation.append_to_current(&delta.text);
                }

                // Track throughput
                self.update_throughput(delta.text.len());

                // Throttled context usage update (every 500ms during streaming)
                if self.last_context_update.elapsed() > std::time::Duration::from_millis(500) {
                    self.update_context_usage();
                    self.last_context_update = std::time::Instant::now();
                }

                // Auto-scroll to bottom if user hasn't scrolled away
                if !self.user_scrolled_away {
                    self.scroll_messages_to_bottom();
                }
            }
            Message::Thinking(thinking) => {
                // Display thinking in a muted style
                self.conversation
                    .append_to_current(&format!("\n\n💭 {}\n\n", thinking.text));
            }
            Message::Tool(tool) => {
                match tool.status {
                    ToolStatus::Executing => {
                        // Check if this tool is from a nested agent
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) = self.active_section_ids.get(agent_name) {
                                // Route to nested section
                                self.conversation.append_tool_call_to_section(
                                    section_id,
                                    &tool.tool_name,
                                    tool.args.clone(),
                                );
                            } else {
                                // Fallback to main content
                                self.conversation
                                    .append_tool_call(&tool.tool_name, tool.args.clone());
                            }
                        } else {
                            self.conversation
                                .append_tool_call(&tool.tool_name, tool.args.clone());
                        }
                    }
                    ToolStatus::Completed => {
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) = self.active_section_ids.get(agent_name) {
                                self.conversation.complete_tool_call_in_section(
                                    section_id,
                                    &tool.tool_name,
                                    true,
                                );
                            } else {
                                self.conversation.complete_tool_call(&tool.tool_name, true);
                            }
                        } else {
                            self.conversation.complete_tool_call(&tool.tool_name, true);
                        }
                        // Update context usage after tool completes
                        self.update_context_usage();
                    }
                    ToolStatus::Failed => {
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) = self.active_section_ids.get(agent_name) {
                                self.conversation.complete_tool_call_in_section(
                                    section_id,
                                    &tool.tool_name,
                                    false,
                                );
                            } else {
                                self.conversation.complete_tool_call(&tool.tool_name, false);
                            }
                        } else {
                            self.conversation.complete_tool_call(&tool.tool_name, false);
                        }
                        // Update context usage after tool fails
                        self.update_context_usage();
                    }
                    _ => {}
                }
            }
            Message::Agent(agent) => match &agent.event {
                AgentEvent::Started => {
                    if self.active_agent_stack.is_empty() {
                        // Main agent starting - existing behavior
                        self.conversation.start_assistant_message();
                        self.is_generating = true;
                        // Reset scroll state and scroll to bottom for new response
                        self.user_scrolled_away = false;
                        self.scroll_messages_to_bottom();
                        // Update context at start of conversation
                        self.update_context_usage();
                        // Reset throughput tracking for new response
                        self.reset_throughput();
                    } else {
                        // Sub-agent starting - create collapsible section
                        if let Some(section_id) = self
                            .conversation
                            .start_nested_agent(&agent.agent_name, &agent.display_name)
                        {
                            self.active_section_ids
                                .insert(agent.agent_name.clone(), section_id);
                        }
                    }
                    self.active_agent_stack.push(agent.agent_name.clone());
                }
                AgentEvent::Completed { .. } => {
                    // Pop this agent from stack
                    if let Some(completed_agent) = self.active_agent_stack.pop() {
                        if let Some(section_id) = self.active_section_ids.remove(&completed_agent) {
                            // Finish the nested section
                            self.conversation.finish_nested_agent(&section_id);
                            // Auto-collapse completed sub-agent sections
                            self.conversation.set_section_collapsed(&section_id, true);
                        }
                    }

                    // Update context usage when agent completes
                    self.update_context_usage();

                    // Only finish generating if main agent completed (stack empty)
                    if self.active_agent_stack.is_empty() {
                        self.conversation.finish_current_message();
                        self.is_generating = false;
                        // Stop throughput tracking
                        self.is_streaming_active = false;
                    }
                }
                AgentEvent::Error { message } => {
                    // Pop all agents down to (and including) the errored one
                    while let Some(agent_name) = self.active_agent_stack.pop() {
                        if let Some(section_id) = self.active_section_ids.remove(&agent_name) {
                            self.conversation.append_to_nested_agent(
                                &section_id,
                                &format!("\n\n❌ Error: {}", message),
                            );
                            self.conversation.finish_nested_agent(&section_id);
                        }
                        if agent_name == agent.agent_name {
                            break; // Found the errored agent, stop unwinding
                        }
                    }

                    // If stack is now empty, the main agent errored
                    if self.active_agent_stack.is_empty() {
                        self.conversation
                            .append_to_current(&format!("\n\n❌ Error: {}", message));
                        self.conversation.finish_current_message();
                        self.is_generating = false;
                        self.error_message = Some(message.clone());
                    }
                }
            },
            _ => {}
        }

        // Throttle notifications during streaming to ~60fps (16ms)
        let should_notify = match &msg {
            Message::TextDelta(_) => {
                if self.last_render_notify.elapsed() > std::time::Duration::from_millis(16) {
                    self.last_render_notify = std::time::Instant::now();
                    true
                } else {
                    false
                }
            }
            _ => true, // All other message types notify immediately
        };

        if should_notify {
            cx.notify();
        }
    }

    /// Toggle a nested agent section's collapsed state
    #[allow(dead_code)]
    pub(super) fn toggle_agent_section(&mut self, section_id: &str, cx: &mut Context<Self>) {
        self.conversation.toggle_section_collapsed(section_id);
        cx.notify();
    }
}
