//! Streaming execution internals for AgentExecutor.
//!
//! Contains the complex streaming logic that processes events and
//! reconstructs message history from stream events.

use std::collections::VecDeque;
use std::sync::Arc;

use futures::StreamExt;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

use serdes_ai_agent::{agent, RunOptions};
use serdes_ai_core::messages::ToolCallArgs;
use serdes_ai_core::messages::{UserContent, UserContentPart};
use serdes_ai_core::{
    ModelRequest, ModelRequestPart, ModelResponse, ModelResponsePart, TextPart, ToolCallPart,
    ToolReturnPart,
};
use serdes_ai_tools::{Tool, ToolDefinition};

use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::EventBridge;
use crate::tools::SpotToolRegistry;

use super::adapters::{ArcModel, RecordingToolExecutor, ToolExecutorAdapter};
use super::mcp::McpToolExecutor;
use super::model_factory::get_model;
use super::sub_agents::{InvokeAgentExecutor, ListAgentsExecutor};
use super::types::{ExecutorError, ExecutorStreamReceiver};
use super::{AgentExecutor, SpotAgent, StreamEvent};

/// Helper struct to track in-progress tool calls during streaming.
struct RawToolCall {
    tool_name: String,
    tool_call_id: Option<String>,
    args_buffer: String,
}

impl<'a> AgentExecutor<'a> {
    /// Process a stream of events and accumulate results.
    ///
    /// Returns (accumulated_text, final_run_id, messages).
    pub(super) async fn process_stream(
        &self,
        stream: &mut ExecutorStreamReceiver,
        bridge: &mut EventBridge,
        mut messages: Vec<ModelRequest>,
        model_name: &str,
        tool_return_recorder: &Arc<Mutex<Vec<ToolReturnPart>>>,
    ) -> Result<(String, Option<String>, Vec<ModelRequest>), ExecutorError> {
        // Accumulate text for the final output
        let mut accumulated_text = String::new();
        let mut final_run_id: Option<String> = None;

        // Track per-response state so we can rebuild `ModelResponse` parts.
        let mut current_response_text = String::new();
        let mut completed_tool_calls: Vec<RawToolCall> = Vec::new();
        let mut in_progress_tool_call: Option<RawToolCall> = None;

        // Track tool return parts emitted by tool executors.
        let mut expected_tool_returns: usize = 0;
        let mut tool_return_index: usize = 0;
        let mut pending_tool_returns: Vec<ToolReturnPart> = Vec::new();
        let mut pending_tool_calls: VecDeque<(String, Option<String>)> = VecDeque::new();

        // Process all events through the bridge
        while let Some(event_result) = stream.recv().await {
            match event_result {
                Ok(event) => {
                    let tool_executed_info = match &event {
                        StreamEvent::ToolExecuted {
                            tool_name,
                            success,
                            error,
                            ..
                        } => Some((tool_name.clone(), *success, error.clone())),
                        _ => None,
                    };

                    match &event {
                        StreamEvent::RequestStart { .. } => {
                            current_response_text.clear();
                            completed_tool_calls.clear();
                            in_progress_tool_call = None;
                        }
                        StreamEvent::TextDelta { text } => {
                            accumulated_text.push_str(text);
                            current_response_text.push_str(text);
                        }
                        StreamEvent::ToolCallStart {
                            tool_name,
                            tool_call_id,
                        } => {
                            if let Some(tc) = in_progress_tool_call.take() {
                                completed_tool_calls.push(tc);
                            }
                            in_progress_tool_call = Some(RawToolCall {
                                tool_name: tool_name.clone(),
                                tool_call_id: tool_call_id.clone(),
                                args_buffer: String::new(),
                            });
                        }
                        StreamEvent::ToolCallDelta { delta, .. } => {
                            if let Some(tc) = in_progress_tool_call.as_mut() {
                                tc.args_buffer.push_str(delta);
                            }
                        }
                        StreamEvent::ResponseComplete { .. } => {
                            if let Some(tc) = in_progress_tool_call.take() {
                                completed_tool_calls.push(tc);
                            }

                            pending_tool_calls = completed_tool_calls
                                .iter()
                                .map(|tc| (tc.tool_name.clone(), tc.tool_call_id.clone()))
                                .collect();

                            let mut response_parts: Vec<ModelResponsePart> = Vec::new();

                            if !current_response_text.is_empty() {
                                response_parts.push(ModelResponsePart::Text(TextPart::new(
                                    current_response_text.clone(),
                                )));
                            }

                            for tc in completed_tool_calls.drain(..) {
                                let mut part = ToolCallPart::new(
                                    tc.tool_name,
                                    ToolCallArgs::from(tc.args_buffer),
                                );
                                if let Some(id) = tc.tool_call_id {
                                    part = part.with_tool_call_id(id);
                                }
                                response_parts.push(ModelResponsePart::ToolCall(part));
                            }

                            if !response_parts.is_empty() {
                                let response = ModelResponse::with_parts(response_parts)
                                    .with_model_name(model_name.to_string());

                                let mut response_req = ModelRequest::new();
                                response_req
                                    .parts
                                    .push(ModelRequestPart::ModelResponse(Box::new(response)));
                                messages.push(response_req);
                            }

                            expected_tool_returns = pending_tool_calls.len();
                            pending_tool_returns.clear();
                            current_response_text.clear();
                        }
                        StreamEvent::RunComplete { run_id } => {
                            final_run_id = Some(run_id.clone());
                        }
                        _ => {}
                    }

                    // Tool return payloads aren't present in stream events, so we stitch them
                    // in from a recorder wrapped around tool executors.
                    if let Some((tool_name, success, error)) = tool_executed_info {
                        if expected_tool_returns > 0 {
                            let tool_call_id =
                                pending_tool_calls.pop_front().and_then(|(_, id)| id);

                            let mut part = {
                                let next_part = {
                                    let recorded = tool_return_recorder.lock().await;
                                    recorded.get(tool_return_index).cloned()
                                };

                                if let Some(part) = next_part {
                                    tool_return_index += 1;
                                    part
                                } else {
                                    let msg = error.unwrap_or_else(|| {
                                        if success {
                                            "Tool executed but no tool return was recorded"
                                                .to_string()
                                        } else {
                                            "Tool failed".to_string()
                                        }
                                    });
                                    ToolReturnPart::error(&tool_name, msg)
                                }
                            };

                            if part.tool_call_id.is_none() {
                                if let Some(id) = tool_call_id {
                                    part = part.with_tool_call_id(id);
                                }
                            }

                            pending_tool_returns.push(part);

                            if pending_tool_returns.len() == expected_tool_returns {
                                let mut tool_req = ModelRequest::new();
                                for part in pending_tool_returns.drain(..) {
                                    tool_req.parts.push(ModelRequestPart::ToolReturn(part));
                                }
                                messages.push(tool_req);
                                expected_tool_returns = 0;
                            }
                        }
                    }

                    bridge.process(event);
                }
                Err(e) => {
                    bridge.agent_error(&e.to_string());
                    return Err(e);
                }
            }
        }

        // Flush any tool returns we managed to capture
        if !pending_tool_returns.is_empty() {
            let mut tool_req = ModelRequest::new();
            for part in pending_tool_returns {
                tool_req.parts.push(ModelRequestPart::ToolReturn(part));
            }
            messages.push(tool_req);
        }

        Ok((accumulated_text, final_run_id, messages))
    }

