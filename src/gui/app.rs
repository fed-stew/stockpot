//! Main application state and rendering

use std::collections::{HashMap, VecDeque};
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

mod actions;
mod agent_dropdown;
mod attachments;
mod error;
mod execution;
mod input;
mod messages;
mod metrics;
mod model_dropdown;
mod model_management;
mod settings;
mod streaming;
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
    /// Estimated tokens currently used in context
    context_tokens_used: usize,
    /// Current model's context window size
    context_window_size: usize,
    /// Last time context usage was updated (for throttling)
    last_context_update: std::time::Instant,
    /// Rolling window of throughput samples: (chars_in_sample, timestamp)
    throughput_samples: Vec<(usize, std::time::Instant)>,
    /// Current calculated throughput in chars/sec (for display)
    current_throughput_cps: f64,
    /// Whether we're actively receiving streaming data
    is_streaming_active: bool,
    /// Historical throughput values for chart display (last 8 values, sampled every 250ms)
    throughput_history: VecDeque<f64>,
    /// Last time we triggered a UI render (for throttling during streaming)
    last_render_notify: std::time::Instant,
    /// Last time we added a history sample
    last_history_sample: std::time::Instant,
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
    /// Whether user has manually scrolled away from bottom (disables auto-scroll)
    user_scrolled_away: bool,

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

    /// MCP settings: selected agent for MCP attachment
    mcp_settings_selected_agent: String,
    /// MCP settings: show import JSON dialog
    show_mcp_import_dialog: bool,
    /// MCP settings: pasted JSON content for import
    mcp_import_json: String,
    /// MCP settings: import error message
    mcp_import_error: Option<String>,

    /// Stack of active agent names (main agent at bottom, sub-agents pushed on top)
    /// Empty means no agent running
    active_agent_stack: Vec<String>,

    /// Map from agent_name to section_id for currently active nested sections
    active_section_ids: HashMap<String, String>,
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
        let mcp_settings_selected_agent = current_agent.clone();
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
            context_tokens_used: 0,
            context_window_size: 0,
            last_context_update: std::time::Instant::now(),
            throughput_samples: Vec::new(),
            current_throughput_cps: 0.0,
            is_streaming_active: false,
            throughput_history: VecDeque::new(),
            last_render_notify: std::time::Instant::now(),
            last_history_sample: std::time::Instant::now(),
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
            user_scrolled_away: false,

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

            mcp_settings_selected_agent,
            show_mcp_import_dialog: false,
            mcp_import_json: String::new(),
            mcp_import_error: None,

            active_agent_stack: Vec::new(),
            active_section_ids: HashMap::new(),
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
                let enabled_count = mcp.config().enabled_servers().count();
                if enabled_count == 0 {
                    tracing::debug!("No MCP servers enabled, skipping startup");
                    return;
                }
                tracing::info!(count = enabled_count, "Starting MCP servers...");
                if let Err(e) = mcp.start_all().await {
                    tracing::error!(error = %e, "Failed to start MCP servers");
                } else {
                    let running = mcp.running_servers().await;
                    tracing::info!(servers = ?running, "MCP servers started successfully");
                }
            },
        )
        .detach();
    }

    /// Import MCP servers from JSON (Claude Desktop format).
    pub(super) fn do_mcp_import(&mut self, cx: &mut Context<Self>) {
        use crate::mcp::{McpConfig, McpServerEntry};

        let json_str = self.mcp_import_json.trim();
        if json_str.is_empty() {
            self.mcp_import_error = Some("No JSON to import".to_string());
            cx.notify();
            return;
        }

        // Try to parse the JSON - support both formats:
        // 1. { "mcpServers": { ... } }  (Claude Desktop format)
        // 2. { "servers": { ... } }     (our native format)
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(json_str);
        let parsed = match parsed {
            Ok(v) => v,
            Err(e) => {
                self.mcp_import_error = Some(format!("Invalid JSON: {}", e));
                cx.notify();
                return;
            }
        };

        // Find the servers object
        let servers_obj = parsed
            .get("mcpServers")
            .or_else(|| parsed.get("servers"))
            .and_then(|v| v.as_object());

        let servers_obj = match servers_obj {
            Some(obj) => obj,
            None => {
                self.mcp_import_error =
                    Some("JSON must contain 'mcpServers' or 'servers' object".to_string());
                cx.notify();
                return;
            }
        };

        // Load existing config and merge
        let mut config = McpConfig::load_or_default();
        let mut imported_count = 0;

        for (name, server_value) in servers_obj {
            // Parse each server entry
            let command = server_value
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if command.is_empty() {
                continue; // Skip entries without command
            }

            let args: Vec<String> = server_value
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            let env: std::collections::HashMap<String, String> = server_value
                .get("env")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();

            let description = server_value
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut entry = McpServerEntry::new(command).with_args(args);
            for (k, v) in env {
                entry = entry.with_env(k, v);
            }
            if let Some(desc) = description {
                entry = entry.with_description(desc);
            }

            config.add_server(name.clone(), entry);
            imported_count += 1;
        }

        if imported_count == 0 {
            self.mcp_import_error = Some("No valid MCP servers found in JSON".to_string());
            cx.notify();
            return;
        }

        // Save the config
        if let Err(e) = config.save_default() {
            self.mcp_import_error = Some(format!("Failed to save: {}", e));
            cx.notify();
            return;
        }

        // Success!
        self.show_mcp_import_dialog = false;
        self.mcp_import_json.clear();
        self.mcp_import_error = None;
        tracing::info!("Imported {} MCP servers", imported_count);
        cx.notify();
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
            .map_err(|e| tracing::error!("this.update() failed: {:?}", e))
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
            .child(self.render_api_keys_dialog(cx))
            .child(self.render_mcp_import_dialog(cx))
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
