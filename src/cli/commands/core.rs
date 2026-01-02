//! Core REPL command handlers.
//!
//! This module contains implementations for basic REPL commands
//! that don't require complex state management.

use crate::agents::{AgentInfo, AgentVisibility};
use crate::config::Settings;
use crate::db::Database;
use crate::models::{ModelConfig, ModelRegistry, ModelSettings};

/// Handle the /cd command - change or show working directory.
pub fn cmd_cd(args: &str) {
    if args.is_empty() {
        if let Ok(cwd) = std::env::current_dir() {
            println!("📁 {}", cwd.display());
        }
    } else {
        let expanded = shellexpand::tilde(args);
        match std::env::set_current_dir(expanded.as_ref()) {
            Ok(()) => {
                if let Ok(cwd) = std::env::current_dir() {
                    println!("📁 Changed to: {}", cwd.display());
                }
            }
            Err(e) => println!("❌ Failed to change directory: {}", e),
        }
    }
}

/// Handle the /show command - display current status.
pub fn cmd_show(
    db: &Database,
    agent_name: &str,
    agent_display_name: &str,
    current_model: &str,
    current_session: Option<&str>,
    message_count: usize,
    registry: &ModelRegistry,
) {
    let settings = Settings::new(db);

    println!("\n\x1b[1m📊 Current Status\x1b[0m\n");
    println!("  Agent:       \x1b[36m{}\x1b[0m", agent_display_name);

    // Check for pinned model
    if let Some(pinned) = settings.get_agent_pinned_model(agent_name) {
        println!(
            "  Model:       \x1b[33m{}\x1b[0m \x1b[2m(pinned)\x1b[0m",
            pinned
        );
    } else {
        println!("  Model:       \x1b[33m{}\x1b[0m", current_model);
    }
    println!(
        "  YOLO mode:   {}",
        if settings.yolo_mode() {
            "\x1b[31mON\x1b[0m"
        } else {
            "\x1b[32moff\x1b[0m"
        }
    );
    println!("  Session:     {}", current_session.unwrap_or("(unsaved)"));
    println!("  Messages:    {}", message_count);

    // Model info from registry
    if let Some(config) = registry.get(current_model) {
        println!("  Context:     {} tokens", config.context_length);

        // Rough token estimate
        let est_tokens = message_count * 500;
        let usage_pct = (est_tokens as f64 / config.context_length as f64) * 100.0;
        println!(
            "  Usage:       ~{:.1}% ({} est. tokens)",
            usage_pct, est_tokens
        );
    }

    // Model-specific settings
    let model_settings = ModelSettings::load(db, current_model).unwrap_or_default();
    if !model_settings.is_empty() {
        println!("\n  \x1b[2mModel settings:\x1b[0m");
        if let Some(t) = model_settings.temperature {
            println!("    temperature: {:.2}", t);
        }
        if let Some(ref r) = model_settings.reasoning_effort {
            println!("    reasoning:   {}", r);
        }
        if let Some(e) = model_settings.extended_thinking {
            println!(
                "    thinking:    {}",
                if e { "enabled" } else { "disabled" }
            );
        }
    }

    println!();
}

/// Handle the /version command.
pub fn cmd_version() {
    println!("🍲 stockpot v{}", env!("CARGO_PKG_VERSION"));
}

/// Handle the /agents command - list available agents.
pub fn cmd_agents(agents: &[AgentInfo], current_name: &str, show_visibility: bool) {
    println!("\n📋 \x1b[1mAvailable Agents:\x1b[0m\n");
    for agent in agents {
        let marker = if agent.name == current_name {
            "→ "
        } else {
            "  "
        };
        let visibility_badge = if show_visibility {
            let label = match agent.visibility {
                AgentVisibility::Main => "Main",
                AgentVisibility::Sub => "Sub",
                AgentVisibility::Hidden => "Hidden",
            };
            format!(" \x1b[2m[{}]\x1b[0m", label)
        } else {
            String::new()
        };

        println!(
            "{}\x1b[1;36m{}\x1b[0m{}",
            marker, agent.display_name, visibility_badge
        );
        println!("    Name: {}", agent.name);
        println!("    {}\n", agent.description);
    }
}

/// Handle the /new command.
pub fn cmd_new() {
    println!("🆕 Started new conversation");
}

