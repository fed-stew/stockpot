//! Test utilities for offline testing of Spot agents.
//!
//! Provides mock implementations of AI models and MCP servers
//! that return configurable responses without hitting real APIs.

use async_trait::async_trait;
use serdes_ai_core::{ModelRequest, ModelResponse, ModelSettings as CoreModelSettings};
use serdes_ai_models::{Model, ModelError, ModelProfile, ModelRequestParameters, StreamedResponse};
use std::sync::{Arc, Mutex};

// =========================================================================
// Mock AI Model
// =========================================================================

/// Configurable behavior for MockModel responses.
#[derive(Debug, Clone)]
pub enum MockBehavior {
    /// Return a text response with the given content.
    Text(String),
    /// Return an error.
    Error(String),
    /// Return responses from a queue (FIFO). Falls back to default text if empty.
    Queue(Vec<MockBehavior>),
}

impl Default for MockBehavior {
    fn default() -> Self {
        MockBehavior::Text("Mock response".to_string())
    }
}

/// A mock AI model that implements the serdesAI `Model` trait.
///
/// Returns configurable responses for testing agent loops
/// without making real API calls.
///
/// # Example
///
/// ```ignore
/// let model = MockModel::new("test-model")
///     .with_behavior(MockBehavior::Text("Hello!".to_string()));
/// let response = model.request(&[], &settings, &params).await.unwrap();
/// ```
pub struct MockModel {
    model_name: String,
    system_name: String,
    profile: ModelProfile,
    behavior: Mutex<MockBehavior>,
    /// Records all requests made to this model for later assertion.
    request_log: Arc<Mutex<Vec<Vec<ModelRequest>>>>,
}

impl MockModel {
    /// Create a new mock model with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            model_name: name.into(),
            system_name: "mock".to_string(),
            profile: ModelProfile::default(),
            behavior: Mutex::new(MockBehavior::default()),
            request_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Set the mock behavior.
    pub fn with_behavior(self, behavior: MockBehavior) -> Self {
        *self.behavior.lock().unwrap() = behavior;
        self
    }

    /// Set the system/provider name.
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system_name = system.into();
        self
    }

    /// Set the model profile.
    pub fn with_profile(mut self, profile: ModelProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Get a clone of the request log for assertions.
    #[allow(dead_code)]
    pub fn request_log(&self) -> Arc<Mutex<Vec<Vec<ModelRequest>>>> {
        Arc::clone(&self.request_log)
    }

    /// Get the number of requests made to this model.
    pub fn request_count(&self) -> usize {
        self.request_log.lock().unwrap().len()
    }

    fn resolve_behavior(&self) -> MockBehavior {
        let mut behavior = self.behavior.lock().unwrap();
        match &mut *behavior {
            MockBehavior::Queue(queue) => {
                if queue.is_empty() {
                    MockBehavior::Text("Mock response (queue exhausted)".to_string())
                } else {
                    queue.remove(0)
                }
            }
            other => other.clone(),
        }
    }
}

#[async_trait]
impl Model for MockModel {
    fn name(&self) -> &str {
        &self.model_name
    }

    fn system(&self) -> &str {
        &self.system_name
    }

    fn identifier(&self) -> String {
        format!("{}:{}", self.system_name, self.model_name)
    }

    async fn request(
        &self,
        messages: &[ModelRequest],
        _settings: &CoreModelSettings,
        _params: &ModelRequestParameters,
    ) -> Result<ModelResponse, ModelError> {
        // Record the request
        self.request_log.lock().unwrap().push(messages.to_vec());

        match self.resolve_behavior() {
            MockBehavior::Text(text) => Ok(ModelResponse::text(text)),
            MockBehavior::Error(msg) => Err(ModelError::Api {
                message: msg,
                code: None,
            }),
            MockBehavior::Queue(_) => unreachable!("resolved above"),
        }
    }

