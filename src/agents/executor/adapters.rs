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

#[cfg(test)]
mod tests {
    use super::*;
    use serdes_ai_agent::ToolExecutor;
    use serdes_ai_models::ModelProfile;

    // Minimal mock Model for testing ArcModel delegation
    struct MockModel {
        name: String,
        system: String,
        identifier: String,
        profile: ModelProfile,
    }

    impl MockModel {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                system: "test system prompt".to_string(),
                identifier: format!("mock/{}", name),
                profile: ModelProfile::default(),
            }
        }
    }

    #[async_trait]
    impl Model for MockModel {
        fn name(&self) -> &str {
            &self.name
        }

        fn system(&self) -> &str {
            &self.system
        }

        fn identifier(&self) -> String {
            self.identifier.clone()
        }

        async fn request(
            &self,
            _messages: &[ModelRequest],
            _settings: &ModelSettings,
            _params: &ModelRequestParameters,
        ) -> Result<ModelResponse, ModelError> {
            Ok(ModelResponse::default())
        }

        async fn request_stream(
            &self,
            _messages: &[ModelRequest],
            _settings: &ModelSettings,
            _params: &ModelRequestParameters,
        ) -> Result<StreamedResponse, ModelError> {
            unimplemented!("not needed for unit tests")
        }

        fn profile(&self) -> &ModelProfile {
            &self.profile
        }

        async fn count_tokens(&self, _messages: &[ModelRequest]) -> Result<u64, ModelError> {
            Ok(100)
        }
    }

    #[test]
    fn arc_model_delegates_name() {
        let mock = Arc::new(MockModel::new("test-model"));
        let arc_model = ArcModel(mock);
        assert_eq!(arc_model.name(), "test-model");
    }

    #[test]
    fn arc_model_delegates_system() {
        let mock = Arc::new(MockModel::new("test"));
        let arc_model = ArcModel(mock);
        assert_eq!(arc_model.system(), "test system prompt");
    }

    #[test]
    fn arc_model_delegates_identifier() {
        let mock = Arc::new(MockModel::new("gpt-4"));
        let arc_model = ArcModel(mock);
        assert_eq!(arc_model.identifier(), "mock/gpt-4");
    }

    #[test]
    fn arc_model_delegates_profile() {
        let mock = Arc::new(MockModel::new("test"));
        let arc_model = ArcModel(mock);
        let profile = arc_model.profile();
        // Default profile has max_tokens = None
        assert!(profile.max_tokens.is_none());
    }

    // Minimal mock Tool for testing ToolExecutorAdapter
    struct MockTool {
        name: String,
        return_value: String,
        should_error: bool,
    }

    impl MockTool {
        fn new(name: &str, return_value: &str) -> Self {
            Self {
                name: name.to_string(),
                return_value: return_value.to_string(),
                should_error: false,
            }
        }

        fn failing(name: &str) -> Self {
            Self {
                name: name.to_string(),
                return_value: String::new(),
                should_error: true,
            }
        }
    }

    #[async_trait]
    impl Tool for MockTool {
        fn definition(&self) -> serdes_ai_tools::ToolDefinition {
            serdes_ai_tools::ToolDefinition::new(self.name.clone(), "mock tool".to_string())
        }

        async fn call(
            &self,
            _ctx: &RunContext<()>,
            _args: JsonValue,
        ) -> Result<ToolReturn, ToolError> {
            if self.should_error {
                Err(ToolError::ExecutionFailed {
                    message: "mock error".to_string(),
                    retryable: false,
                })
            } else {
                Ok(ToolReturn::text(self.return_value.clone()))
            }
        }
    }

    /// Helper to create test context with tool_name and tool_call_id
    fn make_test_ctx(
        model: &str,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
    ) -> serdes_ai_agent::RunContext<()> {
        let mut ctx = serdes_ai_agent::RunContext::new((), model);
        ctx.tool_name = tool_name.map(String::from);
        ctx.tool_call_id = tool_call_id.map(String::from);
        ctx
    }

    #[test]
    fn tool_executor_adapter_new() {
        let tool: Arc<dyn Tool + Send + Sync> = Arc::new(MockTool::new("test", "result"));
        let adapter = ToolExecutorAdapter::new(tool);
        // Just verify construction succeeds
        assert!(std::mem::size_of_val(&adapter) > 0);
    }

    #[tokio::test]
    async fn tool_executor_adapter_execute_success() {
        let tool: Arc<dyn Tool + Send + Sync> =
            Arc::new(MockTool::new("read_file", "file content"));
        let adapter = ToolExecutorAdapter::new(tool);

        let ctx = make_test_ctx("test-model", Some("read_file"), Some("call-123"));

        let result = adapter.execute(serde_json::json!({}), &ctx).await;
        assert!(result.is_ok());
        let ret = result.unwrap();
        assert_eq!(ret.as_text(), Some("file content"));
    }

    #[tokio::test]
    async fn tool_executor_adapter_execute_error() {
        let tool: Arc<dyn Tool + Send + Sync> = Arc::new(MockTool::failing("bad_tool"));
        let adapter = ToolExecutorAdapter::new(tool);

        let ctx = make_test_ctx("test-model", Some("bad_tool"), None);

        let result = adapter.execute(serde_json::json!({}), &ctx).await;
        assert!(result.is_err());
    }

    #[test]
    fn recording_tool_executor_new() {
        let tool: Arc<dyn Tool + Send + Sync> = Arc::new(MockTool::new("test", "result"));
        let adapter = ToolExecutorAdapter::new(tool);
        let recorder = Arc::new(Mutex::new(Vec::new()));
        let recording = RecordingToolExecutor::new(adapter, recorder);
        assert!(std::mem::size_of_val(&recording) > 0);
    }

    #[tokio::test]
    async fn recording_tool_executor_records_success() {
        let tool: Arc<dyn Tool + Send + Sync> = Arc::new(MockTool::new("grep", "match found"));
        let adapter = ToolExecutorAdapter::new(tool);
        let recorder = Arc::new(Mutex::new(Vec::new()));
        let recording = RecordingToolExecutor::new(adapter, Arc::clone(&recorder));

        let ctx = make_test_ctx("gpt-4", Some("grep"), Some("call-456"));

        let result = recording
            .execute(serde_json::json!({"pattern": "foo"}), &ctx)
            .await;
        assert!(result.is_ok());

        let recorded = recorder.lock().await;
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].tool_name, "grep");
        assert_eq!(recorded[0].tool_call_id.as_deref(), Some("call-456"));
    }

    #[tokio::test]
    async fn recording_tool_executor_records_error() {
        let tool: Arc<dyn Tool + Send + Sync> = Arc::new(MockTool::failing("shell"));
        let adapter = ToolExecutorAdapter::new(tool);
        let recorder = Arc::new(Mutex::new(Vec::new()));
        let recording = RecordingToolExecutor::new(adapter, Arc::clone(&recorder));

        let ctx = make_test_ctx("claude", Some("shell"), Some("call-789"));

        let result = recording.execute(serde_json::json!({}), &ctx).await;
        assert!(result.is_err());

        let recorded = recorder.lock().await;
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].tool_name, "shell");
        // Error case should still capture tool_call_id
        assert_eq!(recorded[0].tool_call_id.as_deref(), Some("call-789"));
    }

    #[tokio::test]
    async fn recording_tool_executor_uses_unknown_tool_when_name_missing() {
        let tool: Arc<dyn Tool + Send + Sync> = Arc::new(MockTool::new("test", "ok"));
        let adapter = ToolExecutorAdapter::new(tool);
        let recorder = Arc::new(Mutex::new(Vec::new()));
        let recording = RecordingToolExecutor::new(adapter, Arc::clone(&recorder));

        // Context without tool_name
        let ctx = make_test_ctx("model", None, None);

        let _ = recording.execute(serde_json::json!({}), &ctx).await;

        let recorded = recorder.lock().await;
        assert_eq!(recorded[0].tool_name, "unknown_tool");
        assert!(recorded[0].tool_call_id.is_none());
    }
}
