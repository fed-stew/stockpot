//! Result types and errors for agent execution.
//!
//! Contains:
//! - `ExecutorResult`: The result of agent execution
//! - `ExecutorStreamReceiver`: Wrapper for receiving stream events
//! - `ExecutorError`: Error types for executor operations

use crate::mcp::McpManager;
use crate::tools::SpotToolRegistry;
use serdes_ai_core::ModelRequest;
use thiserror::Error;
use tokio::sync::mpsc;

use super::StreamEvent;

/// Execution context containing tool registry and MCP manager.
///
/// Groups the context parameters needed for agent execution.
pub struct ExecuteContext<'a> {
    /// Tool registry with available tools.
    pub tool_registry: &'a SpotToolRegistry,
    /// MCP server manager for additional tools.
    pub mcp_manager: &'a McpManager,
}

/// Result of agent execution.
#[derive(Debug)]
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
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),
}

impl ExecutorError {
    /// Check if this error indicates a rate limit.
    pub fn is_rate_limit(&self) -> bool {
        match self {
            ExecutorError::RateLimit(_) => true,
            ExecutorError::Execution(msg) | ExecutorError::Model(msg) => {
                let lower = msg.to_lowercase();
                // Check for HTTP status codes
                msg.contains("status: 429")
                    || msg.contains("status:429")
                    // Check for various rate limit message patterns (case insensitive)
                    || lower.contains("rate limit")   // matches "rate limit", "rate limited", "rate-limit"
                    || lower.contains("rate_limit")   // matches "rate_limit_error"
                    || lower.contains("ratelimit")    // matches "ratelimited"
                    || lower.contains("too many requests")
                    || lower.contains("quota exceeded")
                    || lower.contains("throttle") // some APIs use "throttled"
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_error_display_model() {
        let err = ExecutorError::Model("rate limit exceeded".into());
        assert_eq!(err.to_string(), "Model error: rate limit exceeded");
    }

    #[test]
    fn executor_error_display_auth() {
        let err = ExecutorError::Auth("invalid api key".into());
        assert_eq!(err.to_string(), "Authentication error: invalid api key");
    }

    #[test]
    fn executor_error_display_tool() {
        let err = ExecutorError::Tool("file not found".into());
        assert_eq!(err.to_string(), "Tool error: file not found");
    }

    #[test]
    fn executor_error_display_execution() {
        let err = ExecutorError::Execution("timeout".into());
        assert_eq!(err.to_string(), "Execution error: timeout");
    }

    #[test]
    fn executor_error_display_config() {
        let err = ExecutorError::Config("missing model".into());
        assert_eq!(err.to_string(), "Configuration error: missing model");
    }

    #[test]
    fn executor_error_display_rate_limit() {
        let err = ExecutorError::RateLimit("All keys exhausted".into());
        assert_eq!(err.to_string(), "Rate limit exceeded: All keys exhausted");
    }

    #[test]
    fn executor_error_is_rate_limit_direct() {
        let err = ExecutorError::RateLimit("test".into());
        assert!(err.is_rate_limit());
    }

    #[test]
    fn executor_error_is_rate_limit_from_status_429() {
        let err = ExecutorError::Execution("HTTP error status: 429".into());
        assert!(err.is_rate_limit());
    }

    #[test]
    fn executor_error_is_rate_limit_from_message() {
        assert!(ExecutorError::Execution("Rate limit exceeded".into()).is_rate_limit());
        assert!(ExecutorError::Model("rate_limit_error".into()).is_rate_limit());
        assert!(ExecutorError::Execution("Too Many Requests".into()).is_rate_limit());
        assert!(ExecutorError::Model("quota exceeded".into()).is_rate_limit());
    }

    #[test]
    fn executor_error_is_rate_limit_from_model_error() {
        // This is the exact error format we see from the API
        let err =
            ExecutorError::Execution("Model error: Rate limited, retry after Some(60s)".into());
        assert!(
            err.is_rate_limit(),
            "Should detect 'Rate limited' as rate limit error"
        );

        // Also test the raw message
        let err2 = ExecutorError::Execution("Rate limited, retry after Some(60s)".into());
        assert!(err2.is_rate_limit());

        // Test "ratelimited" without space
        let err3 = ExecutorError::Execution("Request ratelimited".into());
        assert!(err3.is_rate_limit());

        // Test "throttled"
        let err4 = ExecutorError::Execution("Request was throttled".into());
        assert!(err4.is_rate_limit());
    }

    #[test]
    fn executor_error_is_rate_limit_false_for_others() {
        assert!(!ExecutorError::Auth("invalid key".into()).is_rate_limit());
        assert!(!ExecutorError::Config("missing".into()).is_rate_limit());
        assert!(!ExecutorError::Execution("timeout".into()).is_rate_limit());
    }

    #[test]
    fn executor_result_fields() {
        let result = ExecutorResult {
            output: "Hello world".to_string(),
            messages: vec![],
            run_id: "run-123".to_string(),
        };
        assert_eq!(result.output, "Hello world");
        assert!(result.messages.is_empty());
        assert_eq!(result.run_id, "run-123");
    }

    #[test]
    fn executor_result_with_messages() {
        let mut msg = ModelRequest::new();
        msg.add_user_prompt("test prompt".to_string());

        let result = ExecutorResult {
            output: "response".to_string(),
            messages: vec![msg],
            run_id: "run-456".to_string(),
        };
        assert_eq!(result.messages.len(), 1);
    }

    #[tokio::test]
    async fn executor_stream_receiver_new_and_recv() {
        let (tx, rx) = mpsc::channel(1);
        let mut receiver = ExecutorStreamReceiver::new(rx);

        // Send an event
        tx.send(Err(ExecutorError::Model("test".into())))
            .await
            .unwrap();
        drop(tx);

        // Receive it
        let event = receiver.recv().await;
        assert!(event.is_some());
        assert!(event.unwrap().is_err());

        // Channel closed
        assert!(receiver.recv().await.is_none());
    }

    #[tokio::test]
    async fn executor_stream_receiver_empty_channel() {
        let (_tx, rx) = mpsc::channel::<Result<StreamEvent, ExecutorError>>(1);
        let mut receiver = ExecutorStreamReceiver::new(rx);

        // Drop sender immediately
        drop(_tx);

        // Should return None
        assert!(receiver.recv().await.is_none());
    }
}
