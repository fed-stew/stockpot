//! Interactive REPL implementation.

use crate::agents::{AgentExecutor, AgentManager, StreamEvent};
use crate::cli::completion_reedline::{create_reedline, SpotCompleter, SpotPrompt};
use crate::cli::model_picker::{edit_model_settings, pick_agent, pick_model, show_model_settings};
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{
    Message, MessageBus, Spinner, TerminalRenderer,
    TerminalRenderer as MsgRenderer,
};
use crate::models::ModelRegistry;
use crate::session::SessionManager;
use crate::tools::SpotToolRegistry;
use reedline::{FileBackedHistory, Signal};
use serdes_ai_core::ModelRequest;
use std::{io::{stdout, Write}, time::Duration};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use super::commands::{context, core, mcp, session};
use super::streaming_markdown::StreamingMarkdownRenderer;

/// REPL state.
pub struct Repl<'a> {
    db: &'a Database,
    agents: AgentManager,
    renderer: TerminalRenderer,
    current_model: String,
    message_history: Vec<ModelRequest>,
    tool_registry: SpotToolRegistry,
    mcp_manager: McpManager,
    session_manager: SessionManager,
    current_session: Option<String>,
    /// Model configuration registry
    model_registry: ModelRegistry,
    /// Message bus for event-driven rendering
    message_bus: MessageBus,
}

impl<'a> Repl<'a> {
    /// Create a new REPL.
    pub fn new(db: &'a Database) -> Self {
        let settings = Settings::new(db);
        let current_model = settings.model();
        
        Self {
            db,
            agents: AgentManager::new(),
            renderer: TerminalRenderer::new(),
            current_model,
            message_history: Vec::new(),
            tool_registry: SpotToolRegistry::new(),
            mcp_manager: McpManager::new(),
            session_manager: SessionManager::new(),
            current_session: None,
            model_registry: ModelRegistry::load_from_db(db).unwrap_or_default(),
            message_bus: MessageBus::new(),
        }
    }

    /// Set the initial agent.
    pub fn with_agent(self, agent_name: &str) -> Self {
        if self.agents.exists(agent_name) {
            let _ = self.agents.switch(agent_name);
        }
        self
    }

    /// Set the initial model.
    pub fn with_model(mut self, model: &str) -> Self {
        self.current_model = model.to_string();
        self
    }

    /// Run the REPL loop.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        // Set up completer
        let mut completer = SpotCompleter::new();
        completer.set_agents(self.agents.list().iter().map(|a| a.name.clone()).collect());
        completer.set_sessions(self.session_manager.list().unwrap_or_default().into_iter().map(|s| s.name).collect());
        completer.set_mcp_servers(self.mcp_manager.config().servers.keys().cloned().collect());
        completer.set_models(self.model_registry.list_available(self.db));

