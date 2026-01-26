//! Base agent trait.

use super::{AgentCapabilities, AgentVisibility};

/// Trait for all Stockpot agents.
pub trait SpotAgent: Send + Sync {
    /// Unique identifier for the agent (e.g., "stockpot", "python-reviewer").
    fn name(&self) -> &str;

    /// Human-readable display name (e.g., "Coding Agent").
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

    /// Get the visibility level for this agent.
    fn visibility(&self) -> AgentVisibility {
        AgentVisibility::Main
    }

    /// Optional model override (if agent requires a specific model).
    fn model_override(&self) -> Option<&str> {
        None
    }
}

/// Boxed agent for dynamic dispatch.
pub type BoxedAgent = Box<dyn SpotAgent>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock agent for testing trait defaults and behavior.
    struct MockAgent {
        name: &'static str,
        display_name: &'static str,
        description: &'static str,
        system_prompt: &'static str,
        tools: Vec<&'static str>,
    }

    impl MockAgent {
        fn new() -> Self {
            Self {
                name: "mock-agent",
                display_name: "Mock Agent",
                description: "A mock agent for testing",
                system_prompt: "You are a mock agent.",
                tools: vec!["tool1", "tool2"],
            }
        }

        fn with_name(mut self, name: &'static str) -> Self {
            self.name = name;
            self
        }

        fn with_tools(mut self, tools: Vec<&'static str>) -> Self {
            self.tools = tools;
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
            self.system_prompt.to_string()
        }

        fn available_tools(&self) -> Vec<&str> {
            self.tools.clone()
        }
    }

    // =========================================================================
    // SpotAgent Trait Default Implementation Tests
    // =========================================================================

    #[test]
    fn test_default_capabilities_all_false() {
        let agent = MockAgent::new();
        let caps = agent.capabilities();

        assert!(!caps.shell, "Default capabilities should have shell=false");
        assert!(
            !caps.file_write,
            "Default capabilities should have file_write=false"
        );
        assert!(
            !caps.file_read,
            "Default capabilities should have file_read=false"
        );
        assert!(
            !caps.sub_agents,
            "Default capabilities should have sub_agents=false"
        );
        assert!(!caps.mcp, "Default capabilities should have mcp=false");
    }

    #[test]
    fn test_default_visibility_is_main() {
        let agent = MockAgent::new();
        assert_eq!(
            agent.visibility(),
            AgentVisibility::Main,
            "Default visibility should be Main"
        );
    }

    #[test]
    fn test_default_model_override_is_none() {
        let agent = MockAgent::new();
        assert!(
            agent.model_override().is_none(),
            "Default model_override should be None"
        );
    }

    // =========================================================================
    // SpotAgent Required Method Tests
    // =========================================================================

    #[test]
    fn test_name_returns_correct_value() {
        let agent = MockAgent::new();
        assert_eq!(agent.name(), "mock-agent");
    }

    #[test]
    fn test_name_with_custom_value() {
        let agent = MockAgent::new().with_name("custom-agent");
        assert_eq!(agent.name(), "custom-agent");
    }

    #[test]
    fn test_display_name_returns_correct_value() {
        let agent = MockAgent::new();
        assert_eq!(agent.display_name(), "Mock Agent");
    }

    #[test]
    fn test_description_returns_correct_value() {
        let agent = MockAgent::new();
        assert_eq!(agent.description(), "A mock agent for testing");
    }

    #[test]
    fn test_system_prompt_returns_owned_string() {
        let agent = MockAgent::new();
        let prompt = agent.system_prompt();
        assert_eq!(prompt, "You are a mock agent.");
        // Verify it's an owned String (can be modified)
        let _owned: String = prompt;
    }

    #[test]
    fn test_available_tools_returns_correct_list() {
        let agent = MockAgent::new();
        let tools = agent.available_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"tool1"));
        assert!(tools.contains(&"tool2"));
    }

    #[test]
    fn test_available_tools_empty_list() {
        let agent = MockAgent::new().with_tools(vec![]);
        let tools = agent.available_tools();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_available_tools_many_tools() {
        let agent = MockAgent::new().with_tools(vec![
            "read_file",
            "write_file",
            "list_files",
            "grep",
            "shell",
        ]);
        let tools = agent.available_tools();
        assert_eq!(tools.len(), 5);
    }

    // =========================================================================
    // BoxedAgent Tests
    // =========================================================================

    #[test]
    fn test_boxed_agent_can_be_created() {
        let agent: BoxedAgent = Box::new(MockAgent::new());
        assert_eq!(agent.name(), "mock-agent");
    }

    #[test]
    fn test_boxed_agent_dynamic_dispatch() {
        let agents: Vec<BoxedAgent> = vec![
            Box::new(MockAgent::new().with_name("agent-1")),
            Box::new(MockAgent::new().with_name("agent-2")),
        ];

        assert_eq!(agents[0].name(), "agent-1");
        assert_eq!(agents[1].name(), "agent-2");
    }

    #[test]
    fn test_boxed_agent_preserves_trait_methods() {
        let agent: BoxedAgent = Box::new(MockAgent::new());

        // All trait methods should work through Box
        assert_eq!(agent.name(), "mock-agent");
        assert_eq!(agent.display_name(), "Mock Agent");
        assert_eq!(agent.description(), "A mock agent for testing");
        assert_eq!(agent.system_prompt(), "You are a mock agent.");
        assert_eq!(agent.available_tools().len(), 2);
        assert_eq!(agent.visibility(), AgentVisibility::Main);
        assert!(agent.model_override().is_none());
    }

    // =========================================================================
    // SpotAgent Custom Implementation Override Tests
    // =========================================================================

    /// Agent that overrides default implementations.
    struct CustomAgent;

    impl SpotAgent for CustomAgent {
        fn name(&self) -> &str {
            "custom"
        }

        fn display_name(&self) -> &str {
            "Custom Agent"
        }

        fn description(&self) -> &str {
            "Agent with custom defaults"
        }

        fn system_prompt(&self) -> String {
            "Custom prompt".to_string()
        }

        fn available_tools(&self) -> Vec<&str> {
            vec![]
        }

        fn capabilities(&self) -> AgentCapabilities {
            AgentCapabilities {
                shell: true,
                file_write: true,
                file_read: true,
                sub_agents: false,
                mcp: true,
            }
        }

        fn visibility(&self) -> AgentVisibility {
            AgentVisibility::Hidden
        }

        fn model_override(&self) -> Option<&str> {
            Some("gpt-4")
        }
    }

    #[test]
    fn test_custom_capabilities_override() {
        let agent = CustomAgent;
        let caps = agent.capabilities();

        assert!(caps.shell);
        assert!(caps.file_write);
        assert!(caps.file_read);
        assert!(!caps.sub_agents);
        assert!(caps.mcp);
    }

    #[test]
    fn test_custom_visibility_override() {
        let agent = CustomAgent;
        assert_eq!(agent.visibility(), AgentVisibility::Hidden);
    }

    #[test]
    fn test_custom_model_override() {
        let agent = CustomAgent;
        assert_eq!(agent.model_override(), Some("gpt-4"));
    }

    // =========================================================================
    // Send + Sync Trait Bound Tests
    // =========================================================================

    #[test]
    fn test_spot_agent_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<MockAgent>();
    }

    #[test]
    fn test_spot_agent_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<MockAgent>();
    }

    #[test]
    fn test_boxed_agent_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<BoxedAgent>();
    }

    #[test]
    fn test_boxed_agent_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<BoxedAgent>();
    }

    #[test]
    fn test_agent_can_be_moved_across_threads() {
        use std::sync::Arc;
        use std::thread;

        let agent: Arc<dyn SpotAgent> = Arc::new(MockAgent::new());
        let agent_clone = Arc::clone(&agent);

        let handle = thread::spawn(move || {
            assert_eq!(agent_clone.name(), "mock-agent");
        });

        handle.join().expect("Thread should complete successfully");
    }
}
