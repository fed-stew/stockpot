//! Agent executor - runs agents using serdesAI's Agent API.
//!
//! This module provides the execution layer for SpotAgents, using
//! serdesAI's agent loop with proper tool calling and streaming support.

use crate::agents::SpotAgent;
use tracing::{debug, info, error, warn};
use crate::auth;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::tools::registry::SpotToolRegistry;

use serdes_ai_agent::{agent, RunOptions};
use serdes_ai_core::{ModelRequest, ModelResponse, ModelSettings};
use serdes_ai_models::{infer_model, Model, ModelError, ModelRequestParameters, StreamedResponse, ModelProfile};
use serdes_ai_tools::{Tool, ToolDefinition, ToolReturn, ToolError, RunContext};

use async_trait::async_trait;
use futures::StreamExt;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::mpsc;

// Re-export stream event for consumers
pub use serdes_ai_agent::AgentStreamEvent as StreamEvent;

/// Wrapper to make Arc<dyn Model> implement Model.
/// 
/// This allows us to use dynamically dispatched models with serdesAI's
/// agent builder, which requires a concrete Model type.
struct ArcModel(Arc<dyn Model>);

#[async_trait]
impl Model for ArcModel {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn system(&self) -> &str {
        self.0.system()
    }

    fn identifier(&self) -> String {
        self.0.identifier()
    }

    async fn request(
        &self,
        messages: &[ModelRequest],
        settings: &ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<ModelResponse, ModelError> {
        self.0.request(messages, settings, params).await
    }

    async fn request_stream(
        &self,
        messages: &[ModelRequest],
        settings: &ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<StreamedResponse, ModelError> {
        self.0.request_stream(messages, settings, params).await
    }

    fn profile(&self) -> &ModelProfile {
        self.0.profile()
    }

    async fn count_tokens(&self, messages: &[ModelRequest]) -> Result<u64, ModelError> {
        self.0.count_tokens(messages).await
    }
}

/// Wrapper that adapts an `Arc<dyn Tool>` to work as a `ToolExecutor<()>`.
/// 
/// This bridges our Tool implementations (which use `call()`) to
/// serdesAI's executor interface (which uses `execute()`).
struct ToolExecutorAdapter {
    tool: Arc<dyn Tool + Send + Sync>,
}

impl ToolExecutorAdapter {
    fn new(tool: Arc<dyn Tool + Send + Sync>) -> Self {
        Self { tool }
    }
}

#[async_trait]
impl serdes_ai_agent::ToolExecutor<()> for ToolExecutorAdapter {
    async fn execute(
        &self,
        args: JsonValue,
        ctx: &serdes_ai_agent::RunContext<()>,
    ) -> Result<ToolReturn, ToolError> {
        // Convert serdes_ai_agent::RunContext to serdes_ai_tools::RunContext
        let tool_ctx = RunContext::minimal(&ctx.model_name);
        self.tool.call(&tool_ctx, args).await
    }
}

/// Executor for running SpotAgents with serdesAI's agent loop.
/// 
/// This replaces raw model calls with proper agent execution including:
/// - Tool calling and execution loop
/// - Streaming support
/// - Message history management
/// - Retry logic
pub struct AgentExecutor<'a> {
    db: &'a Database,
}

impl<'a> AgentExecutor<'a> {
    /// Create a new executor with database access (for OAuth tokens).
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    /// Execute an agent with a prompt (blocking mode).
    /// 
    /// This runs the full agent loop including tool calls until completion.
    /// Returns the final output and message history for context.
    pub async fn execute(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: &str,
        message_history: Option<Vec<ModelRequest>>,
        registry: &SpotToolRegistry,
        _mcp_manager: &McpManager,
    ) -> Result<ExecutorResult, ExecutorError> {
        // Get the model (handles OAuth models)
        let model = get_model(self.db, model_name).await?;
        let wrapped_model = ArcModel(model);
        
        // Get the tools this agent should have access to
        let tool_names = spot_agent.available_tools();
        let tools = registry.tools_by_name(&tool_names);
        
        // Build the serdesAI agent
        let mut builder = agent(wrapped_model)
            .system_prompt(spot_agent.system_prompt())
            .temperature(0.7)
            .max_tokens(4096);
        
        // Register built-in tools with real executors
        for tool in tools {
            let def = tool.definition();
            builder = builder.tool_with_executor(
                def,
                ToolExecutorAdapter::new(Arc::clone(&tool)),
            );
        }
        
        // Add MCP tools
        let mcp_tools = self.collect_mcp_tools(_mcp_manager).await;
        for (def, tool) in mcp_tools {
            builder = builder.tool_with_executor(
                def,
                ToolExecutorAdapter::new(tool),
            );
        }
        
        let serdes_agent = builder.build();
        
        // Set up run options with message history if provided
        let options = match message_history {
            Some(history) => RunOptions::new().message_history(history),
            None => RunOptions::new(),
        };
        
        // Run the agent
        let result = serdes_agent.run_with_options(prompt, (), options).await
            .map_err(|e| ExecutorError::Execution(e.to_string()))?;
        
        Ok(ExecutorResult {
            output: result.output.clone(),
            messages: result.messages,
            run_id: result.run_id,
        })
    }