        let mut line_editor = create_reedline(completer);
        // Load history
        let history_path = crate::config::XdgDirs::new().state.join("history.txt");
        if let Some(parent) = history_path.parent() { let _ = std::fs::create_dir_all(parent); }
        if let Ok(h) = FileBackedHistory::with_file(500, history_path.clone()) {
            line_editor = line_editor.with_history(Box::new(h));
        }
        self.start_mcp_servers().await;
        loop {
            // Check for pinned model for current agent
            let agent_name = self.agents.current_name();
            let settings = Settings::new(self.db);
            let (effective_model, is_pinned) = match settings.get_agent_pinned_model(&agent_name) {
                Some(pinned) => (pinned, true),
                None => (self.current_model.clone(), false),
            };
            
            let prompt = SpotPrompt::with_pinned(
                self.agents.current().map(|a| a.display_name()).unwrap_or("stockpot 🍲"),
                &effective_model,
                is_pinned,
            );

            match line_editor.read_line(&prompt) {
                Ok(Signal::Success(line)) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Try to complete partial commands
                    let line = match super::completion_reedline::try_complete_input(line, &self.completion_context()) {
                        Some(l) => l,
                        None => continue,
                    };

                    match self.handle_input(&line).await {
                        Ok(true) => {
                            self.stop_mcp_servers().await;
                            println!("👋 Stay simmering! 🍲");
                            break;
                        }
                        Ok(false) => {}
                        Err(e) => {
                            let _ = self.renderer.render(&Message::error(format!("Error: {}", e)));
                        }
                    }
                }
                Ok(Signal::CtrlC) => {
                    println!("^C");
                    continue;
                }
                Ok(Signal::CtrlD) => {
                    self.stop_mcp_servers().await;
                    println!("👋 Stay simmering! 🍲");
                    break;
                }
                Err(err) => {
                    let _ = self.renderer.render(&Message::error(format!("Readline error: {}", err)));
                    self.stop_mcp_servers().await;
                    break;
                }
            }
        }

        Ok(())
    }
    async fn start_mcp_servers(&self) {
        let enabled_count = self.mcp_manager.config().enabled_servers().count();
        if enabled_count > 0 {
            println!("\x1b[2m🔌 Starting {} MCP server(s)...\x1b[0m", enabled_count);
            if let Err(e) = self.mcp_manager.start_all().await {
                println!("\x1b[1;33m⚠️  Some MCP servers failed to start: {}\x1b[0m", e);
            } else {
                let running = self.mcp_manager.running_servers().await;
                if !running.is_empty() {
                    println!("\x1b[2m✓ MCP servers running: {}\x1b[0m", running.join(", "));
                }
            }
        }
    }
    async fn stop_mcp_servers(&self) {
        let running = self.mcp_manager.running_servers().await;
        if !running.is_empty() {
            println!("\x1b[2m🔌 Stopping MCP servers...\x1b[0m");
            let _ = self.mcp_manager.stop_all().await;
        }
    }
    fn completion_context(&self) -> super::completion_reedline::CompletionContext {
        super::completion_reedline::CompletionContext {
            models: self.model_registry.list().iter().map(|s| s.to_string()).collect(),
            agents: self.agents.list().iter().map(|a| a.name.clone()).collect(),
            sessions: self.session_manager.list().unwrap_or_default().into_iter().map(|s| s.name).collect(),
            mcp_servers: self.mcp_manager.config().servers.keys().cloned().collect(),
        }
    }
    async fn handle_input(&mut self, input: &str) -> anyhow::Result<bool> {
        if input.starts_with('/') {
            return self.handle_command(input).await;
        }
        self.handle_prompt(input).await?;
        Ok(false)
    }

    async fn handle_command(&mut self, input: &str) -> anyhow::Result<bool> {
        let parts: Vec<&str> = input[1..].splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        // Handle bare "/" - show interactive command picker
        if cmd.is_empty() {
            return self.show_command_picker().await;
        }

        match cmd.as_str() {
            "help" | "h" | "?" => core::show_help(),
            "exit" | "quit" | "q" => return Ok(true),
            "clear" | "cls" => print!("\x1b[2J\x1b[1;1H"),
            "new" => {
                self.message_history.clear();
                self.current_session = None;
                core::cmd_new();
            }
            "model" | "m" => {
                if args.is_empty() {
                    // Interactive picker
                    if let Some(selected) = pick_model(self.db, &self.model_registry, &self.current_model) {
                        self.current_model = selected.clone();
                        let settings = Settings::new(self.db);
                        let _ = settings.set("model", &selected);
                        println!("✅ Switched to model: \x1b[1;33m{}\x1b[0m", selected);
                    }
                } else {
                    // Direct set
                    self.current_model = args.to_string();
                    let settings = Settings::new(self.db);
                    let _ = settings.set("model", args);
                    println!("✅ Switched to model: \x1b[1;33m{}\x1b[0m", args);
                }
            }
            "models" => core::show_models(self.db, &self.model_registry, &self.current_model),
            "agent" | "a" => {
                if args.is_empty() {
                    // Interactive picker
                    let agents: Vec<(String, String)> = self.agents.list()
                        .into_iter()
                        .map(|info| (info.name.clone(), info.display_name.clone()))
                        .collect();
                    
                    if let Some(selected) = pick_agent(&agents, &self.agents.current_name()) {
                        if self.agents.exists(&selected) {
                            self.agents.switch(&selected)?;
                            self.message_history.clear();
                            println!("✅ Switched to agent: {}", self.agents.current().unwrap().display_name());
                        }
                    }
                } else if self.agents.exists(args) {
                    self.agents.switch(args)?;
                    self.message_history.clear();
                    println!("✅ Switched to agent: {}", self.agents.current().unwrap().display_name());
                } else {
                    println!("❌ Agent not found: {}", args);
                    println!("   Use /agents to see available agents");
                }
            }
            "agents" => {
                let agents: Vec<_> = self.agents.list()
                    .into_iter()
                    .map(|i| (i.name.clone(), i.display_name.clone(), i.description.clone()))
                    .collect();
                core::cmd_agents(&agents, &self.agents.current_name());
            }
            "mcp" => mcp::handle(&self.mcp_manager, args).await,
            "set" => self.cmd_set(args)?,
            "yolo" => self.cmd_yolo()?,
            "version" | "v" => core::cmd_version(),
            "chatgpt-auth" => match crate::auth::run_chatgpt_auth(self.db).await {
                Ok(_) => {
                    if let Err(e) = self.model_registry.reload_from_db(self.db) {
                        println!("⚠️  Failed to reload models: {}", e);
                    }
                    println!();
                    println!("🔄 Please restart stockpot to use the new models.");
                    println!("   Or type /exit and run spot again.");
                }
                Err(e) => println!("❌ ChatGPT auth failed: {}", e),
            },
            "claude-code-auth" => match crate::auth::run_claude_code_auth(self.db).await {
                Ok(_) => {
                    if let Err(e) = self.model_registry.reload_from_db(self.db) {
                        println!("⚠️  Failed to reload models: {}", e);
                    }
                    println!();
                    println!("🔄 Please restart stockpot to use the new models.");
                    println!("   Or type /exit and run spot again.");
                }
                Err(e) => println!("❌ Claude Code auth failed: {}", e),
            },
            "add-model" | "add_model" => {
                match super::add_model::run_add_model(self.db).await {
                    Ok(()) => {
                        // Reload registry to pick up the new model
                        if let Err(e) = self.model_registry.reload_from_db(self.db) {
                            println!("\x1b[33m⚠️  Failed to reload models: {}\x1b[0m", e);
                        }
                        // Update completer with new models
                        // Note: completer is owned by line_editor, so we can't update it here
                        // The user can restart or the completer will be updated on next run
                    }
                    Err(e) => println!("❌ Failed to add model: {}", e),
                }
            }
            "extra-models" | "extra_models" => {
                if let Err(e) = super::add_model::list_custom_models(self.db) {
                    println!("❌ Failed to list models: {}", e);
                }
            }
            // Session commands
            "save" => if let Some(name) = session::save(&self.session_manager, &self.agents, &self.message_history, &self.current_model, args) {
                self.current_session = Some(name);
            },
            "load" => if let Some((name, data)) = session::load(&self.session_manager, &mut self.agents, args) {
                self.message_history = data.messages;
                self.current_session = Some(name);
            }
            "sessions" => session::list(&self.session_manager, self.current_session.as_deref()),
            "delete-session" => session::delete(&self.session_manager, args),
            // Context commands
            "truncate" => context::truncate(&mut self.message_history, args),
            "context" => {
                let ctx_len = self.model_registry.get(&self.current_model).map(|c| c.context_length).unwrap_or(128000);
                session::cmd_context(self.db, &self.message_history, self.current_session.as_deref(), &self.agents.current_name(), ctx_len);
            }
            "compact" => session::cmd_compact(&mut self.message_history, args),
            "session" | "s" => session::show_session(self.current_session.as_deref(), Settings::new(self.db).get_bool("auto_save_session").unwrap_or(true)),
            "resume" => if let Some((n, d)) = session::load_interactive(&self.session_manager, &mut self.agents) {
                self.message_history = d.messages;
                self.current_session = Some(n);
            }
            // Model pin commands (persisted to database)
            "pin" => self.cmd_pin(args),
            "unpin" => self.cmd_unpin(args),
            "pins" => context::list_pins(self.db),
            // Core commands (extracted to commands/core.rs)
            "cd" => core::cmd_cd(args),
            "show" => {
                let agent_name = self.agents.current_name();
                let agent_display_name = self.agents.current()
                    .map(|a| a.display_name().to_string())
                    .unwrap_or_else(|| "None".to_string());
                core::cmd_show(
                    self.db,
                    &agent_name,
                    &agent_display_name,
                    &self.current_model,
                    self.current_session.as_deref(),
                    self.message_history.len(),
                    &self.model_registry,
                );
            }
            "tools" => core::cmd_tools(),
            "model_settings" | "ms" => {
                if args.is_empty() {
                    edit_model_settings(self.db, &self.current_model, &self.model_registry);
                } else if args == "--show" {
                    show_model_settings(self.db, &self.current_model);
                } else {
                    // Edit settings for specific model
                    edit_model_settings(self.db, args, &self.model_registry);
                }
            }
            "reasoning" => core::cmd_reasoning(self.db, args),
            "verbosity" => core::cmd_verbosity(self.db, args),
            _ => {
                println!("❓ Unknown command: /{}", cmd);
                println!("   Type /help for available commands");
            }
        }
        Ok(false)
    }

    /// Show interactive command picker when user just types "/"
    async fn show_command_picker(&mut self) -> anyhow::Result<bool> {
        if let Some(cmd) = super::completion_reedline::pick_command("") {
            Box::pin(self.handle_command(&cmd)).await
        } else {
            Ok(false)
        }
    }

    fn cmd_set(&self, args: &str) -> anyhow::Result<()> {
        let settings = Settings::new(self.db);
        if args.is_empty() {
            println!("\n⚙️  \x1b[1mSettings:\x1b[0m\n");
            for (key, value) in settings.list()? {
                println!("  {} = {}", key, value);
            }
            println!();
        } else if let Some((key, value)) = args.split_once('=') {
            settings.set(key.trim(), value.trim())?;
            println!("✅ Set {} = {}", key.trim(), value.trim());
        } else if let Some(value) = settings.get(args)? {
            println!("{} = {}", args, value);
        } else {
            println!("❌ Setting not found: {}", args);
        }
        Ok(())
    }

    fn cmd_yolo(&self) -> anyhow::Result<()> {
        let settings = Settings::new(self.db);
        let new_value = !settings.yolo_mode();
        settings.set("yolo_mode", if new_value { "true" } else { "false" })?;
        if new_value {
            println!("🔥 YOLO mode \x1b[1;31mENABLED\x1b[0m - Commands will run without confirmation!");
        } else {
            println!("🛡️  YOLO mode \x1b[1;32mDISABLED\x1b[0m - Commands will ask for confirmation");
        }
        Ok(())
    }

    /// Handle /pin command with flexible argument parsing.
    /// Supports: `/pin <model>` or `/pin <agent> <model>`
    fn cmd_pin(&mut self, args: &str) {
        let parts: Vec<&str> = args.split_whitespace().collect();
        let current_agent = self.agents.current_name();
        
        match parts.len() {
            0 => {
                // No args - show usage
                println!("❌ Please specify a model: /pin <model>");
                println!("   Or: /pin <agent> <model>");
                println!("   Example: /pin gpt-4o");
                println!("   Example: /pin reviewer gpt-4o");
            }
            1 => {
                // Single arg - pin model to current agent
                let model = parts[0];
                context::pin_model(
                    self.db,
                    &mut self.current_model,
                    &current_agent,
                    model,
                    true, // is current agent
                );
            }
            _ => {
                // Two+ args - first could be agent name, rest is model
                let first_arg = parts[0];
                
                // Check if first arg is a valid agent name
                if self.agents.exists(first_arg) {
                    // Pin to specific agent
                    let model = parts[1..].join(" ");
                    let is_current = first_arg == current_agent;
                    context::pin_model(
                        self.db,
                        &mut self.current_model,
                        first_arg,
                        &model,
                        is_current,
                    );
                } else {
                    // Assume entire args is a model name (might have spaces?)
                    // Fall back to pinning to current agent
                    context::pin_model(
                        self.db,
                        &mut self.current_model,
                        &current_agent,
                        args,
                        true,
                    );
                }
            }
        }
    }

    /// Handle /unpin command with optional agent argument.
    /// Supports: `/unpin` or `/unpin <agent>`
    fn cmd_unpin(&mut self, args: &str) {
        let current_agent = self.agents.current_name();
        
        if args.is_empty() {
            // No args - unpin current agent
            context::unpin_model(
                self.db,
                &mut self.current_model,
                &current_agent,
                true, // is current agent
            );
        } else {
            // Unpin specific agent
            let target_agent = args.trim();
            
            if !self.agents.exists(target_agent) {
                println!("❌ Unknown agent: {}", target_agent);
                println!("   Use /agents to see available agents");
                return;
            }
            
            let is_current = target_agent == current_agent;
            context::unpin_model(
                self.db,
                &mut self.current_model,
                target_agent,
                is_current,
            );
        }
    }

    /// Handle a regular prompt (send to agent).
    ///
    /// This uses the message bus architecture where all events flow through
    /// the bus and are rendered by a subscriber.
    pub async fn handle_prompt(&mut self, prompt: &str) -> anyhow::Result<()> {
        self.handle_prompt_with_bus(prompt).await
    }

    /// Handle a prompt using the message bus architecture.
    ///
    /// This is the new approach where all events flow through the message bus
    /// and are rendered by a subscriber. Much simpler than the old approach!
    async fn handle_prompt_with_bus(&mut self, prompt: &str) -> anyhow::Result<()> {
        debug!(prompt_len = prompt.len(), "handle_prompt_with_bus started");

        let agent = self.agents.current()
            .ok_or_else(|| anyhow::anyhow!("No agent selected"))?;
        let agent_name = agent.name().to_string();

        // Get effective model (respecting agent pins)
        let effective_model = context::get_effective_model(
            self.db,
            &self.current_model,
            &agent_name,
        );

        // Prepare history
        let history = if self.message_history.is_empty() {
            None
        } else {
            Some(self.message_history.clone())
        };

        // Get MCP tool count for display
        let mcp_tools = self.mcp_manager.list_all_tools().await;
        let mcp_tool_count: usize = mcp_tools.values().map(|v| v.len()).sum();
        if mcp_tool_count > 0 {
            debug!(mcp_tool_count, "MCP tools available");
            println!("\n\x1b[2m[{} MCP tools available]\x1b[0m", mcp_tool_count);
        }

        info!(model = %effective_model, agent = %agent_name, "Starting request");

        println!(); // Add spacing before spinner

        // Start spinner
        let spinner = Spinner::new();
        let spinner_handle = spinner.start(format!("Thinking... [{}]", effective_model));

        // Create executor with message bus
        let executor = AgentExecutor::new(self.db, &self.model_registry)
            .with_bus(self.message_bus.sender());

        // Subscribe and create ready signal to avoid race condition
        let mut receiver = self.message_bus.subscribe();
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn render task that stops spinner on first message
        let render_handle = tokio::spawn(async move {
            let renderer = MsgRenderer::new();

            // Signal that we're ready to receive BEFORE waiting
            let _ = ready_tx.send(());

            // Wait for first message, stop spinner, then render
            if let Ok(first_msg) = receiver.recv().await {
                // Stop spinner before rendering anything
                spinner_handle.stop().await;

                // Render the first message
                let _ = renderer.render(&first_msg);

                // Continue with remaining messages
                renderer.run_loop(receiver).await;
            } else {
                // No messages, just stop spinner
                spinner_handle.stop().await;
            }
        });

        // WAIT for render task to be ready before executing!
        // This prevents the race condition where messages are sent
        // before the render task is listening.
        let _ = ready_rx.await;

        // NOW safe to execute - render task is waiting for messages
        let result = executor.execute_with_bus(
            agent,
            &effective_model,
            prompt,
            history,
            &self.tool_registry,
            &self.mcp_manager,
        ).await;

        // Give renderer a moment to finish processing, then abort if needed
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        render_handle.abort();

        match result {
            Ok(exec_result) => {
                self.message_history = exec_result.messages;
                println!(); // Add spacing after response
                self.auto_save_session();
            }
            Err(e) => {
                println!("\n\x1b[1;31m❌ Error:\x1b[0m {}\n", e);
                self.show_error_hints(&e.to_string());
            }
        }

        Ok(())
    }

    /// Legacy prompt handling (direct stream processing).
    ///
    /// Kept for reference and fallback if needed.
    #[allow(dead_code)]
    async fn handle_prompt_legacy(&mut self, prompt: &str) -> anyhow::Result<()> {
        debug!(prompt_len = prompt.len(), "handle_prompt_legacy started");
        
        let agent = self.agents.current()
            .ok_or_else(|| anyhow::anyhow!("No agent selected"))?;
        let display_name = agent.display_name().to_string();
        let agent_name = agent.name().to_string();
        
        debug!(agent = %agent_name, display_name = %display_name, "Agent selected");
        
        println!();
        
        let executor = AgentExecutor::new(self.db, &self.model_registry);
        let history = if self.message_history.is_empty() { None } else { Some(self.message_history.clone()) };
        
        debug!(history_messages = history.as_ref().map(|h| h.len()).unwrap_or(0), "Message history");
        
        // Get MCP tool count
        let mcp_tools = self.mcp_manager.list_all_tools().await;
        let mcp_tool_count: usize = mcp_tools.values().map(|v| v.len()).sum();
        if mcp_tool_count > 0 {
            debug!(mcp_tool_count, "MCP tools available");
            println!("\x1b[2m[{} MCP tools available]\x1b[0m\n", mcp_tool_count);
        }
        
        // Use pinned model if set (from database)
        let effective_model = context::get_effective_model(
            self.db,
            &self.current_model,
            &self.agents.current_name(),
        );
        
        info!(model = %effective_model, agent = %agent_name, "Starting request");
        
        // Start spinner
        let spinner = Spinner::new();
        let mut spinner_handle = Some(spinner.start(format!("Thinking... [{}]", effective_model)));
        
        debug!("Calling execute_stream");
        match executor.execute_stream(
            agent,
            &effective_model,
            prompt,
            history,
            &self.tool_registry,
            &self.mcp_manager,
        ).await {
            Ok(mut stream) => {
                debug!("execute_stream returned successfully, waiting for events");
                let mut first_text = true;
                let mut has_output = false;
                let mut event_count = 0u32;
                let recv_timeout = Duration::from_secs(120); // 2 minute timeout
                let mut md_renderer = StreamingMarkdownRenderer::new();
                
                // Track current tool call for nicer output formatting
                let mut current_tool: Option<String> = None;
                let mut tool_args_buffer = String::new();
                
                loop {
                    // Use timeout to detect if we're stuck
                    let recv_result = timeout(recv_timeout, stream.recv()).await;
                    
                    let event_result = match recv_result {
                        Ok(Some(result)) => result,
                        Ok(None) => {
                            // Stream ended normally
                            debug!(total_events = event_count, "Stream ended (received None)");
                            if event_count == 0 {
                                warn!("Stream ended immediately without any events!");
                                println!("\n\x1b[1;33m⚠️  No response received from model\x1b[0m");
                            }
                            break;
                        }
                        Err(_) => {
                            // Timeout
                            error!("Timeout waiting for stream event after {:?}", recv_timeout);
                            if let Some(handle) = spinner_handle.take() {
                                handle.stop().await;
                            }
                            println!("\n\x1b[1;31m❌ Timeout waiting for response\x1b[0m");
                            break;
                        }
                    };
                    
                    event_count += 1;
                    debug!(event_num = event_count, "Processing event");
                    match event_result {
                        Ok(event) => {
                            match event {
                                StreamEvent::RunStart { .. } => {}
                                StreamEvent::RequestStart { step: _ } => {
                                    // Silently continue - no step indicators needed
                                }
                                StreamEvent::TextDelta { text } => {
                                    // Stop spinner on first text
                                    if let Some(handle) = spinner_handle.take() {
                                        handle.stop().await;
                                    }
                                    if first_text {
                                        println!("\x1b[1;35m{}:\x1b[0m\n", display_name);
                                        first_text = false;
                                    }
                                    // Stream through markdown renderer for live formatting
                                    if let Err(e) = md_renderer.process(&text) {
                                        debug!(error = %e, "Markdown render error, falling back to raw");
                                        print!("{}", text);
                                        let _ = stdout().flush();
                                    }
                                    has_output = true;
                                }
                                StreamEvent::ToolCallStart { tool_name, .. } => {
                                    // Pause spinner during tool execution
                                    if let Some(ref handle) = spinner_handle {
                                        handle.pause();
                                    }
                                    // Track the current tool and reset args buffer
                                    current_tool = Some(tool_name.clone());
                                    tool_args_buffer.clear();
                                    // Print tool name, args will follow after ToolCallComplete
                                    print!("\n\x1b[2m{}\x1b[0m ", tool_name);
                                    let _ = stdout().flush();
                                }
                                StreamEvent::ToolCallDelta { delta } => {
                                    // Accumulate args for later display
                                    tool_args_buffer.push_str(&delta);
                                }
                                StreamEvent::ToolCallComplete { tool_name: _ } => {
                                    // Now show the args in a nice format
                                    if let Some(ref tool) = current_tool {
                                        debug!(tool = %tool, args_buffer = %tool_args_buffer, "ToolCallComplete - parsing args");
                                        // Try to parse args and show nicely
                                        if let Ok(args) = serde_json::from_str::<serde_json::Value>(&tool_args_buffer) {
                                            debug!(parsed_args = %args, "Parsed tool args");
                                            match tool.as_str() {
                                                "read_file" => {
                                                    if let Some(path) = args.get("file_path").and_then(|v| v.as_str()) {
                                                        print!("\x1b[36m{}\x1b[0m", path);
                                                    }
                                                }
                                                "list_files" => {
                                                    if let Some(dir) = args.get("directory").and_then(|v| v.as_str()) {
                                                        print!("\x1b[36m{}\x1b[0m", dir);
                                                    }
                                                }
                                                "grep" => {
                                                    if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                                                        print!("\x1b[36m'{}'\x1b[0m", pattern);
                                                    }
                                                }
                                                "agent_run_shell_command" | "run_shell_command" => {
                                                    if let Some(cmd) = args.get("command").and_then(|v| v.as_str()) {
                                                        // Truncate long commands
                                                        let display_cmd = if cmd.len() > 60 {
                                                            format!("{}...", &cmd[..57])
                                                        } else {
                                                            cmd.to_string()
                                                        };
                                                        print!("\x1b[36m{}\x1b[0m", display_cmd);
                                                    }
                                                }
                                                _ => {
                                                    // For other tools, show compact args
                                                    let compact = args.to_string();
                                                    if compact.len() > 80 {
                                                        print!("\x1b[2m{}...\x1b[0m", &compact[..77]);
                                                    } else {
                                                        print!("\x1b[2m{}\x1b[0m", compact);
                                                    }
                                                }
                                            }
                                        } else if !tool_args_buffer.is_empty() {
                                            // JSON parsing failed - show raw args
                                            debug!(raw_args = %tool_args_buffer, "Failed to parse tool args as JSON");
                                            let display = if tool_args_buffer.len() > 60 {
                                                format!("{}...", &tool_args_buffer[..57])
                                            } else {
                                                tool_args_buffer.clone()
                                            };
                                            print!("\x1b[2m{}\x1b[0m", display);
                                        }
                                    }
                                    let _ = stdout().flush();
                                }
                                StreamEvent::ToolExecuted { tool_name: _, success, error } => {
                                    // Just end the line, show error if failed
                                    if success {
                                        println!();
                                    } else if let Some(err) = error {
                                        // Show truncated error message
                                        let short_err = if err.len() > 60 {
                                            format!("{}...", &err[..57])
                                        } else {
                                            err
                                        };
                                        println!(" \x1b[1;31m✗ {}\x1b[0m", short_err);
                                    } else {
                                        println!(" \x1b[1;31m✗ failed\x1b[0m");
                                    }
                                    current_tool = None;
                                    // Resume spinner after tool
                                    if let Some(ref handle) = spinner_handle {
                                        handle.resume();
                                    }
                                }
                                StreamEvent::ThinkingDelta { text } => {
                                    // Stop spinner for thinking output
                                    if let Some(handle) = spinner_handle.take() {
                                        handle.stop().await;
                                    }
                                    print!("\x1b[2m{}\x1b[0m", text);
                                    let _ = stdout().flush();
                                }
                                StreamEvent::ResponseComplete { .. } | StreamEvent::OutputReady => {}
                                StreamEvent::RunComplete { run_id, .. } => {
                                    debug!(run_id = %run_id, total_events = event_count, "Run completed");
                                    if let Some(handle) = spinner_handle.take() {
                                        handle.stop().await;
                                    }
                                    // Flush the markdown renderer
                                    if let Err(e) = md_renderer.flush() {
                                        debug!(error = %e, "Failed to flush markdown renderer");
                                    }
                                    if has_output {
                                        println!("\n"); // Add newlines after response
                                    }
                                }
                                StreamEvent::Error { message } => {
                                    error!(error = %message, "Stream error event received");
                                    if let Some(handle) = spinner_handle.take() {
                                        handle.stop().await;
                                    }
                                    println!("\n\x1b[1;31m❌ Error: {}\x1b[0m\n", message);
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Stream recv error");
                            if let Some(handle) = spinner_handle.take() {
                                handle.stop().await;
                            }
                            println!("\n\x1b[1;31m❌ Stream error: {}\x1b[0m\n", e);
                            self.show_error_hints(&e.to_string());
                            break;
                        }
                    }
                }
                info!(total_events = event_count, "Stream processing complete");

                // Auto-save if enabled
                self.auto_save_session();
            }
            Err(e) => {
                error!(error = %e, "execute_stream failed");
                if let Some(handle) = spinner_handle.take() {
                    handle.stop().await;
                }
                println!("\x1b[1;31m❌ Error:\x1b[0m {}\n", e);
                self.show_error_hints(&e.to_string());
            }
        }
        Ok(())
    }

    /// Auto-save session if enabled in settings.
    fn auto_save_session(&mut self) {
        let settings = Settings::new(self.db);
        if !settings.get_bool("auto_save_session").unwrap_or(true) {
            return;
        }

        if let Some(name) = session::auto_save(
            &self.session_manager,
            &self.current_session,
            &self.message_history,
            &self.agents.current_name(),
            &self.current_model,
        ) {
            self.current_session = Some(name);
        }
    }

    fn show_error_hints(&self, error_str: &str) {
        if error_str.contains("Auth") || error_str.contains("Not authenticated") {
            if self.current_model.contains("chatgpt") {
                println!("\x1b[2mHint: Run /chatgpt-auth to authenticate\x1b[0m");
            } else if self.current_model.contains("claude-code") {
                println!("\x1b[2mHint: Run /claude-code-auth to authenticate\x1b[0m");
            } else {
                println!("\x1b[2mHint: Make sure your API key is set\x1b[0m");
            }
        } else if error_str.contains("model") || error_str.contains("Model") {
            println!("\x1b[2mHint: Check your model name with /model\x1b[0m");
        }
    }
}
