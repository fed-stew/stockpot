//! Plugin system for Spot.
//!
//! Plugins bundle agents, tools, and configuration together. They extend
//! the existing JSON agent system — a plugin can provide multiple agents
//! and tools in a single package.
//!
//! ## Plugin Discovery
//!
//! Plugins are loaded from `~/.spot/plugins/*.json`. Each JSON file defines
//! a plugin manifest with agents and tools.
//!
//! ## JSON Plugin Format
//!
//! ```json
//! {
//!     "name": "my-plugin",
//!     "version": "1.0.0",
//!     "description": "A custom plugin",
//!     "author": "Jane Doe",
//!     "agents": [
//!         {
//!             "name": "my-agent",
//!             "display_name": "My Agent",
//!             "description": "Does something useful",
//!             "system_prompt": "You are a helpful agent...",
//!             "tools": ["read_file", "grep"],
//!             "visibility": "main"
//!         }
//!     ],
//!     "tools": []
//! }
//! ```

mod json_plugin;
mod manager;

pub use json_plugin::JsonPlugin;
pub use manager::PluginManager;

use crate::agents::SpotAgent;
use serde::{Deserialize, Serialize};
use spot_tools::tools::registry::ArcTool;

/// Metadata about a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin name (e.g., "my-plugin").
    pub name: String,
    /// Semantic version (e.g., "1.0.0").
    pub version: String,
    /// Brief description of what this plugin provides.
    pub description: String,
    /// Optional author name or email.
    #[serde(default)]
    pub author: Option<String>,
}

/// A plugin bundles agents, tools, and configuration together.
///
/// Plugins are the primary extension mechanism for Spot. They can provide
/// multiple agents and tools in a single package, unlike the simpler
/// single-agent JSON format in `~/.spot/agents/`.
pub trait Plugin: Send + Sync {
    /// Plugin metadata.
    fn manifest(&self) -> &PluginManifest;

    /// Agents provided by this plugin.
    fn agents(&self) -> Vec<Box<dyn SpotAgent>>;

    /// Tools provided by this plugin.
    fn tools(&self) -> Vec<ArcTool>;

    /// Called when the plugin is loaded. Return `Err` to prevent loading.
    fn on_init(&self) -> Result<(), String> {
        Ok(())
    }

    /// Called when the plugin is unloaded.
    fn on_shutdown(&self) {}

    /// Called when configuration changes.
    fn on_config_change(&self, _key: &str, _value: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // PluginManifest Tests
    // =========================================================================

    #[test]
    fn test_manifest_serialize_deserialize() {
        let manifest = PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            author: Some("Test Author".to_string()),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: PluginManifest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "test-plugin");
        assert_eq!(deserialized.version, "1.0.0");
        assert_eq!(deserialized.description, "A test plugin");
        assert_eq!(deserialized.author, Some("Test Author".to_string()));
    }

    #[test]
    fn test_manifest_without_author() {
        let json = r#"{"name":"p","version":"0.1.0","description":"d"}"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert!(manifest.author.is_none());
    }

    #[test]
    fn test_manifest_clone() {
        let manifest = PluginManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "desc".to_string(),
            author: None,
        };

        let cloned = manifest.clone();
        assert_eq!(cloned.name, manifest.name);
        assert_eq!(cloned.version, manifest.version);
    }

    #[test]
    fn test_manifest_debug() {
        let manifest = PluginManifest {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: "desc".to_string(),
            author: None,
        };

        let debug = format!("{:?}", manifest);
        assert!(debug.contains("test"));
        assert!(debug.contains("1.0.0"));
    }

    // =========================================================================
    // Plugin Trait Default Implementation Tests
    // =========================================================================

    struct MockPlugin {
        manifest: PluginManifest,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                manifest: PluginManifest {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Mock plugin".to_string(),
                    author: None,
                },
            }
        }
    }

    impl Plugin for MockPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        fn agents(&self) -> Vec<Box<dyn SpotAgent>> {
            vec![]
        }

        fn tools(&self) -> Vec<ArcTool> {
            vec![]
        }
    }

    #[test]
    fn test_default_on_init_succeeds() {
        let plugin = MockPlugin::new("test");
        assert!(plugin.on_init().is_ok());
    }

    #[test]
    fn test_default_on_shutdown_does_not_panic() {
        let plugin = MockPlugin::new("test");
        plugin.on_shutdown();
    }

    #[test]
    fn test_default_on_config_change_does_not_panic() {
        let plugin = MockPlugin::new("test");
        plugin.on_config_change("key", "value");
    }

    #[test]
    fn test_plugin_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<MockPlugin>();
        assert_sync::<MockPlugin>();
    }

    #[test]
    fn test_boxed_plugin_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Box<dyn Plugin>>();
        assert_sync::<Box<dyn Plugin>>();
    }
}
