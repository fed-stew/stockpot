//! Chat message types and operations
//!
//! Defines the core message structure for the conversation UI.

use super::sections::{AgentSection, MessageSection, ThinkingSection};

/// Role of a message sender
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// State of a tool call
#[derive(Debug, Clone)]
pub enum ToolCallState {
    Pending,
    Running,
    Success { output: String },
    Error { message: String },
}

/// A tool call within an assistant message
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub state: ToolCallState,
}

/// A single chat message
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    /// Legacy content field - kept for compatibility, represents flattened view
    pub content: String,
    /// Structured sections (for assistant messages with nested agents)
    pub sections: Vec<MessageSection>,
    pub tool_calls: Vec<ToolCall>,
    pub is_streaming: bool,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content: content_str.clone(),
            sections: vec![MessageSection::Text(content_str)],
            tool_calls: vec![],
            is_streaming: false,
        }
    }

    pub fn assistant() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            sections: vec![],
            tool_calls: vec![],
            is_streaming: true,
        }
    }

    /// Append content to the legacy content field (for backward compatibility)
    pub fn append_content(&mut self, text: &str) {
        self.content.push_str(text);
    }

    pub fn finish_streaming(&mut self) {
        self.is_streaming = false;
    }

    /// Append text to the current active section (last Text section, or creates one)
    pub fn append_to_section(&mut self, text: &str) {
        // Also update the legacy content field
        self.content.push_str(text);

        // Find or create a Text section to append to
        if let Some(MessageSection::Text(ref mut existing)) = self.sections.last_mut() {
            existing.push_str(text);
        } else {
            // Last section is either NestedAgent or there are no sections - create new Text
            self.sections.push(MessageSection::Text(text.to_string()));
        }
    }

    /// Start a new nested agent section, returns the section ID
    pub fn start_nested_section(&mut self, agent_name: &str, display_name: &str) -> String {
        let section = AgentSection::new(agent_name, display_name);
        let id = section.id.clone();
        self.sections.push(MessageSection::NestedAgent(section));
        id
    }

    /// Append text to a specific nested agent section by ID
    pub fn append_to_nested_section(&mut self, section_id: &str, text: &str) {
        // Also update legacy content for flattened view
        self.content.push_str(text);

        for section in &mut self.sections {
            if let MessageSection::NestedAgent(ref mut agent) = section {
                if agent.id == section_id {
                    agent.append(text);
                    return;
                }
            }
        }
    }

    /// Mark a nested section as complete
    pub fn finish_nested_section(&mut self, section_id: &str) {
        for section in &mut self.sections {
            if let MessageSection::NestedAgent(ref mut agent) = section {
                if agent.id == section_id {
                    agent.finish();
                    return;
                }
            }
        }
    }

    /// Toggle the collapsed state of a section
    pub fn toggle_section_collapsed(&mut self, section_id: &str) {
        for section in &mut self.sections {
            if let MessageSection::NestedAgent(ref mut agent) = section {
                if agent.id == section_id {
                    agent.toggle_collapsed();
                    return;
                }
            }
        }
    }

    /// Get a reference to a nested agent section by ID
    pub fn get_nested_section(&self, section_id: &str) -> Option<&AgentSection> {
        for section in &self.sections {
            if let MessageSection::NestedAgent(ref agent) = section {
                if agent.id == section_id {
                    return Some(agent);
                }
            }
        }
        None
    }

    /// Get a mutable reference to a nested agent section by ID
    pub fn get_nested_section_mut(&mut self, section_id: &str) -> Option<&mut AgentSection> {
        for section in &mut self.sections {
            if let MessageSection::NestedAgent(ref mut agent) = section {
                if agent.id == section_id {
                    return Some(agent);
                }
            }
        }
        None
    }

    /// Check if the message has any nested agent sections
    pub fn has_nested_sections(&self) -> bool {
        self.sections.iter().any(|s| s.is_nested_agent())
    }

    /// Get the currently active nested section ID (if any, and if not complete)
    pub fn active_nested_section_id(&self) -> Option<&str> {
        for section in self.sections.iter().rev() {
            if let MessageSection::NestedAgent(ref agent) = section {
                if !agent.is_complete {
                    return Some(&agent.id);
                }
            }
        }
        None
    }

    // =========================================================================
    // Thinking section methods
    // =========================================================================

    /// Start a new thinking section, returns the section ID
    pub fn start_thinking_section(&mut self) -> String {
        let section = ThinkingSection::new();
        let id = section.id.clone();
        self.sections.push(MessageSection::Thinking(section));
        id
    }

    /// Append text to a specific thinking section by ID
    pub fn append_to_thinking_section(&mut self, section_id: &str, text: &str) {
        // Also update legacy content for flattened view
        self.content.push_str(text);

        for section in &mut self.sections {
            if let MessageSection::Thinking(ref mut thinking) = section {
                if thinking.id == section_id {
                    thinking.append(text);
                    return;
                }
            }
        }
    }

    /// Mark a thinking section as complete
    pub fn finish_thinking_section(&mut self, section_id: &str) {
        for section in &mut self.sections {
            if let MessageSection::Thinking(ref mut thinking) = section {
                if thinking.id == section_id {
                    thinking.finish();
                    return;
                }
            }
        }
    }

    /// Get a reference to a thinking section by ID
    pub fn get_thinking_section(&self, section_id: &str) -> Option<&ThinkingSection> {
        for section in &self.sections {
            if let MessageSection::Thinking(ref thinking) = section {
                if thinking.id == section_id {
                    return Some(thinking);
                }
            }
        }
        None
    }

    /// Get the currently active (incomplete) thinking section ID if any
    pub fn active_thinking_section_id(&self) -> Option<&str> {
        for section in self.sections.iter().rev() {
            if let MessageSection::Thinking(ref thinking) = section {
                if !thinking.is_complete {
                    return Some(&thinking.id);
                }
            }
        }
        None
    }

    /// Toggle the collapsed state of a thinking section
    pub fn toggle_thinking_collapsed(&mut self, section_id: &str) {
        for section in &mut self.sections {
            if let MessageSection::Thinking(ref mut thinking) = section {
                if thinking.id == section_id {
                    thinking.toggle_collapsed();
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // ChatMessage nested section tests
    // ==========================================================================

    #[test]
    fn test_chat_message_nested_section_lifecycle() {
        let mut msg = ChatMessage::assistant();

        // Start nested section
        let section_id = msg.start_nested_section("sub-agent", "Sub Agent");
        assert!(!section_id.is_empty());
        assert!(msg.has_nested_sections());

        // Verify the section exists and is initially empty/incomplete
        let section = msg
            .get_nested_section(&section_id)
            .expect("Section should exist");
        assert_eq!(section.agent_name, "sub-agent");
        assert_eq!(section.display_name, "Sub Agent");
        assert_eq!(section.content(), "");
        assert!(!section.is_complete);

        // Append content
        msg.append_to_nested_section(&section_id, "Line 1\n");
        msg.append_to_nested_section(&section_id, "Line 2");

        // Verify content before finishing
        let section = msg
            .get_nested_section(&section_id)
            .expect("Section should exist");
        assert_eq!(section.content(), "Line 1\nLine 2");
        assert!(!section.is_complete);

        // Finish section
        msg.finish_nested_section(&section_id);

        // Verify is_complete is true
        let section = msg
            .get_nested_section(&section_id)
            .expect("Section should exist");
        assert!(section.is_complete);
    }

    #[test]
    fn test_chat_message_active_nested_section_tracking() {
        let mut msg = ChatMessage::assistant();

        // Initially no active nested section
        assert!(msg.active_nested_section_id().is_none());

        // Start a nested section - it should be active
        let section_id_1 = msg.start_nested_section("agent-1", "Agent 1");
        assert_eq!(msg.active_nested_section_id(), Some(section_id_1.as_str()));

        // Finish the section - no longer active
        msg.finish_nested_section(&section_id_1);
        assert!(msg.active_nested_section_id().is_none());

        // Start another section
        let section_id_2 = msg.start_nested_section("agent-2", "Agent 2");
        assert_eq!(msg.active_nested_section_id(), Some(section_id_2.as_str()));
    }

    #[test]
    fn test_chat_message_multiple_nested_sections() {
        let mut msg = ChatMessage::assistant();

        // Add text, then nested section, then more text
        msg.append_to_section("Before\n");

        let section_id = msg.start_nested_section("agent", "Agent");
        msg.append_to_nested_section(&section_id, "Nested content");
        msg.finish_nested_section(&section_id);

        msg.append_to_section("After\n");

        // Should have 3 sections: Text, NestedAgent, Text
        assert_eq!(msg.sections.len(), 3);
        assert!(msg.sections[0].is_text());
        assert!(msg.sections[1].is_nested_agent());
        assert!(msg.sections[2].is_text());
    }

    #[test]
    fn test_toggle_section_collapsed() {
        let mut msg = ChatMessage::assistant();
        let section_id = msg.start_nested_section("agent", "Agent");

        // Initially not collapsed
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(!section.is_collapsed);

        // Toggle to collapsed
        msg.toggle_section_collapsed(&section_id);
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(section.is_collapsed);

        // Toggle back to uncollapsed
        msg.toggle_section_collapsed(&section_id);
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(!section.is_collapsed);
    }

    #[test]
    fn test_get_nested_section_mut() {
        let mut msg = ChatMessage::assistant();
        let section_id = msg.start_nested_section("agent", "Agent");

        // Modify via mutable reference
        if let Some(section) = msg.get_nested_section_mut(&section_id) {
            section.append("Modified via append");
            section.is_collapsed = true;
        }

        // Verify changes
        let section = msg.get_nested_section(&section_id).unwrap();
        assert_eq!(section.content(), "Modified via append");
        assert!(section.is_collapsed);
    }

    // ==========================================================================
    // Edge case tests
    // ==========================================================================

    #[test]
    fn test_append_to_nonexistent_section() {
        let mut msg = ChatMessage::assistant();
        // Should not panic, just silently do nothing useful
        msg.append_to_nested_section("nonexistent-id", "text");
        // The text goes to legacy content but not to any nested section
        assert_eq!(msg.content, "text");
    }

    #[test]
    fn test_finish_nonexistent_section() {
        let mut msg = ChatMessage::assistant();
        // Should not panic
        msg.finish_nested_section("nonexistent-id");
    }

    #[test]
    fn test_toggle_nonexistent_section() {
        let mut msg = ChatMessage::assistant();
        // Should not panic
        msg.toggle_section_collapsed("nonexistent-id");
    }

    #[test]
    fn test_finish_already_finished_section() {
        let mut msg = ChatMessage::assistant();
        let section_id = msg.start_nested_section("agent", "Agent");

        // Finish once
        msg.finish_nested_section(&section_id);
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(section.is_complete);

        // Finish again - should not panic, just stay complete
        msg.finish_nested_section(&section_id);
        let section = msg.get_nested_section(&section_id).unwrap();
        assert!(section.is_complete);
    }

    #[test]
    fn test_get_nonexistent_section() {
        let msg = ChatMessage::assistant();
        assert!(msg.get_nested_section("nonexistent").is_none());
    }

    #[test]
    fn test_has_nested_sections_empty() {
        let msg = ChatMessage::assistant();
        assert!(!msg.has_nested_sections());
    }

    #[test]
    fn test_has_nested_sections_with_only_text() {
        let mut msg = ChatMessage::assistant();
        msg.append_to_section("Just text");
        assert!(!msg.has_nested_sections());
    }

    // ==========================================================================
    // Legacy content sync tests
    // ==========================================================================

    #[test]
    fn test_legacy_content_sync_with_nested_sections() {
        let mut msg = ChatMessage::assistant();

        // Add text to section
        msg.append_to_section("Main: ");

        // Start nested section and add content
        let section_id = msg.start_nested_section("agent", "Agent");
        msg.append_to_nested_section(&section_id, "Nested");

        // Legacy content should have both
        assert_eq!(msg.content, "Main: Nested");
    }

    // ==========================================================================
    // Thinking section tests
    // ==========================================================================

    #[test]
    fn test_start_thinking_section() {
        let mut msg = ChatMessage::assistant();

        let section_id = msg.start_thinking_section();
        assert!(!section_id.is_empty());

        // Verify section was added
        assert_eq!(msg.sections.len(), 1);
        assert!(msg.sections[0].is_thinking());

        // Verify we can retrieve it
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert_eq!(section.id, section_id);
        assert_eq!(section.content, "");
        assert!(!section.is_complete);
    }

    #[test]
    fn test_append_to_thinking_section() {
        let mut msg = ChatMessage::assistant();
        let section_id = msg.start_thinking_section();

        msg.append_to_thinking_section(&section_id, "Thinking about ");
        msg.append_to_thinking_section(&section_id, "the problem");

        let section = msg.get_thinking_section(&section_id).unwrap();
        assert_eq!(section.content, "Thinking about the problem");

        // Also verify legacy content was updated
        assert_eq!(msg.content, "Thinking about the problem");
    }

    #[test]
    fn test_finish_thinking_section() {
        let mut msg = ChatMessage::assistant();
        let section_id = msg.start_thinking_section();

        // Initially not complete
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(!section.is_complete);

        // Finish it
        msg.finish_thinking_section(&section_id);

        // Now complete
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(section.is_complete);
    }

    #[test]
    fn test_get_thinking_section_nonexistent() {
        let msg = ChatMessage::assistant();
        assert!(msg.get_thinking_section("nonexistent").is_none());
    }

    #[test]
    fn test_active_thinking_section_id() {
        let mut msg = ChatMessage::assistant();

        // Initially no active thinking section
        assert!(msg.active_thinking_section_id().is_none());

        // Start a thinking section - it should be active
        let section_id = msg.start_thinking_section();
        assert_eq!(msg.active_thinking_section_id(), Some(section_id.as_str()));

        // Finish the section - no longer active
        msg.finish_thinking_section(&section_id);
        assert!(msg.active_thinking_section_id().is_none());
    }

    #[test]
    fn test_active_thinking_section_id_returns_most_recent() {
        let mut msg = ChatMessage::assistant();

        // Start first thinking section and finish it
        let section_id_1 = msg.start_thinking_section();
        msg.finish_thinking_section(&section_id_1);

        // Start second thinking section (incomplete)
        let section_id_2 = msg.start_thinking_section();

        // active_thinking_section_id should return the second one
        assert_eq!(
            msg.active_thinking_section_id(),
            Some(section_id_2.as_str())
        );
    }

    #[test]
    fn test_thinking_section_lifecycle() {
        let mut msg = ChatMessage::assistant();

        // Start thinking section
        let section_id = msg.start_thinking_section();
        assert!(msg.active_thinking_section_id().is_some());

        // Append content
        msg.append_to_thinking_section(&section_id, "Let me think...\n");
        msg.append_to_thinking_section(&section_id, "I believe the answer is 42.");

        // Verify content
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert_eq!(
            section.content,
            "Let me think...\nI believe the answer is 42."
        );
        assert!(!section.is_complete);

        // Finish section
        msg.finish_thinking_section(&section_id);

        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(section.is_complete);
        assert!(msg.active_thinking_section_id().is_none());
    }

    #[test]
    fn test_append_to_nonexistent_thinking_section() {
        let mut msg = ChatMessage::assistant();
        // Should not panic, just update legacy content
        msg.append_to_thinking_section("nonexistent-id", "text");
        assert_eq!(msg.content, "text");
    }

    #[test]
    fn test_toggle_thinking_collapsed() {
        let mut msg = ChatMessage::assistant();
        let section_id = msg.start_thinking_section();

        // Initially collapsed (default for ThinkingSection)
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(section.is_collapsed);

        // Toggle to uncollapsed
        msg.toggle_thinking_collapsed(&section_id);
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(!section.is_collapsed);

        // Toggle back to collapsed
        msg.toggle_thinking_collapsed(&section_id);
        let section = msg.get_thinking_section(&section_id).unwrap();
        assert!(section.is_collapsed);
    }

    #[test]
    fn test_toggle_thinking_collapsed_nonexistent() {
        let mut msg = ChatMessage::assistant();
        // Should not panic
        msg.toggle_thinking_collapsed("nonexistent-id");
    }

    #[test]
    fn test_finish_nonexistent_thinking_section() {
        let mut msg = ChatMessage::assistant();
        // Should not panic
        msg.finish_thinking_section("nonexistent-id");
    }

    #[test]
    fn test_mixed_sections_with_thinking() {
        let mut msg = ChatMessage::assistant();

        // Add thinking section
        let thinking_id = msg.start_thinking_section();
        msg.append_to_thinking_section(&thinking_id, "Hmm...");
        msg.finish_thinking_section(&thinking_id);

        // Add regular text
        msg.append_to_section("Hello!");

        // Add nested agent section
        let agent_id = msg.start_nested_section("agent", "Agent");
        msg.append_to_nested_section(&agent_id, "Agent output");
        msg.finish_nested_section(&agent_id);

        // Should have 3 sections: Thinking, Text, NestedAgent
        assert_eq!(msg.sections.len(), 3);
        assert!(msg.sections[0].is_thinking());
        assert!(msg.sections[1].is_text());
        assert!(msg.sections[2].is_nested_agent());
    }
}