    async fn request_stream(
        &self,
        messages: &[ModelRequest],
        settings: &CoreModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<StreamedResponse, ModelError> {
        // Fall back to non-streaming for the mock
        let _response = self.request(messages, settings, params).await?;
        Err(ModelError::not_supported("Streaming not supported in mock"))
    }

    fn profile(&self) -> &ModelProfile {
        &self.profile
    }

    async fn count_tokens(&self, _messages: &[ModelRequest]) -> Result<u64, ModelError> {
        // Return a fixed token count for testing
        Ok(100)
    }
}

// =========================================================================
// Mock MCP Manager
// =========================================================================

use crate::mcp::{McpConfig, McpServerEntry};
use std::collections::HashMap;

/// A mock MCP manager for testing MCP tool calls without starting real servers.
///
/// Provides configurable tool results and records tool invocations.
pub struct MockMcpManager {
    /// Configured tool results keyed by tool name.
    tool_results: HashMap<String, serde_json::Value>,
    /// Record of tool invocations: (tool_name, args).
    invocations: Mutex<Vec<(String, serde_json::Value)>>,
    /// The mock configuration.
    config: McpConfig,
}

impl MockMcpManager {
    /// Create a new mock MCP manager with no configured tools.
    pub fn new() -> Self {
        Self {
            tool_results: HashMap::new(),
            invocations: Mutex::new(Vec::new()),
            config: McpConfig::new(),
        }
    }

    /// Register a tool with a fixed result.
    pub fn with_tool_result(
        mut self,
        tool_name: impl Into<String>,
        result: serde_json::Value,
    ) -> Self {
        self.tool_results.insert(tool_name.into(), result);
        self
    }

    /// Add a server to the mock config.
    pub fn with_server(mut self, name: impl Into<String>, entry: McpServerEntry) -> Self {
        self.config.add_server(name, entry);
        self
    }

    /// Simulate calling a tool by name with given args.
    pub fn call_tool(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.invocations
            .lock()
            .unwrap()
            .push((tool_name.to_string(), args));

        self.tool_results
            .get(tool_name)
            .cloned()
            .ok_or_else(|| format!("Tool not found: {}", tool_name))
    }

    /// Get a reference to the mock config.
    pub fn config(&self) -> &McpConfig {
        &self.config
    }

    /// Get the number of tool invocations.
    pub fn invocation_count(&self) -> usize {
        self.invocations.lock().unwrap().len()
    }

    /// Get all recorded invocations.
    pub fn invocations(&self) -> Vec<(String, serde_json::Value)> {
        self.invocations.lock().unwrap().clone()
    }

    /// Check if a specific tool was called.
    pub fn was_tool_called(&self, tool_name: &str) -> bool {
        self.invocations
            .lock()
            .unwrap()
            .iter()
            .any(|(name, _)| name == tool_name)
    }
}

impl Default for MockMcpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // MockModel Tests
    // =========================================================================

    #[tokio::test]
    async fn test_mock_model_text_response() {
        let model = MockModel::new("test-model")
            .with_behavior(MockBehavior::Text("Hello, world!".to_string()));

        let settings = CoreModelSettings::default();
        let params = ModelRequestParameters::default();

        let response = model.request(&[], &settings, &params).await.unwrap();
        let text = response.text_content();
        assert_eq!(text, "Hello, world!");
    }

