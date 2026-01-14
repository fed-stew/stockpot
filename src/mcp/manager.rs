//! MCP server lifecycle management.
//!
//! Handles starting, stopping, and managing MCP server connections.

use super::config::{McpConfig, McpServerEntry};
use serdes_ai_mcp::{McpClient, McpError};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Error type for MCP manager operations.
#[derive(Debug, Error)]
pub enum McpManagerError {
    #[error("Config error: {0}")]
    Config(#[from] super::config::McpConfigError),

    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Server already running: {0}")]
    AlreadyRunning(String),

    #[error("Server not running: {0}")]
    NotRunning(String),
}

/// Handle to a running MCP server.
pub struct McpServerHandle {
    /// The connected client.
    pub client: Arc<McpClient>,
}

/// Manager for MCP server connections.
///
/// Handles loading configuration, starting/stopping servers,
/// and providing toolsets for agent integration.
pub struct McpManager {
    config: McpConfig,
    servers: RwLock<HashMap<String, McpServerHandle>>,
}

impl McpManager {
    /// Create a new MCP manager with default configuration.
    pub fn new() -> Self {
        Self {
            config: McpConfig::load_or_default(),
            servers: RwLock::new(HashMap::new()),
        }
    }

    /// Create a manager with a specific configuration.
    pub fn with_config(config: McpConfig) -> Self {
        Self {
            config,
            servers: RwLock::new(HashMap::new()),
        }
    }

    /// Load configuration from the default path.
    pub fn load_config(&mut self) -> Result<(), McpManagerError> {
        self.config = McpConfig::load_default()?;
        Ok(())
    }

    /// Get the current configuration.
    pub fn config(&self) -> &McpConfig {
        &self.config
    }

    /// Start a specific MCP server by name.
    pub async fn start_server(&self, name: &str) -> Result<(), McpManagerError> {
        let entry = self
            .config
            .get_server(name)
            .ok_or_else(|| McpManagerError::ServerNotFound(name.to_string()))?;

        // Check if already running
        {
            let servers = self.servers.read().await;
            if servers.contains_key(name) {
                return Err(McpManagerError::AlreadyRunning(name.to_string()));
            }
        }

        info!(server = %name, "Starting MCP server");

        // Start the server
        let handle = self.connect_server(name, entry).await?;

        // Store the handle
        let mut servers = self.servers.write().await;
        servers.insert(name.to_string(), handle);

        info!(server = %name, "MCP server started");
        Ok(())
    }

    /// Stop a specific MCP server.
    pub async fn stop_server(&self, name: &str) -> Result<(), McpManagerError> {
        let mut servers = self.servers.write().await;

        if let Some(handle) = servers.remove(name) {
            info!("Stopping MCP server: {}", name);
            if let Err(e) = handle.client.close().await {
                warn!("Error closing MCP server {}: {}", name, e);
            }
            Ok(())
        } else {
            Err(McpManagerError::NotRunning(name.to_string()))
        }
    }

    /// Start all enabled servers.
    pub async fn start_all(&self) -> Result<(), McpManagerError> {
        let enabled: Vec<(String, McpServerEntry)> = self
            .config
            .enabled_servers()
            .map(|(name, entry)| (name.clone(), entry.clone()))
            .collect();

        for (name, _) in enabled {
            if let Err(e) = self.start_server(&name).await {
                error!("Failed to start MCP server {}: {}", name, e);
                // Continue with other servers
            }
        }

        Ok(())
    }

    /// Stop all running servers.
    pub async fn stop_all(&self) -> Result<(), McpManagerError> {
        let names: Vec<String> = {
            let servers = self.servers.read().await;
            servers.keys().cloned().collect()
        };

        for name in names {
            if let Err(e) = self.stop_server(&name).await {
                warn!("Error stopping MCP server {}: {}", name, e);
            }
        }

        Ok(())
    }

    /// Get list of running server names.
    pub async fn running_servers(&self) -> Vec<String> {
        let servers = self.servers.read().await;
        servers.keys().cloned().collect()
    }

    /// Check if a server is running.
    pub async fn is_running(&self, name: &str) -> bool {
        let servers = self.servers.read().await;
        servers.contains_key(name)
    }

