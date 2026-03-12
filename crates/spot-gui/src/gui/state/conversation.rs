//! Conversation state management
//!
//! Manages chat messages and tool calls for the GUI.

use super::message::{ChatMessage, ToolCall, ToolCallState};
use super::sections::{MessageSection, ToolCallSection};
use super::tool_display::get_tool_display_info;

/// A conversation (list of messages)
#[derive(Debug, Clone, Default)]
pub struct Conversation {
    pub messages: Vec<ChatMessage>,
    pub is_generating: bool,
}

impl Conversation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_user_message(&mut self, content: impl Into<String>) {
        self.messages.push(ChatMessage::user(content));
    }

    pub fn start_assistant_message(&mut self) {
        self.messages.push(ChatMessage::assistant());
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
    pub fn append_tool_call(
        &mut self,
        name: &str,
        args: Option<serde_json::Value>,
    ) -> Option<String> {
        let args = args.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
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
        args: Option<serde_json::Value>,
    ) -> Option<String> {
        let args = args.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
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

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Conversation nested agent tests
    // ==========================================================================

    #[test]
    fn test_conversation_nested_agent() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start nested agent
        let section_id = conv.start_nested_agent("helper", "Helper Agent").unwrap();

        // Append to nested agent
        conv.append_to_nested_agent(&section_id, "Nested content");

        // Verify content is in the message
        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        assert_eq!(section.content(), "Nested content");
        assert!(!section.is_complete);

        // Finish nested agent
        conv.finish_nested_agent(&section_id);

        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(section.is_complete);
    }

    #[test]
    fn test_conversation_start_nested_agent_no_message() {
        let mut conv = Conversation::new();

        // No messages exist, should return None
        let result = conv.start_nested_agent("agent", "Agent");
        assert!(result.is_none());
    }

    #[test]
    fn test_conversation_append_to_current_with_active_nested() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Add some main content first
        conv.append_to_main_content("Main text\n");

        // Start nested section
        let section_id = conv.start_nested_agent("agent", "Agent").unwrap();

        // append_to_current should route to the active nested section
        conv.append_to_current("Goes to nested");

        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        assert_eq!(section.content(), "Goes to nested");
    }

    #[test]
    fn test_conversation_append_to_current_without_nested() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // No nested section, should go to main content
        conv.append_to_current("Main content");

        let msg = conv.messages.last().unwrap();
        assert_eq!(msg.content, "Main content");
    }

    #[test]
    fn test_conversation_toggle_section_collapsed() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        let section_id = conv.start_nested_agent("agent", "Agent").unwrap();

        // Toggle via conversation
        conv.toggle_section_collapsed(&section_id);

        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(section.is_collapsed);
    }

    #[test]
    fn test_conversation_active_nested_section_id() {
        let mut conv = Conversation::new();

        // No messages, should be None
        assert!(conv.active_nested_section_id().is_none());

        conv.start_assistant_message();

        // No nested sections, should be None
        assert!(conv.active_nested_section_id().is_none());

        // Start nested section
        let section_id = conv.start_nested_agent("agent", "Agent").unwrap();
        assert_eq!(conv.active_nested_section_id(), Some(section_id.as_str()));

        // Finish it
        conv.finish_nested_agent(&section_id);
        assert!(conv.active_nested_section_id().is_none());
    }

    // ==========================================================================
    // Tool call section tests
    // ==========================================================================

    #[test]
    fn test_append_tool_call_creates_section() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Append tool call
        let args = serde_json::json!({"file_path": "src/main.rs"});
        let section_id = conv.append_tool_call("read_file", Some(args));

        assert!(section_id.is_some(), "Should return section ID");

        // Verify a ToolCall section was created
        let msg = conv.messages.last().unwrap();
        let tool_section = msg.sections.iter().find(|s| s.is_tool_call());
        assert!(tool_section.is_some(), "Should have a ToolCall section");

        if let MessageSection::ToolCall(tool) = tool_section.unwrap() {
            assert_eq!(tool.id, section_id.unwrap());
            assert_eq!(tool.info.verb, "Read");
            assert_eq!(tool.info.subject, "src/main.rs");
            assert!(tool.is_running);
            assert!(tool.succeeded.is_none());
        } else {
            panic!("Expected ToolCall section");
        }
    }

    #[test]
    fn test_append_tool_call_no_message() {
        let mut conv = Conversation::new();

        // No messages exist, should return None
        let result = conv.append_tool_call("read_file", None);
        assert!(result.is_none());
    }

    #[test]
    fn test_complete_tool_call() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Append tool call
        let args = serde_json::json!({"file_path": "test.rs"});
        conv.append_tool_call("read_file", Some(args));

        // Complete with success
        conv.complete_tool_call("read_file", true);

        // Verify the section is marked complete
        let msg = conv.messages.last().unwrap();
        if let Some(MessageSection::ToolCall(tool)) = msg.sections.iter().find(|s| s.is_tool_call())
        {
            assert!(!tool.is_running);
            assert_eq!(tool.succeeded, Some(true));
        } else {
            panic!("Expected ToolCall section");
        }
    }

    #[test]
    fn test_complete_tool_call_failure() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        conv.append_tool_call("edit_file", None);
        conv.complete_tool_call("edit_file", false);

        let msg = conv.messages.last().unwrap();
        if let Some(MessageSection::ToolCall(tool)) = msg.sections.iter().find(|s| s.is_tool_call())
        {
            assert!(!tool.is_running);
            assert_eq!(tool.succeeded, Some(false));
        } else {
            panic!("Expected ToolCall section");
        }
    }

    #[test]
    fn test_complete_tool_call_finds_most_recent_running() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Add two tool calls
        conv.append_tool_call("read_file", None);
        conv.complete_tool_call("read_file", true); // Complete first one
        conv.append_tool_call("edit_file", None);

        // Now complete_tool_call should find the second one
        conv.complete_tool_call("edit_file", true);

        let msg = conv.messages.last().unwrap();
        let tool_calls: Vec<_> = msg
            .sections
            .iter()
            .filter_map(|s| {
                if let MessageSection::ToolCall(tool) = s {
                    Some(tool)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(tool_calls.len(), 2);
        assert!(!tool_calls[0].is_running);
        assert!(!tool_calls[1].is_running);
    }

    // ==========================================================================
    // Section-specific tool call tests (nested sections)
    // ==========================================================================

    #[test]
    fn test_append_tool_call_to_section() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start a nested section
        let section_id = conv.start_nested_agent("sub-agent", "Sub Agent").unwrap();

        // Append tool call to that section
        let args = serde_json::json!({"file_path": "test.rs"});
        let tool_id = conv.append_tool_call_to_section(&section_id, "read_file", Some(args));
        assert!(tool_id.is_some(), "Should return tool ID");

        // Verify content() includes the tool call formatted as markdown
        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        let content = section.content();
        assert!(
            content.contains("**Read**") && content.contains("test.rs"),
            "Tool call should appear in nested section content: {}",
            content
        );
    }

    #[test]
    fn test_complete_tool_call_in_section() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start a nested section and add a tool call
        let section_id = conv.start_nested_agent("sub-agent", "Sub Agent").unwrap();
        let tool_id = conv
            .append_tool_call_to_section(&section_id, "read_file", None)
            .unwrap();

        // Complete the tool call with success
        conv.complete_tool_call_in_section(&section_id, &tool_id, true);

        // Verify the checkmark was added
        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(
            section.content().contains("✓"),
            "Success indicator should appear in nested section"
        );
    }

    #[test]
    fn test_complete_tool_call_in_section_failure() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start a nested section and add a tool call
        let section_id = conv.start_nested_agent("sub-agent", "Sub Agent").unwrap();
        let tool_id = conv
            .append_tool_call_to_section(&section_id, "read_file", None)
            .unwrap();

        // Complete the tool call with failure
        conv.complete_tool_call_in_section(&section_id, &tool_id, false);

        // Verify the X mark was added
        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(
            section.content().contains("✗"),
            "Failure indicator should appear in nested section"
        );
    }

    // ==========================================================================
    // Thinking section tests
    // ==========================================================================

    #[test]
    fn test_start_thinking() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        let section_id = conv.start_thinking().unwrap();
        assert!(!section_id.is_empty());

        // Verify section exists
        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert_eq!(section.content, "");
        assert!(!section.is_complete);
    }

    #[test]
    fn test_start_thinking_no_message() {
        let mut conv = Conversation::new();

        // No messages exist, should return None
        let result = conv.start_thinking();
        assert!(result.is_none());
    }

    #[test]
    fn test_append_to_thinking() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        let section_id = conv.start_thinking().unwrap();
        conv.append_to_thinking(&section_id, "Thinking...");
        conv.append_to_thinking(&section_id, " more thoughts");

        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert_eq!(section.content, "Thinking... more thoughts");
    }

    #[test]
    fn test_finish_thinking() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        let section_id = conv.start_thinking().unwrap();

        // Initially not complete
        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(!section.is_complete);

        // Finish it
        conv.finish_thinking(&section_id);

        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(section.is_complete);
    }

    #[test]
    fn test_active_thinking_section_id() {
        let mut conv = Conversation::new();

        // No messages, should be None
        assert!(conv.active_thinking_section_id().is_none());

        conv.start_assistant_message();

        // No thinking sections, should be None
        assert!(conv.active_thinking_section_id().is_none());

        // Start thinking section
        let section_id = conv.start_thinking().unwrap();
        assert_eq!(conv.active_thinking_section_id(), Some(section_id.as_str()));

        // Finish it
        conv.finish_thinking(&section_id);
        assert!(conv.active_thinking_section_id().is_none());
    }

    #[test]
    fn test_append_thinking_creates_new_section() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // No active thinking section, should create one
        let section_id = conv.append_thinking("New thought").unwrap();

        // Verify section was created with content
        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert_eq!(section.content, "New thought");
    }

    #[test]
    fn test_append_thinking_uses_existing_section() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start a thinking section explicitly
        let section_id_1 = conv.start_thinking().unwrap();
        conv.append_to_thinking(&section_id_1, "First thought");

        // append_thinking should use the existing section
        let section_id_2 = conv.append_thinking(" - more thoughts").unwrap();

        // Should be the same section
        assert_eq!(section_id_1, section_id_2);

        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id_1).unwrap();
        assert_eq!(section.content, "First thought - more thoughts");
    }

    #[test]
    fn test_append_thinking_creates_new_after_finish() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start and finish a thinking section
        let section_id_1 = conv.start_thinking().unwrap();
        conv.append_to_thinking(&section_id_1, "First thought");
        conv.finish_thinking(&section_id_1);

        // append_thinking should create a new section since the previous is complete
        let section_id_2 = conv.append_thinking("New thought").unwrap();

        // Should be different sections
        assert_ne!(section_id_1, section_id_2);

        let msg = conv.messages.last().unwrap();
        let section_1 = msg.get_thinking_section(&section_id_1).unwrap();
        let section_2 = msg.get_thinking_section(&section_id_2).unwrap();
        assert_eq!(section_1.content, "First thought");
        assert!(section_1.is_complete);
        assert_eq!(section_2.content, "New thought");
        assert!(!section_2.is_complete);
    }

    #[test]
    fn test_append_thinking_no_message() {
        let mut conv = Conversation::new();

        // No messages exist, should return None
        let result = conv.append_thinking("thoughts");
        assert!(result.is_none());
    }

    #[test]
    fn test_thinking_lifecycle() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start thinking
        let section_id = conv.start_thinking().unwrap();
        assert!(conv.active_thinking_section_id().is_some());

        // Append content
        conv.append_to_thinking(&section_id, "Let me analyze...\n");
        conv.append_to_thinking(&section_id, "The answer is 42.");

        // Verify content
        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert_eq!(section.content, "Let me analyze...\nThe answer is 42.");

        // Finish thinking
        conv.finish_thinking(&section_id);
        assert!(conv.active_thinking_section_id().is_none());

        // Verify section is complete
        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(section.is_complete);
    }

    #[test]
    fn test_mixed_thinking_and_nested_agents() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start thinking section
        let thinking_id = conv.start_thinking().unwrap();
        conv.append_to_thinking(&thinking_id, "Hmm, let me think...");
        conv.finish_thinking(&thinking_id);

        // Add some regular content
        conv.append_to_main_content("Here's my response:\n");

        // Start nested agent
        let agent_id = conv.start_nested_agent("helper", "Helper").unwrap();
        conv.append_to_nested_agent(&agent_id, "Agent output");
        conv.finish_nested_agent(&agent_id);

        // Verify structure
        let msg = conv.messages.last().unwrap();
        assert_eq!(msg.sections.len(), 3);
        assert!(msg.sections[0].is_thinking());
        assert!(msg.sections[1].is_text());
        assert!(msg.sections[2].is_nested_agent());
    }

    #[test]
    fn test_toggle_thinking_collapsed() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        let section_id = conv.start_thinking().unwrap();

        // Initially collapsed (default for ThinkingSection)
        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(section.is_collapsed);

        // Toggle via conversation
        conv.toggle_thinking_collapsed(&section_id);

        let msg = conv.messages.last().unwrap();
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(!section.is_collapsed);
    }

    #[test]
    fn test_toggle_thinking_collapsed_searches_all_messages() {
        let mut conv = Conversation::new();

        // First message with thinking section
        conv.start_assistant_message();
        let section_id_1 = conv.start_thinking().unwrap();
        conv.append_to_thinking(&section_id_1, "First thoughts");
        conv.finish_thinking(&section_id_1);
        conv.finish_current_message();

        // Second message with another thinking section
        conv.start_assistant_message();
        let section_id_2 = conv.start_thinking().unwrap();
        conv.append_to_thinking(&section_id_2, "Second thoughts");
        conv.finish_current_message();

        // Toggle the first message's thinking section
        conv.toggle_thinking_collapsed(&section_id_1);

        // Verify first section toggled (was collapsed, now uncollapsed)
        let section = conv.messages[0]
            .get_thinking_section(&section_id_1)
            .unwrap();
        assert!(!section.is_collapsed);

        // Verify second section unchanged (still collapsed)
        let section = conv.messages[1]
            .get_thinking_section(&section_id_2)
            .unwrap();
        assert!(section.is_collapsed);
    }

    #[test]
    fn test_toggle_thinking_collapsed_nonexistent() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Should not panic
        conv.toggle_thinking_collapsed("nonexistent-id");
    }

    // ==========================================================================
    // Nested agent thinking tests
    // ==========================================================================

    #[test]
    fn test_append_thinking_in_section_creates_new() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Start a nested agent section
        let section_id = conv.start_nested_agent("sub-agent", "Sub Agent").unwrap();

        // Append thinking - should create a new thinking section
        conv.append_thinking_in_section(&section_id, "Analyzing...");

        // Verify thinking was added to the nested section
        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(section.has_active_thinking());
        // Content should include the thinking indicator
        assert!(section.content().contains("Thinking"));
    }

    #[test]
    fn test_append_thinking_in_section_appends_to_existing() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        let section_id = conv.start_nested_agent("sub-agent", "Sub Agent").unwrap();

        // First append creates thinking section
        conv.append_thinking_in_section(&section_id, "First thought. ");
        // Second append should add to existing
        conv.append_thinking_in_section(&section_id, "Second thought.");

        // Verify only one thinking section exists with combined content
        let msg = conv.messages.last().unwrap();
        let section = msg.get_nested_section(&section_id).unwrap();

        // Count thinking items
        let thinking_count = section
            .items
            .iter()
            .filter(|item| matches!(item, crate::gui::state::AgentContentItem::Thinking { .. }))
            .count();
        assert_eq!(thinking_count, 1, "Should only have one thinking section");
    }

    #[test]
    fn test_append_thinking_in_section_nonexistent() {
        let mut conv = Conversation::new();
        conv.start_assistant_message();

        // Should not panic when section doesn't exist
        conv.append_thinking_in_section("nonexistent", "Some thoughts");
    }
}
