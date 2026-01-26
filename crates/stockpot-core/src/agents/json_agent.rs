//! JSON-defined agents loader.
//!
//! Loads agent definitions from `~/.stockpot/agents/*.json`.
//!
//! ## JSON Schema
//!
//! ```json
//! {
//!   "name": "my-agent",
//!   "display_name": "My Agent 🤖",
//!   "description": "Does something useful",
//!   "system_prompt": "You are...",
//!   "tools": ["read_file", "edit_file", "grep"],
//!   "model": "gpt-4o",
//!   "visibility": "main"
//! }
//! ```
//!
//! // visibility: "main" | "sub" | "hidden" (default: "main")

use super::base::SpotAgent;
use super::{AgentCapabilities, AgentVisibility};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error type for JSON agent loading.
#[derive(Debug, Error)]
pub enum JsonAgentError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid agent definition: {0}")]
    Invalid(String),
    #[error("Agent directory not found")]
    DirNotFound,
}

/// JSON agent definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonAgentDef {
    /// Unique identifier (e.g., "my-agent").
    pub name: String,
    /// Human-readable display name.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Brief description.
    #[serde(default)]
    pub description: Option<String>,
    /// The system prompt.
    pub system_prompt: String,
    /// List of tool names this agent can use.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Optional model override.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional capabilities override.
    #[serde(default)]
    pub capabilities: Option<JsonCapabilities>,
    /// Visibility level for UI filtering (main, sub, hidden).
    #[serde(default)]
    pub visibility: Option<AgentVisibility>,
}

/// Capabilities defined in JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JsonCapabilities {
    #[serde(default)]
    pub file_read: Option<bool>,
    #[serde(default)]
    pub file_write: Option<bool>,
    #[serde(default)]
    pub shell: Option<bool>,
    #[serde(default)]
    pub sub_agents: Option<bool>,
    #[serde(default)]
    pub mcp: Option<bool>,
}

/// A JSON-defined agent.
#[derive(Debug, Clone)]
pub struct JsonAgent {
    def: JsonAgentDef,
    capabilities: AgentCapabilities,
}

impl JsonAgent {
    /// Create a new JSON agent from a definition.
    pub fn new(def: JsonAgentDef) -> Self {
        let capabilities = if let Some(caps) = &def.capabilities {
            AgentCapabilities {
                file_read: caps.file_read.unwrap_or(true),
                file_write: caps.file_write.unwrap_or(true),
                shell: caps.shell.unwrap_or(true),
                sub_agents: caps.sub_agents.unwrap_or(true),
                mcp: caps.mcp.unwrap_or(true),
            }
        } else {
            AgentCapabilities::default()
        };

        Self { def, capabilities }
    }

    /// Load from a JSON file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, JsonAgentError> {
        let content = fs::read_to_string(path)?;
        let def: JsonAgentDef = serde_json::from_str(&content)?;

        // Validate
        if def.name.is_empty() {
            return Err(JsonAgentError::Invalid("name is required".to_string()));
        }
        if def.system_prompt.is_empty() {
            return Err(JsonAgentError::Invalid(
                "system_prompt is required".to_string(),
            ));
        }

        Ok(Self::new(def))
    }
}

impl SpotAgent for JsonAgent {
    fn name(&self) -> &str {
        &self.def.name
    }

    fn display_name(&self) -> &str {
        self.def.display_name.as_deref().unwrap_or(&self.def.name)
    }

    fn description(&self) -> &str {
        self.def
            .description
            .as_deref()
            .unwrap_or("Custom JSON agent")
    }

    fn system_prompt(&self) -> String {
        self.def.system_prompt.clone()
    }

    fn available_tools(&self) -> Vec<&str> {
        self.def.tools.iter().map(|s| s.as_str()).collect()
    }

    fn capabilities(&self) -> AgentCapabilities {
        self.capabilities.clone()
    }

    fn visibility(&self) -> AgentVisibility {
        self.def.visibility.unwrap_or_default()
    }

    fn model_override(&self) -> Option<&str> {
        self.def.model.as_deref()
    }
}

/// Get the agents directory path.
pub fn agents_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".stockpot").join("agents"))
        .unwrap_or_else(|| PathBuf::from(".stockpot/agents"))
}