    /// Internal streaming execution with full control over user content.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn execute_stream_internal(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: UserContent,
        message_history: Option<Vec<ModelRequest>>,
        tool_registry: &SpotToolRegistry,
        mcp_manager: &McpManager,
        tool_return_recorder: Option<Arc<Mutex<Vec<ToolReturnPart>>>>,
    ) -> Result<ExecutorStreamReceiver, ExecutorError> {
        // Get the model (handles OAuth models and custom endpoints)
        let model = get_model(self.db, model_name, self.registry).await?;

        // Get original tool list (before filtering) to check for special tools
        let original_tools = spot_agent.available_tools();
        let wants_invoke = self.wants_invoke_agent(&original_tools);
        let wants_list = self.wants_list_agents(&original_tools);

        // Get the tools this agent should have access to (filtered by settings)
        let tool_names = self.filter_tools(original_tools);
        let tools = tool_registry.tools_by_name(&tool_names);

        // Collect tool definitions and Arc references
        let mut tool_data: Vec<(ToolDefinition, Arc<dyn Tool + Send + Sync>)> =
            tools.into_iter().map(|t| (t.definition(), t)).collect();

        // Collect MCP tools from running servers (filtered by agent attachments)
        let mcp_tool_calls = self
            .collect_mcp_tools(mcp_manager, Some(spot_agent.name()))
            .await;
        tool_data.extend(mcp_tool_calls);

        // Prepare data for the spawned task
        let system_prompt = spot_agent.system_prompt();
        let model_name_owned = model_name.to_string();
        let db_path = self.db.path().to_path_buf();
        let bus = self.bus.clone();
        let tool_return_recorder = tool_return_recorder.clone();
        let (tx, rx) = mpsc::channel(32);

        // Log what we're sending to serdesAI
        match &prompt {
            UserContent::Text(t) => {
                debug!(text_len = t.len(), "Sending text prompt to serdesAI")
            }
            UserContent::Parts(parts) => {
                let image_count = parts
                    .iter()
                    .filter(|p| matches!(p, UserContentPart::Image { .. }))
                    .count();
                info!(
                    parts_count = parts.len(),
                    image_count, "Sending multimodal prompt to serdesAI"
                );
            }
        }

        debug!(tool_count = tool_data.len(), "Spawning streaming task");

        // Spawn a task that owns the agent and sends events through the channel
        tokio::spawn(async move {
            debug!("Streaming task started");

            let wrapped_model = ArcModel(model);

            // Build the serdesAI agent
            debug!("Building serdesAI agent");
            let mut builder = agent(wrapped_model)
                .system_prompt(system_prompt)
                .temperature(0.7)
                .max_tokens(16384);

            match tool_return_recorder {
                Some(recorder) => {
                    // Register tools with recording executors
                    for (def, tool) in tool_data {
                        debug!(tool_name = %def.name, "Registering tool");
                        builder = builder.tool_with_executor(
                            def,
                            RecordingToolExecutor::new(
                                ToolExecutorAdapter::new(tool),
                                recorder.clone(),
                            ),
                        );
                    }

                    // Add invoke_agent with custom executor (has database access)
                    if wants_invoke {
                        let invoke_executor = InvokeAgentExecutor::new_with_path(
                            db_path.clone(),
                            &model_name_owned,
                            bus.clone(),
                        );
                        builder = builder.tool_with_executor(
                            InvokeAgentExecutor::definition(),
                            RecordingToolExecutor::new(invoke_executor, recorder.clone()),
                        );
                    }

                    // Add list_agents with custom executor
                    if wants_list {
                        builder = builder.tool_with_executor(
                            ListAgentsExecutor::definition(),
                            RecordingToolExecutor::new(
                                ListAgentsExecutor::new_with_path(db_path.clone()),
                                recorder.clone(),
                            ),
                        );
                    }
                }
                None => {
                    // Register tools with real executors
                    for (def, tool) in tool_data {
                        debug!(tool_name = %def.name, "Registering tool");
                        builder = builder.tool_with_executor(def, ToolExecutorAdapter::new(tool));
                    }

                    // Add invoke_agent with custom executor (has database access)
                    if wants_invoke {
                        let invoke_executor = InvokeAgentExecutor::new_with_path(
                            db_path.clone(),
                            &model_name_owned,
                            bus.clone(),
                        );
                        builder = builder
                            .tool_with_executor(InvokeAgentExecutor::definition(), invoke_executor);
                    }

                    // Add list_agents with custom executor
                    if wants_list {
                        builder = builder.tool_with_executor(
                            ListAgentsExecutor::definition(),
                            ListAgentsExecutor::new_with_path(db_path.clone()),
                        );
                    }
                }
            }

            let serdes_agent = builder.build();
            debug!("Agent built successfully");

            // Set up run options
            let history_len = message_history.as_ref().map(|h| h.len()).unwrap_or(0);
            debug!(history_messages = history_len, "Setting up run options");

            let options = match message_history {
                Some(history) => RunOptions::new().message_history(history),
                None => RunOptions::new(),
            };

            // Use real streaming from serdesAI
            debug!("Calling run_stream_with_options");

            match serdes_agent
                .run_stream_with_options(prompt, (), options)
                .await
            {
                Ok(mut stream) => {
                    debug!("Stream started, forwarding events");
                    let mut event_count = 0u32;

                    // Forward all events from the stream
                    while let Some(event_result) = stream.next().await {
                        event_count += 1;
                        match event_result {
                            Ok(event) => {
                                debug!(event_num = event_count, "Received stream event");
                                if tx.send(Ok(event)).await.is_err() {
                                    warn!("Receiver dropped, stopping stream");
                                    break;
                                }
                            }
                            Err(e) => {
                                let error_str = e.to_string();
                                error!(error = %error_str, "Stream error");
                                log_http_error(&error_str);

                                let _ = tx
                                    .send(Ok(StreamEvent::Error {
                                        message: error_str.clone(),
                                    }))
                                    .await;
                                let _ = tx.send(Err(ExecutorError::Execution(error_str))).await;
                                break;
                            }
                        }
                    }
                    debug!(total_events = event_count, "Stream completed");
                }
                Err(e) => {
                    let error_str = e.to_string();
                    error!(error = %error_str, "Failed to start stream");
                    log_http_error(&error_str);

                    // Send error event
                    let _ = tx
                        .send(Ok(StreamEvent::Error {
                            message: error_str.clone(),
                        }))
                        .await;
                    let _ = tx.send(Err(ExecutorError::Execution(error_str))).await;
                }
            }
            debug!("Streaming task exiting");
        });

        Ok(ExecutorStreamReceiver::new(rx))
    }
}

