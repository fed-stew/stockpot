//! Action handlers for ChatApp
//!
//! This module contains keyboard action handlers and conversation management:
//! - `new_conversation()` - Start a fresh conversation
//! - `quit()` - Handle quit action
//! - `close_dialog()` - Close active dialogs
//! - `on_send()` - Handle send action
//! - `next_agent()` / `prev_agent()` - Agent navigation
//! - `set_current_agent()` - Set the active agent

use gpui::{Context, Window};

use super::{ChatApp, CloseDialog, NewConversation, NextAgent, PrevAgent, Quit, Send};

impl ChatApp {
    /// Handle new conversation
    pub(super) fn new_conversation(
        &mut self,
        _: &NewConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.conversation.clear();
        self.message_history.clear();
        self.update_context_usage();
        self.active_agent_stack.clear();
        self.active_section_ids.clear();
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.is_generating = false;
        self.show_agent_dropdown = false;
        self.show_model_dropdown = false;
        self.error_message = None;
        cx.notify();
    }

    /// Handle quit action
    pub(super) fn quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Handle escape key to close dialogs
    pub(super) fn close_dialog(
        &mut self,
        _: &CloseDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Close dialogs in order of precedence (most recent first)
        if self.show_add_model_dialog {
            self.show_add_model_dialog = false;
            self.add_model_selected_provider = None;
            self.add_model_selected_model = None;
            self.add_model_models.clear();
            if let Some(input) = &self.add_model_api_key_input_entity {
                input.update(cx, |state, cx| state.set_value("", window, cx));
            }
            self.add_model_error = None;
        } else if self.show_api_keys_dialog {
            self.show_api_keys_dialog = false;
            self.api_key_new_name.clear();
            self.api_key_new_value.clear();
        } else if self.show_settings {
            self.show_settings = false;
            self.show_default_model_dropdown = false;
            self.default_model_dropdown_bounds = None;
        }
        cx.notify();
    }

    /// Handle send action
    pub(super) fn on_send(&mut self, _: &Send, window: &mut Window, cx: &mut Context<Self>) {
        self.send_message(window, cx);
    }

    /// Switch to next agent
    pub(super) fn next_agent(
        &mut self,
        _: &NextAgent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.available_agents.is_empty() {
            return;
        }

        let current_idx = self
            .available_agents
            .iter()
            .position(|(name, _)| name == &self.current_agent)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.available_agents.len();
        if let Some((name, _)) = self.available_agents.get(next_idx) {
            let name = name.clone();
            self.set_current_agent(&name);
        }
        cx.notify();
    }

    /// Switch to previous agent
    pub(super) fn prev_agent(
        &mut self,
        _: &PrevAgent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.available_agents.is_empty() {
            return;
        }

        let current_idx = self
            .available_agents
            .iter()
            .position(|(name, _)| name == &self.current_agent)
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            self.available_agents.len() - 1
        } else {
            current_idx - 1
        };
        if let Some((name, _)) = self.available_agents.get(prev_idx) {
            let name = name.clone();
            self.set_current_agent(&name);
        }
        cx.notify();
    }

    pub(super) fn set_current_agent(&mut self, name: &str) {
        if self.current_agent == name {
            self.show_agent_dropdown = false;
            self.show_model_dropdown = false;
            return;
        }

        self.current_agent = name.to_string();
        let _ = self.agents.switch(name);
        self.message_history.clear();
        self.update_context_usage();
        self.show_agent_dropdown = false;
        self.show_model_dropdown = false;
    }
}
