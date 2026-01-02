//! Main application state and rendering

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    actions, div, prelude::*, px, rgb, App, AsyncApp, Context, Entity, ExternalPaths, FocusHandle,
    Focusable, KeyBinding, MouseButton, ScrollHandle, SharedString, Styled, WeakEntity, Window,
};

use super::components::{ScrollbarDragState, TextInput};
use super::state::{Conversation, MessageRole};
use super::theme::Theme;
use crate::agents::{AgentExecutor, AgentManager, UserMode};
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{AgentEvent, Message, MessageBus, ToolStatus};
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;

actions!(
    stockpot_gui,
    [
        Quit,
        NewConversation,
        OpenSettings,
        Send,
        FocusInput,
        NextAgent,
        PrevAgent,
        CloseDialog,
    ]
);

mod agent_dropdown;
mod error;
mod input;
mod messages;
mod model_dropdown;
mod settings;
mod toolbar;

/// Main application state
pub struct ChatApp {
    /// Focus handle for keyboard input
    focus_handle: FocusHandle,
    /// Text input component
    text_input: Entity<TextInput>,
    /// Current conversation
    conversation: Conversation,
    /// Selected agent name
    current_agent: String,
    /// Selected model name
    current_model: String,
    /// Current user mode (controls agent visibility)
    user_mode: UserMode,
    /// Color theme
    theme: Theme,
    /// Whether we're currently generating a response
    is_generating: bool,
    /// Message bus for agent communication
    message_bus: MessageBus,
    /// Database connection (UI-thread only)
    db: Rc<Database>,
    /// Agent manager
    agents: Arc<AgentManager>,
    /// Model registry
    model_registry: Arc<ModelRegistry>,
    /// Tool registry
    tool_registry: Arc<SpotToolRegistry>,
    /// MCP manager
    mcp_manager: Arc<McpManager>,
    /// Message history for context
    message_history: Vec<serdes_ai_core::ModelRequest>,
    /// Available agents list
    available_agents: Vec<(String, String)>,
    /// Available models list
    available_models: Vec<String>,
    /// Show settings panel
    show_settings: bool,
    /// Active settings tab
    settings_tab: settings::SettingsTab,
    /// Agent selected in settings for pinning
    settings_selected_agent: String,
    /// Show agent dropdown under header
    show_agent_dropdown: bool,
    /// Bounds for agent dropdown positioning
    agent_dropdown_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    /// Show model dropdown under header
    show_model_dropdown: bool,
    /// Bounds for model dropdown positioning
    model_dropdown_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    /// Show default model dropdown in settings
    show_default_model_dropdown: bool,
    /// Bounds of the default model dropdown trigger for positioning
    default_model_dropdown_bounds: Option<gpui::Bounds<gpui::Pixels>>,
    /// Error message to display
    error_message: Option<String>,

    /// Scroll handle for settings content
    settings_scroll_handle: ScrollHandle,
    /// Drag state for the settings scrollbar
    settings_scrollbar_drag: Rc<ScrollbarDragState>,
    /// Scroll handle for add model providers list
    add_model_providers_scroll_handle: ScrollHandle,
    /// Drag state for add model providers scrollbar
    add_model_providers_scrollbar_drag: Rc<ScrollbarDragState>,
    /// Scroll handle for add model models list
    add_model_models_scroll_handle: ScrollHandle,
    /// Drag state for add model models scrollbar
    add_model_models_scrollbar_drag: Rc<ScrollbarDragState>,
    /// Scroll handle for chat messages
    messages_scroll_handle: ScrollHandle,
    /// Drag state for chat messages scrollbar
    messages_scrollbar_drag: Rc<ScrollbarDragState>,

    /// Add model dialog state
    show_add_model_dialog: bool,
    add_model_providers: Vec<crate::cli::add_model::ProviderInfo>,
    add_model_selected_provider: Option<String>,
    add_model_models: Vec<crate::cli::add_model::ModelInfo>,
    add_model_selected_model: Option<String>,
    /// Text input for API key in add model dialog
    add_model_api_key_input_entity: Option<Entity<TextInput>>,
    add_model_loading: bool,
    add_model_error: Option<String>,

