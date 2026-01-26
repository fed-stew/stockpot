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
        let expanded: HashMap<String, String> = self
            .env
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
    use tempfile::TempDir;

    // =========================================================================
    // McpServerEntry Tests
    // =========================================================================

    #[test]
    fn test_server_entry_new() {
        let entry = McpServerEntry::new("npx");

        assert_eq!(entry.command, "npx");
        assert!(entry.args.is_empty());
        assert!(entry.env.is_empty());
        assert!(entry.enabled);
        assert!(entry.description.is_none());
    }

    #[test]
    fn test_server_entry_with_args() {
        let entry =
            McpServerEntry::new("npx").with_args(vec!["-y".to_string(), "server".to_string()]);

        assert_eq!(entry.args.len(), 2);
        assert_eq!(entry.args[0], "-y");
        assert_eq!(entry.args[1], "server");
    }

    #[test]
    fn test_server_entry_with_env() {
        let entry = McpServerEntry::new("node")
            .with_env("API_KEY", "secret")
            .with_env("DEBUG", "true");

        assert_eq!(entry.env.len(), 2);
        assert_eq!(entry.env.get("API_KEY"), Some(&"secret".to_string()));
        assert_eq!(entry.env.get("DEBUG"), Some(&"true".to_string()));
    }

    #[test]
    fn test_server_entry_with_description() {
        let entry = McpServerEntry::new("python").with_description("Python MCP server");

        assert_eq!(entry.description, Some("Python MCP server".to_string()));
    }

    #[test]
    fn test_server_entry_builder_chain() {
        let entry = McpServerEntry::new("npx")
            .with_args(vec!["-y".to_string(), "server".to_string()])
            .with_env("KEY", "value")
            .with_description("Test server");

        assert_eq!(entry.command, "npx");
        assert_eq!(entry.args.len(), 2);
        assert_eq!(entry.env.get("KEY"), Some(&"value".to_string()));
        assert_eq!(entry.description, Some("Test server".to_string()));
        assert!(entry.enabled);
    }

    #[test]
    fn test_server_entry_clone() {
        let entry = McpServerEntry::new("cmd")
            .with_args(vec!["arg1".to_string()])
            .with_env("KEY", "value");

        let cloned = entry.clone();
        assert_eq!(cloned.command, entry.command);
        assert_eq!(cloned.args, entry.args);
        assert_eq!(cloned.env, entry.env);
    }

    // =========================================================================
    // expand_env_var Tests
    // =========================================================================

    #[test]
    fn test_expand_env_var_simple() {
        std::env::set_var("TEST_VAR_SIMPLE", "test_value");

        let result = expand_env_var("${TEST_VAR_SIMPLE}");
        assert_eq!(result, "test_value");
    }

    #[test]
    fn test_expand_env_var_with_prefix_suffix() {
        std::env::set_var("TEST_VAR_PS", "middle");

        let result = expand_env_var("prefix_${TEST_VAR_PS}_suffix");
        assert_eq!(result, "prefix_middle_suffix");
    }

    #[test]
    fn test_expand_env_var_multiple() {
        std::env::set_var("VAR_A", "aaa");
        std::env::set_var("VAR_B", "bbb");

        let result = expand_env_var("${VAR_A}/${VAR_B}");
        assert_eq!(result, "aaa/bbb");
    }

    #[test]
    fn test_expand_env_var_nonexistent() {
        let result = expand_env_var("${NONEXISTENT_VAR_12345}");
        assert_eq!(result, "");
    }

    #[test]
    fn test_expand_env_var_no_vars() {
        let result = expand_env_var("no_variables_here");
        assert_eq!(result, "no_variables_here");
    }

    #[test]
    fn test_expand_env_var_unclosed_brace() {
        let result = expand_env_var("${UNCLOSED");
        assert_eq!(result, "${UNCLOSED");
    }

    #[test]
    fn test_expand_env_var_empty_var_name() {
        let result = expand_env_var("${}");
        // Empty var name returns empty string
        assert_eq!(result, "");
    }

    #[test]
    fn test_server_entry_expand_env_vars() {
        std::env::set_var("MCP_TEST_TOKEN", "secret123");

        let mut entry = McpServerEntry::new("cmd")
            .with_args(vec!["--token=${MCP_TEST_TOKEN}".to_string()])
            .with_env("AUTH", "${MCP_TEST_TOKEN}");

        entry.expand_env_vars();

        assert_eq!(entry.args[0], "--token=secret123");
        assert_eq!(entry.env.get("AUTH"), Some(&"secret123".to_string()));
    }

    // =========================================================================
    // McpConfig Tests
    // =========================================================================

    #[test]
    fn test_config_new_empty() {
        let config = McpConfig::new();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_config_default() {
        let config = McpConfig::default();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_config_add_server() {
        let mut config = McpConfig::new();
        config.add_server("test", McpServerEntry::new("cmd"));

        assert!(config.has_server("test"));
        assert_eq!(config.servers.len(), 1);
    }

    #[test]
    fn test_config_remove_server() {
        let mut config = McpConfig::new();
        config.add_server("test", McpServerEntry::new("cmd"));

        let removed = config.remove_server("test");
        assert!(removed.is_some());
        assert!(!config.has_server("test"));
    }

    #[test]
    fn test_config_remove_nonexistent() {
        let mut config = McpConfig::new();
        let removed = config.remove_server("nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_config_get_server() {
        let mut config = McpConfig::new();
        config.add_server("test", McpServerEntry::new("cmd"));

        let server = config.get_server("test");
        assert!(server.is_some());
        assert_eq!(server.unwrap().command, "cmd");
    }

    #[test]
    fn test_config_get_server_nonexistent() {
        let config = McpConfig::new();
        assert!(config.get_server("nonexistent").is_none());
    }

    #[test]
    fn test_config_has_server() {
        let mut config = McpConfig::new();
        config.add_server("exists", McpServerEntry::new("cmd"));

        assert!(config.has_server("exists"));
        assert!(!config.has_server("not_exists"));
    }

    // =========================================================================
    // Enabled Servers Tests
    // =========================================================================

    #[test]
    fn test_enabled_servers_all_enabled() {
        let mut config = McpConfig::new();
        config.add_server("server1", McpServerEntry::new("cmd1"));
        config.add_server("server2", McpServerEntry::new("cmd2"));

        let enabled: Vec<_> = config.enabled_servers().collect();
        assert_eq!(enabled.len(), 2);
    }

    #[test]
    fn test_enabled_servers_some_disabled() {
        let mut config = McpConfig::new();
        config.add_server("enabled", McpServerEntry::new("cmd1"));

        let mut disabled = McpServerEntry::new("cmd2");
        disabled.enabled = false;
        config.add_server("disabled", disabled);

        let enabled: Vec<_> = config.enabled_servers().collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "enabled");
    }

    #[test]
    fn test_enabled_servers_none_enabled() {
        let mut config = McpConfig::new();

        let mut disabled1 = McpServerEntry::new("cmd1");
        disabled1.enabled = false;
        config.add_server("disabled1", disabled1);

        let mut disabled2 = McpServerEntry::new("cmd2");
        disabled2.enabled = false;
        config.add_server("disabled2", disabled2);

        let enabled: Vec<_> = config.enabled_servers().collect();
        assert!(enabled.is_empty());
    }

    // =========================================================================
    // Sample Config Tests
    // =========================================================================

    #[test]
    fn test_sample_config() {
        let config = McpConfig::sample();

        assert!(config.has_server("filesystem"));
        assert!(config.has_server("github"));
    }

    #[test]
    fn test_sample_config_filesystem_enabled() {
        let config = McpConfig::sample();
        let fs = config.get_server("filesystem").unwrap();

        assert!(fs.enabled);
        assert_eq!(fs.command, "npx");
        assert!(!fs.args.is_empty());
    }

    #[test]
    fn test_sample_config_github_disabled() {
        let config = McpConfig::sample();
        let github = config.get_server("github").unwrap();

        assert!(!github.enabled);
    }

    #[test]
    fn test_sample_enabled_servers() {
        let config = McpConfig::sample();
        let enabled: Vec<_> = config.enabled_servers().collect();

        // Only filesystem should be enabled in sample
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "filesystem");
    }

    // =========================================================================
    // Serialization Tests
    // =========================================================================

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
    fn test_config_serialization_empty() {
        let config = McpConfig::new();

        let json = serde_json::to_string(&config).unwrap();
        let parsed: McpConfig = serde_json::from_str(&json).unwrap();

        assert!(parsed.servers.is_empty());
    }

    #[test]
    fn test_server_entry_serialization() {
        let entry = McpServerEntry::new("npx")
            .with_args(vec!["-y".to_string(), "server".to_string()])
            .with_env("KEY", "value")
            .with_description("Test");

        let json = serde_json::to_string(&entry).unwrap();
        let parsed: McpServerEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.command, "npx");
        assert_eq!(parsed.args.len(), 2);
        assert_eq!(parsed.env.get("KEY"), Some(&"value".to_string()));
        assert_eq!(parsed.description, Some("Test".to_string()));
    }

    #[test]
    fn test_server_entry_deserialization_defaults() {
        // Minimal JSON - should use defaults
        let json = r#"{"command": "test"}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();

        assert_eq!(entry.command, "test");
        assert!(entry.args.is_empty());
        assert!(entry.env.is_empty());
        assert!(entry.enabled); // default_enabled
        assert!(entry.description.is_none());
    }

    // =========================================================================
    // File Operations Tests
    // =========================================================================

    #[test]
    fn test_save_and_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("mcp_servers.json");

        let mut config = McpConfig::new();
        config.add_server(
            "test",
            McpServerEntry::new("cmd").with_description("Test server"),
        );

        config.save_to_path(&path).unwrap();
        assert!(path.exists());

        let loaded = McpConfig::load_from_path(&path).unwrap();
        assert!(loaded.has_server("test"));
        assert_eq!(
            loaded.get_server("test").unwrap().description,
            Some("Test server".to_string())
        );
    }

    #[test]
    fn test_load_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/path/config.json");
        let result = McpConfig::load_from_path(&path);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpConfigError::NotFound(_)));
    }

    #[test]
    fn test_load_invalid_json() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("invalid.json");

        fs::write(&path, "not valid json {{{").unwrap();

        let result = McpConfig::load_from_path(&path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpConfigError::ParseError(_)));
    }

    #[test]
    fn test_load_or_default() {
        // Should return default when file doesn't exist
        let config = McpConfig::load_or_default();
        // Just verify it doesn't panic and returns something
        let _ = config.servers.len();
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("nested").join("dir").join("config.json");

        let config = McpConfig::new();
        config.save_to_path(&path).unwrap();

        assert!(path.exists());
    }

    // =========================================================================
    // Default Path Tests
    // =========================================================================

    #[test]
    fn test_default_config_path() {
        let path = McpConfig::default_config_path();
        assert!(path.to_string_lossy().contains("mcp_servers.json"));
        assert!(path.to_string_lossy().contains(".stockpot"));
    }

    // =========================================================================
    // Error Type Tests
    // =========================================================================

    #[test]
    fn test_error_display() {
        let err = McpConfigError::NotFound(PathBuf::from("/test/path"));
        let msg = format!("{}", err);
        assert!(msg.contains("not found"));
        assert!(msg.contains("/test/path"));
    }

    #[test]
    fn test_error_debug() {
        let err = McpConfigError::NotFound(PathBuf::from("/test"));
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
    }

    // =========================================================================
    // Config Parsing Edge Cases
    // =========================================================================

    #[test]
    fn test_parse_empty_servers_object() {
        let json = r#"{"servers": {}}"#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_parse_missing_servers_field() {
        let json = r#"{}"#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_parse_extra_unknown_fields() {
        // serde by default ignores unknown fields
        let json = r#"{"servers": {}, "unknown_field": "ignored", "another": 123}"#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_parse_server_with_extra_fields() {
        let json = r#"{"command": "test", "extra": "ignored"}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.command, "test");
    }

    #[test]
    fn test_parse_enabled_explicit_true() {
        let json = r#"{"command": "cmd", "enabled": true}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert!(entry.enabled);
    }

    #[test]
    fn test_parse_enabled_explicit_false() {
        let json = r#"{"command": "cmd", "enabled": false}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert!(!entry.enabled);
    }

    #[test]
    fn test_parse_empty_args_array() {
        let json = r#"{"command": "cmd", "args": []}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert!(entry.args.is_empty());
    }

    #[test]
    fn test_parse_empty_env_object() {
        let json = r#"{"command": "cmd", "env": {}}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert!(entry.env.is_empty());
    }

    #[test]
    fn test_parse_null_description() {
        let json = r#"{"command": "cmd", "description": null}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert!(entry.description.is_none());
    }

    #[test]
    fn test_parse_empty_string_description() {
        let json = r#"{"command": "cmd", "description": ""}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.description, Some("".to_string()));
    }

    #[test]
    fn test_parse_empty_command() {
        let json = r#"{"command": ""}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.command, "");
    }

    #[test]
    fn test_parse_command_with_spaces() {
        let json = r#"{"command": "/path/to/my command"}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.command, "/path/to/my command");
    }

    #[test]
    fn test_parse_unicode_in_args() {
        let json = r#"{"command": "cmd", "args": ["日本語", "émoji 🎉"]}"#;
        let entry: McpServerEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.args[0], "日本語");
        assert_eq!(entry.args[1], "émoji 🎉");
    }

    #[test]
    fn test_parse_fails_missing_command() {
        let json = r#"{"args": ["test"]}"#;
        let result: Result<McpServerEntry, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fails_wrong_type_command() {
        let json = r#"{"command": 123}"#;
        let result: Result<McpServerEntry, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fails_wrong_type_args() {
        let json = r#"{"command": "cmd", "args": "not_array"}"#;
        let result: Result<McpServerEntry, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fails_wrong_type_env() {
        let json = r#"{"command": "cmd", "env": ["not", "object"]}"#;
        let result: Result<McpServerEntry, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_fails_wrong_type_enabled() {
        let json = r#"{"command": "cmd", "enabled": "yes"}"#;
        let result: Result<McpServerEntry, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    // =========================================================================
    // Environment Variable Edge Cases
    // =========================================================================

    #[test]
    fn test_expand_env_var_empty_string_value() {
        std::env::set_var("EMPTY_VAR_TEST", "");
        let result = expand_env_var("before${EMPTY_VAR_TEST}after");
        assert_eq!(result, "beforeafter");
    }

    #[test]
    fn test_expand_env_var_with_special_chars() {
        std::env::set_var("SPECIAL_CHARS_VAR", "a=b&c=d");
        let result = expand_env_var("${SPECIAL_CHARS_VAR}");
        assert_eq!(result, "a=b&c=d");
    }

    #[test]
    fn test_expand_env_var_dollar_sign_without_brace() {
        let result = expand_env_var("$NOT_A_VAR");
        assert_eq!(result, "$NOT_A_VAR");
    }

    #[test]
    fn test_expand_env_var_nested_braces() {
        // ${${VAR}} - only outer brace pair should be matched
        std::env::set_var("INNER", "value");
        let result = expand_env_var("${${INNER}}");
        // First expansion tries to find var named "${INNER" which doesn't exist
        // Actually, let's trace: finds "${" at 0, finds "}" at 9, var_name = "${INNER"
        assert_eq!(result, "}");
    }

    #[test]
    fn test_expand_env_var_consecutive() {
        std::env::set_var("CONS_A", "X");
        std::env::set_var("CONS_B", "Y");
        let result = expand_env_var("${CONS_A}${CONS_B}");
        assert_eq!(result, "XY");
    }

    #[test]
    fn test_expand_env_var_in_path() {
        std::env::set_var("HOME_TEST_VAR", "/home/user");
        let result = expand_env_var("${HOME_TEST_VAR}/.config/app");
        assert_eq!(result, "/home/user/.config/app");
    }

    #[test]
    fn test_expand_env_var_partial_syntax() {
        // Just "$" at end
        let result = expand_env_var("test$");
        assert_eq!(result, "test$");
    }

    #[test]
    fn test_expand_env_var_just_opening() {
        let result = expand_env_var("test${");
        assert_eq!(result, "test${");
    }

    #[test]
    fn test_expand_env_var_brace_in_value() {
        std::env::set_var("BRACE_VAL", "has}brace");
        let result = expand_env_var("${BRACE_VAL}");
        assert_eq!(result, "has}brace");
    }

    #[test]
    fn test_server_entry_expand_multiple_args() {
        std::env::set_var("ARG_VAR_1", "val1");
        std::env::set_var("ARG_VAR_2", "val2");

        let mut entry = McpServerEntry::new("cmd").with_args(vec![
            "${ARG_VAR_1}".to_string(),
            "static".to_string(),
            "${ARG_VAR_2}".to_string(),
        ]);

        entry.expand_env_vars();

        assert_eq!(entry.args[0], "val1");
        assert_eq!(entry.args[1], "static");
        assert_eq!(entry.args[2], "val2");
    }

    #[test]
    fn test_server_entry_expand_multiple_env_keys() {
        std::env::set_var("ENV_KEY_VAR", "expanded");

        let mut entry = McpServerEntry::new("cmd")
            .with_env("KEY1", "${ENV_KEY_VAR}")
            .with_env("KEY2", "static")
            .with_env("KEY3", "${ENV_KEY_VAR}_suffix");

        entry.expand_env_vars();

        assert_eq!(entry.env.get("KEY1"), Some(&"expanded".to_string()));
        assert_eq!(entry.env.get("KEY2"), Some(&"static".to_string()));
        assert_eq!(entry.env.get("KEY3"), Some(&"expanded_suffix".to_string()));
    }

    // =========================================================================
    // Server Configuration Edge Cases
    // =========================================================================

    #[test]
    fn test_add_server_overwrites_existing() {
        let mut config = McpConfig::new();
        config.add_server("test", McpServerEntry::new("cmd1"));
        config.add_server("test", McpServerEntry::new("cmd2"));

        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.get_server("test").unwrap().command, "cmd2");
    }

    #[test]
    fn test_server_name_with_special_chars() {
        let mut config = McpConfig::new();
        config.add_server("server-with_special.chars:v1", McpServerEntry::new("cmd"));

        assert!(config.has_server("server-with_special.chars:v1"));
    }

    #[test]
    fn test_server_name_empty_string() {
        let mut config = McpConfig::new();
        config.add_server("", McpServerEntry::new("cmd"));

        assert!(config.has_server(""));
        assert!(config.get_server("").is_some());
    }

    #[test]
    fn test_server_name_unicode() {
        let mut config = McpConfig::new();
        config.add_server("サーバー", McpServerEntry::new("cmd"));

        assert!(config.has_server("サーバー"));
    }

    #[test]
    fn test_clone_isolation() {
        let entry = McpServerEntry::new("cmd")
            .with_args(vec!["arg".to_string()])
            .with_env("KEY", "value");

        let mut cloned = entry.clone();
        cloned.args.push("new_arg".to_string());
        cloned
            .env
            .insert("NEW_KEY".to_string(), "new_value".to_string());

        // Original should be unchanged
        assert_eq!(entry.args.len(), 1);
        assert!(!entry.env.contains_key("NEW_KEY"));
    }

    #[test]
    fn test_config_clone_isolation() {
        let mut config = McpConfig::new();
        config.add_server("test", McpServerEntry::new("cmd"));

        let mut cloned = config.clone();
        cloned.add_server("new_server", McpServerEntry::new("cmd2"));

        assert!(!config.has_server("new_server"));
    }

    // =========================================================================
    // File Operations Edge Cases
    // =========================================================================

    #[test]
    fn test_load_expands_env_vars() {
        std::env::set_var("LOAD_TEST_VAR", "loaded_value");

        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.json");

        let json = r#"{
            "servers": {
                "test": {
                    "command": "cmd",
                    "args": ["${LOAD_TEST_VAR}"],
                    "env": {"KEY": "${LOAD_TEST_VAR}"}
                }
            }
        }"#;
        fs::write(&path, json).unwrap();

        let config = McpConfig::load_from_path(&path).unwrap();
        let server = config.get_server("test").unwrap();

        assert_eq!(server.args[0], "loaded_value");
        assert_eq!(server.env.get("KEY"), Some(&"loaded_value".to_string()));
    }

    #[test]
    fn test_save_does_not_expand_env_vars() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.json");

        let mut config = McpConfig::new();
        config.add_server(
            "test",
            McpServerEntry::new("cmd")
                .with_args(vec!["${SOME_VAR}".to_string()])
                .with_env("KEY", "${ANOTHER_VAR}"),
        );

        config.save_to_path(&path).unwrap();

        // Read raw content to verify vars not expanded
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("${SOME_VAR}"));
        assert!(content.contains("${ANOTHER_VAR}"));
    }

    #[test]
    fn test_load_empty_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("empty.json");

        fs::write(&path, "").unwrap();

        let result = McpConfig::load_from_path(&path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpConfigError::ParseError(_)));
    }

    #[test]
    fn test_load_whitespace_only_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("whitespace.json");

        fs::write(&path, "   \n\t  ").unwrap();

        let result = McpConfig::load_from_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_json_null() {
        // JSON null should fail to parse as McpConfig
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("null.json");

        fs::write(&path, "null").unwrap();

        let result = McpConfig::load_from_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_servers_as_array() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("wrong_servers.json");

        fs::write(&path, r#"{"servers": []}"#).unwrap();

        let result = McpConfig::load_from_path(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_save_pretty_prints() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("pretty.json");

        let mut config = McpConfig::new();
        config.add_server("test", McpServerEntry::new("cmd"));

        config.save_to_path(&path).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        // Pretty print should have newlines
        assert!(content.contains('\n'));
    }

    // =========================================================================
    // Serialization Edge Cases
    // =========================================================================

    #[test]
    fn test_description_not_serialized_when_none() {
        let entry = McpServerEntry::new("cmd");
        let json = serde_json::to_string(&entry).unwrap();

        // skip_serializing_if = "Option::is_none" should exclude description
        assert!(!json.contains("description"));
    }

    #[test]
    fn test_description_serialized_when_some() {
        let entry = McpServerEntry::new("cmd").with_description("test");
        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("description"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_roundtrip_complex_config() {
        let mut config = McpConfig::new();

        config.add_server(
            "server1",
            McpServerEntry::new("npx")
                .with_args(vec!["-y".to_string(), "@scope/package".to_string()])
                .with_env("API_KEY", "${SECRET}")
                .with_env("DEBUG", "true")
                .with_description("Complex server"),
        );

        let mut disabled = McpServerEntry::new("python")
            .with_args(vec!["-m".to_string(), "mcp_server".to_string()]);
        disabled.enabled = false;
        config.add_server("server2", disabled);

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: McpConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.servers.len(), 2);

        let s1 = parsed.get_server("server1").unwrap();
        assert_eq!(s1.command, "npx");
        assert_eq!(s1.args.len(), 2);
        assert_eq!(s1.env.len(), 2);
        assert!(s1.enabled);
        assert_eq!(s1.description, Some("Complex server".to_string()));

        let s2 = parsed.get_server("server2").unwrap();
        assert!(!s2.enabled);
        assert!(s2.description.is_none());
    }

    // =========================================================================
    // Error Type Edge Cases
    // =========================================================================

    #[test]
    fn test_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let config_err: McpConfigError = io_err.into();

        let msg = format!("{}", config_err);
        assert!(msg.contains("read config file"));
    }

    #[test]
    fn test_error_from_json_error() {
        let json_err = serde_json::from_str::<McpConfig>("invalid").unwrap_err();
        let config_err: McpConfigError = json_err.into();

        let msg = format!("{}", config_err);
        assert!(msg.contains("parse config file"));
    }
}
