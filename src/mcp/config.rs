//! MCP configuration file handling.
//!
//! Loads and parses MCP server configurations from JSON files.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error type for MCP configuration operations.
#[derive(Debug, Error)]
pub enum McpConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_json::Error),
    
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),
}

/// MCP server entry in the configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    /// Command to run the MCP server.
    pub command: String,
    
    /// Arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,
    
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
    
    /// Whether this server is enabled.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Optional description of the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_enabled() -> bool {
    true
}

impl McpServerEntry {
    /// Create a new MCP server entry.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
            enabled: true,
            description: None,
        }
    }

    /// Add arguments.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Expand environment variables in the config.
    /// 
    /// Replaces `${VAR_NAME}` patterns with actual environment values.
    pub fn expand_env_vars(&mut self) {
        // Expand in args
        for arg in &mut self.args {
            *arg = expand_env_var(arg);
        }
        
        // Expand in env values
        let expanded: HashMap<String, String> = self.env
            .iter()
            .map(|(k, v)| (k.clone(), expand_env_var(v)))
            .collect();
        self.env = expanded;
    }
}

/// Expand environment variables in a string.
/// 
/// Supports `${VAR_NAME}` syntax.
fn expand_env_var(s: &str) -> String {
    let mut result = s.to_string();
    
    // Simple regex-free expansion for ${VAR} pattern
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }
    
    result
}

/// Root configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    /// Map of server name to configuration.
    #[serde(default)]
    pub servers: HashMap<String, McpServerEntry>,
}

impl McpConfig {
    /// Create a new empty configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from the default path.
    /// 
    /// Default path: `~/.stockpot/mcp_servers.json`
    pub fn load_default() -> Result<Self, McpConfigError> {
        let path = Self::default_config_path();
        Self::load_from_path(&path)
    }

    /// Load configuration from a specific path.
    pub fn load_from_path(path: &Path) -> Result<Self, McpConfigError> {
        if !path.exists() {
            return Err(McpConfigError::NotFound(path.to_path_buf()));
        }
        
        let content = fs::read_to_string(path)?;
        let mut config: McpConfig = serde_json::from_str(&content)?;
        
        // Expand environment variables in all entries
        for entry in config.servers.values_mut() {
            entry.expand_env_vars();
        }
        
        Ok(config)
    }

    /// Try to load configuration, returning empty config if not found.
    pub fn load_or_default() -> Self {
        Self::load_default().unwrap_or_default()
    }

    /// Save configuration to the default path.
    pub fn save_default(&self) -> Result<(), McpConfigError> {
        let path = Self::default_config_path();
        self.save_to_path(&path)
    }

    /// Save configuration to a specific path.
    pub fn save_to_path(&self, path: &Path) -> Result<(), McpConfigError> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the default configuration path.
    pub fn default_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".stockpot")
            .join("mcp_servers.json")
    }

    /// Get enabled servers.
    pub fn enabled_servers(&self) -> impl Iterator<Item = (&String, &McpServerEntry)> {
        self.servers.iter().filter(|(_, entry)| entry.enabled)
    }

    /// Add a server to the configuration.
    pub fn add_server(&mut self, name: impl Into<String>, entry: McpServerEntry) {
        self.servers.insert(name.into(), entry);
    }

    /// Remove a server from the configuration.
    pub fn remove_server(&mut self, name: &str) -> Option<McpServerEntry> {
        self.servers.remove(name)
    }

    /// Check if a server exists.
    pub fn has_server(&self, name: &str) -> bool {
        self.servers.contains_key(name)
    }

    /// Get a server by name.
    pub fn get_server(&self, name: &str) -> Option<&McpServerEntry> {
        self.servers.get(name)
    }

    /// Create a sample configuration.
    pub fn sample() -> Self {
        let mut config = Self::new();
        
        // Filesystem server
        config.add_server(
            "filesystem",
            McpServerEntry::new("npx")
                .with_args(vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                    "/tmp".to_string(),
                ])
                .with_description("Access to filesystem operations".to_string()),
        );
        
        // GitHub server (disabled by default, needs token)
        let mut github = McpServerEntry::new("npx")
            .with_args(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-github".to_string(),
            ])
            .with_env("GITHUB_PERSONAL_ACCESS_TOKEN", "${GITHUB_TOKEN}")
            .with_description("GitHub API access".to_string());
        github.enabled = false;
        config.add_server("github", github);
        
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_entry_new() {
        let entry = McpServerEntry::new("npx")
            .with_args(vec!["-y".to_string(), "server".to_string()])
            .with_env("KEY", "value");
        
        assert_eq!(entry.command, "npx");
        assert_eq!(entry.args.len(), 2);
        assert_eq!(entry.env.get("KEY"), Some(&"value".to_string()));
        assert!(entry.enabled);
    }

    #[test]
    fn test_expand_env_var() {
        std::env::set_var("TEST_VAR", "test_value");
        
        let result = expand_env_var("prefix_${TEST_VAR}_suffix");
        assert_eq!(result, "prefix_test_value_suffix");
        
        let result = expand_env_var("no_var_here");
        assert_eq!(result, "no_var_here");
        
        let result = expand_env_var("${NONEXISTENT_VAR}");
        assert_eq!(result, "");
    }

    #[test]
    fn test_config_serialization() {
        let config = McpConfig::sample();
        
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: McpConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parsed.servers.len(), config.servers.len());
        assert!(parsed.has_server("filesystem"));
        assert!(parsed.has_server("github"));
    }

    #[test]
    fn test_enabled_servers() {
        let config = McpConfig::sample();
        let enabled: Vec<_> = config.enabled_servers().collect();
        
        // Only filesystem should be enabled in sample
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "filesystem");
    }

    #[test]
    fn test_default_config_path() {
        let path = McpConfig::default_config_path();
        assert!(path.to_string_lossy().contains("mcp_servers.json"));
    }
}
