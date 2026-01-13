//! TUI Conversation state management
//!
//! Manages chat messages and tool calls for the TUI.

use super::message::{
    get_tool_display_info, MessageSection, ToolCall, ToolCallSection, ToolCallState, TuiMessage,
};
use serde_json::Value;

/// A conversation (list of messages)
#[derive(Debug, Clone, Default)]
pub struct TuiConversation {
    pub messages: Vec<TuiMessage>,
    pub is_generating: bool,
}

impl TuiConversation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(TuiMessage::user(content));
    }

    pub fn start_assistant_message(&mut self) {
        self.messages.push(TuiMessage::assistant());
        self.is_generating = true;
    }

    /// Append text to the current message, respecting section structure.
    /// If there's an active (incomplete) nested section, appends there.
    /// Otherwise appends to the main text section.
    pub fn append_to_current(&mut self, text: &str) {
        if let Some(msg) = self.messages.last_mut() {
            // Check if there's an active nested section
            if let Some(section_id) = msg.active_nested_section_id().map(String::from) {
                msg.append_to_nested_section(&section_id, text);
            } else {
                msg.append_to_section(text);
            }
        }
    }

    /// Append text directly to the main content (bypassing nested sections)
    pub fn append_to_main_content(&mut self, text: &str) {
        if let Some(msg) = self.messages.last_mut() {
            msg.append_to_section(text);
        }
    }

    pub fn finish_current_message(&mut self) {
        if let Some(msg) = self.messages.last_mut() {
            msg.finish_streaming();
        }
        self.is_generating = false;
    }

    /// Start a nested agent section in the current message.
    /// Returns the section ID if successful.
    pub fn start_nested_agent(&mut self, agent_name: &str, display_name: &str) -> Option<String> {
        self.messages
            .last_mut()
            .map(|msg| msg.start_nested_section(agent_name, display_name))
    }

    /// Append text to a specific nested agent section
    pub fn append_to_nested_agent(&mut self, section_id: &str, text: &str) {
        if let Some(msg) = self.messages.last_mut() {
            msg.append_to_nested_section(section_id, text);
        }
    }

    /// Mark a nested agent section as complete
    pub fn finish_nested_agent(&mut self, section_id: &str) {
        if let Some(msg) = self.messages.last_mut() {
            msg.finish_nested_section(section_id);
        }
    }

    /// Toggle the collapsed state of a nested section
    pub fn toggle_section_collapsed(&mut self, section_id: &str) {
        // Search through all messages for the section
        for msg in &mut self.messages {
            if msg.get_nested_section(section_id).is_some() {
                msg.toggle_section_collapsed(section_id);
                return;
            }
        }
    }

    /// Set the collapsed state of a nested section explicitly
    pub fn set_section_collapsed(&mut self, section_id: &str, collapsed: bool) {
        // Search through all messages for the section
        for msg in &mut self.messages {
            if let Some(section) = msg.get_nested_section_mut(section_id) {
                section.is_collapsed = collapsed;
                return;
            }
        }
    }

    /// Get the currently active nested section ID (if any)
    pub fn active_nested_section_id(&self) -> Option<&str> {
        self.messages
            .last()
            .and_then(|msg| msg.active_nested_section_id())
    }

    // =========================================================================
    // Thinking section methods
    // =========================================================================

    /// Start a new thinking section in the current message.
    /// Returns the section ID if successful.
    pub fn start_thinking(&mut self) -> Option<String> {
        self.messages
            .last_mut()
            .map(|msg| msg.start_thinking_section())
    }

    /// Append text to a specific thinking section
    pub fn append_to_thinking(&mut self, section_id: &str, text: &str) {
        if let Some(msg) = self.messages.last_mut() {
            msg.append_to_thinking_section(section_id, text);
        }
    }

    /// Mark a thinking section as complete
    pub fn finish_thinking(&mut self, section_id: &str) {
        if let Some(msg) = self.messages.last_mut() {
            msg.finish_thinking_section(section_id);
        }
    }

    /// Get the currently active (incomplete) thinking section ID if any
    pub fn active_thinking_section_id(&self) -> Option<&str> {
        self.messages
            .last()
            .and_then(|msg| msg.active_thinking_section_id())
    }

    /// Toggle the collapsed state of a thinking section
    pub fn toggle_thinking_collapsed(&mut self, section_id: &str) {
        // Search through all messages for the section
        for msg in &mut self.messages {
            if msg.get_thinking_section(section_id).is_some() {
                msg.toggle_thinking_collapsed(section_id);
                return;
            }
        }
    }

    /// Append to an existing active thinking section, or create a new one if none exists.
    /// Returns the section ID.
    pub fn append_thinking(&mut self, text: &str) -> Option<String> {
        // First check if there's an active thinking section
        let active_id = self
            .messages
            .last()
            .and_then(|msg| msg.active_thinking_section_id())
            .map(String::from);

        if let Some(section_id) = active_id {
            // Append to existing active thinking section
            self.append_to_thinking(&section_id, text);
            Some(section_id)
        } else {
            // Start a new thinking section and append
            if let Some(section_id) = self.start_thinking() {
                self.append_to_thinking(&section_id, text);
                Some(section_id)
            } else {
                None
            }
        }
    }

    pub fn add_tool_call(&mut self, id: String, name: String, arguments: String) {
        if let Some(msg) = self.messages.last_mut() {
            msg.tool_calls.push(ToolCall {
                id,
                name,
                arguments,
                state: ToolCallState::Pending,
            });
        }
    }

    pub fn update_tool_call(&mut self, id: &str, state: ToolCallState) {
        if let Some(msg) = self.messages.last_mut() {
            if let Some(tool) = msg.tool_calls.iter_mut().find(|t| t.id == id) {
                tool.state = state;
            }
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
        self.is_generating = false;
    }

    /// Append a tool call section to the current message
    /// Returns the section ID for later completion tracking
    pub fn append_tool_call(&mut self, name: &str, args: Option<Value>) -> Option<String> {
        let args = args.unwrap_or(Value::Object(serde_json::Map::new()));
        let info = get_tool_display_info(name, &args);

        if let Some(msg) = self.messages.last_mut() {
            let section = ToolCallSection::new(info);
            let id = section.id.clone();
            msg.sections.push(MessageSection::ToolCall(section));
            Some(id)
        } else {
            None
        }
    }

    /// Append a tool call to a specific nested section (structured for consistent styling)
    /// Returns the tool ID for later completion
    pub fn append_tool_call_to_section(
        &mut self,
        section_id: &str,
        name: &str,
        args: Option<Value>,
    ) -> Option<String> {
        let args = args.unwrap_or(Value::Object(serde_json::Map::new()));
        let info = get_tool_display_info(name, &args);
        if let Some(msg) = self.messages.last_mut() {
            if let Some(section) = msg.get_nested_section_mut(section_id) {
                return Some(section.append_tool_call(info));
            }
        }
        None
    }

    /// Mark the most recent tool call as completed
    pub fn complete_tool_call(&mut self, _name: &str, success: bool) {
        if let Some(msg) = self.messages.last_mut() {
            // Find the last ToolCall section that's still running
            for section in msg.sections.iter_mut().rev() {
                if let MessageSection::ToolCall(ref mut tool) = section {
                    if tool.is_running {
                        tool.complete(success);
                        return;
                    }
                }
            }
        }
    }

    /// Complete a tool call in a specific nested section
    pub fn complete_tool_call_in_section(
        &mut self,
        section_id: &str,
        tool_id: &str,
        success: bool,
    ) {
        if let Some(msg) = self.messages.last_mut() {
            if let Some(section) = msg.get_nested_section_mut(section_id) {
                section.complete_tool_call(tool_id, success);
            }
        }
    }

    /// Start a thinking section in a specific nested agent section
    /// Returns the thinking section ID
    pub fn start_thinking_in_section(&mut self, section_id: &str) -> Option<String> {
        if let Some(msg) = self.messages.last_mut() {
            if let Some(section) = msg.get_nested_section_mut(section_id) {
                return Some(section.start_thinking());
            }
        }
        None
    }

    /// Append to a thinking section in a specific nested agent section
    pub fn append_to_thinking_in_section(&mut self, section_id: &str, text: &str) {
        if let Some(msg) = self.messages.last_mut() {
            if let Some(section) = msg.get_nested_section_mut(section_id) {
                section.append_to_thinking(text);
            }
        }
    }

    /// Append to an existing active thinking section in a nested agent, or create a new one.
    /// This mirrors the behavior of `append_thinking` but for nested agent sections.
    pub fn append_thinking_in_section(&mut self, section_id: &str, text: &str) {
        if let Some(msg) = self.messages.last_mut() {
            if let Some(section) = msg.get_nested_section_mut(section_id) {
                // Check if there's an active thinking section
                if !section.has_active_thinking() {
                    // Start a new one
                    section.start_thinking();
                }
                // Append to the active thinking section
                section.append_to_thinking(text);
            }
        }
    }

    /// Complete a thinking section in a specific nested agent section
    pub fn complete_thinking_in_section(&mut self, section_id: &str) {
        if let Some(msg) = self.messages.last_mut() {
            if let Some(section) = msg.get_nested_section_mut(section_id) {
                section.complete_thinking();
            }
        }
    }
}
