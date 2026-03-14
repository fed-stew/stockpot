//! JSON-defined plugin loader.
//!
//! Loads plugin definitions from `~/.spot/plugins/*.json`.
//!
//! A JSON plugin bundles multiple agents (and optionally tools) into a
//! single file, extending the existing single-agent JSON format.

use super::{Plugin, PluginManifest};
use crate::agents::json_agent::{JsonAgent, JsonAgentDef};
use crate::agents::SpotAgent;
use serde::{Deserialize, Serialize};
use spot_tools::tools::registry::ArcTool;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error type for JSON plugin loading.
#[derive(Debug, Error)]
pub enum JsonPluginError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid plugin definition: {0}")]
    Invalid(String),
}

/// JSON plugin file definition.
///
/// This is the on-disk format for plugin JSON files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPluginDef {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Plugin description.
    #[serde(default)]
    pub description: Option<String>,
    /// Plugin author.
    #[serde(default)]
    pub author: Option<String>,
    /// Agent definitions provided by this plugin.
    #[serde(default)]
    pub agents: Vec<JsonAgentDef>,
    /// Tool names (reserved for future use; tools must be registered via Rust).
    #[serde(default)]
    pub tools: Vec<String>,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

/// A plugin loaded from a JSON file.
///
/// Implements the [`Plugin`] trait by parsing agent definitions from the
/// JSON plugin format.
#[derive(Debug, Clone)]
pub struct JsonPlugin {
    manifest: PluginManifest,
    agent_defs: Vec<JsonAgentDef>,
    source_path: Option<PathBuf>,
}

impl JsonPlugin {
    /// Create a new JSON plugin from a definition.
    pub fn new(def: JsonPluginDef) -> Result<Self, JsonPluginError> {
        if def.name.is_empty() {
            return Err(JsonPluginError::Invalid("name is required".to_string()));
        }

        // Validate agent definitions
        for (i, agent) in def.agents.iter().enumerate() {
            if agent.name.is_empty() {
                return Err(JsonPluginError::Invalid(format!(
                    "agent at index {} has empty name",
                    i
                )));
            }
            if agent.system_prompt.is_empty() {
                return Err(JsonPluginError::Invalid(format!(
                    "agent '{}' has empty system_prompt",
                    agent.name
                )));
            }
        }

        let manifest = PluginManifest {
            name: def.name,
            version: def.version,
            description: def.description.unwrap_or_default(),
            author: def.author,
        };

        Ok(Self {
            manifest,
            agent_defs: def.agents,
            source_path: None,
        })
    }

    /// Load a JSON plugin from a file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, JsonPluginError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)?;
        let def: JsonPluginDef = serde_json::from_str(&content)?;
        let mut plugin = Self::new(def)?;
        plugin.source_path = Some(path.to_path_buf());
        Ok(plugin)
    }

    /// Get the source file path, if loaded from a file.
    pub fn source_path(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }
}

impl Plugin for JsonPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn agents(&self) -> Vec<Box<dyn SpotAgent>> {
        self.agent_defs
            .iter()
            .map(|def| -> Box<dyn SpotAgent> { Box::new(JsonAgent::new(def.clone())) })
            .collect()
    }

    fn tools(&self) -> Vec<ArcTool> {
        // JSON plugins cannot define custom tool implementations.
        // The `tools` field in agent definitions refers to existing tools
        // by name, which are resolved by the tool registry.
        vec![]
    }

    fn on_init(&self) -> Result<(), String> {
        tracing::info!(
            plugin = %self.manifest.name,
            version = %self.manifest.version,
            agents = self.agent_defs.len(),
            "Initializing JSON plugin"
        );
        Ok(())
    }
}

/// Get the plugins directory path.
pub fn plugins_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".spot").join("plugins"))
        .unwrap_or_else(|| PathBuf::from(".spot/plugins"))
}

/// Load all JSON plugins from the plugins directory.
pub fn load_json_plugins() -> Vec<JsonPlugin> {
    load_json_plugins_from_dir(&plugins_dir())
}

