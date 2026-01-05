//! Result types and errors for agent execution.
//!
//! Contains:
//! - `ExecutorResult`: The result of agent execution
//! - `ExecutorStreamReceiver`: Wrapper for receiving stream events
//! - `ExecutorError`: Error types for executor operations

use serdes_ai_core::ModelRequest;
use thiserror::Error;
use tokio::sync::mpsc;

use super::StreamEvent;

/// Result of agent execution.
pub struct ExecutorResult {
    /// The agent's final text output.
    pub output: String,
    /// Full message history (for context continuation).
    pub messages: Vec<ModelRequest>,
    /// Unique run ID for tracing.
    pub run_id: String,
}

/// Receiver for streaming events from agent execution.
///
/// This wraps an mpsc receiver and provides a convenient interface
/// for consuming streaming events.
pub struct ExecutorStreamReceiver {
    rx: mpsc::Receiver<Result<StreamEvent, ExecutorError>>,
}

impl ExecutorStreamReceiver {
    /// Create a new stream receiver from a channel.
    pub(super) fn new(rx: mpsc::Receiver<Result<StreamEvent, ExecutorError>>) -> Self {
        Self { rx }
    }

    /// Receive the next event from the stream.
    ///
    /// Returns `None` when the stream is complete.
    pub async fn recv(&mut self) -> Option<Result<StreamEvent, ExecutorError>> {
        self.rx.recv().await
    }
}

/// Errors that can occur during agent execution.
#[derive(Debug, Error)]
pub enum ExecutorError {
    #[error("Model error: {0}")]
    Model(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Configuration error: {0}")]
    Config(String),
}
