//! Agent manager for registry and switching.

use super::base::{BoxedAgent, SpotAgent};
use super::builtin;
use super::json_agent::load_json_agents;
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
        
        // Reviewers
        self.register(Box::new(builtin::CodeReviewerAgent));
    }

    /// Register JSON-defined agents from ~/.stockpot/agents/.
    fn register_json_agents(&mut self) {
        for agent in load_json_agents() {
            // Skip example agents
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
        self.current_agent.read()
            .map(|n| n.clone())
            .unwrap_or_else(|_| "stockpot".to_string())
    }

    /// Switch to a different agent.
    pub fn switch(&self, name: &str) -> Result<(), AgentError> {
        if !self.agents.contains_key(name) {
            return Err(AgentError::NotFound(name.to_string()));
        }
        
        let mut current = self.current_agent.write()
            .map_err(|_| AgentError::LockError)?;
        *current = name.to_string();
        Ok(())
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<AgentInfo> {
        let mut agents: Vec<_> = self.agents.values()
            .map(|a| AgentInfo {
                name: a.name().to_string(),
                display_name: a.display_name().to_string(),
                description: a.description().to_string(),
            })
            .collect();
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        agents
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
}

/// Agent-related errors.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent not found: {0}")]
    NotFound(String),
    #[error("Lock error")]
    LockError,
}
