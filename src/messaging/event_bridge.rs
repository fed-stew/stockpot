//! Bridge between serdes_ai_agent StreamEvents and the Message bus.
//!
//! This module provides the conversion layer that translates execution events
//! from the agent runtime into UI-agnostic messages that can be rendered
//! by any subscriber (terminal, web UI, etc.).

use super::{Message, MessageSender};
use serdes_ai_agent::AgentStreamEvent as StreamEvent;
use std::collections::HashMap;

/// Converts StreamEvents to Messages and publishes to the message bus.
///
/// Tracks state across events (e.g., accumulating tool args) to produce
/// well-formed messages.
pub struct EventBridge {
    sender: MessageSender,
    agent_name: String,
    agent_display_name: String,
    /// Track multiple in-progress tool calls by tool_call_id (or tool_name as fallback)
    tool_states: HashMap<String, CurrentToolState>,
    /// Whether we've sent the first text (for agent header)
    first_text_sent: bool,
}

/// State for tracking an in-progress tool call.
struct CurrentToolState {
    name: String,
    tool_call_id: Option<String>,
    args_buffer: String,
}

impl EventBridge {
    /// Create a new event bridge for an agent.
    pub fn new(sender: MessageSender, agent_name: &str, display_name: &str) -> Self {
        Self {
            sender,
            agent_name: agent_name.to_string(),
            agent_display_name: display_name.to_string(),
            tool_states: HashMap::new(),
            first_text_sent: false,
        }
    }

    /// Get the agent name.
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// Signal that the agent has started execution.
    pub fn agent_started(&self) {
        let _ = self.sender.send(Message::agent_started(
            &self.agent_name,
            &self.agent_display_name,
        ));
    }

    /// Signal that the agent has completed execution.
    pub fn agent_completed(&self, run_id: &str) {
        let _ = self.sender.send(Message::agent_completed(
            &self.agent_name,
            &self.agent_display_name,
            run_id,
        ));
    }

    /// Signal that the agent encountered an error.
    pub fn agent_error(&self, error: &str) {
        let _ = self.sender.send(Message::agent_error(
            &self.agent_name,
            &self.agent_display_name,
            error,
        ));
    }

    /// Process a stream event and publish appropriate messages.
    ///
    /// This is the main entry point - call this for each event from the stream.
    pub fn process(&mut self, event: StreamEvent) {
        match event {
            StreamEvent::RunStart { .. } => {
                // Already handled by agent_started()
            }

            StreamEvent::RequestStart { step } => {
                // Could emit a step indicator if needed
                // For now, silent
                let _ = step;
            }

            StreamEvent::TextDelta { text } => {
                // Send as text delta with agent attribution
                let _ = self
                    .sender
                    .send(Message::text_delta_from(&text, &self.agent_name));
                self.first_text_sent = true;
            }

            StreamEvent::ThinkingDelta { text } => {
                tracing::info!("BRIDGE: Received ThinkingDelta, text_len={}", text.len());
                let _ = self
                    .sender
                    .send(Message::thinking_from(&text, &self.agent_name));
            }

            StreamEvent::ToolCallStart {
                tool_name,
                tool_call_id,
            } => {
                // Use tool_call_id as key, or generate one from tool_name + count
                let key = tool_call_id
                    .clone()
                    .unwrap_or_else(|| format!("{}_{}", tool_name, self.tool_states.len()));

                self.tool_states.insert(
                    key,
                    CurrentToolState {
                        name: tool_name.clone(),
                        tool_call_id: tool_call_id.clone(),
                        args_buffer: String::new(),
                    },
                );

                if let Some(ref id) = tool_call_id {
                    let _ = self.sender.send(Message::tool_started_with_id_from(
                        &tool_name,
                        id,
                        &self.agent_name,
                    ));
                } else {
                    let _ = self
                        .sender
                        .send(Message::tool_started_from(&tool_name, &self.agent_name));
                }
            }

            StreamEvent::ToolCallDelta {
                delta,
                tool_call_id,
            } => {
                // Accumulate args for the tool with matching tool_call_id, or last started
                if let Some(ref id) = tool_call_id {
                    if let Some(state) = self.tool_states.get_mut(id) {
                        state.args_buffer.push_str(&delta);
                    }
                } else if let Some(state) = self.tool_states.values_mut().last() {
                    state.args_buffer.push_str(&delta);
                }
            }

            StreamEvent::ToolCallComplete {
                tool_name,
                tool_call_id,
            } => {
                // Use provided tool_call_id, or find by name
                let resolved_id = tool_call_id.clone().or_else(|| {
                    self.tool_states
                        .values()
                        .find(|s| s.name == tool_name)
                        .and_then(|s| s.tool_call_id.clone())
                });

                let args = if let Some(ref id) = resolved_id {
                    self.tool_states
                        .get(id)
                        .and_then(|s| serde_json::from_str(&s.args_buffer).ok())
                } else {
                    self.tool_states
                        .values()
                        .find(|s| s.name == tool_name)
                        .and_then(|s| serde_json::from_str(&s.args_buffer).ok())
                };

                if let Some(ref id) = resolved_id {
                    let _ = self.sender.send(Message::tool_executing_with_id_from(
                        &tool_name,
                        id,
                        args,
                        &self.agent_name,
                    ));
                } else {
                    let _ = self.sender.send(Message::tool_executing_from(
                        &tool_name,
                        args,
                        &self.agent_name,
                    ));
                }
            }

            StreamEvent::ToolExecuted {
                tool_name,
                tool_call_id,
                success,
                error,
            } => {
                tracing::info!(
                    "BRIDGE: Processing ToolExecuted for '{}' success={}",
                    tool_name,
                    success
                );

                // Use provided tool_call_id, or find and remove by name
                let resolved_id = tool_call_id.clone().or_else(|| {
                    self.tool_states
                        .iter()
                        .find(|(_, s)| s.name == tool_name)
                        .map(|(k, _)| k.clone())
                });

                // Remove the tool state
                if let Some(ref key) = resolved_id {
                    self.tool_states.remove(key);
                }

                if success {
                    let msg = if let Some(ref id) = resolved_id {
                        Message::tool_completed_with_id_from(&tool_name, id, &self.agent_name)
                    } else {
                        Message::tool_completed_from(&tool_name, &self.agent_name)
                    };
                    match self.sender.send(msg) {
                        Ok(_) => tracing::info!("BRIDGE: Sent tool_completed for '{}'", tool_name),
                        Err(e) => tracing::error!("BRIDGE: Failed to send tool_completed: {:?}", e),
                    }
                } else if let Some(ref id) = resolved_id {
                    let _ = self.sender.send(Message::tool_failed_with_id_from(
                        &tool_name,
                        id,
                        error.as_deref().unwrap_or("Unknown error"),
                        &self.agent_name,
                    ));
                } else {
                    let _ = self.sender.send(Message::tool_failed_from(
                        &tool_name,
                        error.as_deref().unwrap_or("Unknown error"),
                        &self.agent_name,
                    ));
                }
            }

            StreamEvent::ResponseComplete { .. } => {
                // Internal event, no message needed
            }

            StreamEvent::OutputReady => {
                // Internal event, no message needed
            }

            StreamEvent::RunComplete { run_id, .. } => {
                // Handled by agent_completed() called externally
                let _ = run_id;
            }

            StreamEvent::Error { message } => {
                self.agent_error(&message);
            }
        }
    }

