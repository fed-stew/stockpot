//! Message handling and streaming for ChatApp
//!
//! This module handles incoming messages and streaming responses:
//! - `start_ui_event_loop()` - Unified event loop handling both messages and animation ticks
//! - `handle_message()` - Process incoming Message events
//! - `toggle_agent_section()` - Toggle collapsible agent sections
//!
//! NOTE: Auto-scroll is handled manually with smooth animation (see scroll_animation.rs).
//! We use ListAlignment::Top to prevent GPUI from auto-snapping to bottom.
//!
//! ## Race Condition Prevention
//!
//! Previously, we had TWO separate spawned tasks:
//! 1. `start_animation_timer()` - every 8ms calling `this.update()`
//! 2. `start_message_listener()` - on each message calling `this.update()`
//!
//! This caused `RefCell already borrowed` panics when both called update() simultaneously.
//!
//! The solution is a unified event loop that:
//! - Processes EITHER a tick OR a message per iteration
//! - Has only ONE `this.update()` call per loop iteration
//! - Eliminates all race conditions
//!
//! ## Two-Mode Event Loop (Scroll Performance Fix)
//!
//! The event loop has two modes based on `is_generating` state:
//!
//! 1. **Active (streaming)**: Uses 8ms timeout for animation ticks (~120fps).
//!    Needed for smooth throughput display and scroll-to-bottom animation.
//!
//! 2. **Idle (not streaming)**: Waits indefinitely for messages with NO timeout.
//!    This lets GPUI's native vsync handle all frame timing, enabling smooth
//!    60fps scrolling without interference from our polling loop.
//!
//! Previously, the loop always used 8ms polling which caused scroll jank when
//! idle because the constant `cx.notify()` calls interfered with GPUI's vsync.

use std::time::Duration;

use gpui::{AsyncApp, Context, WeakEntity};
use tokio::time::timeout;

use crate::messaging::{AgentEvent, Message, ToolStatus};

use super::ChatApp;

impl ChatApp {
    /// Start the unified UI event loop.
    ///
    /// This is the ONLY place where `this.update()` is called from async tasks,
    /// ensuring no race conditions. The loop:
    /// - Runs for the entire app lifetime
    /// - During streaming: uses 8ms timeout for smooth animation ticks (~120fps)
    /// - When idle: waits indefinitely for messages, letting GPUI control frame timing
    ///
    /// This two-mode approach is critical for scroll performance:
    /// - During streaming: we need fast ticks for throughput/scroll animation
    /// - When idle: GPUI's native vsync handles scrolling smoothly
    ///   (constant 8ms polling would interfere with vsync and cause jank)
    pub(super) fn start_ui_event_loop(&self, cx: &mut Context<Self>) {
        let mut receiver = self.message_bus.subscribe();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            loop {
                // Check if we're actively generating (need fast animation ticks)
                let is_active = this.update(cx, |app, _| app.is_generating).unwrap_or(false);

                if is_active {
                    // During streaming: use 8ms timeout for smooth animation ticks
                    let tick_duration = Duration::from_millis(8);

                    match timeout(tick_duration, receiver.recv()).await {
                        // Message received - process it
                        Ok(Ok(msg)) => {
                            let result = this.update(cx, |app, cx| {
                                app.handle_message(msg, cx);
                            });
                            if result.is_err() {
                                break; // Entity dropped
                            }
                        }
                        // Channel closed - exit loop
                        Ok(Err(_)) => {
                            break;
                        }
                        // Timeout - animation tick
                        Err(_) => {
                            let result = this.update(cx, |app, cx| {
                                app.tick_throughput();
                                app.tick_scroll_animation();
                                cx.notify();
                            });
                            if result.is_err() {
                                break; // Entity dropped
                            }
                        }
                    }
                } else {
                    // When idle: wait indefinitely for messages
                    // Let GPUI handle ALL frame timing natively for smooth scrolling
                    match receiver.recv().await {
                        Ok(msg) => {
                            let result = this.update(cx, |app, cx| {
                                app.handle_message(msg, cx);
                            });
                            if result.is_err() {
                                break; // Entity dropped
                            }
                        }
                        Err(_) => {
                            break; // Channel closed
                        }
                    }
                }
            }
        })
        .detach();
    }

    // NOTE: scroll_messages_to_bottom() was removed. We now use smooth scroll animation
    // via start_smooth_scroll_to_bottom() with ListAlignment::Top for manual control.

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

                // Smooth scroll to bottom when content grows (if user hasn't scrolled away)
                // This prevents the "jumpy" instant scroll, especially with newlines
                if !self.user_scrolled_away {
                    self.start_smooth_scroll_to_bottom();
                }
            }
            Message::Thinking(thinking) => {
                // Create or append to thinking section (collapsible with hover tooltip)
                self.conversation.append_thinking(&thinking.text);
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
                        self.sync_messages_list_state();
                        self.is_generating = true;
                        // NOTE: We no longer call start_animation_timer() here!
                        // The unified event loop handles ticks automatically when is_generating is true.
                        // Reset scroll state for new response
                        self.user_scrolled_away = false;
                        // Trigger smooth scroll to show the new assistant message
                        self.start_smooth_scroll_to_bottom();
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
                        self.sync_messages_list_state();
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

        // Animation timer already calls cx.notify() at 8ms during streaming.
        // TextDelta events just update state - no need to trigger additional renders.
        // This prevents double-rendering and reduces GPU pressure.
        let should_notify = match &msg {
            Message::TextDelta(_) => false, // Animation timer handles render
            _ => true,                      // Other message types still notify immediately
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
