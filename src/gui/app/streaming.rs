//! Message handling and streaming for ChatApp
//!
//! This module handles incoming messages and streaming responses:
//! - `start_ui_event_loop()` - Unified event loop handling both messages and animation ticks
//! - `handle_message()` - Process incoming Message events

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

use gpui::{AppContext, AsyncApp, Context, WeakEntity};
use gpui_component::text::TextViewState;

use crate::gui::state::MessageSection;
use crate::messaging::{AgentEvent, Message, ToolStatus};

use super::ChatApp;

impl ChatApp {
    fn update_text_view_cache(
        &mut self,
        element_id: String,
        full_text: &str,
        delta: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if let Some(state) = self.text_view_cache.borrow().get(&element_id).cloned() {
            state.update(cx, |text, cx| {
                if let Some(delta) = delta {
                    text.push_str(delta, cx);
                } else {
                    text.set_text(full_text, cx);
                }
            });
            return;
        }

        let state = cx.new(|cx| TextViewState::markdown(full_text, cx));
        self.text_view_cache.borrow_mut().insert(element_id, state);
    }

    /// Start the unified UI event loop.
    ///
    /// Uses `tokio::select!` to handle messages and animation ticks independently:
    /// - Animation runs at ~240fps (4ms) for buttery smooth scrolling
    /// - Messages are processed as they arrive without blocking animation
    /// - When idle (not generating), animation timer is disabled to save CPU
    pub(super) fn start_ui_event_loop(&self, cx: &mut Context<Self>) {
        let mut receiver = self.message_bus.subscribe();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            use tokio::time::{interval, Duration};

            // 8ms = 120fps - smooth enough for scrolling, reduces render pressure
            let mut animation_interval = interval(Duration::from_millis(8));
            animation_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                // Check if we're actively generating (need animation ticks)
                let is_active = this.update(cx, |app, _| app.is_generating).unwrap_or(false);

                if is_active {
                    // During streaming: run animation and messages in parallel with select!
                    tokio::select! {
                        biased; // Prioritize in order listed

                        // Animation tick - runs at consistent 120fps
                        _ = animation_interval.tick() => {
                            let result = this.update(cx, |app, cx| {
                                app.tick_throughput();
                                let scroll_moved = app.tick_scroll_animation();
                                // Only trigger re-render if scroll position changed
                                // This prevents render thrashing when sitting at bottom
                                if scroll_moved {
                                    cx.notify();
                                }
                            });
                            if result.is_err() {
                                break; // Entity dropped
                            }
                        }

                        // Message received - process it (no animation here)
                        msg = receiver.recv() => {
                            match msg {
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
                } else {
                    // When idle: wait indefinitely for messages, no animation needed
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

                if let Some(msg) = self.conversation.messages.last() {
                    let (element_id, full_text) = if let Some((idx, text)) = msg
                        .sections
                        .iter()
                        .enumerate()
                        .rev()
                        .find_map(|(idx, section)| match section {
                            MessageSection::Text(text) => Some((idx, text)),
                            _ => None,
                        }) {
                        (format!("msg-{}-sec-{}", msg.id, idx), text.to_string())
                    } else {
                        (format!("msg-{}-content", msg.id), msg.content.clone())
                    };
                    let delta_text = delta.text.clone();
                    self.update_text_view_cache(
                        element_id,
                        &full_text,
                        Some(delta_text.as_str()),
                        cx,
                    );
                }

                // Throttled context usage update (every 500ms during streaming)
                if self.last_context_update.elapsed() > std::time::Duration::from_millis(500) {
                    self.update_context_usage();
                    self.last_context_update = std::time::Instant::now();
                }

                // Mark that we want to scroll to bottom - the independent animation loop handles the actual scrolling
                if !self.user_scrolled_away {
                    self.start_smooth_scroll_to_bottom();
                }
            }
            Message::Thinking(thinking) => {
                // Check if this thinking is from a nested agent
                if let Some(agent_name) = &thinking.agent_name {
                    // Route to the nested agent's section
                    if let Some(section_id) = self.active_section_ids.get(agent_name) {
                        // Append thinking to the nested agent section (creates if needed)
                        self.conversation
                            .append_thinking_in_section(section_id, &thinking.text);
                    } else {
                        // Fallback: append to main conversation if section not found
                        self.conversation.append_thinking(&thinking.text);
                    }
                } else {
                    // No agent attribution - append to main conversation
                    self.conversation.append_thinking(&thinking.text);
                }
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
}