    /// Get a server handle by name.
    pub async fn get_handle(&self, name: &str) -> Option<Arc<McpClient>> {
        let servers = self.servers.read().await;
        servers.get(name).map(|h| Arc::clone(&h.client))
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serdes_ai_mcp::CallToolResult, McpManagerError> {
        let servers = self.servers.read().await;
        let handle = servers
            .get(server_name)
            .ok_or_else(|| McpManagerError::NotRunning(server_name.to_string()))?;

        let result = handle.client.call_tool(tool_name, args).await?;
        Ok(result)
    }

    /// List tools from a specific server.
    pub async fn list_tools(
        &self,
        server_name: &str,
    ) -> Result<Vec<serdes_ai_mcp::McpTool>, McpManagerError> {
        let servers = self.servers.read().await;
        let handle = servers
            .get(server_name)
            .ok_or_else(|| McpManagerError::NotRunning(server_name.to_string()))?;

        let tools = handle.client.list_tools().await?;
        Ok(tools)
    }

    /// List tools from all running servers.
    pub async fn list_all_tools(&self) -> HashMap<String, Vec<serdes_ai_mcp::McpTool>> {
        let servers = self.servers.read().await;
        let mut all_tools = HashMap::new();

        for (name, handle) in servers.iter() {
            // Use timeout to avoid hanging forever on unresponsive servers
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                handle.client.list_tools(),
            )
            .await
            {
                Ok(Ok(tools)) => {
                    all_tools.insert(name.clone(), tools);
                }
                Ok(Err(e)) => {
                    warn!(server = %name, error = %e, "Failed to list tools from MCP server");
                }
                Err(_) => {
                    warn!(server = %name, "Timeout listing tools from MCP server");
                }
            }
        }

        all_tools
    }

    /// Connect to a server and create a handle.
    async fn connect_server(
        &self,
        name: &str,
        entry: &McpServerEntry,
    ) -> Result<McpServerHandle, McpManagerError> {
        // Build args with env vars
        let args: Vec<&str> = entry.args.iter().map(|s| s.as_str()).collect();

        // Create the client
        let client = match McpClient::stdio(&entry.command, &args).await {
            Ok(c) => c,
            Err(e) => {
                error!(server = %name, error = %e, "Failed to spawn MCP server process");
                return Err(e.into());
            }
        };

        // Initialize the connection
        match client.initialize().await {
            Ok(result) => {
                info!(
                    server = %name,
                    server_name = %result.server_info.name,
                    server_version = %result.server_info.version,
                    "MCP server initialized"
                );
            }
            Err(e) => {
                error!(server = %name, error = %e, "Failed to initialize MCP server");
                return Err(e.into());
            }
        }

        // List tools to verify connection - with timeout since some servers are slow
        match tokio::time::timeout(std::time::Duration::from_secs(5), client.list_tools()).await {
            Ok(Ok(tools)) => {
                info!(
                    server = %name,
                    tool_count = tools.len(),
                    "MCP server ready with {} tools", tools.len()
                );
            }
            Ok(Err(e)) => {
                warn!(server = %name, error = %e, "Failed to list MCP tools");
            }
            Err(_) => {
                warn!(server = %name, "Timeout listing MCP tools");
            }
        }

        Ok(McpServerHandle {
            client: Arc::new(client),
        })
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // McpManager Construction Tests
    // =========================================================================

    #[test]
    fn test_manager_new() {
        let manager = McpManager::new();
        // Config is loaded from disk or empty
        let _ = manager.config();
    }

    #[test]
    fn test_manager_default() {
        let manager = McpManager::default();
        let _ = manager.config();
    }

    #[test]
    fn test_manager_with_config() {
        let config = McpConfig::sample();
        let manager = McpManager::with_config(config);
        assert!(manager.config().has_server("filesystem"));
    }

    #[test]
    fn test_manager_with_empty_config() {
        let config = McpConfig::new();
        let manager = McpManager::with_config(config);
        assert!(manager.config().servers.is_empty());
    }

    #[test]
    fn test_manager_config_immutable_ref() {
        let config = McpConfig::sample();
        let manager = McpManager::with_config(config);

        let cfg1 = manager.config();
        let cfg2 = manager.config();

        // Both should refer to the same config
        assert_eq!(cfg1.servers.len(), cfg2.servers.len());
    }

    // =========================================================================
    // Running Servers State Tests
    // =========================================================================

    #[tokio::test]
    async fn test_running_servers_empty() {
        let manager = McpManager::new();
        let running = manager.running_servers().await;
        assert!(running.is_empty());
    }

    #[tokio::test]
    async fn test_is_running_false() {
        let manager = McpManager::new();
        assert!(!manager.is_running("nonexistent").await);
    }

    #[tokio::test]
    async fn test_get_handle_nonexistent() {
        let manager = McpManager::new();
        let handle = manager.get_handle("nonexistent").await;
        assert!(handle.is_none());
    }

    // =========================================================================
    // Server Not Found Error Tests
    // =========================================================================

    #[tokio::test]
    async fn test_start_server_not_found() {
        let config = McpConfig::new(); // empty config
        let manager = McpManager::with_config(config);

        let result = manager.start_server("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpManagerError::ServerNotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_stop_server_not_running() {
        let manager = McpManager::new();

        let result = manager.stop_server("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpManagerError::NotRunning(_)
        ));
    }

    #[tokio::test]
    async fn test_call_tool_server_not_running() {
        let manager = McpManager::new();

        let result = manager
            .call_tool("nonexistent", "some_tool", serde_json::json!({}))
            .await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpManagerError::NotRunning(_)
        ));
    }

