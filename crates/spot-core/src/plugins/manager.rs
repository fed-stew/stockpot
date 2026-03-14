//! Plugin manager for discovering, loading, and managing plugins.

use super::json_plugin::{load_json_plugins, load_json_plugins_from_dir};
use super::Plugin;
use crate::agents::SpotAgent;
use spot_tools::tools::registry::ArcTool;
use std::path::Path;

/// Manages the lifecycle of all loaded plugins.
///
/// The `PluginManager` discovers and loads plugins from `~/.spot/plugins/`,
/// provides access to plugin-provided agents and tools, and handles the
/// init/shutdown lifecycle.
pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    /// Create a new empty plugin manager.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Discover and load all JSON plugins from `~/.spot/plugins/`.
    ///
    /// Returns a list of error messages for plugins that failed to load
    /// or initialize.
    pub fn discover_and_load(&mut self) -> Vec<String> {
        let mut errors = Vec::new();

        for plugin in load_json_plugins() {
            if let Err(e) = plugin.on_init() {
                errors.push(format!(
                    "Plugin '{}' init failed: {}",
                    plugin.manifest().name,
                    e
                ));
                continue;
            }

            tracing::info!(
                plugin = %plugin.manifest().name,
                version = %plugin.manifest().version,
                "Plugin loaded"
            );
            self.plugins.push(Box::new(plugin));
        }

        errors
    }

    /// Discover and load plugins from a specific directory.
    ///
    /// This is primarily useful for testing. Returns error messages for
    /// plugins that failed to load or initialize.
    pub fn discover_and_load_from(&mut self, dir: &Path) -> Vec<String> {
        let mut errors = Vec::new();

        for plugin in load_json_plugins_from_dir(dir) {
            if let Err(e) = plugin.on_init() {
                errors.push(format!(
                    "Plugin '{}' init failed: {}",
                    plugin.manifest().name,
                    e
                ));
                continue;
            }

            self.plugins.push(Box::new(plugin));
        }

        errors
    }

    /// Load a single plugin (already constructed).
    ///
    /// Calls `on_init()` and adds it to the manager. Returns an error
    /// message if initialization fails.
    pub fn load_plugin(&mut self, plugin: Box<dyn Plugin>) -> Result<(), String> {
        plugin
            .on_init()
            .map_err(|e| format!("Plugin '{}' init failed: {}", plugin.manifest().name, e))?;

        tracing::info!(
            plugin = %plugin.manifest().name,
            version = %plugin.manifest().version,
            "Plugin loaded"
        );
        self.plugins.push(plugin);
        Ok(())
    }

    /// Get all agents from all loaded plugins.
    pub fn all_agents(&self) -> Vec<Box<dyn SpotAgent>> {
        self.plugins
            .iter()
            .flat_map(|plugin| plugin.agents())
            .collect()
    }

    /// Get all tools from all loaded plugins.
    pub fn all_tools(&self) -> Vec<ArcTool> {
        self.plugins
            .iter()
            .flat_map(|plugin| plugin.tools())
            .collect()
    }

    /// Get a reference to all loaded plugins.
    pub fn loaded_plugins(&self) -> &[Box<dyn Plugin>] {
        &self.plugins
    }

    /// Get the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }

    /// Notify all plugins of a configuration change.
    pub fn notify_config_change(&self, key: &str, value: &str) {
        for plugin in &self.plugins {
            plugin.on_config_change(key, value);
        }
    }

    /// Shut down all plugins (calls `on_shutdown` in reverse load order).
    pub fn shutdown_all(&self) {
        for plugin in self.plugins.iter().rev() {
            tracing::info!(
                plugin = %plugin.manifest().name,
                "Shutting down plugin"
            );
            plugin.on_shutdown();
        }
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{AgentCapabilities, AgentVisibility};
    use crate::plugins::PluginManifest;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use tempfile::tempdir;

    // =========================================================================
    // Mock Plugin for Testing
    // =========================================================================

    struct MockPlugin {
        manifest: PluginManifest,
        init_called: Arc<AtomicBool>,
        shutdown_called: Arc<AtomicBool>,
        init_should_fail: bool,
    }

    impl MockPlugin {
        fn new(name: &str) -> Self {
            Self {
                manifest: PluginManifest {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: format!("{} plugin", name),
                    author: None,
                },
                init_called: Arc::new(AtomicBool::new(false)),
                shutdown_called: Arc::new(AtomicBool::new(false)),
                init_should_fail: false,
            }
        }

        fn failing(name: &str) -> Self {
            let mut plugin = Self::new(name);
            plugin.init_should_fail = true;
            plugin
        }

        #[allow(dead_code)]
        fn init_was_called(&self) -> bool {
            self.init_called.load(Ordering::SeqCst)
        }

        #[allow(dead_code)]
        fn shutdown_was_called(&self) -> bool {
            self.shutdown_called.load(Ordering::SeqCst)
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

        fn on_init(&self) -> Result<(), String> {
            self.init_called.store(true, Ordering::SeqCst);
            if self.init_should_fail {
                Err("init failed".to_string())
            } else {
                Ok(())
            }
        }

        fn on_shutdown(&self) {
            self.shutdown_called.store(true, Ordering::SeqCst);
        }
    }

    // Mock plugin that provides agents.
    struct AgentPlugin {
        manifest: PluginManifest,
        agent_count: usize,
    }

    impl AgentPlugin {
        fn new(name: &str, agent_count: usize) -> Self {
            Self {
                manifest: PluginManifest {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Plugin with agents".to_string(),
                    author: None,
                },
                agent_count,
            }
        }
    }

    struct MockAgent {
        name: String,
    }

    impl SpotAgent for MockAgent {
        fn name(&self) -> &str {
            &self.name
        }
        fn display_name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "Mock agent from plugin"
        }
        fn system_prompt(&self) -> String {
            format!("You are {}.", self.name)
        }
        fn available_tools(&self) -> Vec<&str> {
            vec![]
        }
        fn capabilities(&self) -> AgentCapabilities {
            AgentCapabilities::default()
        }
        fn visibility(&self) -> AgentVisibility {
            AgentVisibility::Main
        }
    }

    impl Plugin for AgentPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }

        fn agents(&self) -> Vec<Box<dyn SpotAgent>> {
            (0..self.agent_count)
                .map(|i| -> Box<dyn SpotAgent> {
                    Box::new(MockAgent {
                        name: format!("{}-agent-{}", self.manifest.name, i),
                    })
                })
                .collect()
        }

        fn tools(&self) -> Vec<ArcTool> {
            vec![]
        }
    }

    // =========================================================================
    // PluginManager::new() Tests
    // =========================================================================

    #[test]
    fn test_new_creates_empty_manager() {
        let manager = PluginManager::new();
        assert_eq!(manager.plugin_count(), 0);
        assert!(manager.loaded_plugins().is_empty());
        assert!(manager.all_agents().is_empty());
        assert!(manager.all_tools().is_empty());
    }

    #[test]
    fn test_default_is_same_as_new() {
        let from_new = PluginManager::new();
        let from_default = PluginManager::default();
        assert_eq!(from_new.plugin_count(), from_default.plugin_count());
    }

    // =========================================================================
    // load_plugin() Tests
    // =========================================================================

    #[test]
    fn test_load_plugin_success() {
        let mut manager = PluginManager::new();
        let plugin = MockPlugin::new("test");
        let init_called = plugin.init_called.clone();

        let result = manager.load_plugin(Box::new(plugin));
        assert!(result.is_ok());
        assert_eq!(manager.plugin_count(), 1);
        assert!(init_called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_load_plugin_init_failure() {
        let mut manager = PluginManager::new();
        let plugin = MockPlugin::failing("fail-plugin");

        let result = manager.load_plugin(Box::new(plugin));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("init failed"));
        assert_eq!(manager.plugin_count(), 0);
    }

    #[test]
    fn test_load_multiple_plugins() {
        let mut manager = PluginManager::new();

        manager.load_plugin(Box::new(MockPlugin::new("a"))).unwrap();
        manager.load_plugin(Box::new(MockPlugin::new("b"))).unwrap();
        manager.load_plugin(Box::new(MockPlugin::new("c"))).unwrap();

        assert_eq!(manager.plugin_count(), 3);
    }

    // =========================================================================
    // all_agents() Tests
    // =========================================================================

    #[test]
    fn test_all_agents_from_single_plugin() {
        let mut manager = PluginManager::new();
        manager
            .load_plugin(Box::new(AgentPlugin::new("p1", 3)))
            .unwrap();

        let agents = manager.all_agents();
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].name(), "p1-agent-0");
        assert_eq!(agents[1].name(), "p1-agent-1");
        assert_eq!(agents[2].name(), "p1-agent-2");
    }

    #[test]
    fn test_all_agents_from_multiple_plugins() {
        let mut manager = PluginManager::new();
        manager
            .load_plugin(Box::new(AgentPlugin::new("p1", 2)))
            .unwrap();
        manager
            .load_plugin(Box::new(AgentPlugin::new("p2", 3)))
            .unwrap();

        let agents = manager.all_agents();
        assert_eq!(agents.len(), 5);
    }

    #[test]
    fn test_all_agents_empty_when_no_plugins() {
        let manager = PluginManager::new();
        assert!(manager.all_agents().is_empty());
    }

    #[test]
    fn test_all_agents_empty_when_plugins_have_no_agents() {
        let mut manager = PluginManager::new();
        manager
            .load_plugin(Box::new(MockPlugin::new("no-agents")))
            .unwrap();

        assert!(manager.all_agents().is_empty());
    }

    // =========================================================================
    // loaded_plugins() Tests
    // =========================================================================

    #[test]
    fn test_loaded_plugins_returns_all() {
        let mut manager = PluginManager::new();
        manager.load_plugin(Box::new(MockPlugin::new("a"))).unwrap();
        manager.load_plugin(Box::new(MockPlugin::new("b"))).unwrap();

        let plugins = manager.loaded_plugins();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].manifest().name, "a");
        assert_eq!(plugins[1].manifest().name, "b");
    }

    // =========================================================================
    // shutdown_all() Tests
    // =========================================================================

    #[test]
    fn test_shutdown_all_calls_on_shutdown() {
        let mut manager = PluginManager::new();

        let plugin_a = MockPlugin::new("a");
        let shutdown_a = plugin_a.shutdown_called.clone();
        let plugin_b = MockPlugin::new("b");
        let shutdown_b = plugin_b.shutdown_called.clone();

        manager.load_plugin(Box::new(plugin_a)).unwrap();
        manager.load_plugin(Box::new(plugin_b)).unwrap();

        manager.shutdown_all();

        assert!(shutdown_a.load(Ordering::SeqCst));
        assert!(shutdown_b.load(Ordering::SeqCst));
    }

    #[test]
    fn test_shutdown_all_on_empty_manager() {
        let manager = PluginManager::new();
        manager.shutdown_all(); // Should not panic
    }

    // =========================================================================
    // notify_config_change() Tests
    // =========================================================================

    struct ConfigTrackingPlugin {
        manifest: PluginManifest,
        config_changes: Arc<AtomicUsize>,
    }

    impl ConfigTrackingPlugin {
        fn new(name: &str) -> Self {
            Self {
                manifest: PluginManifest {
                    name: name.to_string(),
                    version: "1.0.0".to_string(),
                    description: "Tracks config".to_string(),
                    author: None,
                },
                config_changes: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl Plugin for ConfigTrackingPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.manifest
        }
        fn agents(&self) -> Vec<Box<dyn SpotAgent>> {
            vec![]
        }
        fn tools(&self) -> Vec<ArcTool> {
            vec![]
        }
        fn on_config_change(&self, _key: &str, _value: &str) {
            self.config_changes.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_notify_config_change_reaches_all_plugins() {
        let mut manager = PluginManager::new();

        let plugin = ConfigTrackingPlugin::new("tracker");
        let changes = plugin.config_changes.clone();
        manager.load_plugin(Box::new(plugin)).unwrap();

        manager.notify_config_change("key", "value");
        assert_eq!(changes.load(Ordering::SeqCst), 1);

        manager.notify_config_change("key2", "value2");
        assert_eq!(changes.load(Ordering::SeqCst), 2);
    }

    // =========================================================================
    // discover_and_load_from() Tests
    // =========================================================================

    #[test]
    fn test_discover_and_load_from_directory() {
        let dir = tempdir().unwrap();

        std::fs::write(
            dir.path().join("my-plugin.json"),
            r#"{
                "name": "discovered-plugin",
                "version": "1.0.0",
                "description": "Found via discovery",
                "agents": [
                    {
                        "name": "discovered-agent",
                        "system_prompt": "You were discovered."
                    }
                ]
            }"#,
        )
        .unwrap();

        let mut manager = PluginManager::new();
        let errors = manager.discover_and_load_from(dir.path());

        assert!(errors.is_empty());
        assert_eq!(manager.plugin_count(), 1);
        assert_eq!(
            manager.loaded_plugins()[0].manifest().name,
            "discovered-plugin"
        );

        let agents = manager.all_agents();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name(), "discovered-agent");
    }

    #[test]
    fn test_discover_and_load_from_empty_dir() {
        let dir = tempdir().unwrap();
        let mut manager = PluginManager::new();
        let errors = manager.discover_and_load_from(dir.path());

        assert!(errors.is_empty());
        assert_eq!(manager.plugin_count(), 0);
    }

    #[test]
    fn test_discover_and_load_from_with_invalid_files() {
        let dir = tempdir().unwrap();

        // One valid, one invalid
        std::fs::write(
            dir.path().join("good.json"),
            r#"{"name": "good", "agents": []}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("bad.json"), "not json").unwrap();

        let mut manager = PluginManager::new();
        let errors = manager.discover_and_load_from(dir.path());

        // The bad file is skipped by load_json_plugins_from_dir (logged as warning),
        // not returned as an error from discover_and_load_from.
        assert!(errors.is_empty());
        assert_eq!(manager.plugin_count(), 1);
    }

    // =========================================================================
    // Integration Tests
    // =========================================================================

    #[test]
    fn test_full_lifecycle() {
        let dir = tempdir().unwrap();

        std::fs::write(
            dir.path().join("lifecycle.json"),
            r#"{
                "name": "lifecycle-plugin",
                "version": "2.0.0",
                "description": "Tests full lifecycle",
                "author": "Test",
                "agents": [
                    {
                        "name": "lc-agent-1",
                        "display_name": "Lifecycle Agent 1",
                        "description": "First agent",
                        "system_prompt": "You are agent 1.",
                        "tools": ["read_file"],
                        "visibility": "main"
                    },
                    {
                        "name": "lc-agent-2",
                        "system_prompt": "You are agent 2.",
                        "visibility": "sub"
                    }
                ]
            }"#,
        )
        .unwrap();

        // 1. Create manager and discover
        let mut manager = PluginManager::new();
        let errors = manager.discover_and_load_from(dir.path());
        assert!(errors.is_empty());

        // 2. Verify plugin loaded
        assert_eq!(manager.plugin_count(), 1);
        let plugin = &manager.loaded_plugins()[0];
        assert_eq!(plugin.manifest().name, "lifecycle-plugin");
        assert_eq!(plugin.manifest().version, "2.0.0");

        // 3. Verify agents
        let agents = manager.all_agents();
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name(), "lc-agent-1");
        assert_eq!(agents[0].display_name(), "Lifecycle Agent 1");
        assert_eq!(agents[0].available_tools(), vec!["read_file"]);
        assert_eq!(agents[1].name(), "lc-agent-2");

        // 4. Config change
        manager.notify_config_change("theme", "dark");

        // 5. Shutdown
        manager.shutdown_all();
    }

    #[test]
    fn test_multiple_plugins_with_agents() {
        let mut manager = PluginManager::new();

        manager
            .load_plugin(Box::new(AgentPlugin::new("plugin-x", 2)))
            .unwrap();
        manager
            .load_plugin(Box::new(AgentPlugin::new("plugin-y", 3)))
            .unwrap();
        manager
            .load_plugin(Box::new(MockPlugin::new("plugin-z")))
            .unwrap();

        assert_eq!(manager.plugin_count(), 3);
        assert_eq!(manager.all_agents().len(), 5); // 2 + 3 + 0
    }

    #[test]
    fn test_load_plugin_after_discovery() {
        let dir = tempdir().unwrap();

        std::fs::write(
            dir.path().join("disc.json"),
            r#"{"name": "discovered", "agents": []}"#,
        )
        .unwrap();

        let mut manager = PluginManager::new();
        manager.discover_and_load_from(dir.path());

        // Load additional plugin programmatically
        manager
            .load_plugin(Box::new(MockPlugin::new("programmatic")))
            .unwrap();

        assert_eq!(manager.plugin_count(), 2);

        let names: Vec<_> = manager
            .loaded_plugins()
            .iter()
            .map(|p| p.manifest().name.as_str())
            .collect();
        assert!(names.contains(&"discovered"));
        assert!(names.contains(&"programmatic"));
    }
}
