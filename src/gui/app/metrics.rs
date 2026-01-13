//! Context and throughput metrics for ChatApp
//!
//! This module handles context window tracking and throughput calculations:
//! - `effective_model_for_agent()` - Get the model for a specific agent
//! - `current_effective_model()` - Get the current active model
//! - `update_context_usage()` - Update token/context estimates
//! - `update_throughput()` - Calculate streaming throughput
//! - `reset_throughput()` - Reset throughput metrics
//! - `current_agent_display()` - Get display name for current agent
//! - `truncate_model_name()` - Truncate long model names for display

use crate::config::Settings;

use super::ChatApp;

impl ChatApp {
    /// Get effective model for an agent (pinned or default).
    pub(super) fn effective_model_for_agent(&self, agent_name: &str) -> (String, bool) {
        let settings = Settings::new(&self.db);
        if let Some(pinned) = settings.get_agent_pinned_model(agent_name) {
            (pinned, true)
        } else {
            (self.current_model.clone(), false)
        }
    }

    pub(super) fn current_effective_model(&self) -> (String, bool) {
        self.effective_model_for_agent(&self.current_agent)
    }

    /// Update the context usage metrics based on current message history
    pub(super) fn update_context_usage(&mut self) {
        // Get context window size from current model
        let (effective_model, _) = self.current_effective_model();
        let model = self.model_registry.get(&effective_model);
        self.context_window_size = model.map(|m| m.context_length).unwrap_or(128_000);

        // Calculate base overhead (System Prompt + Tool Definitions)
        let mut overhead_tokens = 0;

        if let Some(agent) = self.agents.get(&self.current_agent) {
            // 1. System Prompt
            let system_prompt = agent.system_prompt();
            if !system_prompt.is_empty() {
                overhead_tokens += (system_prompt.len() / 4).max(10);
            }

            // 2. Tool Definitions
            let tool_names = agent.available_tools();
            // Filter tools based on settings (similar to execution logic)
            // We use the same filter logic as in executor/mod.rs roughly,
            // or just count all available tools since the agent *might* use them.
            // For accuracy, we should replicate the executor's filtering, but
            // for now, counting the agent's declared tools is a safe upper bound.
            let tools = self.tool_registry.tools_by_name(&tool_names);

            for tool in tools {
                let def = tool.definition();
                if let Ok(json) = serde_json::to_string(&def) {
                    overhead_tokens += (json.len() / 4).max(10);
                }
            }
        }

        // Calculate base tokens from confirmed message history (always available)
        let history_tokens = if !self.message_history.is_empty() {
            crate::tokens::estimate_tokens(&self.message_history)
        } else {
            0
        };

        if self.is_generating {
            // During streaming, we take the base history tokens (which are accurate)
            // and ADD the estimated tokens for the current turn (User prompt + Streaming response)

            let mut pending_tokens = 0;

            // 1. Add estimated tokens for the last user message (the prompt)
            if let Some(user_msg) = self
                .conversation
                .messages
                .iter()
                .rfind(|m| m.role == crate::gui::state::MessageRole::User)
            {
                pending_tokens += (user_msg.content.len() / 4).max(10);
            }

            // 2. Add estimated tokens for the active assistant message (streaming response)
            // We check the very last message - if it's from the assistant, it's the one being generated
            if let Some(last_msg) = self.conversation.messages.last() {
                if last_msg.role == crate::gui::state::MessageRole::Assistant {
                    pending_tokens += (last_msg.content.len() / 4).max(10);
                }
            }

            self.context_tokens_used = overhead_tokens + history_tokens + pending_tokens;
        } else if !self.message_history.is_empty() {
            self.context_tokens_used = overhead_tokens + history_tokens;
        } else {
            // Fallback for empty history (e.g. first run, or just cleared)
            let conv_chars: usize = self
                .conversation
                .messages
                .iter()
                .map(|m| m.content.len())
                .sum();
            self.context_tokens_used = overhead_tokens + (conv_chars / 4);
        }

        tracing::debug!(
            model = %effective_model,
            context_window = self.context_window_size,
            tokens_used = self.context_tokens_used,
            overhead_tokens = overhead_tokens,
            history_len = self.message_history.len(),
            "Context usage updated"
        );
    }

    /// Update throughput calculation based on incoming text delta
    pub(super) fn update_throughput(&mut self, chars_received: usize) {
        let now = std::time::Instant::now();

        // Add new sample
        self.throughput_samples.push((chars_received, now));

        // Keep only samples from last 2 seconds
        let cutoff = now - std::time::Duration::from_secs(2);
        self.throughput_samples.retain(|(_, ts)| *ts > cutoff);

        // Calculate throughput if we have samples spanning at least 100ms
        if self.throughput_samples.len() >= 2 {
            let first_ts = self.throughput_samples.first().map(|(_, ts)| *ts).unwrap();
            let elapsed = now.duration_since(first_ts).as_secs_f64();

            if elapsed > 0.1 {
                let total_chars: usize = self.throughput_samples.iter().map(|(c, _)| *c).sum();
                self.current_throughput_cps = total_chars as f64 / elapsed;
            }
        }

        // Add to history every 250ms for chart display
        if self.last_history_sample.elapsed() > std::time::Duration::from_millis(250) {
            self.throughput_history
                .push_back(self.current_throughput_cps);
            // Keep only last 8 samples
            while self.throughput_history.len() > 8 {
                self.throughput_history.pop_front();
            }
            self.last_history_sample = now;
        }

        self.is_streaming_active = true;
    }

    /// Reset throughput tracking (call when streaming starts)
    pub(super) fn reset_throughput(&mut self) {
        self.throughput_samples.clear();
        self.throughput_history.clear();
        self.current_throughput_cps = 0.0;
        self.is_streaming_active = false;
    }

    /// Called by animation timer to keep throughput display fresh even when no data arriving.
    /// Removes old samples and recalculates current throughput.
    pub(super) fn tick_throughput(&mut self) {
        let now = std::time::Instant::now();

        // Remove old samples (older than 2 seconds)
        let cutoff = now - std::time::Duration::from_secs(2);
        self.throughput_samples.retain(|(_, ts)| *ts > cutoff);

        // Recalculate throughput
        if self.throughput_samples.len() >= 2 {
            let first_ts = self.throughput_samples.first().map(|(_, ts)| *ts).unwrap();
            let elapsed = now.duration_since(first_ts).as_secs_f64();
            if elapsed > 0.1 {
                let total_chars: usize = self.throughput_samples.iter().map(|(c, _)| *c).sum();
                self.current_throughput_cps = total_chars as f64 / elapsed;
            }
        } else if self.throughput_samples.is_empty() {
            // Decay to zero when no recent samples
            self.current_throughput_cps = 0.0;
        }

        // Update history for chart every 250ms
        if self.last_history_sample.elapsed() > std::time::Duration::from_millis(250) {
            self.throughput_history
                .push_back(self.current_throughput_cps);
            while self.throughput_history.len() > 8 {
                self.throughput_history.pop_front();
            }
            self.last_history_sample = now;
        }
    }

    /// Get display name for current agent
    pub(super) fn current_agent_display(&self) -> String {
        self.available_agents
            .iter()
            .find(|(name, _)| name == &self.current_agent)
            .map(|(_, display)| display.clone())
            .unwrap_or_else(|| self.current_agent.clone())
    }

    /// Truncate model name for display
    pub(super) fn truncate_model_name(name: &str) -> String {
        name.to_string()
    }
}
