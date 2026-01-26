//! Agent manager for registry and switching.

use super::base::{BoxedAgent, SpotAgent};
use super::builtin;
use super::json_agent::load_json_agents;
use super::{AgentVisibility, UserMode};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Agent manager handles agent registration and switching.
pub struct AgentManager {
    agents: HashMap<String, BoxedAgent>,
    current_agent: Arc<RwLock<String>>,
}

impl AgentManager {
    /// Create a new agent manager with built-in agents.
    pub fn new() -> Self {
        let mut manager = Self {
            agents: HashMap::new(),
            current_agent: Arc::new(RwLock::new("stockpot".to_string())),
        };
        manager.register_builtins();
        manager.register_json_agents();
        manager
    }

    /// Register built-in agents.
    fn register_builtins(&mut self) {
        // Main agents
        self.register(Box::new(builtin::StockpotAgent));
        self.register(Box::new(builtin::PlanningAgent));
        self.register(Box::new(builtin::ExploreAgent));

        // Reviewers
        self.register(Box::new(builtin::CodeReviewerAgent));
    }

    /// Register JSON-defined agents from ~/.stockpot/agents/.
    fn register_json_agents(&mut self) {
        for agent in load_json_agents() {
            // Skip template/dev-only agents.
            if agent.name().starts_with('_') {
                continue;
            }
            self.register(Box::new(agent));
        }
    }

    /// Register an agent.
    pub fn register(&mut self, agent: BoxedAgent) {
        let name = agent.name().to_string();
        self.agents.insert(name, agent);
    }

    /// Get an agent by name.
    pub fn get(&self, name: &str) -> Option<&dyn SpotAgent> {
        self.agents.get(name).map(|a| a.as_ref())
    }

    /// Get the current agent.
    pub fn current(&self) -> Option<&dyn SpotAgent> {
        let name = self.current_agent.read().ok()?;
        self.get(&name)
    }

    /// Get the current agent name.
    pub fn current_name(&self) -> String {
        self.current_agent
            .read()
            .map(|n| n.clone())
            .unwrap_or_else(|_| "stockpot".to_string())
    }

    /// Switch to a different agent.
    pub fn switch(&self, name: &str) -> Result<(), AgentError> {
        if !self.agents.contains_key(name) {
            return Err(AgentError::NotFound(name.to_string()));
        }

        let mut current = self
            .current_agent
            .write()
            .map_err(|_| AgentError::LockError)?;
        *current = name.to_string();
        Ok(())
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<AgentInfo> {
        let mut agents: Vec<_> = self
            .agents
            .values()
            .map(|a| AgentInfo {
                name: a.name().to_string(),
                display_name: a.display_name().to_string(),
                description: a.description().to_string(),
                visibility: a.visibility(),
            })
            .collect();
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        agents
    }

    /// List agents filtered by user mode visibility.
    #[must_use]
    pub fn list_filtered(&self, user_mode: UserMode) -> Vec<AgentInfo> {
        self.list()
            .into_iter()
            .filter(|agent| match user_mode {
                UserMode::Normal => agent.visibility == AgentVisibility::Main,
                UserMode::Expert => agent.visibility != AgentVisibility::Hidden,
                UserMode::Developer => true,
            })
            .collect()
    }

    /// Check if an agent exists.
    pub fn exists(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about an agent.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub visibility: AgentVisibility,
}

/// Agent-related errors.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent not found: {0}")]
    NotFound(String),
    #[error("Lock error")]
    LockError,
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Mock Agent for Testing
    // =========================================================================

    struct MockAgent {
        name: &'static str,
        display_name: &'static str,
        description: &'static str,
        visibility: AgentVisibility,
    }

    impl MockAgent {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                display_name: "Mock Agent",
                description: "A mock agent",
                visibility: AgentVisibility::Main,
            }
        }

        fn with_visibility(mut self, visibility: AgentVisibility) -> Self {
            self.visibility = visibility;
            self
        }

        fn with_display_name(mut self, display_name: &'static str) -> Self {
            self.display_name = display_name;
            self
        }

