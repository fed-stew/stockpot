//! Base agent trait.

use super::AgentCapabilities;

/// Trait for all Stockpot agents.
pub trait SpotAgent: Send + Sync {
    /// Unique identifier for the agent (e.g., "stockpot", "python-reviewer").
    fn name(&self) -> &str;

    /// Human-readable display name (e.g., "stockpot 🍲").
    fn display_name(&self) -> &str;

    /// Brief description of what this agent does.
    fn description(&self) -> &str;

    /// Get the system prompt for this agent.
    fn system_prompt(&self) -> String;

    /// Get list of tool names this agent should have access to.
    fn available_tools(&self) -> Vec<&str>;

    /// Get the agent's capabilities.
    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities::default()
    }

    /// Optional model override (if agent requires a specific model).
    fn model_override(&self) -> Option<&str> {
        None
    }
}

/// Boxed agent for dynamic dispatch.
pub type BoxedAgent = Box<dyn SpotAgent>;
