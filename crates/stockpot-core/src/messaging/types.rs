//! Message types for agent-UI communication.

use serde::{Deserialize, Serialize};

/// Message levels for styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    Info,
    Success,
    Warning,
    Error,
    Debug,
}

/// A text message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessage {
    pub level: MessageLevel,
    pub text: String,
}

/// Agent reasoning/thinking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningMessage {
    pub reasoning: String,
    pub next_steps: Option<String>,
}

/// Agent response (markdown content).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub content: String,
    pub is_streaming: bool,
}

/// Shell command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellMessage {
    pub command: String,
    pub output: Option<String>,
    pub exit_code: Option<i32>,
    pub is_running: bool,
}

/// File operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMessage {
    pub operation: FileOperation,
    pub path: String,
    pub content: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Read,
    Write,
    List,
    Grep,
    Delete,
}

/// Diff display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffMessage {
    pub path: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub content: String,
    pub line_type: DiffLineType,
    pub line_number: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DiffLineType {
    Context,
    Added,
    Removed,
    Header,
}

/// Spinner control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinnerMessage {
    pub text: String,
    pub is_active: bool,
}

/// User input request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRequest {
    pub prompt: String,
    pub request_type: InputType,
    pub options: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InputType {
    Text,
    Confirmation,
    Selection,
}

/// Agent lifecycle events (start, complete, error).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub agent_name: String,
    pub display_name: String,
    pub event: AgentEvent,
}

/// Agent lifecycle event types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AgentEvent {
    Started,
    Completed { run_id: String },
    Error { message: String },
}

/// Tool execution lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolMessage {
    pub tool_name: String,
    /// Unique ID for this tool call (for parallel tool tracking)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub status: ToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Agent that executed this tool (for nested agent routing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
}

/// Tool execution status.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    #[default]
    Started,
    ArgsStreaming,
    Executing,
    Completed,
    Failed,
}

/// Context size information from agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextInfoMessage {
    pub estimated_tokens: usize,
    pub request_bytes: usize,
    pub context_limit: Option<u64>,
}

/// Context compression notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextCompressedMessage {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub strategy: String,
    pub messages_before: usize,
    pub messages_after: usize,
}

/// Streaming text from agent response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDeltaMessage {
    pub text: String,
    /// For nested agent output identification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
}

/// Thinking/reasoning delta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingMessage {
    pub text: String,
    /// Agent name for attribution (None = main agent)
    pub agent_name: Option<String>,
}

/// History update message (for TUI to track conversation history)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryUpdateMessage {
    pub messages: Vec<serdes_ai_core::ModelRequest>,
}

/// API key retry/rotation notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryMessage {
    /// The provider being retried (e.g., "openai", "anthropic")
    pub provider: String,
    /// Current retry attempt number
    pub attempt: u32,
    /// Reason for retry (e.g., "Rate limited (429)")
    pub reason: String,
    /// Seconds to wait (if in cooldown)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_secs: Option<u64>,
    /// True if this is a key rotation, false if waiting for cooldown
    pub is_rotation: bool,
}

/// Any message type (for serialization).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Text(TextMessage),
    Reasoning(ReasoningMessage),
    Response(ResponseMessage),
    Shell(ShellMessage),
    File(FileMessage),
    Diff(DiffMessage),
    Spinner(SpinnerMessage),
    InputRequest(InputRequest),
    Agent(AgentMessage),
    Tool(ToolMessage),
    TextDelta(TextDeltaMessage),
    Thinking(ThinkingMessage),
    HistoryUpdate(HistoryUpdateMessage),
    Retry(RetryMessage),
    ContextInfo(ContextInfoMessage),
    ContextCompressed(ContextCompressedMessage),
    Divider,
    Clear,
}

impl Message {
    /// Create an info message.
    pub fn info(text: impl Into<String>) -> Self {
        Self::Text(TextMessage {
            level: MessageLevel::Info,
            text: text.into(),
        })
    }