    /// API keys dialog state
    show_api_keys_dialog: bool,
    api_keys_list: Vec<String>,
    api_key_new_name: String,
    api_key_new_value: String,
}

impl ChatApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let message_bus = MessageBus::new();
        let theme = Theme::dark();

        // Initialize database
        let db = Rc::new(Database::open().expect("Failed to open database"));

        // Load settings
        let settings = Settings::new(&db);
        let current_model = settings.model();
        let user_mode = settings.user_mode();

        // Initialize model registry
        let model_registry = Arc::new(ModelRegistry::load_from_db(&db).unwrap_or_default());
        let available_models = model_registry.list_available(&db);

        // Initialize agent manager
        let agents = Arc::new(AgentManager::new());
        let current_agent = agents.current_name();
        let settings_selected_agent = current_agent.clone();
        let available_agents: Vec<(String, String)> = agents
            .list_filtered(user_mode)
            .into_iter()
            .map(|info| (info.name.clone(), info.display_name.clone()))
            .collect();

        // Initialize tool registry
        let tool_registry = Arc::new(SpotToolRegistry::new());

        // Initialize MCP manager
        let mcp_manager = Arc::new(McpManager::new());

        // Create text input
        let text_input = cx.new(|cx| TextInput::new(cx, theme.clone()));

        let app = Self {
            focus_handle,
            text_input,
            conversation: Conversation::new(),
            current_agent,
            current_model,
            user_mode,
            theme,
            is_generating: false,
            message_bus,
            db,
            agents,
            model_registry,
            tool_registry,
            mcp_manager,
            message_history: Vec::new(),
            available_agents,
            available_models,
            show_settings: false,
            settings_tab: settings::SettingsTab::PinnedAgents,
            settings_selected_agent,
            show_agent_dropdown: false,
            agent_dropdown_bounds: None,
            show_model_dropdown: false,
            model_dropdown_bounds: None,
            show_default_model_dropdown: false,
            default_model_dropdown_bounds: None,
            error_message: None,

            settings_scroll_handle: ScrollHandle::new(),
            settings_scrollbar_drag: Rc::new(ScrollbarDragState::default()),
            add_model_providers_scroll_handle: ScrollHandle::new(),
            add_model_providers_scrollbar_drag: Rc::new(ScrollbarDragState::default()),
            add_model_models_scroll_handle: ScrollHandle::new(),
            add_model_models_scrollbar_drag: Rc::new(ScrollbarDragState::default()),
            messages_scroll_handle: ScrollHandle::new(),
            messages_scrollbar_drag: Rc::new(ScrollbarDragState::default()),

            show_add_model_dialog: false,
            add_model_providers: Vec::new(),
            add_model_selected_provider: None,
            add_model_models: Vec::new(),
            add_model_selected_model: None,
            add_model_api_key_input_entity: None,
            add_model_loading: false,
            add_model_error: None,

            show_api_keys_dialog: false,
            api_keys_list: Vec::new(),
            api_key_new_name: String::new(),
            api_key_new_value: String::new(),
        };

        // Start message listener
        app.start_message_listener(cx);

        // Start MCP servers in background
        app.start_mcp_servers(cx);

        // Set up keyboard focus
        window.focus(&app.text_input.focus_handle(cx), cx);

        app
    }

    /// Start MCP servers
    fn start_mcp_servers(&self, cx: &mut Context<Self>) {
        let mcp = self.mcp_manager.clone();
        cx.spawn(
            async move |_this: WeakEntity<ChatApp>, _cx: &mut AsyncApp| {
                if let Err(e) = mcp.start_all().await {
                    eprintln!("Failed to start MCP servers: {}", e);
                }
            },
        )
        .detach();
    }

    /// Start listening to the message bus and update UI accordingly
    fn start_message_listener(&self, cx: &mut Context<Self>) {
        let mut receiver = self.message_bus.subscribe();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            while let Ok(msg) = receiver.recv().await {
                let result = this.update(cx, |app, cx| {
                    app.handle_message(msg, cx);
                });
                if result.is_err() {
                    break; // Entity dropped
                }
            }
        })
        .detach();
    }

    /// Handle incoming messages from the agent
    fn handle_message(&mut self, msg: Message, cx: &mut Context<Self>) {
        match msg {
            Message::TextDelta(delta) => {
                self.conversation.append_to_current(&delta.text);
            }
            Message::Thinking(thinking) => {
                // Display thinking in a muted style
                self.conversation
                    .append_to_current(&format!("\n\n💭 {}\n\n", thinking.text));
            }
            Message::Tool(tool) => {
                if matches!(tool.status, ToolStatus::Started) {
                    // Show tool call in conversation
                    self.conversation
                        .append_to_current(&format!("\n\n🔧 {}", tool.tool_name));
                } else if matches!(tool.status, ToolStatus::Completed) {
                    self.conversation.append_to_current(" ✓\n\n");
                } else if matches!(tool.status, ToolStatus::Failed) {
                    self.conversation
                        .append_to_current(&format!(" ✗ {}\n\n", tool.error.unwrap_or_default()));
                }
            }
            Message::Agent(agent) => match agent.event {
                AgentEvent::Started => {
                    self.conversation.start_assistant_message();
                    self.is_generating = true;
                }
                AgentEvent::Completed { .. } => {
                    self.conversation.finish_current_message();
                    self.is_generating = false;
                }
                AgentEvent::Error { message } => {
                    self.conversation
                        .append_to_current(&format!("\n\n❌ Error: {}", message));
                    self.conversation.finish_current_message();
                    self.is_generating = false;
                    self.error_message = Some(message);
                }
            },
            _ => {}
        }
        cx.notify();
    }

    /// Handle sending a message with real agent execution
    fn send_message(&mut self, cx: &mut Context<Self>) {
        let content = self.text_input.read(cx).content().to_string();
        let text = content.trim().to_string();

        if text.is_empty() || self.is_generating {
            return;
        }

        // Add user message to conversation
        self.conversation.add_user_message(&text);

        // Clear input
        self.text_input.update(cx, |input, cx| {
            input.clear(cx);
        });

        // Execute agent
        self.execute_agent(text, cx);

        cx.notify();
    }

    /// Execute the agent with the given prompt
    fn execute_agent(&mut self, prompt: String, cx: &mut Context<Self>) {
        let agent_name = self.current_agent.clone();
        let db = self.db.clone();
        let agents = self.agents.clone();
        let model_registry = self.model_registry.clone();
        let default_model = self.current_model.clone();
        let tool_registry = self.tool_registry.clone();
        let mcp_manager = self.mcp_manager.clone();
        let message_bus_sender = self.message_bus.sender();
        let history = if self.message_history.is_empty() {
            None
        } else {
            Some(self.message_history.clone())
        };

        self.is_generating = true;
        self.error_message = None;

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            // Look up the agent by name inside the async block
            let Some(agent) = agents.get(&agent_name) else {
                this.update(cx, |app, cx| {
                    app.is_generating = false;
                    app.error_message = Some("No agent selected".to_string());
                    cx.notify();
                })
                .ok();
                return;
            };

            // Create executor with message bus
            let executor = AgentExecutor::new(&db, &model_registry).with_bus(message_bus_sender);

            // Get the effective model for this agent (pinned or default)
            let effective_model = {
                let settings = Settings::new(&db);
                settings
                    .get_agent_pinned_model(&agent_name)
                    .unwrap_or_else(|| default_model.clone())
            };

            // Execute the agent
            let result = executor
                .execute_with_bus(
                    agent,
                    &effective_model,
                    &prompt,
                    history,
                    &tool_registry,
                    &mcp_manager,
                )
                .await;

            // Update state based on result
            this.update(cx, |app, cx| {
                app.is_generating = false;
                match result {
                    Ok(exec_result) => {
                        if !exec_result.messages.is_empty() {
                            app.message_history = exec_result.messages;
                        }
                    }
                    Err(e) => {
                        app.error_message = Some(e.to_string());
                        app.conversation
                            .append_to_current(&format!("\n\n❌ Error: {}", e));
                        app.conversation.finish_current_message();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Load providers from the bundled models.conf catalog for the Add Model dialog
    fn fetch_providers(&mut self, cx: &mut Context<Self>) {
        self.add_model_loading = true;
        self.add_model_error = None;
        self.add_model_providers.clear();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            match crate::cli::add_model::fetch_providers().await {
                Ok(providers) => {
                    this.update(cx, |app, cx| {
                        app.add_model_providers = providers.into_values().collect();
                        // Sort by name
                        app.add_model_providers.sort_by(|a, b| a.name.cmp(&b.name));
                        app.add_model_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
                Err(e) => {
                    this.update(cx, |app, cx| {
                        app.add_model_error = Some(format!("Failed to load providers: {}", e));
                        app.add_model_loading = false;
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    /// Add a single model from the Add Models dialog
    fn add_single_model(
        &mut self,
        provider_id: &str,
        model_id: &str,
        env_var: &str,
        cx: &mut Context<Self>,
    ) {
        use crate::models::{CustomEndpoint, ModelConfig, ModelRegistry, ModelType};
        use std::collections::HashMap;

        self.add_model_error = None;

        let api_key_value = self
            .add_model_api_key_input_entity
            .as_ref()
            .map(|e| e.read(cx).content().to_string())
            .unwrap_or_default();

        if !api_key_value.is_empty() {
            if let Err(e) = self.db.save_api_key(env_var, &api_key_value) {
                self.add_model_error = Some(format!("Failed to save API key: {}", e));
                cx.notify();
                return;
            }
        }

        let provider = self
            .add_model_providers
            .iter()
            .find(|p| p.id == provider_id);
        let model = self.add_model_models.iter().find(|m| m.id == model_id);

        let api_url = provider
            .and_then(|p| p.api.clone())
            .unwrap_or_else(|| match provider_id {
                "cerebras" => "https://api.cerebras.ai/v1".to_string(),
                "together" => "https://api.together.xyz/v1".to_string(),
                "groq" => "https://api.groq.com/openai/v1".to_string(),
                "fireworks" => "https://api.fireworks.ai/inference/v1".to_string(),
                "deepseek" => "https://api.deepseek.com/v1".to_string(),
                "mistral" => "https://api.mistral.ai/v1".to_string(),
                "perplexity" => "https://api.perplexity.ai".to_string(),
                "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            });

        let model_name = format!("{}:{}", provider_id, model_id);
        let context_length = model.and_then(|m| m.context_length).unwrap_or(128_000) as usize;
        let description = model
            .and_then(|m| m.name.clone())
            .unwrap_or_else(|| model_id.to_string());

        let config = ModelConfig {
            name: model_name.clone(),
            model_type: ModelType::CustomOpenai,
            model_id: Some(model_id.to_string()),
            context_length,
            supports_thinking: false,
            supports_vision: false,
            supports_tools: true,
            description: Some(description),
            custom_endpoint: Some(CustomEndpoint {
                url: api_url,
                api_key: Some(format!("${}", env_var)),
                headers: HashMap::new(),
                ca_certs_path: None,
            }),
            azure_deployment: None,
            azure_api_version: None,
            round_robin_models: Vec::new(),
        };

        if let Err(e) = ModelRegistry::add_model_to_db(&self.db, &config) {
            self.add_model_error = Some(format!("Failed to save model: {}", e));
            cx.notify();
            return;
        }

        let registry = ModelRegistry::load_from_db(&self.db).unwrap_or_default();
        self.available_models = registry.list_available(&self.db);
        self.model_registry = Arc::new(registry);

        cx.notify();
    }

    /// Delete a model from the registry
    fn delete_model(&mut self, model_name: &str, cx: &mut Context<Self>) {
        use crate::models::ModelRegistry;

        if self.current_model == model_name {
            if let Some(other) = self
                .available_models
                .iter()
                .find(|m| m.as_str() != model_name)
            {
                self.current_model = other.clone();
                let settings = crate::config::Settings::new(&self.db);
                let _ = settings.set("model", &self.current_model);
            }
        }

        if let Err(e) = ModelRegistry::remove_model_from_db(&self.db, model_name) {
            tracing::warn!("Failed to delete model {}: {}", model_name, e);
            return;
        }

        let registry = ModelRegistry::load_from_db(&self.db).unwrap_or_default();
        self.available_models = registry.list_available(&self.db);
        self.model_registry = std::sync::Arc::new(registry);

        cx.notify();
    }

    /// Start OAuth authentication flow
    fn start_oauth_flow(&mut self, provider: &'static str, cx: &mut Context<Self>) {
        let db = self.db.clone();

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            let result = match provider {
                "chatgpt" => crate::auth::run_chatgpt_auth(&db)
                    .await
                    .map_err(|e| e.to_string()),
                "claude-code" => crate::auth::run_claude_code_auth(&db)
                    .await
                    .map_err(|e| e.to_string()),
                _ => Err(format!("Unknown provider: {}", provider)),
            };

            this.update(cx, |app, cx| {
                match result {
                    Ok(_) => {
                        let registry =
                            crate::models::ModelRegistry::load_from_db(&app.db).unwrap_or_default();
                        app.available_models = registry.list_available(&app.db);
                        app.model_registry = std::sync::Arc::new(registry);
                    }
                    Err(e) => {
                        app.error_message = Some(format!("OAuth failed: {}", e));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Refresh the API keys list from database
    fn refresh_api_keys_list(&mut self) {
        self.api_keys_list = self.db.list_api_keys().unwrap_or_default();
    }

    /// Handle file drops
    fn handle_file_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        let file_paths: Vec<String> = paths
            .paths()
            .iter()
            .map(|p| p.display().to_string())
            .collect();

        if file_paths.is_empty() {
            return;
        }

        // Create a message mentioning the dropped files
        let files_text = if file_paths.len() == 1 {
            format!("I'm sharing this file with you: {}", file_paths[0])
        } else {
            format!(
                "I'm sharing these files with you:\n{}",
                file_paths
                    .iter()
                    .map(|p| format!("- {}", p))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };

        // Add to conversation
        self.conversation.add_user_message(&files_text);
        self.execute_agent(files_text, cx);
        cx.notify();
    }

    /// Handle new conversation
    fn new_conversation(
        &mut self,
        _: &NewConversation,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.conversation.clear();
        self.message_history.clear();
        self.text_input.update(cx, |input, cx| {
            input.clear(cx);
        });
        self.is_generating = false;
        self.show_agent_dropdown = false;
        self.show_model_dropdown = false;
        self.error_message = None;
        cx.notify();
    }

    /// Handle quit action
    fn quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Handle escape key to close dialogs
    fn close_dialog(&mut self, _: &CloseDialog, _window: &mut Window, cx: &mut Context<Self>) {
        // Close dialogs in order of precedence (most recent first)
        if self.show_add_model_dialog {
            self.show_add_model_dialog = false;
            self.add_model_selected_provider = None;
            self.add_model_selected_model = None;
            self.add_model_models.clear();
            if let Some(input) = &self.add_model_api_key_input_entity {
                input.update(cx, |input, cx| input.clear(cx));
            }
            self.add_model_error = None;
        } else if self.show_api_keys_dialog {
            self.show_api_keys_dialog = false;
            self.api_key_new_name.clear();
            self.api_key_new_value.clear();
        } else if self.show_settings {
            self.show_settings = false;
            self.show_default_model_dropdown = false;
            self.default_model_dropdown_bounds = None;
        }
        cx.notify();
    }

    /// Handle send action
    fn on_send(&mut self, _: &Send, _window: &mut Window, cx: &mut Context<Self>) {
        self.send_message(cx);
    }

    /// Switch to next agent
    fn next_agent(&mut self, _: &NextAgent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.available_agents.is_empty() {
            return;
        }

        let current_idx = self
            .available_agents
            .iter()
            .position(|(name, _)| name == &self.current_agent)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.available_agents.len();
        if let Some((name, _)) = self.available_agents.get(next_idx) {
            let name = name.clone();
            self.set_current_agent(&name);
        }
        cx.notify();
    }

    /// Switch to previous agent
    fn prev_agent(&mut self, _: &PrevAgent, _window: &mut Window, cx: &mut Context<Self>) {
        if self.available_agents.is_empty() {
            return;
        }

        let current_idx = self
            .available_agents
            .iter()
            .position(|(name, _)| name == &self.current_agent)
            .unwrap_or(0);
        let prev_idx = if current_idx == 0 {
            self.available_agents.len() - 1
        } else {
            current_idx - 1
        };
        if let Some((name, _)) = self.available_agents.get(prev_idx) {
            let name = name.clone();
            self.set_current_agent(&name);
        }
        cx.notify();
    }

    fn set_current_agent(&mut self, name: &str) {
        if self.current_agent == name {
            self.show_agent_dropdown = false;
            self.show_model_dropdown = false;
            return;
        }

        self.current_agent = name.to_string();
        let _ = self.agents.switch(name);
        self.message_history.clear();
        self.show_agent_dropdown = false;
        self.show_model_dropdown = false;
    }

    /// Get effective model for an agent (pinned or default).
    fn effective_model_for_agent(&self, agent_name: &str) -> (String, bool) {
        let settings = Settings::new(&self.db);
        if let Some(pinned) = settings.get_agent_pinned_model(agent_name) {
            (pinned, true)
        } else {
            (self.current_model.clone(), false)
        }
    }

    fn current_effective_model(&self) -> (String, bool) {
        self.effective_model_for_agent(&self.current_agent)
    }

    /// Get display name for current agent
    fn current_agent_display(&self) -> String {
        self.available_agents
            .iter()
            .find(|(name, _)| name == &self.current_agent)
            .map(|(_, display)| display.clone())
            .unwrap_or_else(|| self.current_agent.clone())
    }

    /// Truncate model name for display
    fn truncate_model_name(name: &str) -> String {
        if name.len() > 25 {
            format!("{}...", &name[..22])
        } else {
            name.to_string()
        }
    }
}

impl Focusable for ChatApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::new_conversation))
            .on_action(cx.listener(Self::on_send))
            .on_action(cx.listener(Self::next_agent))
            .on_action(cx.listener(Self::prev_agent))
            .on_action(cx.listener(Self::close_dialog))
            // File drag and drop support
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                this.handle_file_drop(paths, cx);
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(self.theme.background)
            .text_color(self.theme.text)
            .relative()
            .child(self.render_toolbar(cx))
            .child(self.render_agent_dropdown_panel(cx))
            .child(self.render_model_dropdown_panel(cx))
            .child(self.render_error())
            .child(self.render_messages(cx))
            .child(self.render_input(cx))
            .child(self.render_settings(cx))
            .child(self.render_add_model_dialog(cx))
    }
}

/// Register keybindings for the application
pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-n", NewConversation, None),
        KeyBinding::new("enter", Send, Some("TextInput")),
        KeyBinding::new("cmd-]", NextAgent, None),
        KeyBinding::new("cmd-[", PrevAgent, None),
        KeyBinding::new("escape", CloseDialog, None),
        // Text input keybindings
        KeyBinding::new("backspace", super::components::Backspace, Some("TextInput")),
        KeyBinding::new("delete", super::components::Delete, Some("TextInput")),
        KeyBinding::new("left", super::components::Left, Some("TextInput")),
        KeyBinding::new("right", super::components::Right, Some("TextInput")),
        KeyBinding::new(
            "shift-left",
            super::components::SelectLeft,
            Some("TextInput"),
        ),
        KeyBinding::new(
            "shift-right",
            super::components::SelectRight,
            Some("TextInput"),
        ),
        KeyBinding::new("cmd-a", super::components::SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-v", super::components::Paste, Some("TextInput")),
        KeyBinding::new("cmd-c", super::components::Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", super::components::Cut, Some("TextInput")),
        KeyBinding::new("home", super::components::Home, Some("TextInput")),
        KeyBinding::new("end", super::components::End, Some("TextInput")),
    ]);
}
