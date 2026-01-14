//! TUI Chat message types and operations
//!
//! Defines the core message structure for the conversation TUI.

use serde_json::Value;

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

/// Structured tool call display info for styled rendering
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDisplayInfo {
    /// The action verb (e.g., "Edited", "Read", "Searched")
    pub verb: String,
    /// The subject/target (e.g., file path, search pattern)
    pub subject: String,
}

impl ToolDisplayInfo {
    pub fn new(verb: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            verb: verb.into(),
            subject: subject.into(),
        }
    }
}

/// Get structured display info for a tool call
pub fn get_tool_display_info(name: &str, args: &Value) -> ToolDisplayInfo {
    match name {
        "list_files" => {
            let dir = args
                .get("directory")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            let recursive = args
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let rec_str = if recursive { " (recursive)" } else { "" };
            ToolDisplayInfo::new("Listed", format!("{}{}", dir, rec_str))
        }
        "read_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Read", path)
        }
        "edit_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Edited", path)
        }
        "delete_file" => {
            let path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Deleted", path)
        }
        "grep" => {
            let pattern = args
                .get("pattern")
                .or(args.get("search_string"))
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            let dir = args
                .get("directory")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            ToolDisplayInfo::new("Searched", format!("'{}' in {}", pattern, dir))
        }
        "run_shell_command" | "agent_run_shell_command" => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            let preview = if cmd.len() > 60 {
                format!("{}...", &cmd[..57])
            } else {
                cmd.to_string()
            };
            ToolDisplayInfo::new("Ran", preview)
        }
        "invoke_agent" => {
            let agent = args
                .get("agent_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            ToolDisplayInfo::new("Invoked", agent)
        }
        _ => {
            // For unknown tools, use the tool name as the verb
            ToolDisplayInfo::new(name, "")
        }
    }
}

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

    pub fn finish(&mut self) {
        self.is_complete = true;
    }

    pub fn toggle_collapsed(&mut self) {
        self.is_collapsed = !self.is_collapsed;
    }

    /// Get combined content as string
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
}

/// A single chat message
#[derive(Debug, Clone)]
pub struct TuiMessage {
    pub id: String,
    pub role: MessageRole,
    /// Legacy content field - kept for compatibility, represents flattened view
    pub content: String,
    /// Structured sections (for assistant messages with nested agents)
    pub sections: Vec<MessageSection>,
    pub tool_calls: Vec<ToolCall>,
    pub is_streaming: bool,
}

impl TuiMessage {
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
            // Last section is either NestedAgent/Thinking/ToolCall or there are no sections - create new Text
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