    /// Create a success message.
    pub fn success(text: impl Into<String>) -> Self {
        Self::Text(TextMessage {
            level: MessageLevel::Success,
            text: text.into(),
        })
    }

    /// Create a warning message.
    pub fn warning(text: impl Into<String>) -> Self {
        Self::Text(TextMessage {
            level: MessageLevel::Warning,
            text: text.into(),
        })
    }

    /// Create an error message.
    pub fn error(text: impl Into<String>) -> Self {
        Self::Text(TextMessage {
            level: MessageLevel::Error,
            text: text.into(),
        })
    }

    /// Create a response message.
    pub fn response(content: impl Into<String>) -> Self {
        Self::Response(ResponseMessage {
            content: content.into(),
            is_streaming: false,
        })
    }

    /// Create an agent started message.
    pub fn agent_started(name: &str, display_name: &str) -> Self {
        Self::Agent(AgentMessage {
            agent_name: name.to_string(),
            display_name: display_name.to_string(),
            event: AgentEvent::Started,
        })
    }

    /// Create an agent completed message.
    pub fn agent_completed(name: &str, display_name: &str, run_id: &str) -> Self {
        Self::Agent(AgentMessage {
            agent_name: name.to_string(),
            display_name: display_name.to_string(),
            event: AgentEvent::Completed {
                run_id: run_id.to_string(),
            },
        })
    }

    /// Create an agent error message.
    pub fn agent_error(name: &str, display_name: &str, error: &str) -> Self {
        Self::Agent(AgentMessage {
            agent_name: name.to_string(),
            display_name: display_name.to_string(),
            event: AgentEvent::Error {
                message: error.to_string(),
            },
        })
    }