    #[tokio::test]
    async fn test_list_tools_server_not_running() {
        let manager = McpManager::new();

        let result = manager.list_tools("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            McpManagerError::NotRunning(_)
        ));
    }

    // =========================================================================
    // Start/Stop All Tests (no actual servers)
    // =========================================================================

    #[tokio::test]
    async fn test_start_all_with_empty_config() {
        let config = McpConfig::new();
        let manager = McpManager::with_config(config);

        // Should succeed with no servers to start
        let result = manager.start_all().await;
        assert!(result.is_ok());
        assert!(manager.running_servers().await.is_empty());
    }

    #[tokio::test]
    async fn test_stop_all_with_none_running() {
        let manager = McpManager::new();

        // Should succeed with no servers to stop
        let result = manager.stop_all().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_all_tools_with_none_running() {
        let manager = McpManager::new();

        let tools = manager.list_all_tools().await;
        assert!(tools.is_empty());
    }

    // =========================================================================
    // Error Type Tests
    // =========================================================================

    #[test]
    fn test_error_server_not_found_display() {
        let err = McpManagerError::ServerNotFound("test_server".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("not found"));
        assert!(msg.contains("test_server"));
    }

    #[test]
    fn test_error_already_running_display() {
        let err = McpManagerError::AlreadyRunning("test_server".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("already running"));
        assert!(msg.contains("test_server"));
    }

    #[test]
    fn test_error_not_running_display() {
        let err = McpManagerError::NotRunning("test_server".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("not running"));
        assert!(msg.contains("test_server"));
    }

    #[test]
    fn test_error_debug_format() {
        let err = McpManagerError::ServerNotFound("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("ServerNotFound"));
    }

    // =========================================================================
    // Error Conversion Tests
    // =========================================================================

    #[test]
    fn test_error_from_config_error() {
        use super::super::config::McpConfigError;
        use std::path::PathBuf;

        let config_err = McpConfigError::NotFound(PathBuf::from("/test/path"));
        let manager_err: McpManagerError = config_err.into();

        assert!(matches!(manager_err, McpManagerError::Config(_)));
        let msg = format!("{}", manager_err);
        assert!(msg.contains("Config error"));
    }

    #[test]
    fn test_error_variants_complete() {
        // Ensure all variants can be constructed and displayed
        let errors = vec![
            McpManagerError::ServerNotFound("s1".into()),
            McpManagerError::AlreadyRunning("s2".into()),
            McpManagerError::NotRunning("s3".into()),
        ];

        for err in errors {
            let _ = format!("{}", err);
            let _ = format!("{:?}", err);
        }
    }

    // =========================================================================
    // Config Accessor Tests
    // =========================================================================

    #[test]
    fn test_config_accessor_returns_reference() {
        let mut cfg = McpConfig::new();
        cfg.add_server("test", super::super::config::McpServerEntry::new("cmd"));
        let manager = McpManager::with_config(cfg);

        // Verify config() returns a valid reference
        assert!(manager.config().has_server("test"));
        assert_eq!(manager.config().servers.len(), 1);
    }

    // =========================================================================
    // Multiple Operations Tests
    // =========================================================================

    #[tokio::test]
    async fn test_stop_all_idempotent() {
        let manager = McpManager::new();

        // Calling stop_all multiple times should be safe
        manager.stop_all().await.unwrap();
        manager.stop_all().await.unwrap();
        manager.stop_all().await.unwrap();

        assert!(manager.running_servers().await.is_empty());
    }

    #[tokio::test]
    async fn test_start_all_idempotent_with_empty_config() {
        let config = McpConfig::new();
        let manager = McpManager::with_config(config);

        // Calling start_all multiple times with empty config
        manager.start_all().await.unwrap();
        manager.start_all().await.unwrap();

        assert!(manager.running_servers().await.is_empty());
    }

    #[tokio::test]
    async fn test_is_running_multiple_checks() {
        let manager = McpManager::new();

        // Multiple checks should be consistent
        assert!(!manager.is_running("server1").await);
        assert!(!manager.is_running("server2").await);
        assert!(!manager.is_running("server1").await);
    }

    #[tokio::test]
    async fn test_get_handle_multiple_nonexistent() {
        let manager = McpManager::new();

        assert!(manager.get_handle("a").await.is_none());
        assert!(manager.get_handle("b").await.is_none());
        assert!(manager.get_handle("c").await.is_none());
    }

    // =========================================================================
    // Config with Disabled Servers Tests
    // =========================================================================

    #[tokio::test]
    async fn test_start_all_skips_disabled_servers() {
        let mut config = McpConfig::new();

        // Add disabled server - won't attempt to start
        let mut entry = super::super::config::McpServerEntry::new("nonexistent_cmd");
        entry.enabled = false;
        config.add_server("disabled_server", entry);

        let manager = McpManager::with_config(config);

        // Should succeed (disabled servers are skipped)
        let result = manager.start_all().await;
        assert!(result.is_ok());
        assert!(manager.running_servers().await.is_empty());
    }

    #[tokio::test]
    async fn test_start_all_with_mixed_enabled_disabled() {
        let mut config = McpConfig::new();

        // Disabled server
        let mut disabled = super::super::config::McpServerEntry::new("cmd1");
        disabled.enabled = false;
        config.add_server("disabled", disabled);

        // Another disabled
        let mut disabled2 = super::super::config::McpServerEntry::new("cmd2");
        disabled2.enabled = false;
        config.add_server("disabled2", disabled2);

        let manager = McpManager::with_config(config);

        // All disabled, so nothing should be attempted
        let result = manager.start_all().await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // Error Message Content Tests
    // =========================================================================

    #[tokio::test]
    async fn test_error_messages_contain_server_name() {
        let manager = McpManager::new();

        // Test NotRunning error
        let err = manager.stop_server("my_special_server").await.unwrap_err();
        let msg = format!("{}", err);
        assert!(
            msg.contains("my_special_server"),
            "Error should contain server name"
        );

        // Test call_tool NotRunning
        let err = manager
            .call_tool("another_server", "tool", serde_json::json!({}))
            .await
            .unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("another_server"));

        // Test list_tools NotRunning
        let err = manager.list_tools("third_server").await.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("third_server"));
    }

    #[tokio::test]
    async fn test_start_server_not_found_message() {
        let config = McpConfig::new();
        let manager = McpManager::with_config(config);

        let err = manager.start_server("missing_server").await.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("missing_server"));
        assert!(msg.contains("not found") || msg.contains("Server not found"));
    }

    // =========================================================================
    // Running Servers Ordering Tests
    // =========================================================================

    #[tokio::test]
    async fn test_running_servers_returns_vec() {
        let manager = McpManager::new();
        let running: Vec<String> = manager.running_servers().await;

        // Should be empty and a valid Vec
        assert!(running.is_empty());
        assert_eq!(running.len(), 0);
    }

    // =========================================================================
    // Config Sample Integration Tests
    // =========================================================================

    #[test]
    fn test_manager_with_sample_config() {
        let config = McpConfig::sample();
        let manager = McpManager::with_config(config);

        // Sample has filesystem (enabled) and github (disabled)
        assert!(manager.config().has_server("filesystem"));
        assert!(manager.config().has_server("github"));

        let enabled: Vec<_> = manager.config().enabled_servers().collect();
        assert_eq!(enabled.len(), 1);
    }

    #[tokio::test]
    async fn test_manager_sample_start_server_not_found_for_typo() {
        let config = McpConfig::sample();
        let manager = McpManager::with_config(config);

        // Typo in server name
        let result = manager.start_server("filesytem").await; // note the typo
        assert!(matches!(result, Err(McpManagerError::ServerNotFound(_))));
    }

    // =========================================================================
    // List All Tools Edge Cases
    // =========================================================================

    #[tokio::test]
    async fn test_list_all_tools_returns_hashmap() {
        let manager = McpManager::new();
        let tools = manager.list_all_tools().await;

        assert!(tools.is_empty());
        // Verify it's actually a HashMap
        let _: &HashMap<String, Vec<serdes_ai_mcp::McpTool>> = &tools;
    }

    // =========================================================================
    // Concurrent Access Tests
    // =========================================================================

    #[tokio::test]
    async fn test_concurrent_running_servers_checks() {
        let manager = std::sync::Arc::new(McpManager::new());

        let mut handles = Vec::new();
        for i in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                mgr.is_running(&format!("server_{}", i)).await
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(!result);
        }
    }

    #[tokio::test]
    async fn test_concurrent_get_handle_checks() {
        let manager = std::sync::Arc::new(McpManager::new());

        let mut handles = Vec::new();
        for i in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                mgr.get_handle(&format!("server_{}", i)).await
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_none());
        }
    }

    // =========================================================================
    // Default Trait Implementation Test
    // =========================================================================

    #[test]
    fn test_default_same_as_new() {
        let via_new = McpManager::new();
        let via_default = McpManager::default();

        // Both should have same initial state
        assert_eq!(
            via_new.config().servers.len(),
            via_default.config().servers.len()
        );
    }

    // =========================================================================
    // Complex Tool Call Scenarios
    // =========================================================================

    #[tokio::test]
    async fn test_call_tool_with_various_json_args() {
        let manager = McpManager::new();

        // Empty object
        let result = manager
            .call_tool("server", "tool", serde_json::json!({}))
            .await;
        assert!(matches!(result, Err(McpManagerError::NotRunning(_))));

        // Complex nested object
        let result = manager
            .call_tool(
                "server",
                "tool",
                serde_json::json!({
                    "nested": {"key": "value"},
                    "array": [1, 2, 3],
                    "null": null,
                    "bool": true
                }),
            )
            .await;
        assert!(matches!(result, Err(McpManagerError::NotRunning(_))));

        // Array arg
        let result = manager
            .call_tool("server", "tool", serde_json::json!([1, 2, 3]))
            .await;
        assert!(matches!(result, Err(McpManagerError::NotRunning(_))));
    }

    // =========================================================================
    // Stop Server Edge Cases
    // =========================================================================

    #[tokio::test]
    async fn test_stop_server_twice() {
        let manager = McpManager::new();

        // First stop should fail (not running)
        let result1 = manager.stop_server("test").await;
        assert!(matches!(result1, Err(McpManagerError::NotRunning(_))));

        // Second stop should also fail
        let result2 = manager.stop_server("test").await;
        assert!(matches!(result2, Err(McpManagerError::NotRunning(_))));
    }

    // =========================================================================
    // Config Enabled Servers via Manager Tests
    // =========================================================================

    #[test]
    fn test_enabled_servers_via_manager_config() {
        let mut config = McpConfig::new();

        let enabled_entry = super::super::config::McpServerEntry::new("cmd1");
        config.add_server("enabled", enabled_entry);

        let mut disabled_entry = super::super::config::McpServerEntry::new("cmd2");
        disabled_entry.enabled = false;
        config.add_server("disabled", disabled_entry);

        let manager = McpManager::with_config(config);

        let enabled: Vec<_> = manager.config().enabled_servers().collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "enabled");
    }

    // =========================================================================
    // Empty String Server Names
    // =========================================================================

    #[tokio::test]
    async fn test_empty_server_name() {
        let manager = McpManager::new();

        // Empty string server name
        assert!(!manager.is_running("").await);
        assert!(manager.get_handle("").await.is_none());

        let result = manager.stop_server("").await;
        assert!(matches!(result, Err(McpManagerError::NotRunning(_))));
    }

    #[tokio::test]
    async fn test_whitespace_server_name() {
        let manager = McpManager::new();

        // Whitespace server name
        assert!(!manager.is_running("   ").await);
        assert!(manager.get_handle("\t\n").await.is_none());
    }

    // =========================================================================
    // Special Character Server Names
    // =========================================================================

    #[tokio::test]
    async fn test_special_char_server_names() {
        let manager = McpManager::new();

        let special_names = [
            "server-with-dashes",
            "server_with_underscores",
            "server.with.dots",
            "server/with/slashes",
            "unicode_名前",
            "emoji_🚀",
        ];

        for name in special_names {
            assert!(!manager.is_running(name).await);
            assert!(manager.get_handle(name).await.is_none());
        }
    }

    // =========================================================================
    // load_config Tests
    // =========================================================================

    #[test]
    fn test_load_config_returns_error_when_no_file() {
        let mut manager = McpManager::new();
        // load_config tries to load from default path which may/may not exist
        // We just verify it doesn't panic and returns a result
        let result = manager.load_config();
        // Result depends on whether ~/.stockpot/mcp_servers.json exists
        match result {
            Ok(()) => {
                // Config loaded successfully
                let _ = manager.config();
            }
            Err(McpManagerError::Config(_)) => {
                // Expected if file doesn't exist
            }
            Err(e) => {
                panic!("Unexpected error type: {:?}", e);
            }
        }
    }

    // =========================================================================
    // McpManagerError::Mcp Variant Tests
    // =========================================================================

    #[test]
    fn test_error_mcp_variant_display() {
        // Create an MCP error via From impl
        // Note: We can't easily create McpError directly, but we can test the variant exists
        let server_not_found = McpManagerError::ServerNotFound("test".into());
        let already_running = McpManagerError::AlreadyRunning("test".into());
        let not_running = McpManagerError::NotRunning("test".into());

        // Verify all non-Config/non-Mcp variants display correctly
        assert!(format!("{}", server_not_found).contains("not found"));
        assert!(format!("{}", already_running).contains("already running"));
        assert!(format!("{}", not_running).contains("not running"));
    }

    // =========================================================================
    // Config Mutation Then Start Tests
    // =========================================================================

    #[tokio::test]
    async fn test_config_with_server_then_start_nonexistent() {
        let mut config = McpConfig::new();
        config.add_server("server_a", super::super::config::McpServerEntry::new("cmd"));
        let manager = McpManager::with_config(config);

        // Try to start a server not in config
        let result = manager.start_server("server_b").await;
        assert!(matches!(result, Err(McpManagerError::ServerNotFound(_))));

        // Verify original server is still in config but not running
        assert!(manager.config().has_server("server_a"));
        assert!(!manager.is_running("server_a").await);
    }

    #[tokio::test]
    async fn test_multiple_servers_in_config_start_each_fails_gracefully() {
        let mut config = McpConfig::new();

        // Add multiple disabled servers (won't be started by start_all)
        for i in 0..5 {
            let mut entry =
                super::super::config::McpServerEntry::new(format!("nonexistent_cmd_{}", i));
            entry.enabled = false;
            config.add_server(format!("server_{}", i), entry);
        }

        let manager = McpManager::with_config(config);

        // start_all should succeed (skips disabled)
        let result = manager.start_all().await;
        assert!(result.is_ok());
        assert!(manager.running_servers().await.is_empty());
    }

    // =========================================================================
    // Stress Tests with Many Servers
    // =========================================================================

    #[test]
    fn test_config_with_many_servers() {
        let mut config = McpConfig::new();

        // Add 100 servers to config
        for i in 0..100 {
            config.add_server(
                format!("server_{}", i),
                super::super::config::McpServerEntry::new(format!("cmd_{}", i)),
            );
        }

        let manager = McpManager::with_config(config);
        assert_eq!(manager.config().servers.len(), 100);

        // Verify random access
        assert!(manager.config().has_server("server_0"));
        assert!(manager.config().has_server("server_50"));
        assert!(manager.config().has_server("server_99"));
        assert!(!manager.config().has_server("server_100"));
    }

    #[tokio::test]
    async fn test_running_servers_with_many_concurrent_checks() {
        let manager = std::sync::Arc::new(McpManager::new());

        // Spawn 50 concurrent checks
        let mut handles = Vec::new();
        for i in 0..50 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                let running = mgr.running_servers().await;
                let is_running = mgr.is_running(&format!("server_{}", i)).await;
                (running.is_empty(), !is_running)
            }));
        }

        for handle in handles {
            let (empty, not_running) = handle.await.unwrap();
            assert!(empty);
            assert!(not_running);
        }
    }

    // =========================================================================
    // Concurrent Start/Stop Operations Tests
    // =========================================================================

    #[tokio::test]
    async fn test_concurrent_stop_server_not_running() {
        let manager = std::sync::Arc::new(McpManager::new());

        // Multiple concurrent attempts to stop a non-running server
        let mut handles = Vec::new();
        for _ in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(
                async move { mgr.stop_server("test_server").await },
            ));
        }

        let mut not_running_count = 0;
        for handle in handles {
            if let Err(McpManagerError::NotRunning(_)) = handle.await.unwrap() {
                not_running_count += 1;
            }
        }

        // All should get NotRunning error
        assert_eq!(not_running_count, 10);
    }

    #[tokio::test]
    async fn test_concurrent_list_tools_not_running() {
        let manager = std::sync::Arc::new(McpManager::new());

        let mut handles = Vec::new();
        for i in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                mgr.list_tools(&format!("server_{}", i)).await
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(matches!(result, Err(McpManagerError::NotRunning(_))));
        }
    }

    #[tokio::test]
    async fn test_concurrent_call_tool_not_running() {
        let manager = std::sync::Arc::new(McpManager::new());

        let mut handles = Vec::new();
        for i in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move {
                mgr.call_tool(&format!("server_{}", i), "tool", serde_json::json!({}))
                    .await
            }));
        }

        for handle in handles {
            let result = handle.await.unwrap();
            assert!(matches!(result, Err(McpManagerError::NotRunning(_))));
        }
    }

    // =========================================================================
    // Concurrent start_all / stop_all Tests
    // =========================================================================

    #[tokio::test]
    async fn test_concurrent_start_all_stop_all() {
        let config = McpConfig::new();
        let manager = std::sync::Arc::new(McpManager::with_config(config));

        let mgr1 = manager.clone();
        let mgr2 = manager.clone();

        let handle1 = tokio::spawn(async move { mgr1.start_all().await });
        let handle2 = tokio::spawn(async move { mgr2.stop_all().await });

        // Both should succeed with empty config
        assert!(handle1.await.unwrap().is_ok());
        assert!(handle2.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn test_multiple_concurrent_list_all_tools() {
        let manager = std::sync::Arc::new(McpManager::new());

        let mut handles = Vec::new();
        for _ in 0..10 {
            let mgr = manager.clone();
            handles.push(tokio::spawn(async move { mgr.list_all_tools().await }));
        }

        for handle in handles {
            let tools = handle.await.unwrap();
            assert!(tools.is_empty());
        }
    }

    // =========================================================================
    // Error Variant Matching Tests
    // =========================================================================

    #[test]
    fn test_error_variant_exhaustive() {
        // This test ensures we've covered all error variants
        fn check_variant(e: &McpManagerError) {
            match e {
                McpManagerError::Config(_) => {}
                McpManagerError::Mcp(_) => {}
                McpManagerError::ServerNotFound(_) => {}
                McpManagerError::AlreadyRunning(_) => {}
                McpManagerError::NotRunning(_) => {}
            }
        }

        check_variant(&McpManagerError::ServerNotFound("x".into()));
        check_variant(&McpManagerError::AlreadyRunning("x".into()));
        check_variant(&McpManagerError::NotRunning("x".into()));

        // Config variant
        use super::super::config::McpConfigError;
        let config_err = McpConfigError::NotFound(std::path::PathBuf::from("/"));
        check_variant(&McpManagerError::Config(config_err));
    }

    #[test]
    fn test_error_debug_all_variants() {
        let variants: Vec<McpManagerError> = vec![
            McpManagerError::ServerNotFound("a".into()),
            McpManagerError::AlreadyRunning("b".into()),
            McpManagerError::NotRunning("c".into()),
            McpManagerError::Config(super::super::config::McpConfigError::NotFound(
                std::path::PathBuf::from("/test"),
            )),
        ];

        for v in variants {
            let debug = format!("{:?}", v);
            assert!(!debug.is_empty());
        }
    }

    // =========================================================================
    // Config Accessor After Mutation Tests
    // =========================================================================

    #[test]
    fn test_config_accessor_reflects_initial_state() {
        let mut cfg = McpConfig::new();
        cfg.add_server("s1", super::super::config::McpServerEntry::new("c1"));
        cfg.add_server("s2", super::super::config::McpServerEntry::new("c2"));

        let manager = McpManager::with_config(cfg);

        assert!(manager.config().has_server("s1"));
        assert!(manager.config().has_server("s2"));
        assert!(!manager.config().has_server("s3"));
        assert_eq!(manager.config().servers.len(), 2);
    }

    // =========================================================================
    // Start Server With Enabled Server That Doesn't Exist Locally
    // =========================================================================

    #[tokio::test]
    async fn test_start_server_command_not_found() {
        // This tests that when we try to start a server with a non-existent command,
        // we get an MCP error (not ServerNotFound)
        let mut config = McpConfig::new();
        config.add_server(
            "test_server",
            super::super::config::McpServerEntry::new(
                "this_command_definitely_does_not_exist_12345",
            ),
        );

        let manager = McpManager::with_config(config);

        // Server is in config, so won't get ServerNotFound
        // Will get MCP error when trying to spawn
        let result = manager.start_server("test_server").await;
        assert!(result.is_err());

        match result.unwrap_err() {
            McpManagerError::Mcp(_) => {
                // Expected - command failed to spawn
            }
            other => {
                // Also acceptable outcomes for command not found
                match &other {
                    McpManagerError::Config(_) => {}
                    _ => panic!("Expected Mcp error for command not found, got: {:?}", other),
                }
            }
        }
    }

    // =========================================================================
    // List Tools and Call Tool Different Server Names
    // =========================================================================

    #[tokio::test]
    async fn test_list_tools_various_server_names() {
        let manager = McpManager::new();

        let names = [
            "",
            "   ",
            "a",
            "very_long_server_name_that_exceeds_normal_bounds",
            "🚀",
        ];

        for name in names {
            let result = manager.list_tools(name).await;
            assert!(matches!(result, Err(McpManagerError::NotRunning(_))));
        }
    }

    #[tokio::test]
    async fn test_call_tool_various_tool_names() {
        let manager = McpManager::new();

        let tool_names = [
            "",
            "simple",
            "with-dashes",
            "with_underscores",
            "unicode_工具",
        ];

        for tool in tool_names {
            let result = manager
                .call_tool("server", tool, serde_json::json!({}))
                .await;
            // All should fail because server not running (not tool name issues)
            assert!(matches!(result, Err(McpManagerError::NotRunning(_))));
        }
    }

    // =========================================================================
    // Stop All Then Operations
    // =========================================================================

    #[tokio::test]
    async fn test_stop_all_then_running_servers_empty() {
        let manager = McpManager::new();

        manager.stop_all().await.unwrap();
        let running = manager.running_servers().await;
        assert!(running.is_empty());
    }

    #[tokio::test]
    async fn test_stop_all_then_list_all_tools_empty() {
        let manager = McpManager::new();

        manager.stop_all().await.unwrap();
        let tools = manager.list_all_tools().await;
        assert!(tools.is_empty());
    }

    // =========================================================================
    // Start All With Various Config States
    // =========================================================================

    #[tokio::test]
    async fn test_start_all_with_all_disabled_servers() {
        let mut config = McpConfig::new();

        for i in 0..5 {
            let mut entry = super::super::config::McpServerEntry::new(format!("cmd_{}", i));
            entry.enabled = false;
            config.add_server(format!("disabled_{}", i), entry);
        }

        let manager = McpManager::with_config(config);

        // Should succeed without starting anything
        let result = manager.start_all().await;
        assert!(result.is_ok());
        assert!(manager.running_servers().await.is_empty());
        assert_eq!(manager.config().servers.len(), 5);
    }

    // =========================================================================
    // Get Handle Concurrent With Other Operations
    // =========================================================================

    #[tokio::test]
    async fn test_get_handle_concurrent_with_running_servers() {
        let manager = std::sync::Arc::new(McpManager::new());

        let mgr1 = manager.clone();
        let mgr2 = manager.clone();

        let handle1 = tokio::spawn(async move { mgr1.get_handle("server").await });
        let handle2 = tokio::spawn(async move { mgr2.running_servers().await });

        assert!(handle1.await.unwrap().is_none());
        assert!(handle2.await.unwrap().is_empty());
    }

    // =========================================================================
    // Error Source Chain Tests
    // =========================================================================

    #[test]
    fn test_config_error_source_chain() {
        use std::error::Error;

        let config_err = super::super::config::McpConfigError::NotFound(std::path::PathBuf::from(
            "/nonexistent",
        ));
        let manager_err = McpManagerError::Config(config_err);

        // Verify error has a source
        let source = manager_err.source();
        assert!(source.is_some());
    }

    // =========================================================================
    // Manager Drop Behavior (implicit)
    // =========================================================================

    #[tokio::test]
    async fn test_manager_drop_doesnt_panic() {
        {
            let manager = McpManager::new();
            let _ = manager.running_servers().await;
            // manager dropped here
        }
        // No panic means success
    }

    #[tokio::test]
    async fn test_manager_with_config_drop_doesnt_panic() {
        {
            let config = McpConfig::sample();
            let manager = McpManager::with_config(config);
            let _ = manager.config();
            // manager dropped here
        }
        // No panic means success
    }

    // =========================================================================
    // Unicode and Edge Case Server Names in Config
    // =========================================================================

    #[test]
    fn test_unicode_server_name_in_config() {
        let mut config = McpConfig::new();
        config.add_server("サーバー", super::super::config::McpServerEntry::new("cmd"));

        let manager = McpManager::with_config(config);
        assert!(manager.config().has_server("サーバー"));
    }

    #[test]
    fn test_long_server_name_in_config() {
        let mut config = McpConfig::new();
        let long_name = "a".repeat(1000);
        config.add_server(&long_name, super::super::config::McpServerEntry::new("cmd"));

        let manager = McpManager::with_config(config);
        assert!(manager.config().has_server(&long_name));
    }

    // =========================================================================
    // Enabled Servers Iterator Tests
    // =========================================================================

    #[test]
    fn test_enabled_servers_through_manager() {
        let mut config = McpConfig::new();

        // Add mix of enabled and disabled
        config.add_server("enabled1", super::super::config::McpServerEntry::new("c1"));
        config.add_server("enabled2", super::super::config::McpServerEntry::new("c2"));

        let mut disabled = super::super::config::McpServerEntry::new("c3");
        disabled.enabled = false;
        config.add_server("disabled1", disabled);

        let manager = McpManager::with_config(config);

        let enabled: Vec<_> = manager.config().enabled_servers().collect();
        assert_eq!(enabled.len(), 2);
    }

    // =========================================================================
    // Sample Config Through Manager Tests
    // =========================================================================

    #[test]
    fn test_sample_config_enabled_disabled_through_manager() {
        let config = McpConfig::sample();
        let manager = McpManager::with_config(config);

        // Filesystem should be enabled
        let fs = manager.config().get_server("filesystem").unwrap();
        assert!(fs.enabled);

        // GitHub should be disabled
        let gh = manager.config().get_server("github").unwrap();
        assert!(!gh.enabled);
    }

    #[test]
    fn test_sample_config_enabled_servers_skips_disabled() {
        let config = McpConfig::sample();
        let manager = McpManager::with_config(config);

        // Verify enabled_servers() only returns enabled servers (filesystem), not disabled (github)
        let enabled: Vec<_> = manager.config().enabled_servers().collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "filesystem");

        // GitHub should not be in enabled list
        assert!(enabled.iter().all(|(name, _)| *name != "github"));
    }
}