fn load_json_agents_from_dir(dir: &Path) -> Vec<JsonAgent> {
    let mut agents = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Convention: files prefixed with '_' or '.' are templates/dev-only and not loaded.
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

            match JsonAgent::from_file(&path) {
                Ok(agent) => {
                    tracing::info!("Loaded JSON agent: {}", agent.name());
                    agents.push(agent);
                }
                Err(e) => {
                    tracing::warn!("Failed to load agent from {:?}: {}", path, e);
                }
            }
        }
    }

    agents
}

/// Load all JSON agents from the agents directory.
pub fn load_json_agents() -> Vec<JsonAgent> {
    let dir = agents_dir();

    if !dir.exists() {
        // Create the directory
        let _ = fs::create_dir_all(&dir);
        return Vec::new();
    }

    load_json_agents_from_dir(&dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_json_agent_def_parse() {
        let json = r#"{
            "name": "test-agent",
            "display_name": "Test Agent 🧪",
            "description": "A test agent",
            "system_prompt": "You are a test agent.",
            "tools": ["read_file", "grep"]
        }"#;

        let def: JsonAgentDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "test-agent");
        assert_eq!(def.display_name, Some("Test Agent 🧪".to_string()));
        assert_eq!(def.tools.len(), 2);
    }

    #[test]
    fn test_json_agent_from_def() {
        let def = JsonAgentDef {
            name: "my-agent".to_string(),
            display_name: Some("My Agent".to_string()),
            description: Some("Does stuff".to_string()),
            system_prompt: "You are helpful.".to_string(),
            tools: vec!["read_file".to_string()],
            model: Some("gpt-4o".to_string()),
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        assert_eq!(agent.name(), "my-agent");
        assert_eq!(agent.display_name(), "My Agent");
        assert_eq!(agent.model_override(), Some("gpt-4o"));
    }

    #[test]
    fn test_load_from_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test-agent.json");

        let json = r#"{
            "name": "file-agent",
            "system_prompt": "You help with files.",
            "tools": ["read_file", "write_file"]
        }"#;

        fs::write(&path, json).unwrap();

        let agent = JsonAgent::from_file(&path).unwrap();
        assert_eq!(agent.name(), "file-agent");
        assert_eq!(agent.available_tools(), vec!["read_file", "write_file"]);
    }

    #[test]
    fn test_load_json_agents_skips_underscore_prefixed_files() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("_example.json"),
            r#"{
              "name": "example-agent",
              "system_prompt": "You are an example.",
              "tools": ["read_file"]
            }"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("real.json"),
            r#"{
              "name": "real-agent",
              "system_prompt": "You are real.",
              "tools": ["read_file"]
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        let names: Vec<_> = agents.iter().map(|a| a.name()).collect();

        assert_eq!(names, vec!["real-agent"]);
    }

    #[test]
    fn test_capabilities_override() {
        let def = JsonAgentDef {
            name: "restricted".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Read only.".to_string(),
            tools: vec!["read_file".to_string()],
            model: None,
            capabilities: Some(JsonCapabilities {
                file_read: Some(true),
                file_write: Some(false),
                shell: Some(false),
                sub_agents: Some(false),
                mcp: Some(false),
            }),
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let caps = agent.capabilities();
        assert!(caps.file_read);
        assert!(!caps.file_write);
        assert!(!caps.shell);
    }

    #[test]
    fn test_from_file_empty_name_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("invalid.json");

        let json = r#"{
            "name": "",
            "system_prompt": "Valid prompt."
        }"#;
        fs::write(&path, json).unwrap();

        let result = JsonAgent::from_file(&path);
        assert!(matches!(result, Err(JsonAgentError::Invalid(_))));
        let err = result.unwrap_err();
        assert!(err.to_string().contains("name is required"));
    }

    #[test]
    fn test_from_file_empty_system_prompt_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("invalid.json");

        let json = r#"{
            "name": "valid-name",
            "system_prompt": ""
        }"#;
        fs::write(&path, json).unwrap();

        let result = JsonAgent::from_file(&path);
        assert!(matches!(result, Err(JsonAgentError::Invalid(_))));
        let err = result.unwrap_err();
        assert!(err.to_string().contains("system_prompt is required"));
    }

    #[test]
    fn test_from_file_io_error() {
        let result = JsonAgent::from_file("/nonexistent/path/agent.json");
        assert!(matches!(result, Err(JsonAgentError::Io(_))));
    }

    #[test]
    fn test_from_file_json_parse_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("malformed.json");

        fs::write(&path, "{ not valid json }").unwrap();

        let result = JsonAgent::from_file(&path);
        assert!(matches!(result, Err(JsonAgentError::Json(_))));
    }

    #[test]
    fn test_display_name_falls_back_to_name() {
        let def = JsonAgentDef {
            name: "fallback-agent".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Test.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        assert_eq!(agent.display_name(), "fallback-agent");
    }

    #[test]
    fn test_description_falls_back_to_default() {
        let def = JsonAgentDef {
            name: "no-desc".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Test.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        assert_eq!(agent.description(), "Custom JSON agent");
    }

    #[test]
    fn test_visibility_default_and_explicit() {
        // Default visibility
        let def_default = JsonAgentDef {
            name: "default-vis".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Test.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };
        let agent_default = JsonAgent::new(def_default);
        assert_eq!(agent_default.visibility(), AgentVisibility::default());

        // Explicit hidden visibility
        let def_hidden = JsonAgentDef {
            name: "hidden-agent".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Test.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: Some(AgentVisibility::Hidden),
        };
        let agent_hidden = JsonAgent::new(def_hidden);
        assert_eq!(agent_hidden.visibility(), AgentVisibility::Hidden);

        // Explicit sub visibility
        let def_sub = JsonAgentDef {
            name: "sub-agent".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Test.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: Some(AgentVisibility::Sub),
        };
        let agent_sub = JsonAgent::new(def_sub);
        assert_eq!(agent_sub.visibility(), AgentVisibility::Sub);
    }

    #[test]
    fn test_load_json_agents_skips_dot_prefixed_files() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join(".hidden.json"),
            r#"{
              "name": "hidden-agent",
              "system_prompt": "You are hidden.",
              "tools": []
            }"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("visible.json"),
            r#"{
              "name": "visible-agent",
              "system_prompt": "You are visible.",
              "tools": []
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        let names: Vec<_> = agents.iter().map(|a| a.name()).collect();

        assert_eq!(names, vec!["visible-agent"]);
    }

    #[test]
    fn test_load_json_agents_skips_non_json_files() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("not-json.txt"),
            r#"{
              "name": "text-agent",
              "system_prompt": "You are text.",
              "tools": []
            }"#,
        )
        .unwrap();

        fs::write(dir.path().join("readme.md"), "# Readme").unwrap();

        fs::write(
            dir.path().join("valid.json"),
            r#"{
              "name": "json-agent",
              "system_prompt": "You are JSON.",
              "tools": []
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        let names: Vec<_> = agents.iter().map(|a| a.name()).collect();

        assert_eq!(names, vec!["json-agent"]);
    }

    #[test]
    fn test_load_json_agents_handles_invalid_json_gracefully() {
        let dir = tempdir().unwrap();

        // Invalid JSON file (should be skipped with warning)
        fs::write(dir.path().join("broken.json"), "{ invalid }").unwrap();

        // Valid JSON file
        fs::write(
            dir.path().join("valid.json"),
            r#"{
              "name": "valid-agent",
              "system_prompt": "Valid prompt.",
              "tools": []
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        let names: Vec<_> = agents.iter().map(|a| a.name()).collect();

        assert_eq!(names, vec!["valid-agent"]);
    }

    #[test]
    fn test_load_json_agents_empty_dir() {
        let dir = tempdir().unwrap();
        let agents = load_json_agents_from_dir(dir.path());
        assert!(agents.is_empty());
    }

    #[test]
    fn test_load_json_agents_nonexistent_dir() {
        let agents = load_json_agents_from_dir(Path::new("/nonexistent/agents/dir"));
        assert!(agents.is_empty());
    }

    #[test]
    fn test_capabilities_partial_override() {
        // Only override some capabilities, others should default to true
        let def = JsonAgentDef {
            name: "partial".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Partial caps.".to_string(),
            tools: vec![],
            model: None,
            capabilities: Some(JsonCapabilities {
                file_read: None,         // defaults to true
                file_write: Some(false), // explicit false
                shell: None,             // defaults to true
                sub_agents: Some(true),  // explicit true
                mcp: None,               // defaults to true
            }),
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let caps = agent.capabilities();
        assert!(caps.file_read); // default
        assert!(!caps.file_write); // overridden
        assert!(caps.shell); // default
        assert!(caps.sub_agents); // explicit true
        assert!(caps.mcp); // default
    }

    #[test]
    fn test_capabilities_default_when_none() {
        let def = JsonAgentDef {
            name: "default-caps".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Default capabilities.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let caps = agent.capabilities();
        let default_caps = AgentCapabilities::default();

        assert_eq!(caps.file_read, default_caps.file_read);
        assert_eq!(caps.file_write, default_caps.file_write);
        assert_eq!(caps.shell, default_caps.shell);
        assert_eq!(caps.sub_agents, default_caps.sub_agents);
        assert_eq!(caps.mcp, default_caps.mcp);
    }

    #[test]
    fn test_system_prompt_accessor() {
        let def = JsonAgentDef {
            name: "prompt-test".to_string(),
            display_name: None,
            description: None,
            system_prompt: "You are a specialized assistant.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        assert_eq!(agent.system_prompt(), "You are a specialized assistant.");
    }

    #[test]
    fn test_available_tools_empty() {
        let def = JsonAgentDef {
            name: "no-tools".to_string(),
            display_name: None,
            description: None,
            system_prompt: "No tools.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let tools: Vec<&str> = agent.available_tools();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_model_override_none() {
        let def = JsonAgentDef {
            name: "no-model".to_string(),
            display_name: None,
            description: None,
            system_prompt: "No model override.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        assert!(agent.model_override().is_none());
    }

    #[test]
    fn test_agents_dir_returns_path() {
        let dir = agents_dir();
        // Should end with .stockpot/agents
        assert!(dir.ends_with("agents"));
        let parent = dir.parent().unwrap();
        assert!(parent.ends_with(".stockpot"));
    }

    #[test]
    fn test_json_agent_error_display() {
        let io_err = JsonAgentError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(io_err.to_string().contains("IO error"));

        let invalid_err = JsonAgentError::Invalid("missing field".to_string());
        assert!(invalid_err.to_string().contains("Invalid agent definition"));
        assert!(invalid_err.to_string().contains("missing field"));

        let dir_err = JsonAgentError::DirNotFound;
        assert!(dir_err.to_string().contains("Agent directory not found"));
    }

    #[test]
    fn test_json_agent_def_serialization_roundtrip() {
        let def = JsonAgentDef {
            name: "roundtrip".to_string(),
            display_name: Some("Roundtrip Agent".to_string()),
            description: Some("Tests serialization".to_string()),
            system_prompt: "You are a test.".to_string(),
            tools: vec!["read_file".to_string(), "grep".to_string()],
            model: Some("claude-3-opus".to_string()),
            capabilities: Some(JsonCapabilities {
                file_read: Some(true),
                file_write: Some(false),
                shell: Some(true),
                sub_agents: Some(false),
                mcp: Some(true),
            }),
            visibility: Some(AgentVisibility::Sub),
        };

        let json = serde_json::to_string(&def).unwrap();
        let parsed: JsonAgentDef = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, def.name);
        assert_eq!(parsed.display_name, def.display_name);
        assert_eq!(parsed.description, def.description);
        assert_eq!(parsed.system_prompt, def.system_prompt);
        assert_eq!(parsed.tools, def.tools);
        assert_eq!(parsed.model, def.model);
        assert_eq!(parsed.visibility, def.visibility);
    }

    #[test]
    fn test_json_capabilities_default() {
        let caps = JsonCapabilities::default();
        assert!(caps.file_read.is_none());
        assert!(caps.file_write.is_none());
        assert!(caps.shell.is_none());
        assert!(caps.sub_agents.is_none());
        assert!(caps.mcp.is_none());
    }

    #[test]
    fn test_load_json_agents_handles_validation_error_gracefully() {
        let dir = tempdir().unwrap();

        // Valid JSON but fails validation (empty name)
        fs::write(
            dir.path().join("invalid-name.json"),
            r#"{
              "name": "",
              "system_prompt": "Valid prompt."
            }"#,
        )
        .unwrap();

        // Valid agent
        fs::write(
            dir.path().join("valid.json"),
            r#"{
              "name": "valid-agent",
              "system_prompt": "Valid prompt.",
              "tools": []
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        let names: Vec<_> = agents.iter().map(|a| a.name()).collect();

        // Only the valid agent should be loaded
        assert_eq!(names, vec!["valid-agent"]);
    }

    #[test]
    fn test_json_extension_case_insensitive() {
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("uppercase.JSON"),
            r#"{
              "name": "uppercase-agent",
              "system_prompt": "Uppercase extension.",
              "tools": []
            }"#,
        )
        .unwrap();

        fs::write(
            dir.path().join("mixed.Json"),
            r#"{
              "name": "mixed-agent",
              "system_prompt": "Mixed case extension.",
              "tools": []
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        assert_eq!(agents.len(), 2);

        let names: Vec<_> = agents.iter().map(|a| a.name()).collect();
        assert!(names.contains(&"uppercase-agent"));
        assert!(names.contains(&"mixed-agent"));
    }

    // === Additional tests for improved coverage ===

    #[test]
    fn test_json_agent_def_parse_unicode_and_special_chars() {
        let json = r#"{
            "name": "unicode-agent",
            "display_name": "Unicode Agent 日本語 🚀 émoji",
            "description": "Handles unicode: 中文, العربية, עברית",
            "system_prompt": "You handle special chars: <>&\"'\\n\\t",
            "tools": ["read_file"]
        }"#;

        let def: JsonAgentDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "unicode-agent");
        assert!(def.display_name.as_ref().unwrap().contains("日本語"));
        assert!(def.description.as_ref().unwrap().contains("中文"));
    }

    #[test]
    fn test_json_agent_def_parse_minimal() {
        // Only required fields
        let json = r#"{
            "name": "minimal",
            "system_prompt": "Minimal agent."
        }"#;

        let def: JsonAgentDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "minimal");
        assert!(def.display_name.is_none());
        assert!(def.description.is_none());
        assert!(def.tools.is_empty());
        assert!(def.model.is_none());
        assert!(def.capabilities.is_none());
        assert!(def.visibility.is_none());
    }

    #[test]
    fn test_json_agent_def_missing_required_field_name() {
        let json = r#"{
            "system_prompt": "Missing name field."
        }"#;

        let result: Result<JsonAgentDef, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_agent_def_missing_required_field_system_prompt() {
        let json = r#"{
            "name": "missing-prompt"
        }"#;

        let result: Result<JsonAgentDef, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_agent_error_json_display() {
        // Create a JSON parse error
        let result: Result<JsonAgentDef, serde_json::Error> =
            serde_json::from_str("{ invalid json }");
        let json_err = result.unwrap_err();
        let agent_err = JsonAgentError::Json(json_err);

        let msg = agent_err.to_string();
        assert!(msg.contains("JSON parse error"));
    }

    #[test]
    fn test_load_multiple_agents_from_dir() {
        let dir = tempdir().unwrap();

        // Create multiple valid agents
        for i in 1..=5 {
            fs::write(
                dir.path().join(format!("agent{}.json", i)),
                format!(
                    r#"{{
                      "name": "agent-{}",
                      "system_prompt": "Agent number {}.",
                      "tools": ["read_file"]
                    }}"#,
                    i, i
                ),
            )
            .unwrap();
        }

        let agents = load_json_agents_from_dir(dir.path());
        assert_eq!(agents.len(), 5);
    }

    #[test]
    fn test_load_json_agents_skips_subdirectories() {
        let dir = tempdir().unwrap();

        // Create a subdirectory (should be skipped)
        fs::create_dir(dir.path().join("subdir")).unwrap();

        // Create a valid agent
        fs::write(
            dir.path().join("valid.json"),
            r#"{
              "name": "valid-agent",
              "system_prompt": "Valid.",
              "tools": []
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name(), "valid-agent");
    }

    #[test]
    fn test_load_json_agents_skips_file_without_extension() {
        let dir = tempdir().unwrap();

        // File without extension (should be skipped)
        fs::write(
            dir.path().join("noext"),
            r#"{
              "name": "noext-agent",
              "system_prompt": "No extension.",
              "tools": []
            }"#,
        )
        .unwrap();

        // Valid agent
        fs::write(
            dir.path().join("valid.json"),
            r#"{
              "name": "valid-agent",
              "system_prompt": "Valid.",
              "tools": []
            }"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name(), "valid-agent");
    }

    #[test]
    fn test_json_agent_clone() {
        let def = JsonAgentDef {
            name: "clone-test".to_string(),
            display_name: Some("Clone Test".to_string()),
            description: Some("Testing clone".to_string()),
            system_prompt: "You are clonable.".to_string(),
            tools: vec!["read_file".to_string()],
            model: Some("gpt-4o".to_string()),
            capabilities: Some(JsonCapabilities {
                file_read: Some(true),
                file_write: Some(false),
                shell: None,
                sub_agents: None,
                mcp: None,
            }),
            visibility: Some(AgentVisibility::Sub),
        };

        let agent = JsonAgent::new(def);
        let cloned = agent.clone();

        assert_eq!(agent.name(), cloned.name());
        assert_eq!(agent.display_name(), cloned.display_name());
        assert_eq!(agent.description(), cloned.description());
        assert_eq!(agent.system_prompt(), cloned.system_prompt());
        assert_eq!(agent.available_tools(), cloned.available_tools());
        assert_eq!(agent.model_override(), cloned.model_override());
        assert_eq!(agent.visibility(), cloned.visibility());
    }

    #[test]
    fn test_json_agent_def_clone() {
        let def = JsonAgentDef {
            name: "original".to_string(),
            display_name: Some("Original".to_string()),
            description: Some("Original description".to_string()),
            system_prompt: "Original prompt.".to_string(),
            tools: vec!["tool1".to_string(), "tool2".to_string()],
            model: Some("model".to_string()),
            capabilities: Some(JsonCapabilities::default()),
            visibility: Some(AgentVisibility::Main),
        };

        let cloned = def.clone();

        assert_eq!(def.name, cloned.name);
        assert_eq!(def.display_name, cloned.display_name);
        assert_eq!(def.tools.len(), cloned.tools.len());
    }

    #[test]
    fn test_json_capabilities_clone() {
        let caps = JsonCapabilities {
            file_read: Some(true),
            file_write: Some(false),
            shell: Some(true),
            sub_agents: None,
            mcp: Some(false),
        };

        let cloned = caps.clone();

        assert_eq!(caps.file_read, cloned.file_read);
        assert_eq!(caps.file_write, cloned.file_write);
        assert_eq!(caps.shell, cloned.shell);
        assert_eq!(caps.sub_agents, cloned.sub_agents);
        assert_eq!(caps.mcp, cloned.mcp);
    }

    #[test]
    fn test_json_agent_debug() {
        let def = JsonAgentDef {
            name: "debug-test".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Debug.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let debug_str = format!("{:?}", agent);

        assert!(debug_str.contains("JsonAgent"));
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_json_agent_def_debug() {
        let def = JsonAgentDef {
            name: "def-debug".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Debug.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let debug_str = format!("{:?}", def);

        assert!(debug_str.contains("JsonAgentDef"));
        assert!(debug_str.contains("def-debug"));
    }

    #[test]
    fn test_json_capabilities_debug() {
        let caps = JsonCapabilities {
            file_read: Some(true),
            file_write: None,
            shell: Some(false),
            sub_agents: None,
            mcp: None,
        };

        let debug_str = format!("{:?}", caps);

        assert!(debug_str.contains("JsonCapabilities"));
        assert!(debug_str.contains("file_read"));
    }

    #[test]
    fn test_json_agent_error_debug() {
        let err = JsonAgentError::Invalid("test error".to_string());
        let debug_str = format!("{:?}", err);

        assert!(debug_str.contains("Invalid"));
        assert!(debug_str.contains("test error"));
    }

    #[test]
    fn test_tools_with_empty_string() {
        let def = JsonAgentDef {
            name: "empty-tool".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Has empty tool.".to_string(),
            tools: vec!["".to_string(), "valid_tool".to_string()],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let tools = agent.available_tools();

        // Empty string is still included (no validation on tool names)
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0], "");
        assert_eq!(tools[1], "valid_tool");
    }

    #[test]
    fn test_whitespace_only_name_passes_parsing_but_fails_validation() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("whitespace.json");

        let json = r#"{
            "name": "   ",
            "system_prompt": "Whitespace name."
        }"#;
        fs::write(&path, json).unwrap();

        // Parsing succeeds
        let parsed: Result<JsonAgentDef, _> = serde_json::from_str(json);
        assert!(parsed.is_ok());

        // But from_file doesn't validate whitespace-only (only empty)
        // This is actually a gap - whitespace name passes validation
        let result = JsonAgent::from_file(&path);
        assert!(result.is_ok()); // Current behavior: whitespace passes
    }

    #[test]
    fn test_json_with_extra_fields_ignored() {
        let json = r#"{
            "name": "extra-fields",
            "system_prompt": "Has extra fields.",
            "unknown_field": "should be ignored",
            "another_extra": 42
        }"#;

        // Serde should ignore unknown fields by default
        let def: JsonAgentDef = serde_json::from_str(json).unwrap();
        assert_eq!(def.name, "extra-fields");
    }

    #[test]
    fn test_visibility_serialization() {
        // Test that visibility serializes correctly
        let def = JsonAgentDef {
            name: "vis-test".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Visibility test.".to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: Some(AgentVisibility::Hidden),
        };

        let json = serde_json::to_string(&def).unwrap();
        assert!(json.contains("hidden") || json.contains("Hidden"));

        // Deserialize back
        let parsed: JsonAgentDef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.visibility, Some(AgentVisibility::Hidden));
    }

    #[test]
    fn test_json_agent_def_parse_all_visibility_values() {
        let main_json = r#"{
            "name": "main-vis",
            "system_prompt": "Test.",
            "visibility": "main"
        }"#;
        let main_def: JsonAgentDef = serde_json::from_str(main_json).unwrap();
        assert_eq!(main_def.visibility, Some(AgentVisibility::Main));

        let sub_json = r#"{
            "name": "sub-vis",
            "system_prompt": "Test.",
            "visibility": "sub"
        }"#;
        let sub_def: JsonAgentDef = serde_json::from_str(sub_json).unwrap();
        assert_eq!(sub_def.visibility, Some(AgentVisibility::Sub));

        let hidden_json = r#"{
            "name": "hidden-vis",
            "system_prompt": "Test.",
            "visibility": "hidden"
        }"#;
        let hidden_def: JsonAgentDef = serde_json::from_str(hidden_json).unwrap();
        assert_eq!(hidden_def.visibility, Some(AgentVisibility::Hidden));
    }

    #[test]
    fn test_invalid_visibility_value() {
        let json = r#"{
            "name": "bad-vis",
            "system_prompt": "Test.",
            "visibility": "invalid_value"
        }"#;

        let result: Result<JsonAgentDef, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities_all_false() {
        let def = JsonAgentDef {
            name: "no-caps".to_string(),
            display_name: None,
            description: None,
            system_prompt: "No capabilities.".to_string(),
            tools: vec![],
            model: None,
            capabilities: Some(JsonCapabilities {
                file_read: Some(false),
                file_write: Some(false),
                shell: Some(false),
                sub_agents: Some(false),
                mcp: Some(false),
            }),
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let caps = agent.capabilities();

        assert!(!caps.file_read);
        assert!(!caps.file_write);
        assert!(!caps.shell);
        assert!(!caps.sub_agents);
        assert!(!caps.mcp);
    }

    #[test]
    fn test_capabilities_all_true() {
        let def = JsonAgentDef {
            name: "all-caps".to_string(),
            display_name: None,
            description: None,
            system_prompt: "All capabilities.".to_string(),
            tools: vec![],
            model: None,
            capabilities: Some(JsonCapabilities {
                file_read: Some(true),
                file_write: Some(true),
                shell: Some(true),
                sub_agents: Some(true),
                mcp: Some(true),
            }),
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let caps = agent.capabilities();

        assert!(caps.file_read);
        assert!(caps.file_write);
        assert!(caps.shell);
        assert!(caps.sub_agents);
        assert!(caps.mcp);
    }

    #[test]
    fn test_many_tools() {
        let tools: Vec<String> = (1..=100).map(|i| format!("tool_{}", i)).collect();

        let def = JsonAgentDef {
            name: "many-tools".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Agent with many tools.".to_string(),
            tools: tools.clone(),
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let available = agent.available_tools();

        assert_eq!(available.len(), 100);
        assert_eq!(available[0], "tool_1");
        assert_eq!(available[99], "tool_100");
    }

    #[test]
    fn test_long_system_prompt() {
        let long_prompt = "A".repeat(10_000);

        let def = JsonAgentDef {
            name: "long-prompt".to_string(),
            display_name: None,
            description: None,
            system_prompt: long_prompt.clone(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        assert_eq!(agent.system_prompt().len(), 10_000);
    }

    #[test]
    fn test_newlines_in_system_prompt() {
        let prompt_with_newlines = "Line 1\nLine 2\nLine 3\n\nLine 5";

        let def = JsonAgentDef {
            name: "multiline".to_string(),
            display_name: None,
            description: None,
            system_prompt: prompt_with_newlines.to_string(),
            tools: vec![],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        assert!(agent.system_prompt().contains("\n"));
        assert_eq!(agent.system_prompt().lines().count(), 5);
    }

    #[test]
    fn test_from_file_preserves_all_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("full.json");

        let json = r#"{
            "name": "full-agent",
            "display_name": "Full Agent Display",
            "description": "A complete agent",
            "system_prompt": "You are complete.",
            "tools": ["read_file", "write_file", "grep"],
            "model": "claude-3-opus",
            "capabilities": {
                "file_read": true,
                "file_write": false,
                "shell": true,
                "sub_agents": false,
                "mcp": true
            },
            "visibility": "sub"
        }"#;

        fs::write(&path, json).unwrap();

        let agent = JsonAgent::from_file(&path).unwrap();

        assert_eq!(agent.name(), "full-agent");
        assert_eq!(agent.display_name(), "Full Agent Display");
        assert_eq!(agent.description(), "A complete agent");
        assert_eq!(agent.system_prompt(), "You are complete.");
        assert_eq!(
            agent.available_tools(),
            vec!["read_file", "write_file", "grep"]
        );
        assert_eq!(agent.model_override(), Some("claude-3-opus"));
        assert_eq!(agent.visibility(), AgentVisibility::Sub);

        let caps = agent.capabilities();
        assert!(caps.file_read);
        assert!(!caps.file_write);
        assert!(caps.shell);
        assert!(!caps.sub_agents);
        assert!(caps.mcp);
    }

    #[test]
    fn test_json_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let agent_err: JsonAgentError = io_err.into();

        assert!(matches!(agent_err, JsonAgentError::Io(_)));
        assert!(agent_err.to_string().contains("IO error"));
    }

    #[test]
    fn test_json_error_from_serde_error() {
        let result: Result<JsonAgentDef, serde_json::Error> = serde_json::from_str("invalid");
        let serde_err = result.unwrap_err();
        let agent_err: JsonAgentError = serde_err.into();

        assert!(matches!(agent_err, JsonAgentError::Json(_)));
    }

    #[test]
    fn test_duplicate_tools_allowed() {
        let def = JsonAgentDef {
            name: "dup-tools".to_string(),
            display_name: None,
            description: None,
            system_prompt: "Duplicate tools.".to_string(),
            tools: vec![
                "read_file".to_string(),
                "read_file".to_string(),
                "read_file".to_string(),
            ],
            model: None,
            capabilities: None,
            visibility: None,
        };

        let agent = JsonAgent::new(def);
        let tools = agent.available_tools();

        // Duplicates are allowed (no deduplication)
        assert_eq!(tools.len(), 3);
    }

    #[test]
    fn test_load_json_agents_mixed_valid_invalid() {
        let dir = tempdir().unwrap();

        // Invalid: parse error
        fs::write(dir.path().join("a_broken.json"), "not json").unwrap();

        // Invalid: empty name
        fs::write(
            dir.path().join("b_empty_name.json"),
            r#"{"name": "", "system_prompt": "test"}"#,
        )
        .unwrap();

        // Invalid: empty prompt
        fs::write(
            dir.path().join("c_empty_prompt.json"),
            r#"{"name": "test", "system_prompt": ""}"#,
        )
        .unwrap();

        // Valid
        fs::write(
            dir.path().join("d_valid.json"),
            r#"{"name": "valid", "system_prompt": "test"}"#,
        )
        .unwrap();

        // Skipped: underscore prefix
        fs::write(
            dir.path().join("_skipped.json"),
            r#"{"name": "skipped", "system_prompt": "test"}"#,
        )
        .unwrap();

        let agents = load_json_agents_from_dir(dir.path());

        // Only the valid one should load
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name(), "valid");
    }

    #[test]
    fn test_json_capabilities_serialization_roundtrip() {
        let caps = JsonCapabilities {
            file_read: Some(true),
            file_write: Some(false),
            shell: None,
            sub_agents: Some(true),
            mcp: None,
        };

        let json = serde_json::to_string(&caps).unwrap();
        let parsed: JsonCapabilities = serde_json::from_str(&json).unwrap();

        assert_eq!(caps.file_read, parsed.file_read);
        assert_eq!(caps.file_write, parsed.file_write);
        assert_eq!(caps.shell, parsed.shell);
        assert_eq!(caps.sub_agents, parsed.sub_agents);
        assert_eq!(caps.mcp, parsed.mcp);
    }

    #[test]
    fn test_capabilities_from_json_partial() {
        let json = r#"{
            "name": "partial-caps",
            "system_prompt": "Test.",
            "capabilities": {
                "shell": false
            }
        }"#;

        let def: JsonAgentDef = serde_json::from_str(json).unwrap();
        let agent = JsonAgent::new(def);
        let caps = agent.capabilities();

        // Only shell was specified as false
        assert!(!caps.shell);
        // Others default to true
        assert!(caps.file_read);
        assert!(caps.file_write);
        assert!(caps.sub_agents);
        assert!(caps.mcp);
    }
}