    /// Create a tool started message.
    pub fn tool_started(tool_name: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Started,
            ..Default::default()
        })
    }

    /// Create a tool started message with ID.
    pub fn tool_started_with_id(tool_name: &str, tool_call_id: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Started,
            ..Default::default()
        })
    }

    /// Create a tool executing message (with parsed args).
    pub fn tool_executing(tool_name: &str, args: Option<serde_json::Value>) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Executing,
            args,
            ..Default::default()
        })
    }

    /// Create a tool executing message with ID.
    pub fn tool_executing_with_id(
        tool_name: &str,
        tool_call_id: &str,
        args: Option<serde_json::Value>,
    ) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Executing,
            args,
            ..Default::default()
        })
    }

    /// Create a tool completed message.
    pub fn tool_completed(tool_name: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Completed,
            ..Default::default()
        })
    }

    /// Create a tool completed message with ID.
    pub fn tool_completed_with_id(tool_name: &str, tool_call_id: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Completed,
            ..Default::default()
        })
    }

    /// Create a tool failed message.
    pub fn tool_failed(tool_name: &str, error: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Failed,
            error: Some(error.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool failed message with ID.
    pub fn tool_failed_with_id(tool_name: &str, tool_call_id: &str, error: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Failed,
            error: Some(error.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool started message with agent name.
    pub fn tool_started_from(tool_name: &str, agent_name: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Started,
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool started message with ID and agent name.
    pub fn tool_started_with_id_from(
        tool_name: &str,
        tool_call_id: &str,
        agent_name: &str,
    ) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Started,
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool executing message with agent name.
    pub fn tool_executing_from(
        tool_name: &str,
        args: Option<serde_json::Value>,
        agent_name: &str,
    ) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Executing,
            args,
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool executing message with ID and agent name.
    pub fn tool_executing_with_id_from(
        tool_name: &str,
        tool_call_id: &str,
        args: Option<serde_json::Value>,
        agent_name: &str,
    ) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Executing,
            args,
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool completed message with agent name.
    pub fn tool_completed_from(tool_name: &str, agent_name: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Completed,
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool completed message with ID and agent name.
    pub fn tool_completed_with_id_from(
        tool_name: &str,
        tool_call_id: &str,
        agent_name: &str,
    ) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Completed,
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool failed message with agent name.
    pub fn tool_failed_from(tool_name: &str, error: &str, agent_name: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: None,
            status: ToolStatus::Failed,
            error: Some(error.to_string()),
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a tool failed message with ID and agent name.
    pub fn tool_failed_with_id_from(
        tool_name: &str,
        tool_call_id: &str,
        error: &str,
        agent_name: &str,
    ) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            tool_call_id: Some(tool_call_id.to_string()),
            status: ToolStatus::Failed,
            error: Some(error.to_string()),
            agent_name: Some(agent_name.to_string()),
            ..Default::default()
        })
    }

    /// Create a text delta message.
    pub fn text_delta(text: &str) -> Self {
        Self::TextDelta(TextDeltaMessage {
            text: text.to_string(),
            agent_name: None,
        })
    }

    /// Create a text delta from a specific agent.
    pub fn text_delta_from(text: &str, agent_name: &str) -> Self {
        Self::TextDelta(TextDeltaMessage {
            text: text.to_string(),
            agent_name: Some(agent_name.to_string()),
        })
    }

    /// Create a thinking delta message (unattributed, for main agent).
    pub fn thinking(text: &str) -> Self {
        Self::Thinking(ThinkingMessage {
            text: text.to_string(),
            agent_name: None,
        })
    }

    /// Create a thinking delta message with agent attribution.
    pub fn thinking_from(text: &str, agent_name: &str) -> Self {
        Self::Thinking(ThinkingMessage {
            text: text.to_string(),
            agent_name: Some(agent_name.to_string()),
        })
    }

    /// Create a retry message for key rotation.
    pub fn retry_rotation(provider: &str, attempt: u32, reason: &str) -> Self {
        Self::Retry(RetryMessage {
            provider: provider.to_string(),
            attempt,
            reason: reason.to_string(),
            wait_secs: None,
            is_rotation: true,
        })
    }

    /// Create a retry message for cooldown wait.
    pub fn retry_cooldown(provider: &str, attempt: u32, reason: &str, wait_secs: u64) -> Self {
        Self::Retry(RetryMessage {
            provider: provider.to_string(),
            attempt,
            reason: reason.to_string(),
            wait_secs: Some(wait_secs),
            is_rotation: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // MessageLevel Tests
    // =========================================================================

    #[test]
    fn test_message_level_serde() {
        let levels = [
            (MessageLevel::Info, "\"info\""),
            (MessageLevel::Success, "\"success\""),
            (MessageLevel::Warning, "\"warning\""),
            (MessageLevel::Error, "\"error\""),
            (MessageLevel::Debug, "\"debug\""),
        ];

        for (level, expected) in levels {
            let json = serde_json::to_string(&level).unwrap();
            assert_eq!(json, expected);

            let parsed: MessageLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, level);
        }
    }

    // =========================================================================
    // FileOperation Tests
    // =========================================================================

    #[test]
    fn test_file_operation_serde() {
        let ops = [
            (FileOperation::Read, "\"read\""),
            (FileOperation::Write, "\"write\""),
            (FileOperation::List, "\"list\""),
            (FileOperation::Grep, "\"grep\""),
            (FileOperation::Delete, "\"delete\""),
        ];

        for (op, expected) in ops {
            let json = serde_json::to_string(&op).unwrap();
            assert_eq!(json, expected);

            let parsed: FileOperation = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, op);
        }
    }

    // =========================================================================
    // DiffLineType Tests
    // =========================================================================

    #[test]
    fn test_diff_line_type_serde() {
        let types = [
            (DiffLineType::Context, "\"context\""),
            (DiffLineType::Added, "\"added\""),
            (DiffLineType::Removed, "\"removed\""),
            (DiffLineType::Header, "\"header\""),
        ];

        for (lt, expected) in types {
            let json = serde_json::to_string(&lt).unwrap();
            assert_eq!(json, expected);

            let parsed: DiffLineType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, lt);
        }
    }

    // =========================================================================
    // InputType Tests
    // =========================================================================

    #[test]
    fn test_input_type_serde() {
        let types = [
            (InputType::Text, "\"text\""),
            (InputType::Confirmation, "\"confirmation\""),
            (InputType::Selection, "\"selection\""),
        ];

        for (it, expected) in types {
            let json = serde_json::to_string(&it).unwrap();
            assert_eq!(json, expected);

            let parsed: InputType = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, it);
        }
    }

    // =========================================================================
    // ToolStatus Tests
    // =========================================================================

    #[test]
    fn test_tool_status_serde() {
        let statuses = [
            (ToolStatus::Started, "\"started\""),
            (ToolStatus::ArgsStreaming, "\"args_streaming\""),
            (ToolStatus::Executing, "\"executing\""),
            (ToolStatus::Completed, "\"completed\""),
            (ToolStatus::Failed, "\"failed\""),
        ];

        for (status, expected) in statuses {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected);

            let parsed: ToolStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_tool_status_default() {
        let status = ToolStatus::default();
        assert_eq!(status, ToolStatus::Started);
    }

    // =========================================================================
    // AgentEvent Tests
    // =========================================================================

    #[test]
    fn test_agent_event_started_serde() {
        let event = AgentEvent::Started;
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("started"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        matches!(parsed, AgentEvent::Started);
    }

    #[test]
    fn test_agent_event_completed_serde() {
        let event = AgentEvent::Completed {
            run_id: "run-123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("completed"));
        assert!(json.contains("run-123"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        if let AgentEvent::Completed { run_id } = parsed {
            assert_eq!(run_id, "run-123");
        } else {
            panic!("Expected Completed variant");
        }
    }

    #[test]
    fn test_agent_event_error_serde() {
        let event = AgentEvent::Error {
            message: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("error"));
        assert!(json.contains("Something went wrong"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        if let AgentEvent::Error { message } = parsed {
            assert_eq!(message, "Something went wrong");
        } else {
            panic!("Expected Error variant");
        }
    }

    // =========================================================================
    // Message Factory Tests
    // =========================================================================

    #[test]
    fn test_message_info() {
        let msg = Message::info("test info");
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.level, MessageLevel::Info);
            assert_eq!(text_msg.text, "test info");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_message_success() {
        let msg = Message::success("operation succeeded");
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.level, MessageLevel::Success);
            assert_eq!(text_msg.text, "operation succeeded");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_message_warning() {
        let msg = Message::warning("be careful");
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.level, MessageLevel::Warning);
            assert_eq!(text_msg.text, "be careful");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_message_error() {
        let msg = Message::error("something failed");
        if let Message::Text(text_msg) = msg {
            assert_eq!(text_msg.level, MessageLevel::Error);
            assert_eq!(text_msg.text, "something failed");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_message_response() {
        let msg = Message::response("# Hello\n\nWorld");
        if let Message::Response(resp) = msg {
            assert_eq!(resp.content, "# Hello\n\nWorld");
            assert!(!resp.is_streaming);
        } else {
            panic!("Expected Response variant");
        }
    }

    #[test]
    fn test_message_agent_started() {
        let msg = Message::agent_started("stockpot", "Stockpot");
        if let Message::Agent(agent_msg) = msg {
            assert_eq!(agent_msg.agent_name, "stockpot");
            assert_eq!(agent_msg.display_name, "Stockpot");
            matches!(agent_msg.event, AgentEvent::Started);
        } else {
            panic!("Expected Agent variant");
        }
    }

    #[test]
    fn test_message_agent_completed() {
        let msg = Message::agent_completed("stockpot", "Stockpot", "run-abc");
        if let Message::Agent(agent_msg) = msg {
            assert_eq!(agent_msg.agent_name, "stockpot");
            assert_eq!(agent_msg.display_name, "Stockpot");
            if let AgentEvent::Completed { run_id } = agent_msg.event {
                assert_eq!(run_id, "run-abc");
            } else {
                panic!("Expected Completed event");
            }
        } else {
            panic!("Expected Agent variant");
        }
    }

    #[test]
    fn test_message_agent_error() {
        let msg = Message::agent_error("stockpot", "Stockpot", "failed to run");
        if let Message::Agent(agent_msg) = msg {
            assert_eq!(agent_msg.agent_name, "stockpot");
            if let AgentEvent::Error { message } = agent_msg.event {
                assert_eq!(message, "failed to run");
            } else {
                panic!("Expected Error event");
            }
        } else {
            panic!("Expected Agent variant");
        }
    }

    #[test]
    fn test_message_tool_started() {
        let msg = Message::tool_started("read_file");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_name, "read_file");
            assert!(tool_msg.tool_call_id.is_none());
            assert_eq!(tool_msg.status, ToolStatus::Started);
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_started_with_id() {
        let msg = Message::tool_started_with_id("read_file", "call-123");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_name, "read_file");
            assert_eq!(tool_msg.tool_call_id, Some("call-123".to_string()));
            assert_eq!(tool_msg.status, ToolStatus::Started);
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_executing() {
        let args = serde_json::json!({"file_path": "/test.txt"});
        let msg = Message::tool_executing("read_file", Some(args.clone()));
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_name, "read_file");
            assert_eq!(tool_msg.status, ToolStatus::Executing);
            assert_eq!(tool_msg.args, Some(args));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_executing_with_id() {
        let args = serde_json::json!({"file_path": "/test.txt"});
        let msg = Message::tool_executing_with_id("read_file", "call-456", Some(args.clone()));
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_call_id, Some("call-456".to_string()));
            assert_eq!(tool_msg.status, ToolStatus::Executing);
            assert_eq!(tool_msg.args, Some(args));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_completed() {
        let msg = Message::tool_completed("read_file");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_name, "read_file");
            assert_eq!(tool_msg.status, ToolStatus::Completed);
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_completed_with_id() {
        let msg = Message::tool_completed_with_id("read_file", "call-789");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_call_id, Some("call-789".to_string()));
            assert_eq!(tool_msg.status, ToolStatus::Completed);
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_failed() {
        let msg = Message::tool_failed("read_file", "file not found");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_name, "read_file");
            assert_eq!(tool_msg.status, ToolStatus::Failed);
            assert_eq!(tool_msg.error, Some("file not found".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_failed_with_id() {
        let msg = Message::tool_failed_with_id("read_file", "call-err", "permission denied");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_call_id, Some("call-err".to_string()));
            assert_eq!(tool_msg.status, ToolStatus::Failed);
            assert_eq!(tool_msg.error, Some("permission denied".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    // =========================================================================
    // Tool Messages from Agent Tests
    // =========================================================================

    #[test]
    fn test_message_tool_started_from() {
        let msg = Message::tool_started_from("grep", "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_name, "grep");
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
            assert_eq!(tool_msg.status, ToolStatus::Started);
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_started_with_id_from() {
        let msg = Message::tool_started_with_id_from("grep", "call-x", "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_call_id, Some("call-x".to_string()));
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_executing_from() {
        let args = serde_json::json!({"pattern": "TODO"});
        let msg = Message::tool_executing_from("grep", Some(args.clone()), "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.status, ToolStatus::Executing);
            assert_eq!(tool_msg.args, Some(args));
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_executing_with_id_from() {
        let args = serde_json::json!({"pattern": "FIXME"});
        let msg =
            Message::tool_executing_with_id_from("grep", "call-y", Some(args.clone()), "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_call_id, Some("call-y".to_string()));
            assert_eq!(tool_msg.args, Some(args));
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_completed_from() {
        let msg = Message::tool_completed_from("grep", "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.status, ToolStatus::Completed);
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_completed_with_id_from() {
        let msg = Message::tool_completed_with_id_from("grep", "call-z", "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_call_id, Some("call-z".to_string()));
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_failed_from() {
        let msg = Message::tool_failed_from("grep", "regex error", "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.status, ToolStatus::Failed);
            assert_eq!(tool_msg.error, Some("regex error".to_string()));
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    #[test]
    fn test_message_tool_failed_with_id_from() {
        let msg = Message::tool_failed_with_id_from("grep", "call-f", "timeout", "explorer");
        if let Message::Tool(tool_msg) = msg {
            assert_eq!(tool_msg.tool_call_id, Some("call-f".to_string()));
            assert_eq!(tool_msg.error, Some("timeout".to_string()));
            assert_eq!(tool_msg.agent_name, Some("explorer".to_string()));
        } else {
            panic!("Expected Tool variant");
        }
    }

    // =========================================================================
    // Text Delta and Thinking Tests
    // =========================================================================

    #[test]
    fn test_message_text_delta() {
        let msg = Message::text_delta("Hello ");
        if let Message::TextDelta(delta) = msg {
            assert_eq!(delta.text, "Hello ");
            assert!(delta.agent_name.is_none());
        } else {
            panic!("Expected TextDelta variant");
        }
    }

    #[test]
    fn test_message_text_delta_from() {
        let msg = Message::text_delta_from("streaming...", "planner");
        if let Message::TextDelta(delta) = msg {
            assert_eq!(delta.text, "streaming...");
            assert_eq!(delta.agent_name, Some("planner".to_string()));
        } else {
            panic!("Expected TextDelta variant");
        }
    }

    #[test]
    fn test_message_thinking() {
        let msg = Message::thinking("considering options...");
        if let Message::Thinking(thinking) = msg {
            assert_eq!(thinking.text, "considering options...");
            assert!(thinking.agent_name.is_none());
        } else {
            panic!("Expected Thinking variant");
        }
    }

    #[test]
    fn test_message_thinking_from() {
        let msg = Message::thinking_from("analyzing...", "sub-agent");
        if let Message::Thinking(thinking) = msg {
            assert_eq!(thinking.text, "analyzing...");
            assert_eq!(thinking.agent_name, Some("sub-agent".to_string()));
        } else {
            panic!("Expected Thinking variant");
        }
    }

    // =========================================================================
    // Message Struct Tests
    // =========================================================================

    #[test]
    fn test_text_message_serde() {
        let msg = TextMessage {
            level: MessageLevel::Info,
            text: "Hello world".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: TextMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.level, MessageLevel::Info);
        assert_eq!(parsed.text, "Hello world");
    }

    #[test]
    fn test_reasoning_message_serde() {
        let msg = ReasoningMessage {
            reasoning: "I need to analyze this".to_string(),
            next_steps: Some("First, read the file".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ReasoningMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.reasoning, "I need to analyze this");
        assert_eq!(parsed.next_steps, Some("First, read the file".to_string()));
    }

    #[test]
    fn test_reasoning_message_no_steps() {
        let msg = ReasoningMessage {
            reasoning: "Thinking...".to_string(),
            next_steps: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ReasoningMessage = serde_json::from_str(&json).unwrap();
        assert!(parsed.next_steps.is_none());
    }

    #[test]
    fn test_response_message_serde() {
        let msg = ResponseMessage {
            content: "# Title\n\nBody text".to_string(),
            is_streaming: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ResponseMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "# Title\n\nBody text");
        assert!(parsed.is_streaming);
    }

    #[test]
    fn test_shell_message_serde() {
        let msg = ShellMessage {
            command: "ls -la".to_string(),
            output: Some("total 0\nfile.txt".to_string()),
            exit_code: Some(0),
            is_running: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ShellMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "ls -la");
        assert_eq!(parsed.exit_code, Some(0));
        assert!(!parsed.is_running);
    }

    #[test]
    fn test_shell_message_running() {
        let msg = ShellMessage {
            command: "sleep 10".to_string(),
            output: None,
            exit_code: None,
            is_running: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ShellMessage = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_running);
        assert!(parsed.output.is_none());
        assert!(parsed.exit_code.is_none());
    }

    #[test]
    fn test_file_message_serde() {
        let msg = FileMessage {
            operation: FileOperation::Read,
            path: "/tmp/test.txt".to_string(),
            content: Some("file contents".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: FileMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.operation, FileOperation::Read);
        assert_eq!(parsed.path, "/tmp/test.txt");
        assert!(parsed.content.is_some());
        assert!(parsed.error.is_none());
    }

    #[test]
    fn test_file_message_with_error() {
        let msg = FileMessage {
            operation: FileOperation::Write,
            path: "/protected/file.txt".to_string(),
            content: None,
            error: Some("Permission denied".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: FileMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.operation, FileOperation::Write);
        assert_eq!(parsed.error, Some("Permission denied".to_string()));
    }

    #[test]
    fn test_diff_message_serde() {
        let msg = DiffMessage {
            path: "src/main.rs".to_string(),
            lines: vec![
                DiffLine {
                    content: "@@ -1,3 +1,4 @@".to_string(),
                    line_type: DiffLineType::Header,
                    line_number: None,
                },
                DiffLine {
                    content: " fn main() {".to_string(),
                    line_type: DiffLineType::Context,
                    line_number: Some(1),
                },
                DiffLine {
                    content: "-    println!(\"old\");".to_string(),
                    line_type: DiffLineType::Removed,
                    line_number: Some(2),
                },
                DiffLine {
                    content: "+    println!(\"new\");".to_string(),
                    line_type: DiffLineType::Added,
                    line_number: Some(2),
                },
            ],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: DiffMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.path, "src/main.rs");
        assert_eq!(parsed.lines.len(), 4);
        assert_eq!(parsed.lines[0].line_type, DiffLineType::Header);
        assert_eq!(parsed.lines[2].line_type, DiffLineType::Removed);
    }

    #[test]
    fn test_spinner_message_serde() {
        let msg = SpinnerMessage {
            text: "Loading...".to_string(),
            is_active: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: SpinnerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, "Loading...");
        assert!(parsed.is_active);
    }

    #[test]
    fn test_input_request_serde() {
        let msg = InputRequest {
            prompt: "Enter your name:".to_string(),
            request_type: InputType::Text,
            options: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: InputRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.prompt, "Enter your name:");
        assert_eq!(parsed.request_type, InputType::Text);
    }

    #[test]
    fn test_input_request_with_options() {
        let msg = InputRequest {
            prompt: "Choose an option:".to_string(),
            request_type: InputType::Selection,
            options: Some(vec!["Option A".to_string(), "Option B".to_string()]),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: InputRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.request_type, InputType::Selection);
        assert_eq!(parsed.options.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_agent_message_serde() {
        let msg = AgentMessage {
            agent_name: "stockpot".to_string(),
            display_name: "Stockpot".to_string(),
            event: AgentEvent::Started,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.agent_name, "stockpot");
        assert_eq!(parsed.display_name, "Stockpot");
    }

    #[test]
    fn test_tool_message_serde() {
        let msg = ToolMessage {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("call-123".to_string()),
            status: ToolStatus::Completed,
            args: Some(serde_json::json!({"file_path": "/test.txt"})),
            result: Some("file contents".to_string()),
            error: None,
            agent_name: Some("explorer".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ToolMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tool_name, "read_file");
        assert_eq!(parsed.tool_call_id, Some("call-123".to_string()));
        assert_eq!(parsed.status, ToolStatus::Completed);
        assert!(parsed.result.is_some());
    }

    #[test]
    fn test_tool_message_default() {
        let msg = ToolMessage::default();
        assert!(msg.tool_name.is_empty());
        assert!(msg.tool_call_id.is_none());
        assert_eq!(msg.status, ToolStatus::Started);
        assert!(msg.args.is_none());
        assert!(msg.result.is_none());
        assert!(msg.error.is_none());
        assert!(msg.agent_name.is_none());
    }

    #[test]
    fn test_text_delta_message_serde() {
        let msg = TextDeltaMessage {
            text: "Hello ".to_string(),
            agent_name: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: TextDeltaMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, "Hello ");
        assert!(parsed.agent_name.is_none());
    }

    #[test]
    fn test_thinking_message_serde() {
        let msg = ThinkingMessage {
            text: "Analyzing the problem...".to_string(),
            agent_name: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ThinkingMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, "Analyzing the problem...");
        assert!(parsed.agent_name.is_none());
    }

    #[test]
    fn test_thinking_message_serde_with_agent() {
        let msg = ThinkingMessage {
            text: "Sub-agent thinking...".to_string(),
            agent_name: Some("helper".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: ThinkingMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, "Sub-agent thinking...");
        assert_eq!(parsed.agent_name, Some("helper".to_string()));
    }

    // =========================================================================
    // Full Message Enum Tests
    // =========================================================================

    #[test]
    fn test_message_enum_text_serde() {
        let msg = Message::Text(TextMessage {
            level: MessageLevel::Info,
            text: "test".to_string(),
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"text\""));

        let parsed: Message = serde_json::from_str(&json).unwrap();
        matches!(parsed, Message::Text(_));
    }

    #[test]
    fn test_message_enum_divider_serde() {
        let msg = Message::Divider;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"divider\""));

        let parsed: Message = serde_json::from_str(&json).unwrap();
        matches!(parsed, Message::Divider);
    }

    #[test]
    fn test_message_enum_clear_serde() {
        let msg = Message::Clear;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"clear\""));

        let parsed: Message = serde_json::from_str(&json).unwrap();
        matches!(parsed, Message::Clear);
    }

    // =========================================================================
    // RetryMessage Tests
    // =========================================================================

    #[test]
    fn test_retry_message_rotation() {
        let msg = RetryMessage {
            provider: "openai".to_string(),
            attempt: 1,
            reason: "Rate limited (429)".to_string(),
            wait_secs: None,
            is_rotation: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: RetryMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, "openai");
        assert_eq!(parsed.attempt, 1);
        assert!(parsed.is_rotation);
        assert!(parsed.wait_secs.is_none());
    }

    #[test]
    fn test_retry_message_cooldown() {
        let msg = RetryMessage {
            provider: "anthropic".to_string(),
            attempt: 3,
            reason: "All keys exhausted".to_string(),
            wait_secs: Some(15),
            is_rotation: false,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: RetryMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.provider, "anthropic");
        assert_eq!(parsed.attempt, 3);
        assert!(!parsed.is_rotation);
        assert_eq!(parsed.wait_secs, Some(15));
    }

    #[test]
    fn test_retry_message_skip_serializing_none_wait() {
        let msg = RetryMessage {
            provider: "openai".to_string(),
            attempt: 1,
            reason: "429".to_string(),
            wait_secs: None,
            is_rotation: true,
        };
        let json = serde_json::to_string(&msg).unwrap();
        // wait_secs should not appear in JSON when None
        assert!(!json.contains("wait_secs"));
    }

    #[test]
    fn test_message_retry_rotation_helper() {
        let msg = Message::retry_rotation("openai", 1, "Rate limited");
        match msg {
            Message::Retry(rm) => {
                assert_eq!(rm.provider, "openai");
                assert_eq!(rm.attempt, 1);
                assert_eq!(rm.reason, "Rate limited");
                assert!(rm.is_rotation);
                assert!(rm.wait_secs.is_none());
            }
            _ => panic!("Expected Message::Retry"),
        }
    }

    #[test]
    fn test_message_retry_cooldown_helper() {
        let msg = Message::retry_cooldown("anthropic", 2, "Keys exhausted", 30);
        match msg {
            Message::Retry(rm) => {
                assert_eq!(rm.provider, "anthropic");
                assert_eq!(rm.attempt, 2);
                assert_eq!(rm.reason, "Keys exhausted");
                assert!(!rm.is_rotation);
                assert_eq!(rm.wait_secs, Some(30));
            }
            _ => panic!("Expected Message::Retry"),
        }
    }

    #[test]
    fn test_message_enum_retry_serde() {
        let msg = Message::Retry(RetryMessage {
            provider: "google".to_string(),
            attempt: 5,
            reason: "Quota exceeded".to_string(),
            wait_secs: Some(60),
            is_rotation: false,
        });
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"retry\""));
        assert!(json.contains("\"provider\":\"google\""));

        let parsed: Message = serde_json::from_str(&json).unwrap();
        matches!(parsed, Message::Retry(_));
    }
}
