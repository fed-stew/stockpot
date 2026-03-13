//! Message bus event handling.

use super::TuiApp;
use crate::tui::activity::Activity;
use spot_core::messaging::{AgentEvent, Message, ToolStatus};

impl TuiApp {
    /// Handle incoming messages from the message bus
    pub(super) fn handle_bus_message(&mut self, msg: Message) {
        match msg {
            Message::TextDelta(delta) => {
                // Check if this delta is from a nested agent
                if let Some(agent_name) = &delta.agent_name {
                    // Route to the nested agent's section
                    if let Some(section_id) = self.active_section_ids.get(agent_name).cloned() {
                        self.conversation
                            .append_to_nested_agent(&section_id, &delta.text);
                    } else {
                        // Fallback: append to main content if section not found
                        self.conversation.append_to_current(&delta.text);
                    }
                } else {
                    // No agent attribution - append to current (handles main agent)
                    self.conversation.append_to_current(&delta.text);
                }

                // Activity feed: update or create Streaming activity
                if let Some(Activity::Streaming {
                    content, elapsed, ..
                }) = self.activities.last_mut()
                {
                    content.push_str(&delta.text);
                    *elapsed = self.stream_start.map(|s| s.elapsed()).unwrap_or_default();
                } else {
                    self.activities
                        .push(Activity::streaming("Responding", true));
                    if let Some(Activity::Streaming { content, .. }) = self.activities.last_mut() {
                        content.push_str(&delta.text);
                    }
                }

                self.update_throughput(delta.text.len());
                // Scroll to bottom on new content
                self.message_list_state.scroll_to_bottom();
                self.activity_scroll_to_bottom();
            }
            Message::Thinking(thinking) => {
                tracing::info!(
                    "TUI: Received Thinking message, text_len={}, agent={:?}",
                    thinking.text.len(),
                    thinking.agent_name
                );
                if let Some(agent_name) = &thinking.agent_name {
                    if let Some(section_id) = self.active_section_ids.get(agent_name).cloned() {
                        self.conversation
                            .append_thinking_in_section(&section_id, &thinking.text);
                    } else {
                        self.conversation.append_thinking(&thinking.text);
                    }
                } else {
                    self.conversation.append_thinking(&thinking.text);
                }

                // Activity feed: append to existing Thinking or create new
                if let Some(Activity::Thinking { content, .. }) = self.activities.last_mut() {
                    // Append to existing thinking block
                    content.push_str(&thinking.text);
                } else {
                    // Create new thinking activity
                    self.activities.push(Activity::thinking(&thinking.text));
                }

                self.message_list_state.scroll_to_bottom();
                self.activity_scroll_to_bottom();
            }
            Message::Tool(tool) => {
                tracing::info!(
                    "TOOL MSG: name='{}' status={:?} args={:?}",
                    tool.tool_name,
                    tool.status,
                    tool.args
                );

                // Existing conversation-based tool tracking
                match tool.status {
                    ToolStatus::Executing => {
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) =
                                self.active_section_ids.get(agent_name).cloned()
                            {
                                self.conversation.append_tool_call_to_section(
                                    &section_id,
                                    &tool.tool_name,
                                    tool.args.clone(),
                                );
                            } else {
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
                            if let Some(section_id) =
                                self.active_section_ids.get(agent_name).cloned()
                            {
                                self.conversation.complete_tool_call_in_section(
                                    &section_id,
                                    &tool.tool_name,
                                    true,
                                );
                            } else {
                                self.conversation.complete_tool_call(&tool.tool_name, true);
                            }
                        } else {
                            self.conversation.complete_tool_call(&tool.tool_name, true);
                        }
                    }
                    ToolStatus::Failed => {
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) =
                                self.active_section_ids.get(agent_name).cloned()
                            {
                                self.conversation.complete_tool_call_in_section(
                                    &section_id,
                                    &tool.tool_name,
                                    false,
                                );
                            } else {
                                self.conversation.complete_tool_call(&tool.tool_name, false);
                            }
                        } else {
                            self.conversation.complete_tool_call(&tool.tool_name, false);
                        }
                    }
                    _ => {}
                }

