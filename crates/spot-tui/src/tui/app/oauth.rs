//! OAuth authentication flow handling.

use spot_core::messaging::MessageSender;

use super::TuiApp;

impl TuiApp {
    pub(super) fn start_oauth_flow(&mut self) {
        // Determine which provider is selected
        let (provider, _provider_name) = match self.settings_state.oauth_selected_index {
            0 => ("claude-code", "Claude Code"),
            1 => ("chatgpt", "ChatGPT"),
            2 => ("google", "Google"),
            _ => return,
        };

        // Don't start if already in progress
        if self.settings_state.oauth_in_progress.is_some() {
            return;
        }

        // Mark as in progress
        self.settings_state.oauth_in_progress = Some(provider.to_string());

        // Clone what we need for the async task
        let db = self.db.clone();
        let sender = self.message_bus.sender();
        let dialog_tx = self.oauth_dialog_tx.clone();
        let completion_tx = self.oauth_completion_tx.clone();
        let provider_str = provider.to_string();
        let progress = MessageBusProgress::new(sender, provider_str.clone(), dialog_tx);

        // Spawn OAuth flow as local task (Database is not Send-safe)
        tokio::task::spawn_local(async move {
            let result = match provider_str.as_str() {
                "chatgpt" => spot_core::auth::run_chatgpt_auth_with_progress(&db, &progress)
                    .await
                    .map_err(|e| e.to_string()),
                "claude-code" => {
                    spot_core::auth::run_claude_code_auth_with_progress(&db, &progress)
                        .await
                        .map_err(|e| e.to_string())
                }
                "google" => spot_core::auth::run_google_auth_with_progress(&db, &progress)
                    .await
                    .map_err(|e| e.to_string()),
                _ => Err(format!("Unknown provider: {}", provider_str)),
            };

            // Signal completion via channel
            let _ = completion_tx.send((provider_str, result.map(|_| ())));
        });
    }

    /// Refresh model registry after OAuth completion
    pub fn refresh_model_registry(&mut self) {
        if let Ok(registry) = spot_core::models::ModelRegistry::load_from_db(&self.db) {
            self.model_registry = std::sync::Arc::new(registry);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OAuth Progress Reporter for TUI
// ─────────────────────────────────────────────────────────────────────────────

/// Progress reporter that sends messages to the TUI via MessageBus.
///
/// This implements `AuthProgress` to route OAuth flow messages
/// through the TUI's activity feed instead of printing to stdout.
pub struct MessageBusProgress {
    sender: MessageSender,
    provider: String,
    dialog_tx: tokio::sync::mpsc::UnboundedSender<(String, String, u16)>,
}

impl MessageBusProgress {
    pub fn new(
        sender: MessageSender,
        provider: String,
        dialog_tx: tokio::sync::mpsc::UnboundedSender<(String, String, u16)>,
    ) -> Self {
        Self {
            sender,
            provider,
            dialog_tx,
        }
    }
}

impl spot_core::auth::AuthProgress for MessageBusProgress {
    fn info(&self, msg: &str) {
        self.sender.info(msg);
    }

    fn success(&self, msg: &str) {
        self.sender.success(msg);
    }

    fn warning(&self, msg: &str) {
        self.sender.warning(msg);
    }

    fn error(&self, msg: &str) {
        self.sender.error(msg);
    }

    fn on_auth_url(&self, url: &str, port: u16) {
        // Send URL/port to TUI to show in dialog
        let _ = self
            .dialog_tx
            .send((self.provider.clone(), url.to_string(), port));
    }
}