/// Handle the /tools command - list available tools.
pub fn cmd_tools() {
    println!("\n\x1b[1m🔧 Available Tools\x1b[0m\n");
    println!("  \x1b[36mlist_files\x1b[0m       List directory contents");
    println!("  \x1b[36mread_file\x1b[0m        Read file contents");
    println!("  \x1b[36medit_file\x1b[0m        Create or modify files");
    println!("  \x1b[36mgrep\x1b[0m             Search text in files");
    println!("  \x1b[36mshell\x1b[0m            Execute shell commands");
    println!("  \x1b[36minvoke_agent\x1b[0m     Call sub-agent");
    println!("  \x1b[36mlist_agents\x1b[0m      List available agents\n");
}

/// Handle the /reasoning command - set reasoning effort level.
pub fn cmd_reasoning(db: &Database, args: &str) {
    let valid = ["minimal", "low", "medium", "high", "xhigh"];
    if args.is_empty() || !valid.contains(&args) {
        let settings = Settings::new(db);
        let current = settings.get_or("reasoning_effort", "medium");
        println!("Current reasoning effort: \x1b[33m{}\x1b[0m", current);
        println!("Usage: /reasoning <{}>", valid.join("|"));
    } else {
        let settings = Settings::new(db);
        let _ = settings.set("reasoning_effort", args);
        println!("✅ Reasoning effort set to: \x1b[33m{}\x1b[0m", args);
    }
}

/// Handle the /verbosity command - set verbosity level.
pub fn cmd_verbosity(db: &Database, args: &str) {
    let valid = ["low", "medium", "high"];
    if args.is_empty() || !valid.contains(&args) {
        let settings = Settings::new(db);
        let current = settings.get_or("verbosity", "medium");
        println!("Current verbosity: \x1b[33m{}\x1b[0m", current);
        println!("Usage: /verbosity <{}>", valid.join("|"));
    } else {
        let settings = Settings::new(db);
        let _ = settings.set("verbosity", args);
        println!("✅ Verbosity set to: \x1b[33m{}\x1b[0m", args);
    }
}