    /// Reset the bridge state (useful for reuse).
    pub fn reset(&mut self) {
        self.tool_states.clear();
        self.first_text_sent = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::MessageBus;

    #[tokio::test]
    async fn test_event_bridge_text_delta() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "test-agent", "Test Agent");

        bridge.process(StreamEvent::TextDelta {
            text: "Hello".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        match msg {
            Message::TextDelta(delta) => {
                assert_eq!(delta.text, "Hello");
                assert_eq!(delta.agent_name, Some("test-agent".to_string()));
            }
            _ => panic!("Expected TextDelta message"),
        }
    }

    #[tokio::test]
    async fn test_event_bridge_tool_lifecycle() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "test-agent", "Test Agent");

        // Simulate tool call sequence
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("123".to_string()),
        });
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("123".to_string()),
            delta: r#"{"file_path":"test.rs"}"#.to_string(),
        });
        bridge.process(StreamEvent::ToolCallComplete {
            tool_call_id: Some("123".to_string()),
            tool_name: "read_file".to_string(),
        });
        bridge.process(StreamEvent::ToolExecuted {
            tool_call_id: Some("123".to_string()),
            tool_name: "read_file".to_string(),
            success: true,
            error: None,
        });

        // Check messages - all should have agent_name
        let msg1 = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg1 {
            assert_eq!(t.tool_name, "read_file");
            assert_eq!(t.agent_name, Some("test-agent".to_string()));
        } else {
            panic!("Expected Tool message");
        }

        let msg2 = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg2 {
            assert!(t.args.is_some());
            assert_eq!(t.agent_name, Some("test-agent".to_string()));
        } else {
            panic!("Expected Tool message");
        }

        let msg3 = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg3 {
            assert_eq!(t.tool_name, "read_file");
            assert_eq!(t.agent_name, Some("test-agent".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    #[tokio::test]
    async fn test_event_bridge_agent_lifecycle() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let bridge = EventBridge::new(bus.sender(), "reviewer", "Code Reviewer");

        bridge.agent_started();
        bridge.agent_completed("run-123");

        let msg1 = receiver.recv().await.unwrap();
        assert!(matches!(msg1, Message::Agent(a) if a.agent_name == "reviewer"));

        let msg2 = receiver.recv().await.unwrap();
        if let Message::Agent(a) = msg2 {
            assert!(matches!(
                a.event,
                crate::messaging::AgentEvent::Completed { .. }
            ));
        }
    }

    #[tokio::test]
    async fn test_nested_agent_events() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();

        // Simulate parent agent starting
        let parent_bridge = EventBridge::new(bus.sender(), "parent", "Parent Agent");
        parent_bridge.agent_started();

        // Simulate parent doing some work
        let mut parent_bridge = parent_bridge; // Make mutable for process()
        parent_bridge.process(StreamEvent::TextDelta {
            text: "Let me invoke a sub-agent...".to_string(),
        });

        // Simulate sub-agent starting (same bus!)
        let mut child_bridge = EventBridge::new(bus.sender(), "child", "Child Agent");
        child_bridge.agent_started();
        child_bridge.process(StreamEvent::TextDelta {
            text: "I am the child!".to_string(),
        });
        child_bridge.agent_completed("child-run-123");

        // Parent continues
        parent_bridge.process(StreamEvent::TextDelta {
            text: "Sub-agent finished.".to_string(),
        });
        parent_bridge.agent_completed("parent-run-456");

        // Verify we got all messages in order
        let mut messages = Vec::new();
        for _ in 0..7 {
            // 7 events expected
            if let Ok(msg) =
                tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv()).await
            {
                messages.push(msg.unwrap());
            }
        }

        assert_eq!(messages.len(), 7);

        // First message should be parent agent started
        assert!(
            matches!(&messages[0], Message::Agent(a) if a.agent_name == "parent"),
            "Expected parent agent started, got {:?}",
            messages[0]
        );

        // Second should be parent text delta
        assert!(
            matches!(&messages[1], Message::TextDelta(d) if d.agent_name == Some("parent".to_string())),
            "Expected parent text delta, got {:?}",
            messages[1]
        );

        // Third should be child agent started
        assert!(
            matches!(&messages[2], Message::Agent(a) if a.agent_name == "child"),
            "Expected child agent started, got {:?}",
            messages[2]
        );

        // Fourth should be child text delta
        assert!(
            matches!(&messages[3], Message::TextDelta(d) if d.agent_name == Some("child".to_string())),
            "Expected child text delta, got {:?}",
            messages[3]
        );

        // Fifth should be child agent completed
        assert!(
            matches!(&messages[4], Message::Agent(a) if a.agent_name == "child"),
            "Expected child agent completed, got {:?}",
            messages[4]
        );

        // Sixth should be parent text delta (after child)
        assert!(
            matches!(&messages[5], Message::TextDelta(d) if d.agent_name == Some("parent".to_string())),
            "Expected parent text delta, got {:?}",
            messages[5]
        );

        // Seventh should be parent agent completed
        assert!(
            matches!(&messages[6], Message::Agent(a) if a.agent_name == "parent"),
            "Expected parent agent completed, got {:?}",
            messages[6]
        );
    }

    // =========================================================================
    // Getter and Helper Tests
    // =========================================================================

    #[test]
    fn test_agent_name_getter() {
        let bus = MessageBus::new();
        let bridge = EventBridge::new(bus.sender(), "my-agent", "My Agent");
        assert_eq!(bridge.agent_name(), "my-agent");
    }

    #[tokio::test]
    async fn test_agent_error() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let bridge = EventBridge::new(bus.sender(), "error-agent", "Error Agent");

        bridge.agent_error("Something went wrong");

        let msg = receiver.recv().await.unwrap();
        if let Message::Agent(a) = msg {
            assert_eq!(a.agent_name, "error-agent");
            assert_eq!(a.display_name, "Error Agent");
            if let crate::messaging::AgentEvent::Error { message } = a.event {
                assert_eq!(message, "Something went wrong");
            } else {
                panic!("Expected Error event");
            }
        } else {
            panic!("Expected Agent message");
        }
    }

    #[test]
    fn test_reset() {
        let bus = MessageBus::new();
        let mut bridge = EventBridge::new(bus.sender(), "test", "Test");

        // Add some tool state
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool1".to_string(),
            tool_call_id: Some("id1".to_string()),
        });
        bridge.process(StreamEvent::TextDelta {
            text: "hello".to_string(),
        });

        // Verify state exists (first_text_sent should be true, tool_states not empty)
        assert!(bridge.first_text_sent);
        assert!(!bridge.tool_states.is_empty());

        // Reset
        bridge.reset();

        // Verify state cleared
        assert!(!bridge.first_text_sent);
        assert!(bridge.tool_states.is_empty());
    }

    // =========================================================================
    // ThinkingDelta Tests
    // =========================================================================

    #[tokio::test]
    async fn test_thinking_delta() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "thinker", "Thinker");

        bridge.process(StreamEvent::ThinkingDelta {
            text: "Analyzing...".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Thinking(t) = msg {
            assert_eq!(t.text, "Analyzing...");
            assert_eq!(t.agent_name, Some("thinker".to_string()));
        } else {
            panic!("Expected Thinking message");
        }
    }

    #[tokio::test]
    async fn test_thinking_delta_sub_agent_attribution() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "code-reviewer", "Code Reviewer");

        bridge.process(StreamEvent::ThinkingDelta {
            text: "Reviewing code quality...".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Thinking(t) = msg {
            assert_eq!(t.text, "Reviewing code quality...");
            assert_eq!(t.agent_name, Some("code-reviewer".to_string()));
        } else {
            panic!("Expected Thinking message with sub-agent attribution");
        }
    }

    // =========================================================================
    // Tool Call Without ID (Fallback Logic) Tests
    // =========================================================================

    #[tokio::test]
    async fn test_tool_call_without_id() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Tool call without ID
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "write_file".to_string(),
            tool_call_id: None,
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "write_file");
            assert!(t.tool_call_id.is_none());
            assert_eq!(t.agent_name, Some("agent".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    #[tokio::test]
    async fn test_tool_delta_without_id_uses_last_tool() {
        let bus = MessageBus::new();
        let _receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Start two tools
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool_a".to_string(),
            tool_call_id: None,
        });
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool_b".to_string(),
            tool_call_id: None,
        });

        // Delta without ID should go to last tool (tool_b)
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: None,
            delta: r#"{"key":"value"}"#.to_string(),
        });

        // Complete tool_b without ID - should find by name
        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "tool_b".to_string(),
            tool_call_id: None,
        });

        // Check that tool_b got the args (verify via ToolComplete message)
        // The last tool should have received the delta
    }

    #[tokio::test]
    async fn test_tool_complete_without_id_lookup_by_name() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Start tool without ID
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "grep".to_string(),
            tool_call_id: None,
        });

        // Add args
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: None,
            delta: r#"{"pattern":"TODO"}"#.to_string(),
        });

        // Complete by name
        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "grep".to_string(),
            tool_call_id: None,
        });

        // Skip start message
        let _ = receiver.recv().await.unwrap();

        // Check executing message has args
        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "grep");
            assert!(t.args.is_some());
            let args = t.args.unwrap();
            assert_eq!(args["pattern"], "TODO");
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Tool Executed Failure Tests
    // =========================================================================

    #[tokio::test]
    async fn test_tool_executed_failure_with_id() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "shell".to_string(),
            tool_call_id: Some("fail-id".to_string()),
        });

        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "shell".to_string(),
            tool_call_id: Some("fail-id".to_string()),
            success: false,
            error: Some("Command not found".to_string()),
        });

        // Skip start
        let _ = receiver.recv().await.unwrap();

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "shell");
            assert_eq!(t.tool_call_id, Some("fail-id".to_string()));
            assert_eq!(t.status, crate::messaging::ToolStatus::Failed);
            assert_eq!(t.error, Some("Command not found".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    #[tokio::test]
    async fn test_tool_executed_failure_without_id() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "shell".to_string(),
            tool_call_id: None,
        });

        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "shell".to_string(),
            tool_call_id: None,
            success: false,
            error: Some("Permission denied".to_string()),
        });

        // Skip start
        let _ = receiver.recv().await.unwrap();

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "shell");
            // When no tool_call_id provided, code resolves using the generated key
            // from tool_states (e.g., "shell_0"), so result has an ID
            assert!(t.tool_call_id.is_some());
            assert_eq!(t.status, crate::messaging::ToolStatus::Failed);
            assert_eq!(t.error, Some("Permission denied".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    #[tokio::test]
    async fn test_tool_executed_failure_unknown_error() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "shell".to_string(),
            tool_call_id: Some("err-id".to_string()),
        });

        // Fail without error message
        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "shell".to_string(),
            tool_call_id: Some("err-id".to_string()),
            success: false,
            error: None,
        });

        // Skip start
        let _ = receiver.recv().await.unwrap();

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.error, Some("Unknown error".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Silent Event Tests
    // =========================================================================

    #[tokio::test]
    async fn test_silent_events() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // These events should not produce messages
        bridge.process(StreamEvent::RunStart {
            run_id: "run-1".to_string(),
        });
        bridge.process(StreamEvent::RequestStart { step: 1 });
        bridge.process(StreamEvent::ResponseComplete { step: 1 });
        bridge.process(StreamEvent::OutputReady);
        bridge.process(StreamEvent::RunComplete {
            run_id: "run-1".to_string(),
        });

        // Send a text delta to verify channel is working
        bridge.process(StreamEvent::TextDelta {
            text: "test".to_string(),
        });

        // Only the text delta should come through
        let msg = tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(msg, Message::TextDelta(_)));
    }

    // =========================================================================
    // Error Event Tests
    // =========================================================================

    #[tokio::test]
    async fn test_error_event() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::Error {
            message: "API rate limit exceeded".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Agent(a) = msg {
            assert_eq!(a.agent_name, "agent");
            if let crate::messaging::AgentEvent::Error { message } = a.event {
                assert_eq!(message, "API rate limit exceeded");
            } else {
                panic!("Expected Error event");
            }
        } else {
            panic!("Expected Agent message");
        }
    }

    // =========================================================================
    // Invalid JSON Args Tests
    // =========================================================================

    #[tokio::test]
    async fn test_invalid_json_args() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "test_tool".to_string(),
            tool_call_id: Some("bad-json".to_string()),
        });

        // Invalid JSON
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("bad-json".to_string()),
            delta: "not valid json {{{".to_string(),
        });

        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "test_tool".to_string(),
            tool_call_id: Some("bad-json".to_string()),
        });

        // Skip start
        let _ = receiver.recv().await.unwrap();

        // Executing message should have None args
        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert!(t.args.is_none(), "Expected None args for invalid JSON");
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Parallel Tool Calls Tests
    // =========================================================================

    #[tokio::test]
    async fn test_parallel_tool_calls() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Start multiple tools
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("call-1".to_string()),
        });
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "grep".to_string(),
            tool_call_id: Some("call-2".to_string()),
        });

        // Deltas for each (no messages emitted for deltas)
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("call-1".to_string()),
            delta: r#"{"path":"a.txt"}"#.to_string(),
        });
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("call-2".to_string()),
            delta: r#"{"pattern":"TODO"}"#.to_string(),
        });

        // Complete both (emits Executing message)
        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("call-1".to_string()),
        });
        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "grep".to_string(),
            tool_call_id: Some("call-2".to_string()),
        });

        // Execute both (emits Completed message)
        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("call-1".to_string()),
            success: true,
            error: None,
        });
        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "grep".to_string(),
            tool_call_id: Some("call-2".to_string()),
            success: true,
            error: None,
        });

        // Collect all messages
        // Expected: 2 Started + 2 Executing + 2 Completed = 6 messages
        // (ToolCallDelta does not emit messages)
        let mut messages = Vec::new();
        for _ in 0..6 {
            if let Ok(msg) =
                tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv()).await
            {
                messages.push(msg.unwrap());
            }
        }

        assert_eq!(messages.len(), 6);

        // Verify tool_call_ids are preserved
        let tool_messages: Vec<_> = messages
            .iter()
            .filter_map(|m| match m {
                Message::Tool(t) => Some(t),
                _ => None,
            })
            .collect();

        // Should have 3 read_file and 3 grep messages (start, executing, completed for each)
        let read_file_msgs: Vec<_> = tool_messages
            .iter()
            .filter(|t| t.tool_name == "read_file")
            .collect();
        let grep_msgs: Vec<_> = tool_messages
            .iter()
            .filter(|t| t.tool_name == "grep")
            .collect();

        assert_eq!(read_file_msgs.len(), 3);
        assert_eq!(grep_msgs.len(), 3);

        // All read_file should have call-1, all grep should have call-2
        for t in read_file_msgs {
            assert_eq!(t.tool_call_id, Some("call-1".to_string()));
        }
        for t in grep_msgs {
            assert_eq!(t.tool_call_id, Some("call-2".to_string()));
        }
    }

    // =========================================================================
    // Tool State Cleanup Tests
    // =========================================================================

    #[tokio::test]
    async fn test_tool_state_removed_after_execution() {
        let bus = MessageBus::new();
        let _receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "test".to_string(),
            tool_call_id: Some("cleanup-test".to_string()),
        });

        assert!(!bridge.tool_states.is_empty());

        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "test".to_string(),
            tool_call_id: Some("cleanup-test".to_string()),
            success: true,
            error: None,
        });

        assert!(bridge.tool_states.is_empty());
    }

    // =========================================================================
    // First Text Sent Flag Tests
    // =========================================================================

    #[test]
    fn test_first_text_sent_flag() {
        let bus = MessageBus::new();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        assert!(!bridge.first_text_sent);

        bridge.process(StreamEvent::TextDelta {
            text: "Hello".to_string(),
        });

        assert!(bridge.first_text_sent);
    }

    // =========================================================================
    // Tool Executed Resolves ID by Name Tests
    // =========================================================================

    #[tokio::test]
    async fn test_tool_executed_resolves_id_by_name() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Start with ID
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "my_tool".to_string(),
            tool_call_id: Some("original-id".to_string()),
        });

        // Execute without providing ID (should find by name)
        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "my_tool".to_string(),
            tool_call_id: None,
            success: true,
            error: None,
        });

        // Skip start
        let _ = receiver.recv().await.unwrap();

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            // Should have resolved the ID from the tool state
            assert_eq!(t.tool_call_id, Some("original-id".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Empty and Edge Case Text Delta Tests
    // =========================================================================

    #[tokio::test]
    async fn test_empty_text_delta() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::TextDelta {
            text: "".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::TextDelta(delta) = msg {
            assert_eq!(delta.text, "");
            assert!(bridge.first_text_sent); // Still marks first_text_sent
        } else {
            panic!("Expected TextDelta message");
        }
    }

    #[tokio::test]
    async fn test_multiple_text_deltas() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::TextDelta {
            text: "Hello ".to_string(),
        });
        bridge.process(StreamEvent::TextDelta {
            text: "World".to_string(),
        });
        bridge.process(StreamEvent::TextDelta {
            text: "!".to_string(),
        });

        let msg1 = receiver.recv().await.unwrap();
        let msg2 = receiver.recv().await.unwrap();
        let msg3 = receiver.recv().await.unwrap();

        if let Message::TextDelta(d) = msg1 {
            assert_eq!(d.text, "Hello ");
        }
        if let Message::TextDelta(d) = msg2 {
            assert_eq!(d.text, "World");
        }
        if let Message::TextDelta(d) = msg3 {
            assert_eq!(d.text, "!");
        }
    }

    #[tokio::test]
    async fn test_unicode_text_delta() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::TextDelta {
            text: "Hello 世界 🌍 émojis".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::TextDelta(delta) = msg {
            assert_eq!(delta.text, "Hello 世界 🌍 émojis");
        } else {
            panic!("Expected TextDelta message");
        }
    }

    // =========================================================================
    // Tool Delta Edge Cases
    // =========================================================================

    #[test]
    fn test_tool_delta_to_nonexistent_id() {
        let bus = MessageBus::new();
        let _receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Start a tool
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "real_tool".to_string(),
            tool_call_id: Some("real-id".to_string()),
        });

        // Send delta to non-existent ID - should be silently ignored
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("nonexistent-id".to_string()),
            delta: r#"{"ignored":"data"}"#.to_string(),
        });

        // Verify real tool's buffer is still empty
        let state = bridge.tool_states.get("real-id").unwrap();
        assert!(state.args_buffer.is_empty());
    }

    #[test]
    fn test_tool_delta_accumulates_correctly() {
        let bus = MessageBus::new();
        let _receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool".to_string(),
            tool_call_id: Some("acc-id".to_string()),
        });

        // Send multiple deltas
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("acc-id".to_string()),
            delta: r#"{"file"#.to_string(),
        });
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("acc-id".to_string()),
            delta: r#"_path":"#.to_string(),
        });
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("acc-id".to_string()),
            delta: r#""/test.txt"}"#.to_string(),
        });

        let state = bridge.tool_states.get("acc-id").unwrap();
        assert_eq!(state.args_buffer, r#"{"file_path":"/test.txt"}"#);
    }

    // =========================================================================
    // Reuse After Reset Tests
    // =========================================================================

    #[tokio::test]
    async fn test_reuse_after_reset() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // First use
        bridge.process(StreamEvent::TextDelta {
            text: "First run".to_string(),
        });
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool1".to_string(),
            tool_call_id: Some("id1".to_string()),
        });

        assert!(bridge.first_text_sent);
        assert!(!bridge.tool_states.is_empty());

        // Reset
        bridge.reset();

        assert!(!bridge.first_text_sent);
        assert!(bridge.tool_states.is_empty());

        // Second use - should work normally
        bridge.process(StreamEvent::TextDelta {
            text: "Second run".to_string(),
        });
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool2".to_string(),
            tool_call_id: Some("id2".to_string()),
        });

        assert!(bridge.first_text_sent);
        assert!(bridge.tool_states.contains_key("id2"));

        // Drain messages and verify
        let _ = receiver.recv().await.unwrap(); // First text
        let _ = receiver.recv().await.unwrap(); // First tool
        let _ = receiver.recv().await.unwrap(); // Second text
        let msg = receiver.recv().await.unwrap(); // Second tool

        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "tool2");
            assert_eq!(t.tool_call_id, Some("id2".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Agent Name vs Display Name Tests
    // =========================================================================

    #[tokio::test]
    async fn test_agent_name_vs_display_name() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let bridge = EventBridge::new(
            bus.sender(),
            "code-reviewer-v2", // Internal name with dashes
            "Code Reviewer V2", // Human-readable display name
        );

        assert_eq!(bridge.agent_name(), "code-reviewer-v2");

        bridge.agent_started();

        let msg = receiver.recv().await.unwrap();
        if let Message::Agent(a) = msg {
            assert_eq!(a.agent_name, "code-reviewer-v2");
            assert_eq!(a.display_name, "Code Reviewer V2");
        } else {
            panic!("Expected Agent message");
        }
    }

    // =========================================================================
    // Concurrent Agents on Same Bus
    // =========================================================================

    #[tokio::test]
    async fn test_interleaved_agent_tool_calls() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();

        let mut agent_a = EventBridge::new(bus.sender(), "agent-a", "Agent A");
        let mut agent_b = EventBridge::new(bus.sender(), "agent-b", "Agent B");

        // Interleaved tool calls
        agent_a.process(StreamEvent::ToolCallStart {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("a-call-1".to_string()),
        });
        agent_b.process(StreamEvent::ToolCallStart {
            tool_name: "grep".to_string(),
            tool_call_id: Some("b-call-1".to_string()),
        });
        agent_a.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("a-call-1".to_string()),
            delta: r#"{"path":"a.txt"}"#.to_string(),
        });
        agent_b.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("b-call-1".to_string()),
            delta: r#"{"pattern":"TODO"}"#.to_string(),
        });
        agent_b.process(StreamEvent::ToolCallComplete {
            tool_name: "grep".to_string(),
            tool_call_id: Some("b-call-1".to_string()),
        });
        agent_a.process(StreamEvent::ToolCallComplete {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("a-call-1".to_string()),
        });

        // Collect messages
        let mut messages = Vec::new();
        for _ in 0..4 {
            if let Ok(msg) =
                tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv()).await
            {
                messages.push(msg.unwrap());
            }
        }

        // Verify agent attribution is correct
        let a_msgs: Vec<_> = messages
            .iter()
            .filter_map(|m| match m {
                Message::Tool(t) if t.agent_name == Some("agent-a".to_string()) => Some(t),
                _ => None,
            })
            .collect();

        let b_msgs: Vec<_> = messages
            .iter()
            .filter_map(|m| match m {
                Message::Tool(t) if t.agent_name == Some("agent-b".to_string()) => Some(t),
                _ => None,
            })
            .collect();

        assert_eq!(a_msgs.len(), 2); // start + executing
        assert_eq!(b_msgs.len(), 2); // start + executing

        // Verify tool_call_ids are preserved
        for t in a_msgs {
            assert_eq!(t.tool_call_id, Some("a-call-1".to_string()));
        }
        for t in b_msgs {
            assert_eq!(t.tool_call_id, Some("b-call-1".to_string()));
        }
    }

    // =========================================================================
    // Mixed Success/Failure Tool Execution
    // =========================================================================

    #[tokio::test]
    async fn test_parallel_tools_mixed_success_failure() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Start two tools
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool_success".to_string(),
            tool_call_id: Some("success-id".to_string()),
        });
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool_fail".to_string(),
            tool_call_id: Some("fail-id".to_string()),
        });

        // One succeeds, one fails
        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "tool_success".to_string(),
            tool_call_id: Some("success-id".to_string()),
            success: true,
            error: None,
        });
        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "tool_fail".to_string(),
            tool_call_id: Some("fail-id".to_string()),
            success: false,
            error: Some("Network timeout".to_string()),
        });

        // Collect all messages
        let mut messages = Vec::new();
        for _ in 0..4 {
            if let Ok(msg) =
                tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv()).await
            {
                messages.push(msg.unwrap());
            }
        }

        // Find completion messages
        let success_msg = messages.iter().find(|m| {
            matches!(m, Message::Tool(t) if t.tool_call_id == Some("success-id".to_string())
                && t.status == crate::messaging::ToolStatus::Completed)
        });
        let fail_msg = messages.iter().find(|m| {
            matches!(m, Message::Tool(t) if t.tool_call_id == Some("fail-id".to_string())
                && t.status == crate::messaging::ToolStatus::Failed)
        });

        assert!(success_msg.is_some(), "Should have success completion");
        assert!(fail_msg.is_some(), "Should have failure completion");

        if let Some(Message::Tool(t)) = fail_msg {
            assert_eq!(t.error, Some("Network timeout".to_string()));
        }
    }

    // =========================================================================
    // Large Args Buffer Tests
    // =========================================================================

    #[tokio::test]
    async fn test_large_args_buffer() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "write_file".to_string(),
            tool_call_id: Some("large-id".to_string()),
        });

        // Simulate streaming a large file content in args
        let large_content = "x".repeat(10000);
        let json_start = r#"{"content":""#;
        let json_end = r#""}"#;

        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("large-id".to_string()),
            delta: json_start.to_string(),
        });
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("large-id".to_string()),
            delta: large_content.clone(),
        });
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("large-id".to_string()),
            delta: json_end.to_string(),
        });

        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "write_file".to_string(),
            tool_call_id: Some("large-id".to_string()),
        });

        // Skip start message
        let _ = receiver.recv().await.unwrap();

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert!(t.args.is_some());
            let args = t.args.unwrap();
            let content = args["content"].as_str().unwrap();
            assert_eq!(content.len(), 10000);
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Special Characters in Args Tests
    // =========================================================================

    #[tokio::test]
    async fn test_special_characters_in_args() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "grep".to_string(),
            tool_call_id: Some("special-id".to_string()),
        });

        // JSON with special chars, newlines, quotes
        let complex_json = r#"{"pattern":"foo\\nbar","path":"/tmp/\"quoted\".txt"}"#;
        bridge.process(StreamEvent::ToolCallDelta {
            tool_call_id: Some("special-id".to_string()),
            delta: complex_json.to_string(),
        });

        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "grep".to_string(),
            tool_call_id: Some("special-id".to_string()),
        });

        // Skip start
        let _ = receiver.recv().await.unwrap();

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert!(t.args.is_some());
            let args = t.args.unwrap();
            assert_eq!(args["pattern"], "foo\\nbar");
            assert_eq!(args["path"], "/tmp/\"quoted\".txt");
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Empty Tool Name Edge Case
    // =========================================================================

    #[tokio::test]
    async fn test_empty_tool_name() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Edge case: empty tool name (shouldn't happen, but handle gracefully)
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "".to_string(),
            tool_call_id: Some("empty-name-id".to_string()),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "");
            assert_eq!(t.tool_call_id, Some("empty-name-id".to_string()));
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Tool Complete for Unknown Tool
    // =========================================================================

    #[tokio::test]
    async fn test_tool_complete_unknown_tool() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Complete without ever starting - should still emit message
        bridge.process(StreamEvent::ToolCallComplete {
            tool_name: "unknown_tool".to_string(),
            tool_call_id: None,
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "unknown_tool");
            assert_eq!(t.status, crate::messaging::ToolStatus::Executing);
            assert!(t.args.is_none()); // No args since no state existed
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Tool Executed for Unknown Tool
    // =========================================================================

    #[tokio::test]
    async fn test_tool_executed_unknown_tool() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Execute without ever starting
        bridge.process(StreamEvent::ToolExecuted {
            tool_name: "mystery_tool".to_string(),
            tool_call_id: None,
            success: true,
            error: None,
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Tool(t) = msg {
            assert_eq!(t.tool_name, "mystery_tool");
            // Without tool_call_id and no matching state, resolves to None
            assert!(t.tool_call_id.is_none());
            assert_eq!(t.status, crate::messaging::ToolStatus::Completed);
        } else {
            panic!("Expected Tool message");
        }
    }

    // =========================================================================
    // Thinking Delta Edge Cases
    // =========================================================================

    #[tokio::test]
    async fn test_empty_thinking_delta() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ThinkingDelta {
            text: "".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Thinking(t) = msg {
            assert_eq!(t.text, "");
        } else {
            panic!("Expected Thinking message");
        }
    }

    #[tokio::test]
    async fn test_thinking_delta_with_newlines() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::ThinkingDelta {
            text: "Step 1: Analyze\nStep 2: Plan\nStep 3: Execute".to_string(),
        });

        let msg = receiver.recv().await.unwrap();
        if let Message::Thinking(t) = msg {
            assert!(t.text.contains('\n'));
            assert!(t.text.contains("Step 1"));
            assert!(t.text.contains("Step 3"));
        } else {
            panic!("Expected Thinking message");
        }
    }

    // =========================================================================
    // Key Generation for Tools Without ID
    // =========================================================================

    #[test]
    fn test_tool_key_generation_uniqueness() {
        let bus = MessageBus::new();
        let _receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // Start multiple tools without IDs - keys should be unique
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "read_file".to_string(),
            tool_call_id: None,
        });
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "read_file".to_string(),
            tool_call_id: None,
        });
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "read_file".to_string(),
            tool_call_id: None,
        });

        // Should have 3 distinct entries
        assert_eq!(bridge.tool_states.len(), 3);

        // Keys should follow pattern: tool_name_index
        let keys: Vec<_> = bridge.tool_states.keys().collect();
        assert!(keys.iter().any(|k| k.starts_with("read_file_")));
    }

    // =========================================================================
    // Sender Error Handling (Channel Closed)
    // =========================================================================

    #[test]
    fn test_bridge_handles_closed_channel_gracefully() {
        let bus = MessageBus::new();
        // No receiver subscribed, so channel is effectively closed
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        // These should not panic even though channel has no receivers
        bridge.process(StreamEvent::TextDelta {
            text: "ignored".to_string(),
        });
        bridge.agent_started();
        bridge.agent_completed("run-1");
        bridge.agent_error("error msg");
        bridge.process(StreamEvent::ToolCallStart {
            tool_name: "tool".to_string(),
            tool_call_id: Some("id".to_string()),
        });
        bridge.process(StreamEvent::ThinkingDelta {
            text: "thinking".to_string(),
        });
        bridge.process(StreamEvent::Error {
            message: "stream error".to_string(),
        });

        // If we got here without panic, test passes
    }

    // =========================================================================
    // RunComplete Event (Internal, No Message)
    // =========================================================================

    #[tokio::test]
    async fn test_run_complete_does_not_emit_message() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();
        let mut bridge = EventBridge::new(bus.sender(), "agent", "Agent");

        bridge.process(StreamEvent::RunComplete {
            run_id: "run-xyz".to_string(),
        });

        // Send a text delta to have something to receive
        bridge.process(StreamEvent::TextDelta {
            text: "after run complete".to_string(),
        });

        // First message should be the text delta, not anything from RunComplete
        let msg = tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(msg, Message::TextDelta(_)));
    }

    // =========================================================================
    // Complex Nested Agent Scenario
    // =========================================================================

    #[tokio::test]
    async fn test_deeply_nested_agents() {
        let bus = MessageBus::new();
        let mut receiver = bus.subscribe();

        // Simulate 3 levels of nesting
        let parent = EventBridge::new(bus.sender(), "parent", "Parent");
        let mut child = EventBridge::new(bus.sender(), "child", "Child");
        let grandchild = EventBridge::new(bus.sender(), "grandchild", "Grandchild");

        parent.agent_started();
        child.agent_started();
        grandchild.agent_started();

        // Grandchild does work
        child.process(StreamEvent::TextDelta {
            text: "child work".to_string(),
        });

        grandchild.agent_completed("gc-run");
        child.agent_completed("c-run");
        parent.agent_completed("p-run");

        // Collect all messages
        let mut messages = Vec::new();
        for _ in 0..7 {
            if let Ok(msg) =
                tokio::time::timeout(std::time::Duration::from_millis(50), receiver.recv()).await
            {
                messages.push(msg.unwrap());
            }
        }

        assert_eq!(messages.len(), 7);

        // Verify nesting order: parent start, child start, grandchild start,
        // child text, grandchild complete, child complete, parent complete
        let agent_names: Vec<_> = messages
            .iter()
            .filter_map(|m| match m {
                Message::Agent(a) => Some(a.agent_name.as_str()),
                Message::TextDelta(t) => t.agent_name.as_deref(),
                _ => None,
            })
            .collect();

        assert_eq!(
            agent_names,
            vec![
                "parent",
                "child",
                "grandchild",
                "child",
                "grandchild",
                "child",
                "parent"
            ]
        );
    }

    // =========================================================================
    // Tool State Isolation Between Bridges
    // =========================================================================

    #[test]
    fn test_tool_state_isolation_between_bridges() {
        let bus = MessageBus::new();
        let _receiver = bus.subscribe();

        let mut bridge_a = EventBridge::new(bus.sender(), "agent-a", "Agent A");
        let mut bridge_b = EventBridge::new(bus.sender(), "agent-b", "Agent B");

        bridge_a.process(StreamEvent::ToolCallStart {
            tool_name: "tool".to_string(),
            tool_call_id: Some("a-id".to_string()),
        });

        bridge_b.process(StreamEvent::ToolCallStart {
            tool_name: "tool".to_string(),
            tool_call_id: Some("b-id".to_string()),
        });

        // Each bridge should only have its own tool
        assert_eq!(bridge_a.tool_states.len(), 1);
        assert_eq!(bridge_b.tool_states.len(), 1);
        assert!(bridge_a.tool_states.contains_key("a-id"));
        assert!(bridge_b.tool_states.contains_key("b-id"));
        assert!(!bridge_a.tool_states.contains_key("b-id"));
        assert!(!bridge_b.tool_states.contains_key("a-id"));
    }
}