/// Load JSON plugins from a specific directory.
pub fn load_json_plugins_from_dir(dir: &Path) -> Vec<JsonPlugin> {
    let mut plugins = Vec::new();

    if !dir.exists() {
        // Create the directory for discoverability.
        let _ = fs::create_dir_all(dir);
        return plugins;
    }

    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::warn!("Failed to read plugins directory {:?}: {}", dir, e);
            return plugins;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden and template files.
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if file_name.starts_with('_') || file_name.starts_with('.') {
            continue;
        }

        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false);
        if !is_json {
            continue;
        }

        match JsonPlugin::from_file(&path) {
            Ok(plugin) => {
                tracing::info!(
                    "Loaded plugin '{}' v{} from {:?}",
                    plugin.manifest.name,
                    plugin.manifest.version,
                    path
                );
                plugins.push(plugin);
            }
            Err(e) => {
                tracing::warn!("Failed to load plugin from {:?}: {}", path, e);
            }
        }
    }

    plugins
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // =========================================================================
    // JsonPluginDef Parsing Tests
    // =========================================================================

    #[test]
    fn test_parse_full_plugin_def() {
        let json = r#"{
            "name": "test-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "author": "Test Author",
            "agents": [
                {
                    "name": "agent-one",
                    "display_name": "Agent One",
                    "description": "First agent",
                    "system_prompt": "You are agent one.",
                    "tools": ["read_file", "grep"],
                    "visibility": "main"
                },
                {
                    "name": "agent-two",
                    "system_prompt": "You are agent two.",
                    "tools": ["edit_file"]
                }
            ],
            "tools": []
        }"#;

        let def: JsonPluginDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "test-plugin");
        assert_eq!(def.version, "1.0.0");
        assert_eq!(def.description, Some("A test plugin".to_string()));
        assert_eq!(def.author, Some("Test Author".to_string()));
        assert_eq!(def.agents.len(), 2);
        assert_eq!(def.agents[0].name, "agent-one");
        assert_eq!(def.agents[1].name, "agent-two");
    }

    #[test]
    fn test_parse_minimal_plugin_def() {
        let json = r#"{
            "name": "minimal",
            "agents": []
        }"#;

        let def: JsonPluginDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "minimal");
        assert_eq!(def.version, "0.1.0"); // default
        assert!(def.description.is_none());
        assert!(def.author.is_none());
        assert!(def.agents.is_empty());
        assert!(def.tools.is_empty());
    }

    #[test]
    fn test_parse_plugin_default_version() {
        let json = r#"{"name": "no-ver", "agents": []}"#;
        let def: JsonPluginDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.version, "0.1.0");
    }

    // =========================================================================
    // JsonPlugin::new() Tests
    // =========================================================================

    #[test]
    fn test_new_with_valid_def() {
        let def = JsonPluginDef {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test".to_string()),
            author: None,
            agents: vec![JsonAgentDef {
                name: "agent-1".to_string(),
                display_name: None,
                description: None,
                system_prompt: "You are a test agent.".to_string(),
                tools: vec![],
                model: None,
                capabilities: None,
                visibility: None,
            }],
            tools: vec![],
        };

        let plugin = JsonPlugin::new(def).unwrap();
        assert_eq!(plugin.manifest.name, "test");
        assert_eq!(plugin.manifest.version, "1.0.0");
        assert_eq!(plugin.manifest.description, "A test");
        assert!(plugin.source_path.is_none());
    }

    #[test]
    fn test_new_empty_name_fails() {
        let def = JsonPluginDef {
            name: "".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            agents: vec![],
            tools: vec![],
        };

        let err = JsonPlugin::new(def).unwrap_err();
        assert!(err.to_string().contains("name is required"));
    }

    #[test]
    fn test_new_agent_with_empty_name_fails() {
        let def = JsonPluginDef {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            agents: vec![JsonAgentDef {
                name: "".to_string(),
                display_name: None,
                description: None,
                system_prompt: "prompt".to_string(),
                tools: vec![],
                model: None,
                capabilities: None,
                visibility: None,
            }],
            tools: vec![],
        };

        let err = JsonPlugin::new(def).unwrap_err();
        assert!(err.to_string().contains("empty name"));
    }

    #[test]
    fn test_new_agent_with_empty_prompt_fails() {
        let def = JsonPluginDef {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            agents: vec![JsonAgentDef {
                name: "agent-1".to_string(),
                display_name: None,
                description: None,
                system_prompt: "".to_string(),
                tools: vec![],
                model: None,
                capabilities: None,
                visibility: None,
            }],
            tools: vec![],
        };

        let err = JsonPlugin::new(def).unwrap_err();
        assert!(err.to_string().contains("empty system_prompt"));
    }

    #[test]
    fn test_new_with_no_agents() {
        let def = JsonPluginDef {
            name: "empty-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            agents: vec![],
            tools: vec![],
        };

        let plugin = JsonPlugin::new(def).unwrap();
        assert_eq!(plugin.agents().len(), 0);
    }

    // =========================================================================
    // Plugin Trait Implementation Tests
    // =========================================================================

    #[test]
    fn test_manifest_returns_correct_data() {
        let def = JsonPluginDef {
            name: "my-plugin".to_string(),
            version: "2.0.0".to_string(),
            description: Some("My plugin".to_string()),
            author: Some("Me".to_string()),
            agents: vec![],
            tools: vec![],
        };

        let plugin = JsonPlugin::new(def).unwrap();
        let manifest = plugin.manifest();
        assert_eq!(manifest.name, "my-plugin");
        assert_eq!(manifest.version, "2.0.0");
        assert_eq!(manifest.description, "My plugin");
        assert_eq!(manifest.author, Some("Me".to_string()));
    }

    #[test]
    fn test_agents_returns_json_agents() {
        let def = JsonPluginDef {
            name: "multi".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            agents: vec![
                JsonAgentDef {
                    name: "agent-a".to_string(),
                    display_name: Some("Agent A".to_string()),
                    description: Some("First".to_string()),
                    system_prompt: "You are A.".to_string(),
                    tools: vec!["read_file".to_string()],
                    model: None,
                    capabilities: None,
                    visibility: None,
                },
                JsonAgentDef {
                    name: "agent-b".to_string(),
                    display_name: None,
                    description: None,
                    system_prompt: "You are B.".to_string(),
                    tools: vec![],
                    model: None,
                    capabilities: None,
                    visibility: None,
                },
            ],
            tools: vec![],
        };

        let plugin = JsonPlugin::new(def).unwrap();
        let agents = plugin.agents();

        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name(), "agent-a");
        assert_eq!(agents[0].display_name(), "Agent A");
        assert_eq!(agents[0].description(), "First");
        assert_eq!(agents[0].system_prompt(), "You are A.");
        assert_eq!(agents[0].available_tools(), vec!["read_file"]);

        assert_eq!(agents[1].name(), "agent-b");
        assert_eq!(agents[1].system_prompt(), "You are B.");
    }

    #[test]
    fn test_tools_returns_empty() {
        let def = JsonPluginDef {
            name: "no-tools".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            agents: vec![],
            tools: vec!["some_tool".to_string()],
        };

        let plugin = JsonPlugin::new(def).unwrap();
        // JSON plugins cannot define custom tool implementations.
        assert!(plugin.tools().is_empty());
    }

    #[test]
    fn test_on_init_succeeds() {
        let def = JsonPluginDef {
            name: "init-test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            agents: vec![],
            tools: vec![],
        };

        let plugin = JsonPlugin::new(def).unwrap();
        assert!(plugin.on_init().is_ok());
    }

    // =========================================================================
    // File Loading Tests
    // =========================================================================

    #[test]
    fn test_from_file_valid() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test-plugin.json");
        fs::write(
            &file,
            r#"{
                "name": "file-plugin",
                "version": "1.0.0",
                "description": "Loaded from file",
                "agents": [
                    {
                        "name": "file-agent",
                        "system_prompt": "You are a file agent."
                    }
                ]
            }"#,
        )
        .unwrap();

        let plugin = JsonPlugin::from_file(&file).unwrap();
        assert_eq!(plugin.manifest().name, "file-plugin");
        assert_eq!(plugin.source_path(), Some(file.as_path()));
        assert_eq!(plugin.agents().len(), 1);
    }

    #[test]
    fn test_from_file_not_found() {
        let err = JsonPlugin::from_file("/nonexistent/path.json").unwrap_err();
        assert!(matches!(err, JsonPluginError::Io(_)));
    }

    #[test]
    fn test_from_file_invalid_json() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("bad.json");
        fs::write(&file, "not json").unwrap();

        let err = JsonPlugin::from_file(&file).unwrap_err();
        assert!(matches!(err, JsonPluginError::Json(_)));
    }

    #[test]
    fn test_from_file_invalid_plugin() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("invalid.json");
        fs::write(&file, r#"{"name": "", "agents": []}"#).unwrap();

        let err = JsonPlugin::from_file(&file).unwrap_err();
        assert!(matches!(err, JsonPluginError::Invalid(_)));
    }

    // =========================================================================
    // Directory Loading Tests
    // =========================================================================

    #[test]
    fn test_load_from_dir_multiple_plugins() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("plugin-a.json"),
            r#"{"name": "plugin-a", "version": "1.0.0", "agents": [
                {"name": "a-agent", "system_prompt": "Agent A"}
            ]}"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("plugin-b.json"),
            r#"{"name": "plugin-b", "version": "2.0.0", "agents": [
                {"name": "b-agent", "system_prompt": "Agent B"}
            ]}"#,
        )
        .unwrap();

        let plugins = load_json_plugins_from_dir(dir.path());
        assert_eq!(plugins.len(), 2);

        let names: Vec<_> = plugins.iter().map(|p| p.manifest().name.as_str()).collect();
        assert!(names.contains(&"plugin-a"));
        assert!(names.contains(&"plugin-b"));
    }

    #[test]
    fn test_load_from_dir_skips_hidden_files() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join(".hidden.json"),
            r#"{"name": "hidden", "agents": []}"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("_template.json"),
            r#"{"name": "template", "agents": []}"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("visible.json"),
            r#"{"name": "visible", "agents": []}"#,
        )
        .unwrap();

        let plugins = load_json_plugins_from_dir(dir.path());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest().name, "visible");
    }

    #[test]
    fn test_load_from_dir_skips_non_json() {
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("readme.txt"), "not a plugin").unwrap();
        fs::write(dir.path().join("notes.md"), "# Notes").unwrap();
        fs::write(
            dir.path().join("real.json"),
            r#"{"name": "real", "agents": []}"#,
        )
        .unwrap();

        let plugins = load_json_plugins_from_dir(dir.path());
        assert_eq!(plugins.len(), 1);
    }

    #[test]
    fn test_load_from_dir_skips_invalid_files() {
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("bad.json"), "not json").unwrap();
        fs::write(
            dir.path().join("good.json"),
            r#"{"name": "good", "agents": []}"#,
        )
        .unwrap();

        let plugins = load_json_plugins_from_dir(dir.path());
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest().name, "good");
    }

    #[test]
    fn test_load_from_nonexistent_dir() {
        let dir = tempdir().unwrap();
        let nonexistent = dir.path().join("nonexistent");

        let plugins = load_json_plugins_from_dir(&nonexistent);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_load_from_empty_dir() {
        let dir = tempdir().unwrap();
        let plugins = load_json_plugins_from_dir(dir.path());
        assert!(plugins.is_empty());
    }

    // =========================================================================
    // Error Type Tests
    // =========================================================================

    #[test]
    fn test_error_display_io() {
        let err = JsonPluginError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "not found",
        ));
        assert!(err.to_string().contains("IO error"));
    }

    #[test]
    fn test_error_display_invalid() {
        let err = JsonPluginError::Invalid("bad data".to_string());
        assert!(err.to_string().contains("bad data"));
    }

    #[test]
    fn test_error_is_debug() {
        let err = JsonPluginError::Invalid("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("Invalid"));
    }
}
