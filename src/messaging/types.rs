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
}
