//! Main application state and rendering

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    actions, div, prelude::*, px, rgb, App, AsyncApp, Context, Entity,
    ExternalPaths, FocusHandle, Focusable, KeyBinding, MouseButton, SharedString, Styled,
    WeakEntity, Window,
};

use super::components::{TextInput, ZedMarkdownText};
use super::state::{Conversation, MessageRole};
use super::theme::Theme;
use crate::agents::{AgentExecutor, AgentManager};
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
    ]
);

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
    /// Color theme
    theme: Theme,
    /// Whether we're currently generating a response
    is_generating: bool,
    /// Message bus for agent communication
    message_bus: MessageBus,
    /// Database connection (wrapped for async use)
    db: Arc<Database>,
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
    /// Error message to display
    error_message: Option<String>,
    /// Rendered markdown entities for each message (keyed by message ID)
    message_texts: HashMap<String, Entity<ZedMarkdownText>>,
}

impl ChatApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let message_bus = MessageBus::new();
        let theme = Theme::dark();

        // Initialize database
        let db = Arc::new(Database::open().expect("Failed to open database"));

        // Load settings
        let settings = Settings::new(&db);
        let current_model = settings.model();

        // Initialize model registry
        let model_registry = Arc::new(ModelRegistry::load_from_db(&db).unwrap_or_default());
        let available_models = model_registry.list_available(&db);

        // Initialize agent manager
        let agents = Arc::new(AgentManager::new());
        let current_agent = agents.current_name();
        let available_agents: Vec<(String, String)> = agents
            .list()
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
            error_message: None,
            message_texts: HashMap::new(),
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
        cx.spawn(async move |_this: WeakEntity<ChatApp>, _cx: &mut AsyncApp| {
            if let Err(e) = mcp.start_all().await {
                eprintln!("Failed to start MCP servers: {}", e);
            }
        })
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
                // Update the SelectableText entity for the current message
                if let Some(current_msg) = self.conversation.messages.last() {
                    let id = current_msg.id.clone();
                    let content = current_msg.content.clone();
                    self.update_message_text(&id, &content, cx);
                }
            }
            Message::Thinking(thinking) => {
                // Display thinking in a muted style
                self.conversation
                    .append_to_current(&format!("\n\n💭 {}\n\n", thinking.text));
                if let Some(current_msg) = self.conversation.messages.last() {
                    let id = current_msg.id.clone();
                    let content = current_msg.content.clone();
                    self.update_message_text(&id, &content, cx);
                }
            }
            Message::Tool(tool) => {
                if matches!(tool.status, ToolStatus::Started) {
                    // Show tool call in conversation
                    self.conversation
                        .append_to_current(&format!("\n🔧 {}", tool.tool_name));
                } else if matches!(tool.status, ToolStatus::Completed) {
                    self.conversation.append_to_current(" ✓\n");
                } else if matches!(tool.status, ToolStatus::Failed) {
                    self.conversation
                        .append_to_current(&format!(" ✗ {}\n", tool.error.unwrap_or_default()));
                }
                if let Some(current_msg) = self.conversation.messages.last() {
                    let id = current_msg.id.clone();
                    let content = current_msg.content.clone();
                    self.update_message_text(&id, &content, cx);
                }
            }
            Message::Agent(agent) => match agent.event {
                AgentEvent::Started => {
                    self.conversation.start_assistant_message();
                    // Create SelectableText entity for the new assistant message
                    if let Some(msg) = self.conversation.messages.last() {
                        let id = msg.id.clone();
                        self.create_message_text(&id, "", cx);
                    }
                    self.is_generating = true;
                }
                AgentEvent::Completed { .. } => {
                    self.conversation.finish_current_message();
                    self.is_generating = false;
                }
                AgentEvent::Error { message } => {
                    self.conversation
                        .append_to_current(&format!("\n\n❌ Error: {}", message));
                    if let Some(current_msg) = self.conversation.messages.last() {
                        let id = current_msg.id.clone();
                        let content = current_msg.content.clone();
                        self.update_message_text(&id, &content, cx);
                    }
                    self.conversation.finish_current_message();
                    self.is_generating = false;
                    self.error_message = Some(message);
                }
            },
            _ => {}
        }
        cx.notify();
    }

    /// Create a Zed markdown-rendered entity for a message
    fn create_message_text(&mut self, id: &str, content: &str, cx: &mut Context<Self>) {
        let theme = self.theme.clone();
        let entity = cx.new(|cx| ZedMarkdownText::new(cx, content.to_string(), theme));
        self.message_texts.insert(id.to_string(), entity);
    }

    /// Update a message entity's content
    fn update_message_text(&mut self, id: &str, content: &str, cx: &mut Context<Self>) {
        if let Some(entity) = self.message_texts.get(id) {
            entity.update(cx, |text, cx| {
                text.set_content(content.to_string(), cx);
            });
        }
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

        // Create markdown-rendered entity for this message
        if let Some(msg) = self.conversation.messages.last() {
            let id = msg.id.clone();
            self.create_message_text(&id, &text, cx);
        }

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
        let current_model = self.current_model.clone();
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
                }).ok();
                return;
            };

            // Create executor with message bus
            let executor = AgentExecutor::new(&db, &model_registry).with_bus(message_bus_sender);

            // Execute the agent
            let result = executor
                .execute_with_bus(agent, &current_model, &prompt, history, &tool_registry, &mcp_manager)
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
                file_paths.iter().map(|p| format!("- {}", p)).collect::<Vec<_>>().join("\n")
            )
        };

        // Add to conversation and create SelectableText entity
        self.conversation.add_user_message(&files_text);
        if let Some(msg) = self.conversation.messages.last() {
            let id = msg.id.clone();
            self.create_message_text(&id, &files_text, cx);
        }
        self.execute_agent(files_text, cx);
        cx.notify();
    }

    /// Handle new conversation
    fn new_conversation(&mut self, _: &NewConversation, _window: &mut Window, cx: &mut Context<Self>) {
        self.conversation.clear();
        self.message_history.clear();
        self.message_texts.clear();
        self.text_input.update(cx, |input, cx| {
            input.clear(cx);
        });
        self.is_generating = false;
        self.error_message = None;
        cx.notify();
    }

    /// Handle quit action
    fn quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Handle send action
    fn on_send(&mut self, _: &Send, _window: &mut Window, cx: &mut Context<Self>) {
        self.send_message(cx);
    }

    /// Switch to next agent
    fn next_agent(&mut self, _: &NextAgent, _window: &mut Window, cx: &mut Context<Self>) {
        let current_idx = self
            .available_agents
            .iter()
            .position(|(name, _)| name == &self.current_agent)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.available_agents.len();
        if let Some((name, _)) = self.available_agents.get(next_idx) {
            self.current_agent = name.clone();
            let _ = self.agents.switch(name);
            self.message_history.clear();
        }
        cx.notify();
    }

    /// Switch to previous agent
    fn prev_agent(&mut self, _: &PrevAgent, _window: &mut Window, cx: &mut Context<Self>) {
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
            self.current_agent = name.clone();
            let _ = self.agents.switch(name);
            self.message_history.clear();
        }
        cx.notify();
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

    /// Render the toolbar
    fn render_toolbar(&self, cx: &Context<Self>) -> impl IntoElement {
        let agent_display = self.current_agent_display();
        let model_display = Self::truncate_model_name(&self.current_model);

        div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.))
            .py(px(10.))
            .border_b_1()
            .border_color(self.theme.border)
            .bg(self.theme.panel_background)
            .child(
                // Left side - branding and selectors
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    // Logo
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().text_size(px(18.)).child("🍲"))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(self.theme.text)
                                    .child("Stockpot"),
                            ),
                    )
                    // Agent selector
                    .child(
                        div()
                            .id("agent-selector")
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(6.))
                            .bg(self.theme.tool_card)
                            .text_color(self.theme.text)
                            .text_size(px(12.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .child(format!("🤖 {}", agent_display)),
                    )
                    // Model selector
                    .child(
                        div()
                            .id("model-selector")
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(6.))
                            .bg(self.theme.tool_card)
                            .text_color(self.theme.text)
                            .text_size(px(12.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .child(format!("📦 {}", model_display)),
                    ),
            )
            .child(
                // Right side - actions
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    // MCP status
                    .child(
                        div()
                            .px(px(10.))
                            .py(px(5.))
                            .rounded(px(6.))
                            .bg(self.theme.tool_card)
                            .text_color(self.theme.text_muted)
                            .text_size(px(12.))
                            .child("🔌 MCP"),
                    )
                    // New conversation
                    .child(
                        div()
                            .id("new-btn")
                            .px(px(12.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .bg(self.theme.accent)
                            .text_color(rgb(0xffffff))
                            .text_size(px(12.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.9))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    this.new_conversation(&NewConversation, window, cx);
                                }),
                            )
                            .child("+ New"),
                    )
                    // Settings
                    .child(
                        div()
                            .id("settings-btn")
                            .px(px(12.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .bg(self.theme.tool_card)
                            .text_color(self.theme.text)
                            .text_size(px(12.))
                            .cursor_pointer()
                            .hover(|s| s.opacity(0.8))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.show_settings = !this.show_settings;
                                    cx.notify();
                                }),
                            )
                            .child("⚙"),
                    ),
            )
    }

    /// Render the message list
    fn render_messages(&self, _cx: &Context<Self>) -> impl IntoElement {
        let messages = self.conversation.messages.clone();
        let theme = self.theme.clone();
        let has_messages = !messages.is_empty();
        let message_texts = self.message_texts.clone();

        div()
            .id("messages-container")
            .flex_1()
            .overflow_y_scroll()
            .p(px(16.))
            .when(!has_messages, |d| {
                d.flex().items_center().justify_center().child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(12.))
                        .child(div().text_size(px(56.)).child("🍲"))
                        .child(
                            div()
                                .text_size(px(20.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(theme.text)
                                .child("Welcome to Stockpot"),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(theme.text_muted)
                                .child("Your AI-powered coding assistant"),
                        )
                        .child(
                            div()
                                .mt(px(16.))
                                .text_size(px(13.))
                                .text_color(theme.text_muted)
                                .child("Type a message below to get started"),
                        )
                        .child(
                            div()
                                .mt(px(8.))
                                .text_size(px(12.))
                                .text_color(theme.text_muted)
                                .child("📁 Drag and drop files here to share them"),
                        ),
                )
            })
            .when(has_messages, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(16.))
                        .children(messages.into_iter().enumerate().map(|(idx, msg)| {
                            let is_user = msg.role == super::state::MessageRole::User;
                            let bubble_bg = if is_user {
                                theme.user_bubble
                            } else {
                                theme.assistant_bubble
                            };
                            let text_entity = message_texts.get(&msg.id).cloned();
                            let has_entity = text_entity.is_some();
                            let msg_content = msg.content.clone();
                            let is_streaming = msg.is_streaming;

                            div()
                                .id(SharedString::from(format!("msg-{}", idx)))
                                .flex()
                                .flex_col()
                                .when(is_user, |d| d.items_end())
                                .when(!is_user, |d| d.items_start())
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(theme.text_muted)
                                        .mb(px(4.))
                                        .child(if is_user { "You" } else { "Assistant" }),
                                )
                                .child(
                                    div()
                                        .max_w(px(700.))
                                        .min_w(px(100.))
                                        .p(px(12.))
                                        .rounded(px(8.))
                                        .bg(bubble_bg)
                                        .text_color(theme.text)
                                        .when_some(text_entity, |d, entity| {
                                            d.child(entity)
                                        })
                                        .when(!has_entity, |d| {
                                            // Fallback to plain text if no entity exists
                                            d.child(msg_content)
                                        })
                                        .when(is_streaming, |d: gpui::Div| {
                                            d.child(
                                                div()
                                                    .ml(px(2.))
                                                    .text_color(theme.accent)
                                                    .child("▋"),
                                            )
                                        }),
                                )
                        })),
                )
            })
    }

    /// Render error message if present
    fn render_error(&self) -> impl IntoElement {
        let theme = self.theme.clone();
        let error = self.error_message.clone();

        div().when_some(error, |d, msg| {
            d.px(px(16.))
                .py(px(8.))
                .bg(theme.error)
                .text_color(rgb(0xffffff))
                .text_size(px(13.))
                .child(format!("⚠️ {}", msg))
        })
    }

    /// Render the input area
    fn render_input(&self, cx: &Context<Self>) -> impl IntoElement {
        let is_generating = self.is_generating;
        let theme = self.theme.clone();

        div()
            .flex()
            .items_end()
            .gap(px(12.))
            .p(px(16.))
            .border_t_1()
            .border_color(self.theme.border)
            .bg(self.theme.panel_background)
            .child(self.text_input.clone())
            .child(
                div()
                    .id("send-btn")
                    .px(px(16.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .bg(if is_generating {
                        theme.text_muted
                    } else {
                        theme.accent
                    })
                    .text_color(rgb(0xffffff))
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _window, cx| {
                            if !this.is_generating {
                                this.send_message(cx);
                            }
                        }),
                    )
                    .child(if is_generating { "⏳" } else { "Send →" }),
            )
    }

    /// Render settings panel
    fn render_settings(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let show = self.show_settings;
        let available_agents = self.available_agents.clone();
        let current_agent = self.current_agent.clone();
        let available_models = self.available_models.clone();
        let current_model = self.current_model.clone();

        div()
            .when(show, |d| {
                d.absolute()
                    .top_0()
                    .right_0()
                    .w(px(320.))
                    .h_full()
                    .bg(theme.panel_background)
                    .border_l_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .child(
                        // Header
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .p(px(16.))
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.text)
                                    .child("Settings"),
                            )
                            .child(
                                div()
                                    .id("close-settings")
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(theme.tool_card))
                                    .text_color(theme.text_muted)
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.show_settings = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child("✕"),
                            ),
                    )
                    .child(
                        // Content
                        div()
                            .id("settings-content")
                            .flex_1()
                            .overflow_y_scroll()
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(20.))
                            // Agent section
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child("Agent"),
                                    )
                                    .children(available_agents.iter().map(|(name, display)| {
                                        let is_selected = name == &current_agent;
                                        let name_clone = name.clone();
                                        div()
                                            .id(SharedString::from(format!("agent-{}", name)))
                                            .px(px(12.))
                                            .py(px(8.))
                                            .rounded(px(6.))
                                            .bg(if is_selected {
                                                theme.accent
                                            } else {
                                                theme.tool_card
                                            })
                                            .text_color(if is_selected {
                                                rgb(0xffffff)
                                            } else {
                                                theme.text
                                            })
                                            .text_size(px(13.))
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.9))
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.current_agent = name_clone.clone();
                                                    let _ = this.agents.switch(&name_clone);
                                                    this.message_history.clear();
                                                    cx.notify();
                                                }),
                                            )
                                            .child(display.clone())
                                    })),
                            )
                            // Model section
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child("Model"),
                                    )
                                    .child(
                                        div()
                                            .id("models-list")
                                            .max_h(px(200.))
                                            .overflow_y_scroll()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.))
                                            .children(available_models.iter().map(|model| {
                                                let is_selected = model == &current_model;
                                                let model_clone = model.clone();
                                                div()
                                                    .id(SharedString::from(format!("model-{}", model)))
                                                    .px(px(12.))
                                                    .py(px(8.))
                                                    .rounded(px(6.))
                                                    .bg(if is_selected {
                                                        theme.accent
                                                    } else {
                                                        theme.tool_card
                                                    })
                                                    .text_color(if is_selected {
                                                        rgb(0xffffff)
                                                    } else {
                                                        theme.text
                                                    })
                                                    .text_size(px(12.))
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.9))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |this, _, _, cx| {
                                                            this.current_model = model_clone.clone();
                                                            let settings = Settings::new(&this.db);
                                                            let _ = settings.set("model", &model_clone);
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .child(Self::truncate_model_name(model))
                                            })),
                                    ),
                            )
                            // Auth section
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(theme.text)
                                            .child("Authentication"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme.text_muted)
                                            .child("Use the CLI to configure API keys:"),
                                    )
                                    .child(
                                        div()
                                            .p(px(8.))
                                            .rounded(px(4.))
                                            .bg(theme.background)
                                            .text_size(px(11.))
                                            .text_color(theme.text_muted)
                                            .child("spot /chatgpt-auth"),
                                    )
                                    .child(
                                        div()
                                            .p(px(8.))
                                            .rounded(px(4.))
                                            .bg(theme.background)
                                            .text_size(px(11.))
                                            .text_color(theme.text_muted)
                                            .child("spot /claude-code-auth"),
                                    ),
                            ),
                    )
            })
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
            .child(self.render_error())
            .child(self.render_messages(cx))
            .child(self.render_input(cx))
            .child(self.render_settings(cx))
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
        // Text input keybindings
        KeyBinding::new("backspace", super::components::Backspace, Some("TextInput")),
        KeyBinding::new("delete", super::components::Delete, Some("TextInput")),
        KeyBinding::new("left", super::components::Left, Some("TextInput")),
        KeyBinding::new("right", super::components::Right, Some("TextInput")),
        KeyBinding::new("shift-left", super::components::SelectLeft, Some("TextInput")),
        KeyBinding::new("shift-right", super::components::SelectRight, Some("TextInput")),
        KeyBinding::new("cmd-a", super::components::SelectAll, Some("TextInput")),
        KeyBinding::new("cmd-v", super::components::Paste, Some("TextInput")),
        KeyBinding::new("cmd-c", super::components::Copy, Some("TextInput")),
        KeyBinding::new("cmd-x", super::components::Cut, Some("TextInput")),
        KeyBinding::new("home", super::components::Home, Some("TextInput")),
        KeyBinding::new("end", super::components::End, Some("TextInput")),
        // Markdown keybindings (for message content)
        KeyBinding::new("cmd-c", markdown::Copy, Some("Markdown")),
        KeyBinding::new("ctrl-c", markdown::Copy, Some("Markdown")),
    ]);
}
