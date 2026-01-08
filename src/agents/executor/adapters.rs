//! Model and tool adapters for serdesAI integration.
//!
//! Contains wrapper types that bridge our implementations to serdesAI's interfaces:
//! - `ArcModel`: Wraps `Arc<dyn Model>` to implement `Model` trait
//! - `ToolExecutorAdapter`: Adapts `Arc<dyn Tool>` to `ToolExecutor<()>`
//! - `RecordingToolExecutor`: Records tool returns during streaming

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::Mutex;

use serdes_ai_core::{ModelRequest, ModelResponse, ModelSettings, ToolReturnPart};
use serdes_ai_models::{Model, ModelError, ModelProfile, ModelRequestParameters, StreamedResponse};
use serdes_ai_tools::{RunContext, Tool, ToolError, ToolReturn};

/// Wrapper to make `Arc<dyn Model>` implement `Model`.
///
/// This allows us to use dynamically dispatched models with serdesAI's
/// agent builder, which requires a concrete `Model` type.
pub(super) struct ArcModel(pub Arc<dyn Model>);

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
pub(super) struct ToolExecutorAdapter {
    tool: Arc<dyn Tool + Send + Sync>,
}

impl ToolExecutorAdapter {
    pub fn new(tool: Arc<dyn Tool + Send + Sync>) -> Self {
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

/// Wraps a tool executor and records tool returns during streaming.
///
/// `serdes_ai_agent::AgentStreamEvent` does not include tool return payloads, but we
/// need them for accurate `message_history` reconstruction.
pub(super) struct RecordingToolExecutor<E> {
    inner: E,
    recorder: Arc<Mutex<Vec<ToolReturnPart>>>,
}

impl<E> RecordingToolExecutor<E> {
    pub fn new(inner: E, recorder: Arc<Mutex<Vec<ToolReturnPart>>>) -> Self {
        Self { inner, recorder }
    }
}

#[async_trait]
impl<E> serdes_ai_agent::ToolExecutor<()> for RecordingToolExecutor<E>
where
    E: serdes_ai_agent::ToolExecutor<()> + Send + Sync,
{
    async fn execute(
        &self,
        args: JsonValue,
        ctx: &serdes_ai_agent::RunContext<()>,
    ) -> Result<ToolReturn, ToolError> {
        let result = self.inner.execute(args, ctx).await;

        // Best-effort tool name/id capture; used to reconstruct `ToolReturnPart`s.
        let tool_name = ctx
            .tool_name
            .clone()
            .unwrap_or_else(|| "unknown_tool".to_string());

        let mut part = match &result {
            Ok(ret) => ToolReturnPart::new(&tool_name, ret.content.clone()),
            Err(e) => ToolReturnPart::error(&tool_name, format!("Tool error: {}", e)),
        };

        if let Some(tool_call_id) = ctx.tool_call_id.clone() {
            part = part.with_tool_call_id(tool_call_id);
        }

        self.recorder.lock().await.push(part);
        result
    }
}