    #[tokio::test]
    async fn test_mock_model_error_response() {
        let model = MockModel::new("error-model")
            .with_behavior(MockBehavior::Error("API rate limited".to_string()));

        let settings = CoreModelSettings::default();
        let params = ModelRequestParameters::default();

        let result = model.request(&[], &settings, &params).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("API rate limited"));
    }

    #[tokio::test]
    async fn test_mock_model_queue_behavior() {
        let model = MockModel::new("queue-model").with_behavior(MockBehavior::Queue(vec![
            MockBehavior::Text("First".to_string()),
            MockBehavior::Text("Second".to_string()),
            MockBehavior::Error("Third fails".to_string()),
        ]));

        let settings = CoreModelSettings::default();
        let params = ModelRequestParameters::default();

        // First call
        let r1 = model.request(&[], &settings, &params).await.unwrap();
        assert_eq!(r1.text_content(), "First");

        // Second call
        let r2 = model.request(&[], &settings, &params).await.unwrap();
        assert_eq!(r2.text_content(), "Second");

        // Third call - should error
        let r3 = model.request(&[], &settings, &params).await;
        assert!(r3.is_err());

        // Fourth call - queue exhausted, falls back to default
        let r4 = model.request(&[], &settings, &params).await.unwrap();
        assert!(r4.text_content().contains("queue exhausted"));
    }

    #[tokio::test]
    async fn test_mock_model_records_requests() {
        let model = MockModel::new("logging-model");

        let settings = CoreModelSettings::default();
        let params = ModelRequestParameters::default();

        assert_eq!(model.request_count(), 0);

        model.request(&[], &settings, &params).await.unwrap();
        assert_eq!(model.request_count(), 1);

        model.request(&[], &settings, &params).await.unwrap();
        assert_eq!(model.request_count(), 2);
    }

    #[test]
    fn test_mock_model_name_and_system() {
        let model = MockModel::new("gpt-4o-mock").with_system("openai");

        assert_eq!(model.name(), "gpt-4o-mock");
        assert_eq!(model.system(), "openai");
        assert_eq!(model.identifier(), "openai:gpt-4o-mock");
    }

    #[test]
    fn test_mock_model_profile() {
        let profile = ModelProfile {
            supports_tools: true,
            supports_images: true,
            ..Default::default()
        };

        let model = MockModel::new("profile-model").with_profile(profile);
        assert!(model.profile().supports_tools);
        assert!(model.profile().supports_images);
    }

    #[tokio::test]
    async fn test_mock_model_count_tokens() {
        let model = MockModel::new("token-model");
        let count = model.count_tokens(&[]).await.unwrap();
        assert_eq!(count, 100);
    }

    #[tokio::test]
    async fn test_mock_model_stream_not_supported() {
        let model = MockModel::new("stream-model");
        let settings = CoreModelSettings::default();
        let params = ModelRequestParameters::default();

        let result = model.request_stream(&[], &settings, &params).await;
        assert!(result.is_err());
    }

    // =========================================================================
    // MockMcpManager Tests
    // =========================================================================

    #[test]
    fn test_mock_mcp_manager_call_tool() {
        let manager = MockMcpManager::new()
            .with_tool_result("read_file", serde_json::json!({"content": "file contents"}));

        let result = manager
            .call_tool("read_file", serde_json::json!({"path": "/tmp/test.txt"}))
            .unwrap();

        assert_eq!(result["content"], "file contents");
        assert_eq!(manager.invocation_count(), 1);
        assert!(manager.was_tool_called("read_file"));
    }

    #[test]
    fn test_mock_mcp_manager_unknown_tool() {
        let manager = MockMcpManager::new();

        let result = manager.call_tool("nonexistent", serde_json::json!({}));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Tool not found"));
    }

    #[test]
    fn test_mock_mcp_manager_multiple_tools() {
        let manager = MockMcpManager::new()
            .with_tool_result("tool_a", serde_json::json!("result_a"))
            .with_tool_result("tool_b", serde_json::json!("result_b"));

        let a = manager.call_tool("tool_a", serde_json::json!({})).unwrap();
        let b = manager.call_tool("tool_b", serde_json::json!({})).unwrap();

        assert_eq!(a, "result_a");
        assert_eq!(b, "result_b");
        assert_eq!(manager.invocation_count(), 2);
    }

    #[test]
    fn test_mock_mcp_manager_records_args() {
        let manager =
            MockMcpManager::new().with_tool_result("grep", serde_json::json!({"matches": []}));

        let args = serde_json::json!({"pattern": "TODO", "path": "/src"});
        manager.call_tool("grep", args.clone()).unwrap();

        let invocations = manager.invocations();
        assert_eq!(invocations.len(), 1);
        assert_eq!(invocations[0].0, "grep");
        assert_eq!(invocations[0].1, args);
    }

    #[test]
    fn test_mock_mcp_manager_with_server_config() {
        let manager = MockMcpManager::new()
            .with_server("test-server", McpServerEntry::new("echo"))
            .with_tool_result("echo_tool", serde_json::json!("echoed"));

        assert!(manager.config().has_server("test-server"));
        assert_eq!(
            manager.config().get_server("test-server").unwrap().command,
            "echo"
        );
    }

    #[test]
    fn test_mock_mcp_manager_was_tool_called() {
        let manager = MockMcpManager::new()
            .with_tool_result("called", serde_json::json!(null))
            .with_tool_result("not_called", serde_json::json!(null));

        manager.call_tool("called", serde_json::json!({})).unwrap();

        assert!(manager.was_tool_called("called"));
        assert!(!manager.was_tool_called("not_called"));
    }
}
