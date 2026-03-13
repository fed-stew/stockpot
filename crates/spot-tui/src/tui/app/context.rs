//! Context window and token tracking.

use spot_core::config::Settings;

use super::TuiApp;

impl TuiApp {
    /// Get context usage as percentage
    pub fn context_percentage(&self) -> u8 {
        if self.context_window_size == 0 {
            return 0;
        }
        ((self.context_tokens_used as f64 / self.context_window_size as f64) * 100.0) as u8
    }

    /// Get effective model for an agent (pinned or default)
    pub(super) fn effective_model_for_agent(&self, agent_name: &str) -> String {
        let settings = Settings::new(&self.db);
        settings
            .get_agent_pinned_model(agent_name)
            .unwrap_or_else(|| self.current_model.clone())
    }

    /// Update the context window size based on current model (tokens come from agent ContextInfo)
    pub fn update_context_usage(&mut self) {
        // Update window size from current model
        if let Some(model) = self.model_registry.get(&self.current_model) {
            self.context_window_size = model.context_length;
        }
        // Note: context_tokens_used is ONLY updated by ContextInfo events from the agent
    }
}