                // Activity feed: convert tool to activities
                let new_activities = self.activity_converter.process_tool(&tool);
                tracing::info!(
                    "ACTIVITIES: got {} from converter for '{}' status={:?}",
                    new_activities.len(),
                    tool.tool_name,
                    tool.status
                );
                for activity in new_activities {
                    tracing::info!(
                        "ACTIVITY: processing {:?}",
                        std::mem::discriminant(&activity)
                    );
                    // Merge consecutive Explored activities (file reads/lists)
                    if let (
                        Some(Activity::Explored { actions, .. }),
                        Activity::Explored {
                            actions: new_actions,
                            ..
                        },
                    ) = (self.activities.last_mut(), &activity)
                    {
                        tracing::info!("ACTIVITY: merging into existing Explored");
                        actions.extend(new_actions.clone());
                    } else {
                        tracing::info!(
                            "ACTIVITY: pushing new activity, total now {}",
                            self.activities.len() + 1
                        );
                        self.activities.push(activity);
                    }
                }
                if !self.activities.is_empty() {
                    self.activity_scroll_to_bottom();
                }

                self.message_list_state.scroll_to_bottom();
            }
            Message::Agent(agent) => match agent.event {
                AgentEvent::Started => {
                    if self.active_agent_stack.is_empty() {
                        // Main agent starting
                        self.conversation.start_assistant_message();
                        self.is_generating = true;
                        self.reset_throughput();
                        self.stream_start = Some(Instant::now());
                    } else {
                        // Sub-agent starting
                        if let Some(section_id) = self
                            .conversation
                            .start_nested_agent(&agent.agent_name, &agent.display_name)
                        {
                            self.active_section_ids
                                .insert(agent.agent_name.clone(), section_id);
                        }
                        // Activity feed: add nested agent activity
                        self.activities.push(Activity::nested_agent(
                            &agent.agent_name,
                            &agent.display_name,
                        ));
                    }
                    self.active_agent_stack.push(agent.agent_name.clone());
                    self.message_list_state.scroll_to_bottom();
                    self.activity_scroll_to_bottom();
                }
                AgentEvent::Completed { .. } => {
                    if let Some(completed_agent) = self.active_agent_stack.pop() {
                        if let Some(section_id) = self.active_section_ids.remove(&completed_agent) {
                            self.conversation.finish_nested_agent(&section_id);
                            self.conversation.set_section_collapsed(&section_id, true);
                        }
                    }
                    if self.active_agent_stack.is_empty() {
                        self.conversation.finish_current_message();
                        self.is_generating = false;
                        self.stream_start = None;
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
                            break;
                        }
                    }

                    if self.active_agent_stack.is_empty() {
                        self.conversation
                            .append_to_current(&format!("\n\n❌ Error: {}", message));
                        self.conversation.finish_current_message();
                        self.is_generating = false;
                        self.stream_start = None;
                    }

                    // Activity feed: add error as a failed task
                    let mut error_task = Activity::task(format!("❌ Error: {}", message));
                    if let Activity::Task { completed, .. } = &mut error_task {
                        *completed = false;
                    }
                    self.activities.push(error_task);
                }
            },
            Message::HistoryUpdate(history_update) => {
                // Update message history from executor result
                if !history_update.messages.is_empty() {
                    self.message_history = history_update.messages;
                    tracing::debug!(
                        history_len = self.message_history.len(),
                        "Updated message history from executor"
                    );
                }
            }
            Message::ContextInfo(info) => {
                // Update context usage with real data from the agent
                self.context_tokens_used = info.estimated_tokens;
                if let Some(limit) = info.context_limit {
                    self.context_window_size = limit as usize;
                }
                tracing::debug!(
                    tokens = info.estimated_tokens,
                    bytes = info.request_bytes,
                    limit = ?info.context_limit,
                    "Context info received from agent"
                );
            }
            Message::ContextCompressed(compressed) => {
                tracing::info!(
                    original = compressed.original_tokens,
                    compressed = compressed.compressed_tokens,
                    strategy = %compressed.strategy,
                    messages_before = compressed.messages_before,
                    messages_after = compressed.messages_after,
                    "Context was compressed"
                );
                self.context_tokens_used = compressed.compressed_tokens;
            }
            _ => {}
        }
    }
}

use std::time::Instant;