        fn with_description(mut self, description: &'static str) -> Self {
            self.description = description;
            self
        }
    }

    impl SpotAgent for MockAgent {
        fn name(&self) -> &str {
            self.name
        }

        fn display_name(&self) -> &str {
            self.display_name
        }

        fn description(&self) -> &str {
            self.description
        }

        fn system_prompt(&self) -> String {
            format!("You are {}", self.name)
        }

        fn available_tools(&self) -> Vec<&str> {
            vec![]
        }

        fn visibility(&self) -> AgentVisibility {
            self.visibility
        }
    }

    // =========================================================================
    // AgentManager::new() Tests
    // =========================================================================

    #[test]
    fn test_new_registers_builtin_agents() {
        let manager = AgentManager::new();

        // Core built-ins should exist
        assert!(manager.exists("stockpot"), "stockpot agent missing");
        assert!(manager.exists("planning-agent"), "planning-agent missing");
        assert!(manager.exists("explore"), "explore agent missing");
        assert!(
            manager.exists("code-reviewer"),
            "code-reviewer agent missing"
        );
    }

    #[test]
    fn test_new_sets_default_agent_to_stockpot() {
        let manager = AgentManager::new();
        assert_eq!(manager.current_name(), "stockpot");
    }

    // =========================================================================
    // AgentManager::default() Tests
    // =========================================================================

    #[test]
    fn test_default_is_equivalent_to_new() {
        let from_new = AgentManager::new();
        let from_default = AgentManager::default();

        assert_eq!(from_new.current_name(), from_default.current_name());
        assert_eq!(from_new.list().len(), from_default.list().len());
    }

    // =========================================================================
    // register() Tests
    // =========================================================================

    #[test]
    fn test_register_adds_agent() {
        let mut manager = AgentManager::new();
        let initial_count = manager.list().len();

        manager.register(Box::new(MockAgent::new("test-agent")));

        assert_eq!(manager.list().len(), initial_count + 1);
        assert!(manager.exists("test-agent"));
    }

    #[test]
    fn test_register_overwrites_existing_agent() {
        let mut manager = AgentManager::new();

        manager.register(Box::new(
            MockAgent::new("duplicate").with_description("first"),
        ));
        manager.register(Box::new(
            MockAgent::new("duplicate").with_description("second"),
        ));

        let agent = manager.get("duplicate").unwrap();
        assert_eq!(agent.description(), "second");
    }

    // =========================================================================
    // get() Tests
    // =========================================================================

    #[test]
    fn test_get_existing_agent() {
        let manager = AgentManager::new();

        let agent = manager.get("stockpot");
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().name(), "stockpot");
    }

    #[test]
    fn test_get_nonexistent_agent() {
        let manager = AgentManager::new();

        let agent = manager.get("nonexistent-agent-xyz");
        assert!(agent.is_none());
    }

    #[test]
    fn test_get_returns_correct_agent_properties() {
        let mut manager = AgentManager::new();
        manager.register(Box::new(
            MockAgent::new("props-test")
                .with_display_name("Props Test Agent")
                .with_description("Testing properties"),
        ));

        let agent = manager.get("props-test").unwrap();
        assert_eq!(agent.name(), "props-test");
        assert_eq!(agent.display_name(), "Props Test Agent");
        assert_eq!(agent.description(), "Testing properties");
    }

    // =========================================================================
    // current() Tests
    // =========================================================================

    #[test]
    fn test_current_returns_default_agent() {
        let manager = AgentManager::new();

        let current = manager.current();
        assert!(current.is_some());
        assert_eq!(current.unwrap().name(), "stockpot");
    }

    #[test]
    fn test_current_returns_switched_agent() {
        let manager = AgentManager::new();
        manager.switch("explore").unwrap();

        let current = manager.current();
        assert!(current.is_some());
        assert_eq!(current.unwrap().name(), "explore");
    }

    // =========================================================================
    // current_name() Tests
    // =========================================================================

    #[test]
    fn test_current_name_default() {
        let manager = AgentManager::new();
        assert_eq!(manager.current_name(), "stockpot");
    }

    #[test]
    fn test_current_name_after_switch() {
        let manager = AgentManager::new();
        manager.switch("explore").unwrap();
        assert_eq!(manager.current_name(), "explore");
    }

    // =========================================================================
    // switch() Tests
    // =========================================================================

    #[test]
    fn test_switch_to_existing_agent() {
        let manager = AgentManager::new();

        let result = manager.switch("explore");
        assert!(result.is_ok());
        assert_eq!(manager.current_name(), "explore");
    }

    #[test]
    fn test_switch_to_nonexistent_agent() {
        let manager = AgentManager::new();

        let result = manager.switch("nonexistent-agent-xyz");
        assert!(result.is_err());

        match result {
            Err(AgentError::NotFound(name)) => {
                assert_eq!(name, "nonexistent-agent-xyz");
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    #[test]
    fn test_switch_preserves_original_on_error() {
        let manager = AgentManager::new();
        let original = manager.current_name();

        let _ = manager.switch("nonexistent");

        assert_eq!(manager.current_name(), original);
    }

    #[test]
    fn test_switch_multiple_times() {
        let manager = AgentManager::new();

        manager.switch("code-reviewer").unwrap();
        assert_eq!(manager.current_name(), "code-reviewer");

        manager.switch("explore").unwrap();
        assert_eq!(manager.current_name(), "explore");

        manager.switch("stockpot").unwrap();
        assert_eq!(manager.current_name(), "stockpot");
    }

    #[test]
    fn test_switch_to_same_agent() {
        let manager = AgentManager::new();

        manager.switch("stockpot").unwrap();
        assert_eq!(manager.current_name(), "stockpot");

        // Should succeed without error
        let result = manager.switch("stockpot");
        assert!(result.is_ok());
    }

    // =========================================================================
    // exists() Tests
    // =========================================================================

    #[test]
    fn test_exists_builtin_agent() {
        let manager = AgentManager::new();
        assert!(manager.exists("stockpot"));
        assert!(manager.exists("explore"));
    }

    #[test]
    fn test_exists_nonexistent_agent() {
        let manager = AgentManager::new();
        assert!(!manager.exists("does-not-exist"));
        assert!(!manager.exists(""));
    }

    #[test]
    fn test_exists_registered_agent() {
        let mut manager = AgentManager::new();

        assert!(!manager.exists("new-agent"));
        manager.register(Box::new(MockAgent::new("new-agent")));
        assert!(manager.exists("new-agent"));
    }

    // =========================================================================
    // list() Tests
    // =========================================================================

    #[test]
    fn test_list_returns_all_agents() {
        let manager = AgentManager::new();
        let agents = manager.list();

        // Should have at least the built-in agents
        assert!(agents.len() >= 4);

        let names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"stockpot"));
        assert!(names.contains(&"explore"));
    }

    #[test]
    fn test_list_is_sorted_by_name() {
        let manager = AgentManager::new();
        let agents = manager.list();

        let names: Vec<_> = agents.iter().map(|a| &a.name).collect();
        let mut sorted = names.clone();
        sorted.sort();

        assert_eq!(names, sorted);
    }

    #[test]
    fn test_list_includes_registered_agents() {
        let mut manager = AgentManager::new();
        manager.register(Box::new(MockAgent::new("zzz-last-agent")));

        let agents = manager.list();
        let last = agents.last().unwrap();
        assert_eq!(last.name, "zzz-last-agent");
    }

    #[test]
    fn test_agent_info_includes_visibility() {
        let manager = AgentManager::new();
        let agents = manager.list();

        // All agents should have visibility set
        for agent in &agents {
            // Just verify visibility is accessible (not panicking)
            let _ = agent.visibility;
        }

        // Check specific built-in agents have correct visibility
        let stockpot = agents.iter().find(|a| a.name == "stockpot");
        assert!(stockpot.is_some());
        assert_eq!(stockpot.unwrap().visibility, AgentVisibility::Main);
    }

    // =========================================================================
    // list_filtered() Tests
    // =========================================================================

    #[test]
    fn test_list_filtered_normal_mode() {
        let manager = AgentManager::new();
        let filtered = manager.list_filtered(UserMode::Normal);

        // Should only contain Main agents
        for agent in &filtered {
            assert_eq!(
                agent.visibility,
                AgentVisibility::Main,
                "Normal mode should only show Main agents, but found {:?} for {}",
                agent.visibility,
                agent.name
            );
        }
    }

    #[test]
    fn test_list_filtered_expert_mode() {
        let manager = AgentManager::new();
        let filtered = manager.list_filtered(UserMode::Expert);

        // Should not contain Hidden agents
        for agent in &filtered {
            assert_ne!(
                agent.visibility,
                AgentVisibility::Hidden,
                "Expert mode should not show Hidden agents, but found {}",
                agent.name
            );
        }
    }

    #[test]
    fn test_list_filtered_developer_mode() {
        let manager = AgentManager::new();
        let all = manager.list();
        let filtered = manager.list_filtered(UserMode::Developer);

        // Developer mode should show all agents
        assert_eq!(
            all.len(),
            filtered.len(),
            "Developer mode should show all agents"
        );
    }

    #[test]
    fn test_list_filtered_with_all_visibility_types() {
        let mut manager = AgentManager::new();

        // Register agents with each visibility
        manager.register(Box::new(
            MockAgent::new("main-test").with_visibility(AgentVisibility::Main),
        ));
        manager.register(Box::new(
            MockAgent::new("sub-test").with_visibility(AgentVisibility::Sub),
        ));
        manager.register(Box::new(
            MockAgent::new("hidden-test").with_visibility(AgentVisibility::Hidden),
        ));

        // Normal: only Main
        let normal = manager.list_filtered(UserMode::Normal);
        assert!(normal.iter().any(|a| a.name == "main-test"));
        assert!(!normal.iter().any(|a| a.name == "sub-test"));
        assert!(!normal.iter().any(|a| a.name == "hidden-test"));

        // Expert: Main + Sub
        let expert = manager.list_filtered(UserMode::Expert);
        assert!(expert.iter().any(|a| a.name == "main-test"));
        assert!(expert.iter().any(|a| a.name == "sub-test"));
        assert!(!expert.iter().any(|a| a.name == "hidden-test"));

        // Developer: all
        let dev = manager.list_filtered(UserMode::Developer);
        assert!(dev.iter().any(|a| a.name == "main-test"));
        assert!(dev.iter().any(|a| a.name == "sub-test"));
        assert!(dev.iter().any(|a| a.name == "hidden-test"));
    }

    // =========================================================================
    // AgentError Tests
    // =========================================================================

    #[test]
    fn test_agent_error_not_found_display() {
        let error = AgentError::NotFound("test-agent".to_string());
        assert_eq!(error.to_string(), "Agent not found: test-agent");
    }

    #[test]
    fn test_agent_error_lock_error_display() {
        let error = AgentError::LockError;
        assert_eq!(error.to_string(), "Lock error");
    }

    #[test]
    fn test_agent_error_is_debug() {
        let error = AgentError::NotFound("x".to_string());
        let debug = format!("{:?}", error);
        assert!(debug.contains("NotFound"));
    }

    // =========================================================================
    // AgentInfo Tests
    // =========================================================================

    #[test]
    fn test_agent_info_clone() {
        let info = AgentInfo {
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: "Desc".to_string(),
            visibility: AgentVisibility::Main,
        };

        let cloned = info.clone();
        assert_eq!(cloned.name, info.name);
        assert_eq!(cloned.display_name, info.display_name);
        assert_eq!(cloned.description, info.description);
        assert_eq!(cloned.visibility, info.visibility);
    }

    #[test]
    fn test_agent_info_debug() {
        let info = AgentInfo {
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: "Desc".to_string(),
            visibility: AgentVisibility::Sub,
        };

        let debug = format!("{:?}", info);
        assert!(debug.contains("test"));
        assert!(debug.contains("Sub"));
    }

    // =========================================================================
    // UserMode Tests
    // =========================================================================

    #[test]
    fn test_user_mode_display_and_parse() {
        assert_eq!(UserMode::Normal.to_string(), "normal");
        assert_eq!(UserMode::Expert.to_string(), "expert");
        assert_eq!(UserMode::Developer.to_string(), "developer");

        assert_eq!("normal".parse::<UserMode>().unwrap(), UserMode::Normal);
        assert_eq!("expert".parse::<UserMode>().unwrap(), UserMode::Expert);
        assert_eq!(
            "developer".parse::<UserMode>().unwrap(),
            UserMode::Developer
        );

        assert!("invalid".parse::<UserMode>().is_err());
    }

    #[test]
    fn test_user_mode_parse_case_insensitive() {
        assert_eq!("NORMAL".parse::<UserMode>().unwrap(), UserMode::Normal);
        assert_eq!("Expert".parse::<UserMode>().unwrap(), UserMode::Expert);
        assert_eq!(
            "DEVELOPER".parse::<UserMode>().unwrap(),
            UserMode::Developer
        );
    }

    #[test]
    fn test_user_mode_parse_with_whitespace() {
        assert_eq!("  normal  ".parse::<UserMode>().unwrap(), UserMode::Normal);
        assert_eq!("\texpert\n".parse::<UserMode>().unwrap(), UserMode::Expert);
    }

    // =========================================================================
    // Thread Safety Tests
    // =========================================================================

    #[test]
    fn test_concurrent_switch_operations() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(AgentManager::new());
        let mut handles = vec![];

        // Spawn multiple threads switching agents
        for i in 0..10 {
            let mgr = Arc::clone(&manager);
            let agent = if i % 2 == 0 { "planning" } else { "stockpot" };

            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let _ = mgr.switch(agent);
                    let _ = mgr.current_name();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Manager should still be in a valid state
        let name = manager.current_name();
        assert!(name == "planning" || name == "stockpot");
    }

    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(AgentManager::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let mgr = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    let _ = mgr.current_name();
                    let _ = mgr.list();
                    let _ = mgr.exists("stockpot");
                    let _ = mgr.get("planning");
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    // =========================================================================
    // Edge Cases: Agent Names
    // =========================================================================

    #[test]
    fn test_register_agent_with_empty_name() {
        let mut manager = AgentManager::new();
        manager.register(Box::new(MockAgent::new("")));

        assert!(manager.exists(""));
        assert!(manager.get("").is_some());
    }

    #[test]
    fn test_switch_to_empty_name_agent() {
        let mut manager = AgentManager::new();
        manager.register(Box::new(MockAgent::new("")));

        let result = manager.switch("");
        assert!(result.is_ok());
        assert_eq!(manager.current_name(), "");
    }

    #[test]
    fn test_agent_with_special_characters() {
        let mut manager = AgentManager::new();
        let special_names = [
            "agent-with-dashes",
            "agent_with_underscores",
            "agent.with.dots",
            "agent:with:colons",
            "agent/with/slashes",
            "agent@with@at",
            "agent with spaces",
        ];

        for name in special_names {
            manager.register(Box::new(MockAgent::new(name)));
            assert!(manager.exists(name), "Failed for name: {}", name);
            assert!(manager.switch(name).is_ok(), "Switch failed for: {}", name);
        }
    }

    #[test]
    fn test_agent_with_unicode_name() {
        let mut manager = AgentManager::new();

        manager.register(Box::new(MockAgent::new("агент"))); // Russian
        manager.register(Box::new(MockAgent::new("代理"))); // Chinese
        manager.register(Box::new(MockAgent::new("🤖"))); // Emoji

        assert!(manager.exists("агент"));
        assert!(manager.exists("代理"));
        assert!(manager.exists("🤖"));
        assert!(manager.switch("🤖").is_ok());
    }

    #[test]
    fn test_agent_name_case_sensitivity() {
        let mut manager = AgentManager::new();

        manager.register(Box::new(MockAgent::new("Agent")));
        manager.register(Box::new(MockAgent::new("agent")));
        manager.register(Box::new(MockAgent::new("AGENT")));

        // All three should coexist
        assert!(manager.exists("Agent"));
        assert!(manager.exists("agent"));
        assert!(manager.exists("AGENT"));

        // Should not find wrong case
        assert!(!manager.exists("aGeNt"));
    }

    // =========================================================================
    // Edge Cases: Registration
    // =========================================================================

    #[test]
    fn test_register_many_agents() {
        let mut manager = AgentManager::new();
        let initial_count = manager.list().len();

        for i in 0..100 {
            let name = format!("agent-{}", i);
            // Use static str via Box::leak for test
            let name_static: &'static str = Box::leak(name.into_boxed_str());
            manager.register(Box::new(MockAgent::new(name_static)));
        }

        assert_eq!(manager.list().len(), initial_count + 100);
    }

    #[test]
    fn test_overwrite_builtin_agent() {
        let mut manager = AgentManager::new();

        // Get original description
        let original = manager.get("stockpot").unwrap().description().to_string();

        // Overwrite with mock
        manager.register(Box::new(
            MockAgent::new("stockpot").with_description("Overwritten"),
        ));

        // Should have new description
        let new = manager.get("stockpot").unwrap().description();
        assert_ne!(original, new);
        assert_eq!(new, "Overwritten");
    }

    #[test]
    fn test_overwrite_preserves_agent_count() {
        let mut manager = AgentManager::new();
        let count_before = manager.list().len();

        manager.register(Box::new(MockAgent::new("stockpot")));

        assert_eq!(manager.list().len(), count_before);
    }

    // =========================================================================
    // Edge Cases: Get and Lookup
    // =========================================================================

    #[test]
    fn test_get_after_overwrite() {
        let mut manager = AgentManager::new();

        manager.register(Box::new(MockAgent::new("test").with_description("v1")));
        assert_eq!(manager.get("test").unwrap().description(), "v1");

        manager.register(Box::new(MockAgent::new("test").with_description("v2")));
        assert_eq!(manager.get("test").unwrap().description(), "v2");
    }

    #[test]
    fn test_get_with_similar_names() {
        let mut manager = AgentManager::new();

        manager.register(Box::new(MockAgent::new("test")));
        manager.register(Box::new(MockAgent::new("test-agent")));
        manager.register(Box::new(MockAgent::new("testing")));

        // Each should return exactly the right one
        assert!(manager.get("test").is_some());
        assert!(manager.get("test-agent").is_some());
        assert!(manager.get("testing").is_some());

        // Partial matches should not work
        assert!(manager.get("tes").is_none());
        assert!(manager.get("test-").is_none());
    }

    #[test]
    fn test_current_after_overwriting_current_agent() {
        let mut manager = AgentManager::new();

        manager.switch("stockpot").unwrap();

        // Overwrite current agent
        manager.register(Box::new(
            MockAgent::new("stockpot").with_description("New Stockpot"),
        ));

        // current() should return the new version
        let current = manager.current().unwrap();
        assert_eq!(current.description(), "New Stockpot");
    }

    // =========================================================================
    // Edge Cases: Switch
    // =========================================================================

    #[test]
    fn test_switch_back_and_forth() {
        let manager = AgentManager::new();

        for _ in 0..50 {
            manager.switch("explore").unwrap();
            assert_eq!(manager.current_name(), "explore");

            manager.switch("stockpot").unwrap();
            assert_eq!(manager.current_name(), "stockpot");
        }
    }

    #[test]
    fn test_switch_to_newly_registered_agent() {
        let mut manager = AgentManager::new();

        // Agent doesn't exist yet
        assert!(manager.switch("brand-new").is_err());

        // Register it
        manager.register(Box::new(MockAgent::new("brand-new")));

        // Now it works
        assert!(manager.switch("brand-new").is_ok());
        assert_eq!(manager.current_name(), "brand-new");
    }

    // =========================================================================
    // Edge Cases: List and Filtered
    // =========================================================================

    #[test]
    fn test_list_empty_after_clearing() {
        // This tests the internal state if we had a way to clear
        // Since we don't have clear(), test that list works with custom manager

        // Create manager without builtins
        let manager = AgentManager {
            agents: HashMap::new(),
            current_agent: Arc::new(RwLock::new("none".to_string())),
        };

        assert!(manager.list().is_empty());
    }

    #[test]
    fn test_list_filtered_returns_empty_when_no_matching() {
        // Create manager with only Hidden agents
        let mut manager = AgentManager {
            agents: HashMap::new(),
            current_agent: Arc::new(RwLock::new("hidden".to_string())),
        };
        manager.register(Box::new(
            MockAgent::new("hidden").with_visibility(AgentVisibility::Hidden),
        ));

        // Normal mode should see nothing
        let filtered = manager.list_filtered(UserMode::Normal);
        assert!(filtered.is_empty());

        // Expert mode should also see nothing
        let filtered = manager.list_filtered(UserMode::Expert);
        assert!(filtered.is_empty());

        // Developer mode should see it
        let filtered = manager.list_filtered(UserMode::Developer);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn test_list_filtered_maintains_sort_order() {
        let mut manager = AgentManager::new();

        manager.register(Box::new(
            MockAgent::new("zzz-visible").with_visibility(AgentVisibility::Main),
        ));
        manager.register(Box::new(
            MockAgent::new("aaa-visible").with_visibility(AgentVisibility::Main),
        ));

        let filtered = manager.list_filtered(UserMode::Normal);
        let names: Vec<_> = filtered.iter().map(|a| &a.name).collect();
        let mut sorted = names.clone();
        sorted.sort();

        assert_eq!(names, sorted);
    }

    // =========================================================================
    // Edge Cases: Exists
    // =========================================================================

    #[test]
    fn test_exists_with_whitespace_variations() {
        let manager = AgentManager::new();

        // These should all return false (no trimming)
        assert!(!manager.exists(" stockpot"));
        assert!(!manager.exists("stockpot "));
        assert!(!manager.exists(" stockpot "));
        assert!(!manager.exists("\tstockpot"));
        assert!(!manager.exists("stockpot\n"));
    }

    #[test]
    fn test_exists_after_many_operations() {
        let mut manager = AgentManager::new();

        for i in 0..50 {
            let name = format!("temp-{}", i);
            let name_static: &'static str = Box::leak(name.into_boxed_str());
            manager.register(Box::new(MockAgent::new(name_static)));
        }

        // Original builtins should still exist
        assert!(manager.exists("stockpot"));
        assert!(manager.exists("explore"));
        assert!(manager.exists("planning-agent"));

        // All temp agents should exist
        for i in 0..50 {
            let name = format!("temp-{}", i);
            assert!(manager.exists(&name), "temp-{} should exist", i);
        }
    }

    // =========================================================================
    // Edge Cases: Current
    // =========================================================================

    #[test]
    fn test_current_with_nonexistent_default() {
        // If somehow current_agent points to nonexistent agent
        let manager = AgentManager {
            agents: HashMap::new(),
            current_agent: Arc::new(RwLock::new("nonexistent".to_string())),
        };

        // current() should return None
        assert!(manager.current().is_none());

        // current_name() should still return the name
        assert_eq!(manager.current_name(), "nonexistent");
    }

    // =========================================================================
    // AgentError Edge Cases
    // =========================================================================

    #[test]
    fn test_agent_error_not_found_with_empty_name() {
        let error = AgentError::NotFound("".to_string());
        assert_eq!(error.to_string(), "Agent not found: ");
    }

    #[test]
    fn test_agent_error_not_found_with_special_chars() {
        let error = AgentError::NotFound("test<>&\"'".to_string());
        assert_eq!(error.to_string(), "Agent not found: test<>&\"'");
    }

    #[test]
    fn test_agent_error_is_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<AgentError>();
        assert_sync::<AgentError>();
    }

    // =========================================================================
    // AgentInfo Edge Cases
    // =========================================================================

    #[test]
    fn test_agent_info_with_empty_fields() {
        let info = AgentInfo {
            name: "".to_string(),
            display_name: "".to_string(),
            description: "".to_string(),
            visibility: AgentVisibility::Main,
        };

        // Should not panic
        let _ = info.clone();
        let _ = format!("{:?}", info);
    }

    #[test]
    fn test_agent_info_with_long_description() {
        let long_desc = "x".repeat(10000);
        let info = AgentInfo {
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: long_desc.clone(),
            visibility: AgentVisibility::Main,
        };

        assert_eq!(info.description.len(), 10000);
        let cloned = info.clone();
        assert_eq!(cloned.description.len(), 10000);
    }

    // =========================================================================
    // Thread Safety: Additional Scenarios
    // =========================================================================

    #[test]
    fn test_concurrent_switch_and_current() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(AgentManager::new());
        let mut handles = vec![];

        // Writers
        for _ in 0..5 {
            let mgr = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    let _ = mgr.switch("explore");
                    let _ = mgr.switch("stockpot");
                }
            }));
        }

        // Readers calling current()
        for _ in 0..5 {
            let mgr = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    // Should never panic
                    let current = mgr.current();
                    if let Some(agent) = current {
                        let _ = agent.name();
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_concurrent_switch_to_invalid() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(AgentManager::new());
        let mut handles = vec![];

        for _ in 0..10 {
            let mgr = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                for _ in 0..50 {
                    // These should all fail but not panic
                    let _ = mgr.switch("nonexistent-1");
                    let _ = mgr.switch("nonexistent-2");
                    // Intersperse valid switches
                    let _ = mgr.switch("stockpot");
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should be in valid state
        assert!(manager.current().is_some());
    }

    // =========================================================================
    // Integration-style Tests
    // =========================================================================

    #[test]
    fn test_full_lifecycle() {
        let mut manager = AgentManager::new();

        // 1. Check initial state
        assert_eq!(manager.current_name(), "stockpot");
        assert!(manager.exists("stockpot"));

        // 2. Register custom agent
        manager.register(Box::new(
            MockAgent::new("custom")
                .with_display_name("Custom Agent")
                .with_description("A custom agent")
                .with_visibility(AgentVisibility::Main),
        ));

        // 3. Switch to it
        manager.switch("custom").unwrap();
        assert_eq!(manager.current_name(), "custom");
        assert_eq!(manager.current().unwrap().display_name(), "Custom Agent");

        // 4. List should include it
        let list = manager.list();
        assert!(list.iter().any(|a| a.name == "custom"));

        // 5. Filtered list should include it (Main visibility)
        let filtered = manager.list_filtered(UserMode::Normal);
        assert!(filtered.iter().any(|a| a.name == "custom"));

        // 6. Overwrite it
        manager.register(Box::new(
            MockAgent::new("custom").with_description("Updated"),
        ));
        assert_eq!(manager.current().unwrap().description(), "Updated");

        // 7. Switch back
        manager.switch("stockpot").unwrap();
        assert_eq!(manager.current_name(), "stockpot");
    }

    #[test]
    fn test_agent_visibility_filtering_complete() {
        let mut manager = AgentManager {
            agents: HashMap::new(),
            current_agent: Arc::new(RwLock::new("main-1".to_string())),
        };

        // Register 2 of each visibility
        manager.register(Box::new(
            MockAgent::new("main-1").with_visibility(AgentVisibility::Main),
        ));
        manager.register(Box::new(
            MockAgent::new("main-2").with_visibility(AgentVisibility::Main),
        ));
        manager.register(Box::new(
            MockAgent::new("sub-1").with_visibility(AgentVisibility::Sub),
        ));
        manager.register(Box::new(
            MockAgent::new("sub-2").with_visibility(AgentVisibility::Sub),
        ));
        manager.register(Box::new(
            MockAgent::new("hidden-1").with_visibility(AgentVisibility::Hidden),
        ));
        manager.register(Box::new(
            MockAgent::new("hidden-2").with_visibility(AgentVisibility::Hidden),
        ));

        // Total: 6 agents
        assert_eq!(manager.list().len(), 6);

        // Normal: only Main (2)
        assert_eq!(manager.list_filtered(UserMode::Normal).len(), 2);

        // Expert: Main + Sub (4)
        assert_eq!(manager.list_filtered(UserMode::Expert).len(), 4);

        // Developer: all (6)
        assert_eq!(manager.list_filtered(UserMode::Developer).len(), 6);
    }
}