    /// Execute an agent with streaming output.
    /// 
    /// Returns a stream receiver for consuming events in real-time.
    /// Uses a channel internally to handle the lifetime issues with agent streams.
    /// 
    /// # Example
    /// ```ignore
    /// let registry = SpotToolRegistry::new();
    /// let mut stream = executor.execute_stream(agent, model, prompt, None, &registry).await?;
    /// while let Some(event) = stream.recv().await {
    ///     match event? {
    ///         StreamEvent::TextDelta { text } => print!("{}", text),
    ///         StreamEvent::ToolCallStart { tool_name, .. } => println!("🔧 Calling: {}", tool_name),
    ///         _ => {}
    ///     }
    /// }
    /// ```
    pub async fn execute_stream(
        &self,
        spot_agent: &dyn SpotAgent,
        model_name: &str,
        prompt: &str,
        message_history: Option<Vec<ModelRequest>>,
        registry: &SpotToolRegistry,
        mcp_manager: &McpManager,
    ) -> Result<ExecutorStreamReceiver, ExecutorError> {
        // Get the model (handles OAuth models)
        let model = get_model(self.db, model_name).await?;
        
        // Get the tools this agent should have access to
        let tool_names = spot_agent.available_tools();
        let tools = registry.tools_by_name(&tool_names);
        
        // Collect tool definitions and Arc references
        // We need to move these into the spawned task
        let mut tool_data: Vec<(ToolDefinition, Arc<dyn Tool + Send + Sync>)> = tools
            .into_iter()
            .map(|t| (t.definition(), t))
            .collect();
        
        // Collect MCP tools from running servers
        let mcp_tool_calls = self.collect_mcp_tools(mcp_manager).await;
        tool_data.extend(mcp_tool_calls);
        
        // Prepare data for the spawned task
        let system_prompt = spot_agent.system_prompt();
        let prompt = prompt.to_string();
        let (tx, rx) = mpsc::channel(32);
        
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
                .max_tokens(4096);
            
            // Register tools with real executors
            for (def, tool) in tool_data {
                debug!(tool_name = %def.name, "Registering tool");
                builder = builder.tool_with_executor(
                    def,
                    ToolExecutorAdapter::new(tool),
                );
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
            debug!(prompt_len = prompt.len(), "Calling run_stream_with_options");
            
            match serdes_agent.run_stream_with_options(prompt, (), options).await {
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
                                
                                // Log common error patterns
                                if error_str.contains("status: 400") {
                                    error!("HTTP 400 Bad Request - likely invalid model name");
                                } else if error_str.contains("status: 401") {
                                    error!("HTTP 401 Unauthorized - token may be expired");
                                } else if error_str.contains("status: 404") {
                                    error!("HTTP 404 Not Found - model name may be invalid");
                                }
                                
                                let _ = tx.send(Ok(StreamEvent::Error { 
                                    message: error_str.clone() 
                                })).await;
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
                    
                    // Check for common error patterns
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
                    
                    // Log the full error body if present
                    if error_str.contains("body:") {
                        error!("API Error Body: {}", error_str);
                    }
                    
                    // Send error event
                    let _ = tx.send(Ok(StreamEvent::Error { 
                        message: error_str.clone() 
                    })).await;
                    let _ = tx.send(Err(ExecutorError::Execution(error_str))).await;
                }
            }
            debug!("Streaming task exiting");
        });
        
        Ok(ExecutorStreamReceiver { rx })
    }
}



/// Result from agent execution.
#[derive(Debug, Clone)]
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
    /// Receive the next event from the stream.
    /// 
    /// Returns `None` when the stream is complete.
    pub async fn recv(&mut self) -> Option<Result<StreamEvent, ExecutorError>> {
        self.rx.recv().await
    }
}

/// Get a model by name, handling OAuth models specially.
/// 
/// OAuth models (chatgpt-*, claude-code-*) require token refresh,
/// so we handle them separately from standard API key models.
pub async fn get_model(db: &Database, model_name: &str) -> Result<Arc<dyn Model>, ExecutorError> {
    debug!(model_name = %model_name, "get_model called");
    
    // Check for OAuth models
    if model_name.starts_with("chatgpt-") || model_name.starts_with("chatgpt_") {
        debug!("Detected ChatGPT OAuth model, fetching token...");
        let model = auth::get_chatgpt_model(db, model_name).await
            .map_err(|e| {
                error!(error = %e, "Failed to get ChatGPT model");
                ExecutorError::Auth(e.to_string())
            })?;
        info!(model_id = %model.identifier(), "ChatGPT OAuth model ready");
        return Ok(Arc::new(model));
    }
    
    if model_name.starts_with("claude-code-") || model_name.starts_with("claude_code_") {
        debug!("Detected Claude Code OAuth model, fetching token...");
        let model = auth::get_claude_code_model(db, model_name).await
            .map_err(|e| {
                error!(error = %e, "Failed to get Claude Code model");
                ExecutorError::Auth(e.to_string())
            })?;
        info!(model_id = %model.identifier(), "Claude Code OAuth model ready");
        return Ok(Arc::new(model));
    }
    
    // Try to infer model from name (uses API keys from environment)
    debug!("Using API key model inference");
    let model = infer_model(model_name)
        .map_err(|e| {
            error!(error = %e, "Failed to infer model");
            ExecutorError::Model(e.to_string())
        })?;
    
    info!(model_name = %model_name, "Model ready");
    Ok(model)
}

