//! Main application state and rendering

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    actions, div, prelude::*, px, rgb, App, AsyncApp, Context, Entity, ExternalPaths, FocusHandle,
    Focusable, KeyBinding, MouseButton, ScrollHandle, SharedString, Styled, WeakEntity, Window,
};
use gpui_component::input::{Input, InputEvent, InputState};

use super::components::ScrollbarDragState;
use super::state::{Conversation, MessageRole};
use super::theme::Theme;
use crate::agents::{AgentExecutor, AgentManager, UserMode};
use crate::config::{PdfMode, Settings};
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{AgentEvent, Message, MessageBus, ToolStatus};
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;
use serdes_ai_core::messages::ImageMediaType;

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
        PasteAttachment,
    ]
);

mod agent_dropdown;
mod attachments;
mod error;
mod input;
mod messages;
mod model_dropdown;
mod settings;
mod toolbar;

pub use attachments::{
    PendingAttachment, PendingFile, PendingImage, PendingPdf, MAX_ATTACHMENTS, MAX_IMAGE_DIMENSION,
    THUMBNAIL_SIZE,
};

/// Main application state
pub struct ChatApp {
    /// Focus handle for keyboard input
    focus_handle: FocusHandle,
    /// Input state for the message input field (gpui-component)
    input_state: Entity<InputState>,
    /// Current conversation
    conversation: Conversation,
    /// Selected agent name
    current_agent: String,
    /// Selected model name
    current_model: String,
    /// Current user mode (controls agent visibility)
    user_mode: UserMode,
    /// PDF processing mode (image vs text extraction)
    pdf_mode: PdfMode,
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
    add_model_api_key_input_entity: Option<Entity<InputState>>,
    add_model_loading: bool,
    add_model_error: Option<String>,

    /// API keys dialog state
    show_api_keys_dialog: bool,
    api_keys_list: Vec<String>,
    api_key_new_name: String,
    api_key_new_value: String,
    /// Pending attachments (images and files) waiting to be sent
    pending_attachments: Vec<PendingAttachment>,
}