/// Log common HTTP error patterns for debugging.
fn log_http_error(error_str: &str) {
    if error_str.contains("status: 400") {
        error!("HTTP 400 Bad Request - likely invalid model name or request format");
    } else if error_str.contains("status: 401") {
        error!("HTTP 401 Unauthorized - token may be expired or invalid");
    } else if error_str.contains("status: 403") {
        error!("HTTP 403 Forbidden - token may not have required permissions");
    } else if error_str.contains("status: 404") {
        error!("HTTP 404 Not Found - model name may be invalid");
    } else if error_str.contains("status: 429") {
        error!("HTTP 429 Rate Limited - too many requests");
    } else if error_str.contains("status: 5") {
        error!("HTTP 5xx Server Error - API issue");
    }

    if error_str.contains("body:") {
        error!("API Error Body: {}", error_str);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // log_http_error Tests
    // =========================================================================

    // Note: log_http_error doesn't return anything, so we test it doesn't panic
    // and verify correct behavior via tracing subscriber if needed.

    #[test]
    fn test_log_http_error_400_bad_request() {
        // Should not panic
        log_http_error("HTTP error status: 400 Bad Request");
    }

    #[test]
    fn test_log_http_error_401_unauthorized() {
        log_http_error("HTTP error status: 401 Unauthorized");
    }

    #[test]
    fn test_log_http_error_403_forbidden() {
        log_http_error("HTTP error status: 403 Forbidden");
    }

    #[test]
    fn test_log_http_error_404_not_found() {
        log_http_error("HTTP error status: 404 Not Found");
    }

    #[test]
    fn test_log_http_error_429_rate_limited() {
        log_http_error("HTTP error status: 429 Too Many Requests");
    }

    #[test]
    fn test_log_http_error_500_server_error() {
        log_http_error("HTTP error status: 500 Internal Server Error");
    }

    #[test]
    fn test_log_http_error_502_bad_gateway() {
        log_http_error("HTTP error status: 502 Bad Gateway");
    }

    #[test]
    fn test_log_http_error_503_service_unavailable() {
        log_http_error("HTTP error status: 503 Service Unavailable");
    }

    #[test]
    fn test_log_http_error_with_body() {
        log_http_error("Error body: {\"error\": \"invalid_api_key\"}");
    }

    #[test]
    fn test_log_http_error_combined_status_and_body() {
        log_http_error("HTTP error status: 401, body: {\"error\": \"unauthorized\"}");
    }

    #[test]
    fn test_log_http_error_unknown_error() {
        // Should handle unknown errors gracefully (no panic)
        log_http_error("Connection refused");
    }

    #[test]
    fn test_log_http_error_empty_string() {
        log_http_error("");
    }

    // =========================================================================
    // RawToolCall Tests (struct is private, but we can test it within module)
    // =========================================================================

    #[test]
    fn test_raw_tool_call_creation() {
        let tc = RawToolCall {
            tool_name: "read_file".to_string(),
            tool_call_id: Some("call_123".to_string()),
            args_buffer: String::new(),
        };

        assert_eq!(tc.tool_name, "read_file");
        assert_eq!(tc.tool_call_id, Some("call_123".to_string()));
        assert!(tc.args_buffer.is_empty());
    }

    #[test]
    fn test_raw_tool_call_without_id() {
        let tc = RawToolCall {
            tool_name: "shell_command".to_string(),
            tool_call_id: None,
            args_buffer: "{\"command\": \"ls\"}".to_string(),
        };

        assert_eq!(tc.tool_name, "shell_command");
        assert!(tc.tool_call_id.is_none());
        assert_eq!(tc.args_buffer, "{\"command\": \"ls\"}");
    }

    #[test]
    fn test_raw_tool_call_args_buffer_append() {
        let mut tc = RawToolCall {
            tool_name: "test".to_string(),
            tool_call_id: None,
            args_buffer: String::new(),
        };

        tc.args_buffer.push_str("{\"key\":");
        tc.args_buffer.push_str(" \"value\"}");

        assert_eq!(tc.args_buffer, "{\"key\": \"value\"}");
    }
}
