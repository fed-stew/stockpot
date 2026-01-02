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
}