impl ChatApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let message_bus = MessageBus::new();
        let theme = Theme::dark();

        // Initialize database
        let db = Rc::new(Database::open().expect("Failed to open database"));

        // Run migrations to ensure schema is up to date
        db.migrate().expect("Failed to run database migrations");

        // Load settings
        let settings = Settings::new(&db);
        let current_model = settings.model();
        let user_mode = settings.user_mode();
        let pdf_mode = settings.pdf_mode();

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

        // Create input state with auto-grow (1-3 lines, then scrollbar)
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Type a message...")
                .auto_grow(1, 3)
        });

        // Subscribe to input events for Enter key
        cx.subscribe(&input_state, |this, _input, event: &InputEvent, cx| {
            if let InputEvent::PressEnter { secondary: false } = event {
                if !this.is_generating {
                    cx.dispatch_action(&Send);
                }
            }
        })
        .detach();

        let app = Self {
            focus_handle,
            input_state,
            conversation: Conversation::new(),
            current_agent,
            current_model,
            user_mode,
            pdf_mode,
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
            pending_attachments: Vec::new(),
        };

        // Start message listener
        app.start_message_listener(cx);

        // Start MCP servers in background
        app.start_mcp_servers(cx);

        // Set up keyboard focus on the main app
        window.focus(&app.focus_handle, cx);

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

    /// Refresh the model registry and available models list.
    /// Call this when opening settings, after OAuth, or after adding API keys.
    pub(super) fn refresh_models(&mut self) {
        tracing::debug!("refresh_models: starting");
        match ModelRegistry::load_from_db(&self.db) {
            Ok(registry) => {
                let total_in_registry = registry.len();
                self.available_models = registry.list_available(&self.db);
                let available_count = self.available_models.len();
                tracing::debug!(
                    total_in_registry = total_in_registry,
                    available_count = available_count,
                    models = ?self.available_models,
                    "refresh_models: complete"
                );
                self.model_registry = Arc::new(registry);
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to refresh model registry");
            }
        }
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
    fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.input_state.read(cx).value().to_string();
        let text = content.trim().to_string();
        let has_attachments = !self.pending_attachments.is_empty();

        // Need either text or attachments
        if text.is_empty() && !has_attachments {
            return;
        }

        if self.is_generating {
            return;
        }

        // Build the message including attachments
        let mut full_message = text.clone();

        // Add file references for non-image attachments
        let file_refs: Vec<String> = self
            .pending_attachments
            .iter()
            .filter_map(|att| match att {
                PendingAttachment::File(f) => Some(format!("File: {}", f.path.display())),
                _ => None,
            })
            .collect();

        if !file_refs.is_empty() {
            if !full_message.is_empty() {
                full_message.push_str("\n\n");
            }
            full_message.push_str(&file_refs.join("\n"));
        }

        // Collect images for vision model support
        // Each image is (PNG bytes, ImageMediaType::Png)
        let mut images: Vec<(Vec<u8>, ImageMediaType)> = self
            .pending_attachments
            .iter()
            .filter_map(|att| match att {
                PendingAttachment::Image(img) => {
                    Some((img.processed_data.clone(), ImageMediaType::Png))
                }
                _ => None,
            })
            .collect();

        // Log collected images
        tracing::info!(
            image_count = images.len(),
            total_bytes = images.iter().map(|(b, _)| b.len()).sum::<usize>(),
            "Collected images for sending"
        );

        // Process PDF attachments based on current mode
        use crate::gui::pdf_processing::{extract_pdf_text, render_pdf_to_images};

        let pdf_attachments: Vec<_> = self
            .pending_attachments
            .iter()
            .filter_map(|att| match att {
                PendingAttachment::Pdf(pdf) => Some(pdf.clone()),
                _ => None,
            })
            .collect();

        if !pdf_attachments.is_empty() {
            match self.pdf_mode {
                PdfMode::Image => {
                    // IMAGE MODE: Render PDF pages to images
                    for pdf in &pdf_attachments {
                        tracing::info!(
                            filename = %pdf.filename,
                            page_count = pdf.page_count,
                            "Converting PDF to images"
                        );
                        match render_pdf_to_images(&pdf.path, MAX_IMAGE_DIMENSION) {
                            Ok(page_images) => {
                                let pages_rendered = page_images.len();
                                for page in page_images {
                                    images.push((page.processed_data, ImageMediaType::Png));
                                }
                                tracing::info!(
                                    filename = %pdf.filename,
                                    pages_rendered = pages_rendered,
                                    "PDF pages converted to images"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    filename = %pdf.filename,
                                    error = %e,
                                    "Failed to render PDF to images"
                                );
                                self.error_message = Some(format!(
                                    "Failed to convert PDF '{}': {}",
                                    pdf.filename, e
                                ));
                            }
                        }
                    }
                }
                PdfMode::TextExtract => {
                    // TEXT MODE: Extract text and append to message
                    for pdf in &pdf_attachments {
                        tracing::info!(
                            filename = %pdf.filename,
                            page_count = pdf.page_count,
                            "Extracting text from PDF"
                        );
                        match extract_pdf_text(&pdf.path) {
                            Ok(text) => {
                                if !full_message.is_empty() {
                                    full_message.push_str("\n\n");
                                }
                                full_message.push_str(&format!(
                                    "--- PDF: {} ({} pages) ---\n{}",
                                    pdf.filename, pdf.page_count, text
                                ));
                                tracing::info!(
                                    filename = %pdf.filename,
                                    text_length = text.len(),
                                    "PDF text extracted"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    filename = %pdf.filename,
                                    error = %e,
                                    "Failed to extract PDF text"
                                );
                                self.error_message = Some(format!(
                                    "Failed to extract text from '{}': {}",
                                    pdf.filename, e
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Check if the model supports vision - if not, warn and strip images
        let has_images = !images.is_empty();
        if has_images {
            // Get the effective model for the current agent
            let effective_model_name = {
                let settings = Settings::new(&self.db);
                settings
                    .get_agent_pinned_model(&self.current_agent)
                    .unwrap_or_else(|| self.current_model.clone())
            };

            // Check if model supports vision
            let model_config = self.model_registry.get(&effective_model_name);

            // Log the vision check for debugging
            tracing::info!(
                model_name = %effective_model_name,
                model_found = model_config.is_some(),
                supports_vision = model_config.map(|m| m.supports_vision),
                "Vision support check"
            );

            // Default to TRUE if model not found (assume modern models support vision)
            let supports_vision = model_config.map(|m| m.supports_vision).unwrap_or(true);

            if !supports_vision {
                // Show warning to user
                tracing::warn!(
                    model_name = %effective_model_name,
                    "Model doesn't support vision, stripping images"
                );
                self.error_message = Some(format!(
                    "⚠️ Model '{}' doesn't support images. Images removed from message.",
                    effective_model_name
                ));
                // Strip images
                images.clear();
            }
        }

        // Add user message to conversation
        if has_attachments {
            let attachment_note = format!(
                "{}\n\n📎 {} attachment(s)",
                if text.is_empty() {
                    "(Image attached)".to_string()
                } else {
                    text.clone()
                },
                self.pending_attachments.len()
            );
            self.conversation.add_user_message(&attachment_note);
        } else {
            self.conversation.add_user_message(&text);
        }

        // Clear input and attachments
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
        self.pending_attachments.clear();

        // Execute agent with message and images
        self.execute_agent(full_message, images, cx);

        cx.notify();
    }

    /// Execute the agent with the given prompt and optional images
    fn execute_agent(
        &mut self,
        prompt: String,
        images: Vec<(Vec<u8>, ImageMediaType)>,
        cx: &mut Context<Self>,
    ) {
        // Bundle all data that needs to be moved into the async closure
        // This ensures everything is captured as a single unit
        struct ExecuteData {
            agent_name: String,
            db: Rc<Database>,
            agents: Arc<AgentManager>,
            model_registry: Arc<ModelRegistry>,
            default_model: String,
            tool_registry: Arc<SpotToolRegistry>,
            mcp_manager: Arc<McpManager>,
            message_bus_sender: crate::messaging::MessageSender,
            prompt: String,
            images: Vec<(Vec<u8>, ImageMediaType)>,
            history: Option<Vec<serdes_ai_core::ModelRequest>>,
        }

        let data = ExecuteData {
            agent_name: self.current_agent.clone(),
            db: self.db.clone(),
            agents: self.agents.clone(),
            model_registry: self.model_registry.clone(),
            default_model: self.current_model.clone(),
            tool_registry: self.tool_registry.clone(),
            mcp_manager: self.mcp_manager.clone(),
            message_bus_sender: self.message_bus.sender(),
            prompt,
            images,
            history: if self.message_history.is_empty() {
                None
            } else {
                Some(self.message_history.clone())
            },
        };

        // Log BEFORE the spawn to verify data is correct in struct
        tracing::info!(
            image_count_in_struct = data.images.len(),
            prompt_len_in_struct = data.prompt.len(),
            "execute_agent: data struct created BEFORE spawn"
        );

        self.is_generating = true;
        self.error_message = None;

        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            // Destructure the data bundle
            let ExecuteData {
                agent_name,
                db,
                agents,
                model_registry,
                default_model,
                tool_registry,
                mcp_manager,
                message_bus_sender,
                prompt,
                images,
                history,
            } = data;

            // Log images inside async block to verify they survived the move
            tracing::info!(
                image_count = images.len(),
                prompt_len = prompt.len(),
                "execute_agent async: checking images before execution"
            );

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

            // Execute the agent - use execute_with_images if we have images
            tracing::info!(
                images_empty = images.is_empty(),
                "execute_agent: about to choose execution path"
            );
            let result = if images.is_empty() {
                executor
                    .execute_with_bus(
                        agent,
                        &effective_model,
                        &prompt,
                        history,
                        &tool_registry,
                        &mcp_manager,
                    )
                    .await
            } else {
                executor
                    .execute_with_images(
                        agent,
                        &effective_model,
                        &prompt,
                        &images,
                        history,
                        &tool_registry,
                        &mcp_manager,
                    )
                    .await
            };

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
            .map(|e| e.read(cx).value().to_string())
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
                        // Refresh models to pick up newly registered OAuth models
                        app.refresh_models();
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

    /// Handle file drops - add to pending attachments instead of sending immediately
    fn handle_file_drop(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        use crate::gui::image_processing::{is_image_file, process_image_from_path};
        use crate::gui::pdf_processing::{get_pdf_preview, is_pdf_file};

        let dropped_paths: Vec<_> = paths.paths().to_vec();

        if dropped_paths.is_empty() {
            return;
        }

        // Check how many we can add
        let available_slots = MAX_ATTACHMENTS.saturating_sub(self.pending_attachments.len());
        if available_slots == 0 {
            self.error_message = Some(format!(
                "Maximum {} attachments reached. Remove some to add more.",
                MAX_ATTACHMENTS
            ));
            cx.notify();
            return;
        }

        // Process files (limited to available slots)
        let paths_to_process: Vec<_> = dropped_paths.into_iter().take(available_slots).collect();

        // Spawn async task to process images without blocking UI
        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
            let mut new_attachments = Vec::new();

            for path in paths_to_process {
                if is_pdf_file(&path) {
                    // Process PDF - get preview (page count + thumbnail)
                    match get_pdf_preview(&path) {
                        Ok(preview) => {
                            let filename = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("document.pdf")
                                .to_string();
                            new_attachments.push(PendingAttachment::Pdf(PendingPdf {
                                path: path.clone(),
                                filename,
                                page_count: preview.page_count,
                                thumbnail_data: if preview.thumbnail_data.is_empty() {
                                    None
                                } else {
                                    Some(preview.thumbnail_data)
                                },
                            }));
                        }
                        Err(e) => {
                            tracing::warn!("Failed to process PDF {:?}: {}", path, e);
                            // Fall back to treating it as a generic file
                            let filename = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file")
                                .to_string();
                            new_attachments.push(PendingAttachment::File(PendingFile {
                                path: path.clone(),
                                filename,
                                extension: "pdf".to_string(),
                            }));
                        }
                    }
                } else if is_image_file(&path) {
                    // Process image (resize, convert to PNG, generate thumbnail)
                    match process_image_from_path(&path) {
                        Ok(pending_image) => {
                            new_attachments.push(PendingAttachment::Image(pending_image));
                        }
                        Err(e) => {
                            tracing::warn!("Failed to process image {:?}: {}", path, e);
                            // Fall back to treating it as a file
                            let filename = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("file")
                                .to_string();
                            let extension = path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_lowercase();
                            new_attachments.push(PendingAttachment::File(PendingFile {
                                path: path.clone(),
                                filename,
                                extension,
                            }));
                        }
                    }
                } else {
                    // Non-image file
                    let filename = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("file")
                        .to_string();
                    let extension = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    new_attachments.push(PendingAttachment::File(PendingFile {
                        path: path.clone(),
                        filename,
                        extension,
                    }));
                }
            }

            // Update UI with new attachments
            this.update(cx, |app, cx| {
                app.pending_attachments.extend(new_attachments);
                cx.notify();
            })
            .ok();
        })
        .detach();

        cx.notify();
    }

    /// Handle clipboard paste - check for images first, then text
    pub fn handle_clipboard_paste(&mut self, cx: &mut Context<Self>) -> bool {
        use crate::gui::image_processing::process_image_from_bytes;
        use gpui::ClipboardEntry;

        // Check if we have room for more attachments
        if self.pending_attachments.len() >= MAX_ATTACHMENTS {
            self.error_message = Some(format!(
                "Maximum {} attachments reached. Remove some to add more.",
                MAX_ATTACHMENTS
            ));
            cx.notify();
            return false;
        }

        // Try to get image data from clipboard
        if let Some(clipboard_item) = cx.read_from_clipboard() {
            let entries = clipboard_item.entries();
            tracing::info!("Clipboard has {} entries", entries.len());

            // Log and process entries
            for (i, entry) in entries.iter().enumerate() {
                match entry {
                    ClipboardEntry::Image(image) => {
                        tracing::info!("Entry {}: Image ({} bytes)", i, image.bytes().len());

                        let image_bytes = image.bytes().to_vec();

                        // Spawn async task to process the image
                        cx.spawn(async move |this: WeakEntity<ChatApp>, cx: &mut AsyncApp| {
                            match process_image_from_bytes(&image_bytes, None) {
                                Ok(pending_image) => {
                                    this.update(cx, |app, cx| {
                                        app.pending_attachments
                                            .push(PendingAttachment::Image(pending_image));
                                        cx.notify();
                                    })
                                    .ok();
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to process clipboard image: {}", e);
                                    this.update(cx, |app, cx| {
                                        app.error_message =
                                            Some("Failed to process pasted image".to_string());
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            }
                        })
                        .detach();

                        return true; // Handled as image
                    }
                    ClipboardEntry::String(_) => {
                        tracing::info!("Entry {}: String", i);
                    }
                    _ => {
                        tracing::info!("Entry {}: Other type", i);
                    }
                }
            }
        } else {
            tracing::info!("Clipboard is empty or unreadable");
        }

        false // Not an image, Input handles text paste internally
    }

    /// Remove an attachment by index
    fn remove_attachment(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.pending_attachments.len() {
            self.pending_attachments.remove(index);
            cx.notify();
        }
    }

    /// Handle paste action - check for image, fallback to text paste
    fn on_paste_attachment(
        &mut self,
        _: &PasteAttachment,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Try to paste as image attachment
        // Returns true if an image was found and processed
        let was_image = self.handle_clipboard_paste(cx);

        // If not an image, propagate the action so Input can handle text paste
        if !was_image {
            cx.propagate();
        }
    }

    /// Handle new conversation
    fn new_conversation(
        &mut self,
        _: &NewConversation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.conversation.clear();
        self.message_history.clear();
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
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
    fn close_dialog(&mut self, _: &CloseDialog, window: &mut Window, cx: &mut Context<Self>) {
        // Close dialogs in order of precedence (most recent first)
        if self.show_add_model_dialog {
            self.show_add_model_dialog = false;
            self.add_model_selected_provider = None;
            self.add_model_selected_model = None;
            self.add_model_models.clear();
            if let Some(input) = &self.add_model_api_key_input_entity {
                input.update(cx, |state, cx| state.set_value("", window, cx));
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
    fn on_send(&mut self, _: &Send, window: &mut Window, cx: &mut Context<Self>) {
        self.send_message(window, cx);
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
            .on_action(cx.listener(Self::on_paste_attachment))
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
        // App-level shortcuts (macOS: cmd, Linux/Windows: ctrl)
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("cmd-n", NewConversation, None),
        KeyBinding::new("ctrl-n", NewConversation, None),
        KeyBinding::new("cmd-]", NextAgent, None),
        KeyBinding::new("ctrl-]", NextAgent, None),
        KeyBinding::new("cmd-[", PrevAgent, None),
        KeyBinding::new("ctrl-[", PrevAgent, None),
        KeyBinding::new("escape", CloseDialog, None),
        // PasteAttachment checks for image in clipboard; propagates to Input if text
        KeyBinding::new("cmd-v", PasteAttachment, None),
        KeyBinding::new("ctrl-v", PasteAttachment, None),
        // Note: Enter key for Send is handled via InputEvent::PressEnter subscription
        // Note: Text input keybindings (copy, cut, paste, etc.) are handled by gpui-component Input internally
    ]);
}
