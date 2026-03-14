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

use crate::gui::state::MessageSection;
use spot_core::messaging::{AgentEvent, Message, ToolStatus};

use super::ChatApp;

impl ChatApp {
    fn update_text_view_cache(
        &mut self,
        element_id: String,
        full_text: &str,
        delta: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if let Some(entity) = self.text_view_cache.borrow().get(&element_id).cloned() {
            entity.update(cx, |view, cx| {
                // If full_text is empty and we have a delta, it's a buffered update
                if full_text.is_empty() {
                    if let Some(d) = delta {
                        view.append_delta(d, cx);
                        return;
                    }
                }
                // Otherwise, standard full update
                view.update_content(full_text, cx);
            });
            return;
        }

        let theme = self.theme.clone();
        let full_text = full_text.to_string();
        let entity = cx.new(|cx| {
            let mut view = crate::gui::components::StreamingMarkdownView::new(theme);
            view.update_content(&full_text, cx);
            view
        });
        self.text_view_cache.borrow_mut().insert(element_id, entity);
    }

    /// Start the unified UI event loop.
    ///
    /// Uses `tokio::select!` to handle messages and animation ticks independently:
    /// - Animation frame rate adapts based on VDI mode (120fps normal, ~15fps VDI)
    /// - Only triggers re-renders when content actually changes
    /// - When idle (not generating), animation timer is disabled to save CPU
    pub(super) fn start_ui_event_loop(&self, cx: &mut Context<Self>) {
        let mut receiver = self.message_bus.subscribe();

        // Determine frame interval based on VDI mode
        let frame_interval_ms = if self.vdi_mode {
            let settings = spot_core::config::Settings::new(&self.db);
            settings.get_vdi_frame_interval_ms()
        } else {
            8 // 120fps default
        };
        tracing::info!(
            vdi_mode = self.vdi_mode,
            frame_interval_ms,
            "Starting UI event loop"
        );

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            use tokio::time::{interval, Duration};

            let mut animation_interval = interval(Duration::from_millis(frame_interval_ms));
            animation_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                // Check if we're actively generating (need animation ticks)
                let is_active = this.update(cx, |app, _| app.is_generating).unwrap_or(false);

                if is_active {
                    // During streaming: alternate between messages and animation ticks.
                    // Message branch micro-batches all pending tokens per frame to reduce
                    // latency and calls cx.notify() for immediate repaint.
                    tokio::select! {
                        // Animation tick — update scroll/throughput and trigger render
                        _ = animation_interval.tick() => {
                            let result = this.update(cx, |app, cx| {
                                app.tick_throughput();
                                let scroll_moved = app.tick_scroll_animation();

                                if scroll_moved || app.needs_render {
                                    app.needs_render = false;
                                    cx.notify();
                                }
                            });
                            if result.is_err() {
                                break; // Entity dropped
                            }
                        }

                        // Message arrives — micro-batch all pending messages for this frame
                        msg = receiver.recv() => {
                            match msg {
                                Ok(msg) => {
                                    // Collect this message plus all immediately available ones
                                    let mut messages = vec![msg];
                                    // Drain pending (non-blocking) — micro-batch for this frame
                                    loop {
                                        match receiver.try_recv() {
                                            Ok(Some(m)) => messages.push(m),
                                            Ok(None) => break,      // No more pending
                                            Err(_) => break,        // Lagged or closed — recover on next delta
                                        }
                                    }
                                    // Process all collected messages in one update call
                                    let result = this.update(cx, |app, cx| {
                                        for m in messages {
                                            app.handle_message(m, cx);
                                        }
                                        // Trigger immediate repaint — don't wait for next tick
                                        if app.needs_render {
                                            app.needs_render = false;
                                            cx.notify();
                                        }
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
                let routed_to_nested = if let Some(agent_name) = &delta.agent_name {
                    // Route to the nested agent's section
                    if let Some(section_id) = self.active_section_ids.get(agent_name) {
                        self.conversation
                            .append_to_nested_agent(section_id, &delta.text);
                        true // Was routed to nested
                    } else {
                        // Fallback: append to main content if section not found
                        self.conversation.append_to_current(&delta.text);
                        false
                    }
                } else {
                    // No agent attribution - append to current (handles main agent)
                    self.conversation.append_to_current(&delta.text);
                    false
                };

                // Track throughput
                self.update_throughput(delta.text.len());

                // Prepare cache update data (to avoid double borrowing self)
                let cache_update = if routed_to_nested {
                    // Handle nested agent cache update
                    if let Some(agent_name) = &delta.agent_name {
                        if let Some(section_id) = self.active_section_ids.get(agent_name) {
                            if let Some(msg) = self.conversation.messages.last() {
                                if let Some(section) = msg.get_nested_section(section_id) {
                                    // Find the last text item (where we just appended)
                                    section.items.iter().enumerate().rev().find_map(
                                        |(idx, item)| match item {
                                            crate::gui::state::AgentContentItem::Text(t) => Some((
                                                format!("agent-{}-text-{}", section.id, idx),
                                                t.clone(),
                                            )),
                                            _ => None,
                                        },
                                    )
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    // Handle main content cache update
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
                        Some((element_id, full_text))
                    } else {
                        None
                    }
                };

                // Apply delta directly to the text view — no buffering.
                // This ensures each token is applied as it arrives rather
                // than being batched into large chunks.
                if let Some((element_id, full_text)) = cache_update {
                    let delta_text = delta.text.clone();
                    self.update_text_view_cache(element_id, &full_text, Some(&delta_text), cx);
                    self.needs_render = true;
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

                    // Prepare cache update (to avoid double borrowing self)
                    let cache_update = self.conversation.messages.last().and_then(|msg| {
                        msg.active_thinking_section_id().and_then(|section_id| {
                            msg.get_thinking_section(section_id).map(|section| {
                                (
                                    format!("thinking-{}-content", section.id),
                                    section.content.clone(),
                                )
                            })
                        })
                    });

                    // Apply directly to the text view
                    if let Some((element_id, full_text)) = cache_update {
                        let delta_text = thinking.text.clone();
                        self.update_text_view_cache(
                            element_id,
                            &full_text,
                            Some(delta_text.as_str()),
                            cx,
                        );
                        self.needs_render = true;
                    }
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
                        // Use .get() instead of .remove() - keep section_id for late-arriving events
                        if let Some(section_id) = self.active_section_ids.get(&completed_agent) {
                            // Finish the nested section
                            self.conversation.finish_nested_agent(section_id);
                            // Auto-collapse completed sub-agent sections
                            self.conversation.set_section_collapsed(section_id, true);
                        }
                    }

                    // Only finish generating if main agent completed (stack empty)
                    if self.active_agent_stack.is_empty() {
                        self.conversation.finish_current_message();
                        self.is_generating = false;
                        self.sync_messages_list_state();
                        // Stop throughput tracking
                        self.is_streaming_active = false;

                        // Finalize any in-progress tables in all cached text views
                        for entity in self.text_view_cache.borrow().values() {
                            entity.update(cx, |view, cx| {
                                view.finalize_tables(cx);
                            });
                        }

                        // Clear section mappings - safe now since main agent is done
                        self.active_section_ids.clear();
                    }
                }
                AgentEvent::Error { message } => {
                    // Pop all agents down to (and including) the errored one
                    while let Some(agent_name) = self.active_agent_stack.pop() {
                        if let Some(section_id) = self.active_section_ids.get(&agent_name) {
                            self.conversation.append_to_nested_agent(
                                section_id,
                                &format!("\n\n❌ Error: {}", message),
                            );
                            self.conversation.finish_nested_agent(section_id);
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

                        // Clear section mappings
                        self.active_section_ids.clear();
                    }
                }
            },
            Message::ContextInfo(info) => {
                // Update context usage with real data from the agent
                self.context_tokens_used = info.estimated_tokens;
                if let Some(limit) = info.context_limit {
                    self.context_window_size = limit as usize;
                }
                // Log for debugging
                tracing::debug!(
                    tokens = info.estimated_tokens,
                    bytes = info.request_bytes,
                    limit = ?info.context_limit,
                    "Context info received from agent"
                );
            }
            Message::ContextCompressed(compressed) => {
                // Log compression event
                tracing::info!(
                    original = compressed.original_tokens,
                    compressed = compressed.compressed_tokens,
                    strategy = %compressed.strategy,
                    messages_before = compressed.messages_before,
                    messages_after = compressed.messages_after,
                    "Context was compressed"
                );
                // Update displayed token count
                self.context_tokens_used = compressed.compressed_tokens;

                // Show notification
                let saved_tokens = compressed
                    .original_tokens
                    .saturating_sub(compressed.compressed_tokens);
                let notification = format!(
                    "📦 Context compressed: {} → {} tokens ({} saved) using {}",
                    spot_core::tokens::format_tokens_with_separator(compressed.original_tokens),
                    spot_core::tokens::format_tokens_with_separator(compressed.compressed_tokens),
                    spot_core::tokens::format_tokens_with_separator(saved_tokens),
                    compressed.strategy
                );
                self.compression_notification = Some((std::time::Instant::now(), notification));
            }
            _ => {}
        }

        // For non-streaming messages, flag a render or notify immediately.
        // TextDelta and Thinking already set needs_render above.
        if !matches!(&msg, Message::TextDelta(_) | Message::Thinking(_)) {
            if self.is_generating {
                self.needs_render = true;
            } else {
                cx.notify();
            }
        }
    }
}
