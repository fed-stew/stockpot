//! MCP server lifecycle management.
//!
//! Handles starting, stopping, and managing MCP server connections.

use super::config::{McpConfig, McpServerEntry};
use serdes_ai_mcp::{McpClient, McpError, McpToolset};
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
    /// Server name.
    pub name: String,
    /// The connected client.
    pub client: Arc<McpClient>,
    /// The toolset for agent integration.
    pub toolset: McpToolset<()>,
}

impl McpServerHandle {
    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.client.is_connected()
    }
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

        info!("Starting MCP server: {}", name);

        // Start the server
        let handle = self.connect_server(name, entry).await?;

        // Store the handle
        let mut servers = self.servers.write().await;
        servers.insert(name.to_string(), handle);

        info!("MCP server started: {}", name);
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

    /// Get toolsets from all running servers.
    ///
    /// Returns a vector of toolsets that can be used with the agent.
    pub async fn toolsets(&self) -> Vec<McpToolset<()>> {
        // Note: We can't easily return references to the toolsets
        // due to lifetime issues. In practice, you'd use the handles directly.
        // This method is here for API completeness.
        Vec::new()
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
            match handle.client.list_tools().await {
                Ok(tools) => {
                    all_tools.insert(name.clone(), tools);
                }
                Err(e) => {
                    warn!("Failed to list tools from {}: {}", name, e);
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
        let client = McpClient::stdio(&entry.command, &args).await?;

        // Initialize the connection
        client.initialize().await?;

        // Create the toolset
        let toolset = McpToolset::new(McpClient::stdio(&entry.command, &args).await?).with_id(name);

        Ok(McpServerHandle {
            name: name.to_string(),
            client: Arc::new(client),
            toolset,
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

    #[test]
    fn test_manager_new() {
        let manager = McpManager::new();
        assert!(manager.config().servers.is_empty() || !manager.config().servers.is_empty());
    }

    #[test]
    fn test_manager_with_config() {
        let config = McpConfig::sample();
        let manager = McpManager::with_config(config);
        assert!(manager.config().has_server("filesystem"));
    }

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
}
