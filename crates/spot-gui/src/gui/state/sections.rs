//! Message section types for structured assistant messages
//!
//! Provides section abstractions for collapsible nested agent output.

use super::tool_display::ToolDisplayInfo;

/// Content item within a nested agent section
#[derive(Debug, Clone)]
pub enum AgentContentItem {
    /// Plain text/markdown content
    Text(String),
    /// A tool call with display info
    ToolCall {
        id: String,
        info: ToolDisplayInfo,
        is_running: bool,
        succeeded: Option<bool>,
    },
    /// A thinking/reasoning section
    Thinking {
        id: String,
        content: String,
        is_complete: bool,
    },
}

impl AgentContentItem {
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    pub fn tool_call(info: ToolDisplayInfo) -> Self {
        Self::ToolCall {
            id: uuid::Uuid::new_v4().to_string(),
            info,
            is_running: true,
            succeeded: None,
        }
    }

    pub fn thinking() -> Self {
        Self::Thinking {
            id: uuid::Uuid::new_v4().to_string(),
            content: String::new(),
            is_complete: false,
        }
    }
}

/// A collapsible section containing output from a nested agent
#[derive(Debug, Clone)]
pub struct AgentSection {
    /// Unique ID for this section
    pub id: String,
    /// Agent's internal name
    pub agent_name: String,
    /// Agent's display name (shown in header)
    pub display_name: String,
    /// Content items (text and tool calls)
    pub items: Vec<AgentContentItem>,
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
            items: Vec::new(),
            is_collapsed: false,
            is_complete: false,
        }
    }

    /// Append text content - merges with last text item if possible
    pub fn append(&mut self, text: &str) {
        if let Some(AgentContentItem::Text(existing)) = self.items.last_mut() {
            existing.push_str(text);
        } else {
            self.items.push(AgentContentItem::text(text));
        }
    }

    /// Append a tool call
    pub fn append_tool_call(&mut self, info: ToolDisplayInfo) -> String {
        let item = AgentContentItem::tool_call(info);
        let id = match &item {
            AgentContentItem::ToolCall { id, .. } => id.clone(),
            _ => unreachable!(),
        };
        self.items.push(item);
        id
    }

    /// Complete a tool call by ID
    pub fn complete_tool_call(&mut self, tool_id: &str, success: bool) {
        for item in &mut self.items {
            if let AgentContentItem::ToolCall {
                id,
                is_running,
                succeeded,
                ..
            } = item
            {
                if id == tool_id && *is_running {
                    *is_running = false;
                    *succeeded = Some(success);
                    return;
                }
            }
        }
    }

    /// Start a new thinking section, returns the section ID
    pub fn start_thinking(&mut self) -> String {
        let item = AgentContentItem::thinking();
        let id = match &item {
            AgentContentItem::Thinking { id, .. } => id.clone(),
            _ => unreachable!(),
        };
        self.items.push(item);
        id
    }

    /// Append to the most recent thinking section
    pub fn append_to_thinking(&mut self, text: &str) {
        // Find the last Thinking item and append
        for item in self.items.iter_mut().rev() {
            if let AgentContentItem::Thinking {
                content,
                is_complete,
                ..
            } = item
            {
                if !*is_complete {
                    content.push_str(text);
                    return;
                }
            }
        }
    }

    /// Complete the most recent thinking section
    pub fn complete_thinking(&mut self) {
        for item in self.items.iter_mut().rev() {
            if let AgentContentItem::Thinking { is_complete, .. } = item {
                if !*is_complete {
                    *is_complete = true;
                    return;
                }
            }
        }
    }

    /// Check if there's an active (incomplete) thinking section
    pub fn has_active_thinking(&self) -> bool {
        self.items.iter().rev().any(|item| {
            matches!(
                item,
                AgentContentItem::Thinking {
                    is_complete: false,
                    ..
                }
            )
        })
    }

    /// Get combined content as string (for backwards compatibility / plain text export)
    pub fn content(&self) -> String {
        self.items
            .iter()
            .map(|item| match item {
                AgentContentItem::Text(s) => s.clone(),
                AgentContentItem::ToolCall {
                    info, succeeded, ..
                } => {
                    let status = match succeeded {
                        Some(true) => " ✓",
                        Some(false) => " ✗",
                        None => "",
                    };
                    if info.subject.is_empty() {
                        format!("• **{}**{}\n", info.verb, status)
                    } else {
                        format!("• **{}** {}{}\n", info.verb, info.subject, status)
                    }
                }
                AgentContentItem::Thinking { content, .. } => {
                    if content.is_empty() {
                        String::new()
                    } else {
                        // Get first line preview
                        let preview: String = content
                            .lines()
                            .next()
                            .unwrap_or("")
                            .chars()
                            .take(50)
                            .collect();
                        format!("• **Thinking** {}...\n", preview)
                    }
                }
            })
            .collect()
    }

    pub fn finish(&mut self) {
        self.is_complete = true;
    }

    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }
}

