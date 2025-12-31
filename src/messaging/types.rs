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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub status: ToolStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Tool execution status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    #[default]
    Started,
    ArgsStreaming,
    Executing,
    Completed,
    Failed,
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
            status: ToolStatus::Started,
            ..Default::default()
        })
    }

    /// Create a tool executing message (with parsed args).
    pub fn tool_executing(tool_name: &str, args: Option<serde_json::Value>) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            status: ToolStatus::Executing,
            args,
            ..Default::default()
        })
    }

    /// Create a tool completed message.
    pub fn tool_completed(tool_name: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            status: ToolStatus::Completed,
            ..Default::default()
        })
    }

    /// Create a tool failed message.
    pub fn tool_failed(tool_name: &str, error: &str) -> Self {
        Self::Tool(ToolMessage {
            tool_name: tool_name.to_string(),
            status: ToolStatus::Failed,
            error: Some(error.to_string()),
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

    /// Create a thinking delta message.
    pub fn thinking(text: &str) -> Self {
        Self::Thinking(ThinkingMessage {
            text: text.to_string(),
        })
    }
}