/// Display the help message.
pub fn show_help() {
    println!(
        "
\x1b[1m🍲 Stockpot Commands\x1b[0m

  \x1b[1;36m/help, /h, /?\x1b[0m          Show this help message
  \x1b[1;36m/exit, /quit, /q\x1b[0m       Exit stockpot
  \x1b[1;36m/clear, /cls\x1b[0m           Clear the screen
  \x1b[1;36m/new\x1b[0m                   Start a new conversation
  \x1b[1;36m/cd [path]\x1b[0m             Show or change working directory

\x1b[1mAgents & Models:\x1b[0m
  \x1b[1;33m/model [name]\x1b[0m          Show/set model (interactive picker if no name)
  \x1b[1;33m/models\x1b[0m                List available models
  \x1b[1;33m/agent [name]\x1b[0m          Show/switch agent (interactive picker if no name)
  \x1b[1;33m/agents\x1b[0m                List all available agents
  \x1b[1;33m/pin <model>\x1b[0m           Pin a model to the current agent
  \x1b[1;33m/pin <agent> <model>\x1b[0m   Pin a model to a specific agent
  \x1b[1;33m/unpin [agent]\x1b[0m         Remove model pin (current or specific agent)
  \x1b[1;33m/pins\x1b[0m                  List all agent model pins

\x1b[1mModel Settings:\x1b[0m
  \x1b[1;32m/ms, /model_settings\x1b[0m   Edit model settings (interactive)
  \x1b[1;32m/ms --show\x1b[0m             Show current model settings
  \x1b[1;32m/reasoning <level>\x1b[0m     Set reasoning effort (minimal/low/medium/high/xhigh)
  \x1b[1;32m/verbosity <level>\x1b[0m     Set verbosity (low/medium/high)

\x1b[1mSessions & Context:\x1b[0m
  \x1b[1;35m/save [name]\x1b[0m           Save current session
  \x1b[1;35m/load [name]\x1b[0m           Load a session (picker if no name)
  \x1b[1;35m/resume\x1b[0m                Interactive session loader
  \x1b[1;35m/sessions\x1b[0m              List saved sessions
  \x1b[1;35m/session, /s\x1b[0m           Show current session info
  \x1b[1;35m/context\x1b[0m               Show context usage (with visual bar)
  \x1b[1;35m/compact [n]\x1b[0m           Compact message history (keep first + last N)
  \x1b[1;35m/truncate [n]\x1b[0m          Keep only last N messages (default: 10)
  \x1b[1;35m/delete-session <n>\x1b[0m    Delete a saved session

\x1b[1mConfiguration:\x1b[0m
  \x1b[1;32m/show\x1b[0m                  Show current status (agent, model, context)
  \x1b[1;32m/set [key=value]\x1b[0m       Show or set configuration
  \x1b[1;32m/yolo\x1b[0m                  Toggle YOLO mode (auto-approve commands)
  \x1b[1;32m/tools\x1b[0m                 List available tools

\x1b[1mMCP:\x1b[0m
  \x1b[1;34m/mcp\x1b[0m                   MCP server management (try /mcp help)
  \x1b[1;34m/mcp list\x1b[0m              List configured servers
  \x1b[1;34m/mcp status\x1b[0m            Show running servers
  \x1b[1;34m/mcp add\x1b[0m               Add new server (interactive wizard)
  \x1b[1;34m/mcp start [name]\x1b[0m      Start a server
  \x1b[1;34m/mcp stop [name]\x1b[0m       Stop a server

\x1b[1mAuth:\x1b[0m
  \x1b[1;34m/chatgpt-auth\x1b[0m          Authenticate with ChatGPT OAuth
  \x1b[1;34m/claude-code-auth\x1b[0m      Authenticate with Claude Code OAuth

\x1b[1mModel Discovery:\x1b[0m
  \x1b[1;34m/add-model\x1b[0m             Browse and add models from models.dev
  \x1b[1;34m/extra-models\x1b[0m          List configured extra models

  \x1b[1;34m/version, /v\x1b[0m           Show version

\x1b[2mJust type normally to chat with the current agent!\x1b[0m
\x1b[2mTab completion available for commands, models, and agents.\x1b[0m
\x1b[2mJust type normally to chat with the current agent!\x1b[0m
\x1b[2mTab completion available for commands, models, and agents.\x1b[0m
"
    );
}

/// Display the list of available models from the registry.
pub fn show_models(db: &Database, registry: &ModelRegistry, current_model: &str) {
    use crate::models::ModelType;
    use std::collections::HashMap;

    // Get all available models (ones with valid API keys/OAuth)
    let available = registry.list_available(db);

    if available.is_empty() {
        println!("\n\x1b[2mNo available models found.\x1b[0m");
        println!("\x1b[2mUse /add-model to add models from models.dev\x1b[0m");
        println!("\x1b[2mOr set API keys: OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.\x1b[0m\n");
        return;
    }

    // Group models by provider type
    let mut by_type: HashMap<String, Vec<(&str, Option<&str>)>> = HashMap::new();

    for name in &available {
        if let Some(config) = registry.get(name) {
            let type_label = match config.model_type {
                ModelType::Openai => "OpenAI",
                ModelType::Anthropic => "Anthropic",
                ModelType::Gemini => "Google Gemini",
                ModelType::ClaudeCode => "Claude Code (OAuth)",
                ModelType::ChatgptOauth => "ChatGPT (OAuth)",
                ModelType::AzureOpenai => "Azure OpenAI",
                ModelType::Openrouter => "OpenRouter",
                ModelType::CustomOpenai | ModelType::CustomAnthropic => {
                    // For custom endpoints, try to extract provider from name
                    if let Some(idx) = name.find(':') {
                        let provider = &name[..idx];
                        // Capitalize first letter
                        let mut chars = provider.chars();
                        match chars.next() {
                            Some(c) => {
                                let capitalized: String = c.to_uppercase().chain(chars).collect();
                                // Leak is fine here - static lifetime for display
                                Box::leak(capitalized.into_boxed_str()) as &str
                            }
                            None => "Custom",
                        }
                    } else {
                        "Custom"
                    }
                }
                ModelType::RoundRobin => "Round Robin",
            };

            by_type
                .entry(type_label.to_string())
                .or_default()
                .push((name.as_str(), config.description.as_deref()));
        }
    }

    println!("\n\x1b[1m📦 Available Models:\x1b[0m\n");

    // Sort provider types for consistent display
    let mut types: Vec<_> = by_type.keys().cloned().collect();
    types.sort();

    for type_label in types {
        if let Some(models) = by_type.get(&type_label) {
            println!("\x1b[1m{}:\x1b[0m", type_label);
            for (name, desc) in models {
                let marker = if *name == current_model { "→" } else { " " };
                let desc_str = desc.unwrap_or("");
                if desc_str.is_empty() {
                    println!("  {} \x1b[36m{}\x1b[0m", marker, name);
                } else {
                    println!("  {} \x1b[36m{}\x1b[0m", marker, name);
                    println!("      \x1b[2m{}\x1b[0m", desc_str);
                }
            }
            println!();
        }
    }

    println!("\x1b[2mCurrent: {}\x1b[0m", current_model);
    println!("\x1b[2mUse /model <name> to switch\x1b[0m\n");
}