/// A collapsible section containing model thinking/reasoning content
#[derive(Debug, Clone)]
pub struct ThinkingSection {
    /// Unique ID for this section
    pub id: String,
    /// Full thinking content (accumulated)
    pub content: String,
    /// Whether thinking is finished
    pub is_complete: bool,
    /// Whether the section is collapsed in UI
    pub is_collapsed: bool,
}

impl ThinkingSection {
    /// Creates a new empty ThinkingSection with a UUID
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content: String::new(),
            is_complete: false,
            is_collapsed: true,
        }
    }

    /// Appends text to the thinking content
    pub fn append(&mut self, text: &str) {
        self.content.push_str(text);
    }

    /// Marks thinking as complete
    pub fn finish(&mut self) {
        self.is_complete = true;
    }

    /// Toggles the collapsed state
    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }

    /// Returns first line or first 50 chars (whichever is shorter), with "..." if truncated
    pub fn preview(&self) -> String {
        if self.content.is_empty() {
            return String::new();
        }

        // Get first line
        let first_line = self.content.lines().next().unwrap_or("");

        // Take at most 50 chars
        let truncated: String = first_line.chars().take(50).collect();

        // Add "..." if we truncated (either by line break or char limit)
        let needs_ellipsis = truncated.len() < first_line.len() || self.content.contains('\n');

        if needs_ellipsis {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }
}

impl Default for ThinkingSection {
    fn default() -> Self {
        Self::new()
    }
}

/// A tool call display section
#[derive(Debug, Clone)]
pub struct ToolCallSection {
    /// Unique ID for this section
    pub id: String,
    /// The tool display info (verb + subject)
    pub info: ToolDisplayInfo,
    /// Whether the tool call is still running
    pub is_running: bool,
    /// Whether the tool call succeeded (None if still running)
    pub succeeded: Option<bool>,
}

impl ToolCallSection {
    pub fn new(info: ToolDisplayInfo) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            info,
            is_running: true,
            succeeded: None,
        }
    }

    pub fn complete(&mut self, success: bool) {
        self.is_running = false;
        self.succeeded = Some(success);
    }
}

/// A section within an assistant message
#[derive(Debug, Clone)]
pub enum MessageSection {
    /// Plain text/markdown content
    Text(String),
    /// Nested agent output (collapsible)
    NestedAgent(AgentSection),
    /// Model thinking/reasoning (collapsible)
    Thinking(ThinkingSection),
    /// Tool call display (styled)
    ToolCall(ToolCallSection),
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

    /// Returns true if this is a Thinking section
    pub fn is_thinking(&self) -> bool {
        matches!(self, MessageSection::Thinking(_))
    }

    /// Get the section ID if it's a thinking section
    pub fn thinking_section_id(&self) -> Option<&str> {
        match self {
            MessageSection::Thinking(section) => Some(&section.id),
            _ => None,
        }
    }

    /// Returns true if this is a ToolCall section
    pub fn is_tool_call(&self) -> bool {
        matches!(self, MessageSection::ToolCall(_))
    }

