//! Message section types for structured assistant messages
//!
//! Provides section abstractions for collapsible nested agent output.

/// A collapsible section containing output from a nested agent
#[derive(Debug, Clone)]
pub struct AgentSection {
    /// Unique ID for this section
    pub id: String,
    /// Agent's internal name
    pub agent_name: String,
    /// Agent's display name (shown in header)
    pub display_name: String,
    /// Content accumulated from this agent
    pub content: String,
    /// Whether the section is collapsed in UI
    pub is_collapsed: bool,
    /// Whether the agent has completed
    pub is_complete: bool,
}

impl AgentSection {
    pub fn new(agent_name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            agent_name: agent_name.into(),
            display_name: display_name.into(),
            content: String::new(),
            is_collapsed: false,
            is_complete: false,
        }
    }

    pub fn append(&mut self, text: &str) {
        self.content.push_str(text);
    }

    pub fn finish(&mut self) {
        self.is_complete = true;
    }

    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }
}

/// A section within an assistant message
#[derive(Debug, Clone)]
pub enum MessageSection {
    /// Plain text/markdown content
    Text(String),
    /// Nested agent output (collapsible)
    NestedAgent(AgentSection),
}

impl MessageSection {
    /// Returns true if this is a Text section
    pub fn is_text(&self) -> bool {
        matches!(self, MessageSection::Text(_))
    }

    /// Returns true if this is a NestedAgent section
    pub fn is_nested_agent(&self) -> bool {
        matches!(self, MessageSection::NestedAgent(_))
    }

    /// Get the section ID if it's a nested agent section
    pub fn agent_section_id(&self) -> Option<&str> {
        match self {
            MessageSection::NestedAgent(section) => Some(&section.id),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // AgentSection tests
    // ==========================================================================

    #[test]
    fn test_agent_section_new() {
        let section = AgentSection::new("test-agent", "Test Agent");
        assert_eq!(section.agent_name, "test-agent");
        assert_eq!(section.display_name, "Test Agent");
        assert_eq!(section.content, "");
        assert!(!section.is_collapsed);
        assert!(!section.is_complete);
        assert!(!section.id.is_empty(), "ID should be generated");
    }

    #[test]
    fn test_agent_section_append() {
        let mut section = AgentSection::new("agent", "Agent");
        section.append("Hello ");
        section.append("World");
        assert_eq!(section.content, "Hello World");
    }

    #[test]
    fn test_agent_section_finish() {
        let mut section = AgentSection::new("agent", "Agent");
        assert!(!section.is_complete);
        section.finish();
        assert!(section.is_complete);
    }

    #[test]
    fn test_agent_section_toggle_collapsed() {
        let mut section = AgentSection::new("agent", "Agent");
        assert!(!section.is_collapsed, "Should start uncollapsed");

        section.toggle_collapsed();
        assert!(
            section.is_collapsed,
            "Should be collapsed after first toggle"
        );

        section.toggle_collapsed();
        assert!(
            !section.is_collapsed,
            "Should be uncollapsed after second toggle"
        );
    }

    // ==========================================================================
    // MessageSection tests
    // ==========================================================================

    #[test]
    fn test_message_section_is_text() {
        let text_section = MessageSection::Text("hello".to_string());
        let agent_section = MessageSection::NestedAgent(AgentSection::new("a", "A"));

        assert!(text_section.is_text());
        assert!(!text_section.is_nested_agent());
        assert!(!agent_section.is_text());
        assert!(agent_section.is_nested_agent());
    }

    #[test]
    fn test_message_section_agent_section_id() {
        let text_section = MessageSection::Text("hello".to_string());
        let agent = AgentSection::new("a", "A");
        let expected_id = agent.id.clone();
        let agent_section = MessageSection::NestedAgent(agent);

        assert!(text_section.agent_section_id().is_none());
        assert_eq!(agent_section.agent_section_id(), Some(expected_id.as_str()));
    }
}