/// Legacy execute_agent function for backwards compatibility.
/// 
/// Prefer using `AgentExecutor` directly for new code.
#[deprecated(since = "0.2.0", note = "Use AgentExecutor::execute() instead")]
pub async fn execute_agent(
    db: &Database,
    agent: &dyn SpotAgent,
    model_name: &str,
    prompt: &str,
    message_history: &mut Vec<ModelRequest>,
) -> Result<String, ExecutorError> {
    let executor = AgentExecutor::new(db);
    let registry = SpotToolRegistry::new();
    let mcp_manager = McpManager::new();
    
    // Convert mutable history to owned
    let history = if message_history.is_empty() {
        None
    } else {
        Some(message_history.clone())
    };
    
    let result = executor.execute(agent, model_name, prompt, history, &registry, &mcp_manager).await?;
    
    // Update the caller's history
    *message_history = result.messages;
    
    Ok(result.output)
}

impl<'a> AgentExecutor<'a> {
    /// Collect MCP tools from running servers.
    async fn collect_mcp_tools(
        &self,
        mcp_manager: &McpManager,
    ) -> Vec<(ToolDefinition, Arc<dyn Tool + Send + Sync>)> {
        let mut tools = Vec::new();
        
        // Get all tools from running MCP servers
        let all_mcp_tools = mcp_manager.list_all_tools().await;
        
        for (server_name, server_tools) in all_mcp_tools {
            for mcp_tool in server_tools {
                // Create a tool definition from MCP tool
                let def = ToolDefinition::new(
                    mcp_tool.name.clone(),
                    mcp_tool.description.clone().unwrap_or_default(),
                ).with_parameters(mcp_tool.input_schema.clone());
                
                // Create an MCP tool executor
                let executor = McpToolExecutor {
                    server_name: server_name.clone(),
                    tool_name: mcp_tool.name.clone(),
                    mcp_manager_ptr: mcp_manager as *const McpManager,
                };
                
                tools.push((def, Arc::new(executor) as Arc<dyn Tool + Send + Sync>));
            }
        }
        
        tools
    }
}

/// Tool executor that calls MCP server tools.
/// 
/// Note: We use a raw pointer to McpManager because we can't easily
/// share Arc across async boundaries here. The pointer is valid for
/// the duration of the executor run.
struct McpToolExecutor {
    server_name: String,
    tool_name: String,
    mcp_manager_ptr: *const McpManager,
}

// Safety: The pointer is only used during a single executor run
// where the McpManager is guaranteed to outlive the tool executor.
unsafe impl Send for McpToolExecutor {}
unsafe impl Sync for McpToolExecutor {}

#[async_trait]
impl Tool for McpToolExecutor {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            self.tool_name.clone(),
            format!("MCP tool from {}", self.server_name),
        )
    }

    async fn call(
        &self,
        _ctx: &RunContext<()>,
        args: JsonValue,
    ) -> Result<ToolReturn, ToolError> {
        // Safety: The McpManager outlives this executor
        let manager = unsafe { &*self.mcp_manager_ptr };
        
        match manager.call_tool(&self.server_name, &self.tool_name, args).await {
            Ok(result) => {
                // Convert MCP result to ToolReturn
                if result.is_error {
                    let error_msg = result.content
                        .first()
                        .map(|c| match c {
                            serdes_ai_mcp::ToolResultContent::Text { text } => text.clone(),
                            _ => "MCP tool error".to_string(),
                        })
                        .unwrap_or_else(|| "Unknown error".to_string());
                    Ok(ToolReturn::error(error_msg))
                } else {
                    let text = result.content
                        .into_iter()
                        .filter_map(|c| match c {
                            serdes_ai_mcp::ToolResultContent::Text { text } => Some(text),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(ToolReturn::text(text))
                }
            }
            Err(e) => {
                Err(ToolError::ExecutionFailed {
                    message: e.to_string(),
                    retryable: false,
                })
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Model error: {0}")]
    Model(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Tool error: {0}")]
    Tool(String),
    #[error("Execution error: {0}")]
    Execution(String),
}