    /// Get the section ID if it's a tool call section
    pub fn tool_call_section_id(&self) -> Option<&str> {
        match self {
            MessageSection::ToolCall(section) => Some(&section.id),
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
        assert_eq!(section.content(), "");
        assert!(section.items.is_empty());
        assert!(!section.is_collapsed);
        assert!(!section.is_complete);
        assert!(!section.id.is_empty(), "ID should be generated");
    }

    #[test]
    fn test_agent_section_append() {
        let mut section = AgentSection::new("agent", "Agent");
        section.append("Hello ");
        section.append("World");
        assert_eq!(section.content(), "Hello World");
        // Text should be merged into a single item
        assert_eq!(section.items.len(), 1);
    }

    #[test]
    fn test_agent_section_tool_call() {
        let mut section = AgentSection::new("agent", "Agent");
        section.append("Starting...\n");
        let tool_id = section.append_tool_call(ToolDisplayInfo::new("Edited", "main.rs"));
        section.append("Done!\n");

        // Should have 3 items: text, tool call, text
        assert_eq!(section.items.len(), 3);

        // Complete the tool call
        section.complete_tool_call(&tool_id, true);

        // Check content() output includes the tool call
        let content = section.content();
        assert!(content.contains("Starting..."));
        assert!(content.contains("Edited"));
        assert!(content.contains("main.rs"));
        assert!(content.contains("✓"));
        assert!(content.contains("Done!"));
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

    #[test]
    fn test_agent_section_has_active_thinking() {
        let mut section = AgentSection::new("agent", "Agent");

        // No thinking yet
        assert!(!section.has_active_thinking());

        // Start thinking
        section.start_thinking();
        assert!(section.has_active_thinking());

        // Complete thinking
        section.complete_thinking();
        assert!(!section.has_active_thinking());

        // Start another
        section.start_thinking();
        assert!(section.has_active_thinking());
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

    // ==========================================================================
    // ThinkingSection tests
    // ==========================================================================

    #[test]
    fn test_thinking_section_new() {
        let section = ThinkingSection::new();
        assert!(!section.id.is_empty(), "ID should be generated");
        assert_eq!(section.content, "");
        assert!(!section.is_complete);
        assert!(section.is_collapsed, "Should start collapsed by default");
    }

    #[test]
    fn test_thinking_section_append() {
        let mut section = ThinkingSection::new();
        section.append("Thinking about ");
        section.append("the problem");
        assert_eq!(section.content, "Thinking about the problem");
    }

    #[test]
    fn test_thinking_section_finish() {
        let mut section = ThinkingSection::new();
        assert!(!section.is_complete);
        section.finish();
        assert!(section.is_complete);
    }

    #[test]
    fn test_thinking_section_toggle_collapsed() {
        let mut section = ThinkingSection::new();
        assert!(section.is_collapsed, "Should start collapsed");

        section.toggle_collapsed();
        assert!(
            !section.is_collapsed,
            "Should be uncollapsed after first toggle"
        );

        section.toggle_collapsed();
        assert!(
            section.is_collapsed,
            "Should be collapsed after second toggle"
        );
    }

    #[test]
    fn test_thinking_section_preview_empty() {
        let section = ThinkingSection::new();
        assert_eq!(section.preview(), "");
    }

    #[test]
    fn test_thinking_section_preview_short_single_line() {
        let mut section = ThinkingSection::new();
        section.append("Short thought");
        assert_eq!(section.preview(), "Short thought");
    }

    #[test]
    fn test_thinking_section_preview_multiline() {
        let mut section = ThinkingSection::new();
        section.append("First line\nSecond line\nThird line");
        assert_eq!(section.preview(), "First line...");
    }

    #[test]
    fn test_thinking_section_preview_long_first_line() {
        let mut section = ThinkingSection::new();
        section
            .append("This is a very long line that definitely exceeds fifty characters in length");
        let preview = section.preview();
        assert!(preview.ends_with("..."));
        // 50 chars + "..."
        assert_eq!(preview.len(), 53);
    }

    #[test]
    fn test_thinking_section_preview_exactly_50_chars_no_newline() {
        let mut section = ThinkingSection::new();
        // Exactly 50 chars, no newline - should NOT have ellipsis
        section.append("12345678901234567890123456789012345678901234567890");
        assert_eq!(
            section.preview(),
            "12345678901234567890123456789012345678901234567890"
        );
    }

    #[test]
    fn test_message_section_is_thinking() {
        let text_section = MessageSection::Text("hello".to_string());
        let agent_section = MessageSection::NestedAgent(AgentSection::new("a", "A"));
        let thinking_section = MessageSection::Thinking(ThinkingSection::new());

        assert!(!text_section.is_thinking());
        assert!(!agent_section.is_thinking());
        assert!(thinking_section.is_thinking());
    }

    #[test]
    fn test_message_section_thinking_section_id() {
        let text_section = MessageSection::Text("hello".to_string());
        let thinking = ThinkingSection::new();
        let expected_id = thinking.id.clone();
        let thinking_section = MessageSection::Thinking(thinking);

        assert!(text_section.thinking_section_id().is_none());
        assert_eq!(
            thinking_section.thinking_section_id(),
            Some(expected_id.as_str())
        );
    }

    // ==========================================================================
    // ToolCallSection tests
    // ==========================================================================

    #[test]
    fn test_tool_call_section_new() {
        let info = ToolDisplayInfo::new("Edited", "src/main.rs");
        let section = ToolCallSection::new(info);
        assert!(!section.id.is_empty());
        assert!(section.is_running);
        assert!(section.succeeded.is_none());
        assert_eq!(section.info.verb, "Edited");
        assert_eq!(section.info.subject, "src/main.rs");
    }

    #[test]
    fn test_tool_call_section_complete() {
        let info = ToolDisplayInfo::new("Read", "file.rs");
        let mut section = ToolCallSection::new(info);

        section.complete(true);
        assert!(!section.is_running);
        assert_eq!(section.succeeded, Some(true));

        let info2 = ToolDisplayInfo::new("Deleted", "old.rs");
        let mut section2 = ToolCallSection::new(info2);
        section2.complete(false);
        assert_eq!(section2.succeeded, Some(false));
    }
}
