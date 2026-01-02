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
}
