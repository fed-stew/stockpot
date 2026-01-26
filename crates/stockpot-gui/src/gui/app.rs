//! Main application state and rendering

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    actions, div, prelude::*, px, App, AsyncApp, Context, Entity, ExternalPaths, FocusHandle,
    Focusable, KeyBinding, ListAlignment, ListState, ScrollHandle, Styled, WeakEntity, Window,
};
use gpui_component::input::{InputEvent, InputState};

use super::components::{ListScrollbarDragState, ScrollbarDragState};
use super::state::Conversation;
use super::theme::Theme;
use stockpot_core::agents::{AgentManager, UserMode};
use stockpot_core::config::{PdfMode, Settings};
use stockpot_core::db::Database;
use stockpot_core::mcp::McpManager;
use stockpot_core::messaging::MessageBus;
use stockpot_core::models::ModelRegistry;
use stockpot_core::tools::{set_global_context, SpotToolRegistry, ToolContext};

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
mod scroll_animation;
mod settings;
mod streaming;
mod system_executions;
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
    /// Whether to show agent reasoning in the UI
    show_reasoning: bool,
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
    /// Rolling window of throughput samples: (chars_in_sample, timestamp)
    throughput_samples: Vec<(usize, std::time::Instant)>,
    /// Current calculated throughput in chars/sec (for display)
    current_throughput_cps: f64,
    /// Whether we're actively receiving streaming data
    is_streaming_active: bool,
    /// Historical throughput values for chart display (last 8 values, sampled every 250ms)
    throughput_history: VecDeque<f64>,

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
    /// Which model is currently expanded in settings (if any)
    expanded_settings_model: Option<String>,
    /// Input state for editing model temperature
    model_temp_input_entity: Option<Entity<InputState>>,
    /// Input state for editing model top_p
    model_top_p_input_entity: Option<Entity<InputState>>,
    /// Input state for editing model API key
    model_api_key_input_entity: Option<Entity<InputState>>,
    /// Timestamp of last successful model settings save (for visual feedback)
    model_settings_save_success: Option<std::time::Instant>,
    /// Error message to display
    error_message: Option<String>,
    /// Compression notification (shown temporarily when context is compressed)
    pub compression_notification: Option<(std::time::Instant, String)>,

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
    // NOTE: messages_scroll_handle and messages_scrollbar_drag were removed.
    // The list() component uses ListState for scrolling, which is incompatible with ScrollHandle.
    /// List state for virtualized message rendering
    messages_list_state: ListState,
    /// Whether user has manually scrolled away from bottom (disables auto-scroll)
    user_scrolled_away: bool,
    /// Drag state for messages list scrollbar
    messages_list_scrollbar_drag: Rc<ListScrollbarDragState>,

    /// Add model dialog state
    show_add_model_dialog: bool,
    add_model_providers: Vec<stockpot_core::models::catalog::ProviderInfo>,
    add_model_selected_provider: Option<String>,
    add_model_models: Vec<stockpot_core::models::catalog::ModelInfo>,
    add_model_selected_model: Option<String>,
    /// Text input for API key in add model dialog
    add_model_api_key_input_entity: Option<Entity<InputState>>,
    add_model_loading: bool,
    add_model_error: Option<String>,
    /// Filter input for provider list in add model dialog
    add_model_provider_filter_input: Option<Entity<InputState>>,

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

    // ── Key Pool Dialog State ──────────────────────────────────────────────────
    /// Whether the key pool management dialog is open
    show_key_pool_dialog: bool,
    /// The provider being managed (e.g., "OPENAI_API_KEY")
    key_pool_provider: Option<String>,
    /// The provider's display name for the dialog title
    key_pool_provider_display: Option<String>,
    /// List of keys for the current provider
    key_pool_keys: Vec<stockpot_core::db::PoolKey>,
    /// Input for new key value
    key_pool_new_key_input: Option<Entity<InputState>>,
    /// Input for new key label
    key_pool_new_label_input: Option<Entity<InputState>>,

    /// System Executions sidebar state
    system_executions: system_executions::SystemExecutionsState,

    /// Stack of active agent names (main agent at bottom, sub-agents pushed on top)
    /// Empty means no agent running
    active_agent_stack: Vec<String>,

    /// Map from agent_name to section_id for currently active nested sections
    active_section_ids: HashMap<String, String>,
    /// Cache of TextViewState entities keyed by element id
    text_view_cache:
        RefCell<HashMap<String, Entity<crate::gui::components::StreamingMarkdownView>>>,

    /// Pending text updates to be applied in the next animation tick
    /// Key: element_id, Value: pending delta string
    pending_cache_updates: HashMap<String, String>,
    /// Last time we flushed text updates (for throttling re-renders)
    last_text_flush: std::time::Instant,

    // ── Smooth scroll animation state ──────────────────────────────────────────
    /// Target scroll offset for smooth animation (None = not animating)
    /// Uses lerp-based "chase" interpolation - the actual target (bottom) is computed per tick
    scroll_animation_target: Option<gpui::Point<gpui::Pixels>>,
    /// Last tick time for delta-time based scroll animation
    last_animation_tick: std::time::Instant,
    /// Current scroll velocity for momentum-based smoothing (pixels per second)
    current_scroll_velocity: f32,
    /// Last scroll offset Y for detecting user scroll during streaming
    last_scroll_offset_y: gpui::Pixels,
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
        let show_reasoning = settings.get_bool("show_reasoning").unwrap_or(false);

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

        let mut app = Self {
            focus_handle,
            input_state,
            conversation: Conversation::new(),
            current_agent,
            current_model,
            user_mode,
            pdf_mode,
            show_reasoning,
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
            throughput_samples: Vec::new(),
            current_throughput_cps: 0.0,
            is_streaming_active: false,
            throughput_history: VecDeque::new(),
            last_history_sample: std::time::Instant::now(),
            available_agents,
            available_models,
            show_settings: false,
            settings_tab: settings::SettingsTab::General,
            settings_selected_agent,
            show_agent_dropdown: false,
            agent_dropdown_bounds: None,
            show_model_dropdown: false,
            model_dropdown_bounds: None,
            show_default_model_dropdown: false,
            default_model_dropdown_bounds: None,
            expanded_settings_model: None,
            model_temp_input_entity: None,
            model_top_p_input_entity: None,
            model_api_key_input_entity: None,
            model_settings_save_success: None,
            error_message: None,
            compression_notification: None,

            settings_scroll_handle: ScrollHandle::new(),
            settings_scrollbar_drag: Rc::new(ScrollbarDragState::default()),
            add_model_providers_scroll_handle: ScrollHandle::new(),
            add_model_providers_scrollbar_drag: Rc::new(ScrollbarDragState::default()),
            add_model_models_scroll_handle: ScrollHandle::new(),
            add_model_models_scrollbar_drag: Rc::new(ScrollbarDragState::default()),
            // NOTE: messages_scroll_handle and messages_scrollbar_drag removed (ListState handles scrolling)
            // Initialize with 0 items, TOP alignment (we handle scroll-to-bottom manually with smooth animation),
            // 800px overdraw for smooth scrolling (increased from 200px to reduce markdown re-parsing
            // when scrolling into new territory). Using Top instead of Bottom prevents GPUI from
            // auto-snapping to bottom on every render, allowing our lerp animation to work.
            messages_list_state: ListState::new(0, ListAlignment::Top, px(800.0)),
            user_scrolled_away: false,
            messages_list_scrollbar_drag: Rc::new(ListScrollbarDragState::default()),

            show_add_model_dialog: false,
            add_model_providers: Vec::new(),
            add_model_selected_provider: None,
            add_model_models: Vec::new(),
            add_model_selected_model: None,
            add_model_api_key_input_entity: None,
            add_model_loading: false,
            add_model_error: None,
            add_model_provider_filter_input: None,

            show_api_keys_dialog: false,
            api_keys_list: Vec::new(),
            api_key_new_name: String::new(),
            api_key_new_value: String::new(),
            pending_attachments: Vec::new(),

            mcp_settings_selected_agent,
            show_mcp_import_dialog: false,
            mcp_import_json: String::new(),
            mcp_import_error: None,

            // Key pool dialog state
            show_key_pool_dialog: false,
            key_pool_provider: None,
            key_pool_provider_display: None,
            key_pool_keys: Vec::new(),
            key_pool_new_key_input: None,
            key_pool_new_label_input: None,

            system_executions: system_executions::SystemExecutionsState::new(),

            active_agent_stack: Vec::new(),
            active_section_ids: HashMap::new(),
            text_view_cache: RefCell::new(HashMap::new()),
            pending_cache_updates: HashMap::new(),
            last_text_flush: std::time::Instant::now(),

            // Smooth scroll animation (None = not animating)
            scroll_animation_target: None,
            last_animation_tick: std::time::Instant::now(),
            current_scroll_velocity: 0.0,
            last_scroll_offset_y: px(0.0),
        };

        // Start the unified UI event loop (handles both messages and animation ticks)
        // This is a single loop to avoid race conditions from multiple async tasks
        app.start_ui_event_loop(cx);

        // Start MCP servers in background
        app.start_mcp_servers(cx);

        // Initialize tool context for terminal integration
        app.init_tool_context(cx);

        // Set up keyboard focus on the main app
        window.focus(&app.focus_handle, cx);

        app
    }

    /// Initialize the global tool context for terminal integration.
    fn init_tool_context(&mut self, cx: &mut Context<Self>) {
        let store = self.system_executions.store.clone();
        let request_tx = self.system_executions.request_tx.clone();

        // Create and set global tool context
        let tool_ctx = ToolContext::new(store, request_tx);
        if let Err(_existing) = set_global_context(tool_ctx) {
            tracing::warn!("Tool context already initialized");
        }

        // Start request handler loop
        if let Some(mut request_rx) = self.system_executions.take_request_rx() {
            cx.spawn(async move |this, cx| {
                while let Some(request) = request_rx.recv().await {
                    let _ = this.update(cx, |app, cx| {
                        app.handle_exec_request(request, cx);
                    });
                }
            })
            .detach();
        }
    }

    /// Render the main content area (messages + input + optional sidebar)
    fn render_main_content_area(&self, cx: &Context<Self>) -> impl IntoElement {
        let sidebar_visible = self.system_executions.visible;
        let sidebar_width = self.system_executions.sidebar_width;
        let is_resizing = self.system_executions.is_resizing;

        div()
            .flex()
            .flex_row()
            .flex_1()
            .min_h(px(0.))
            .overflow_hidden()
            .relative()
            // Main content (messages + input)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(0.))
                    .overflow_hidden()
                    .child(self.render_messages(cx))
                    .child(self.render_input(cx)),
            )
            // Sidebar (when visible)
            .when(sidebar_visible, |d| {
                d.child(self.render_system_executions_panel(cx, sidebar_width))
            })
            // Sidebar resize overlay - captures mouse events during resize drag
            .when(is_resizing, |d| {
                d.child(
                    div()
                        .id("resize-overlay")
                        .absolute()
                        .inset_0()
                        .cursor(gpui::CursorStyle::ResizeLeftRight)
                        .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                            if this.system_executions.is_resizing {
                                if let (Some(start_x), Some(start_width)) = (
                                    this.system_executions.resize_start_x,
                                    this.system_executions.resize_start_width,
                                ) {
                                    let current_x: f32 = event.position.x.into();
                                    let delta = start_x - current_x;
                                    let new_width = (start_width + delta).clamp(200.0, 600.0);
                                    this.system_executions.sidebar_width = new_width;
                                    cx.notify();
                                }
                            }
                        }))
                        .on_mouse_up(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.system_executions.is_resizing = false;
                                this.system_executions.resize_start_x = None;
                                this.system_executions.resize_start_width = None;
                                cx.notify();
                            }),
                        ),
                )
            })
            // Terminal height resize overlay
            .when(self.system_executions.resizing_terminal.is_some(), |d| {
                d.child(
                    div()
                        .id("terminal-resize-overlay")
                        .absolute()
                        .inset_0()
                        .cursor(gpui::CursorStyle::ResizeUpDown)
                        .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                            if let Some(ref process_id) =
                                this.system_executions.resizing_terminal.clone()
                            {
                                if let (Some(start_y), Some(start_height)) = (
                                    this.system_executions.resize_start_y,
                                    this.system_executions.resize_start_height,
                                ) {
                                    let current_y: f32 = event.position.y.into();
                                    let delta = current_y - start_y;
                                    let new_height = (start_height + delta).clamp(100.0, 600.0);
                                    this.system_executions
                                        .terminal_heights
                                        .insert(process_id.clone(), new_height);
                                    cx.notify();
                                }
                            }
                        }))
                        .on_mouse_up(
                            gpui::MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.system_executions.resizing_terminal = None;
                                this.system_executions.resize_start_y = None;
                                this.system_executions.resize_start_height = None;
                                cx.notify();
                            }),
                        ),
                )
            })
    }

    /// Update the list state when messages change
    pub(super) fn sync_messages_list_state(&mut self) {
        let new_count = self.conversation.messages.len();

        // If the user has scrolled away from the bottom, keep their scroll position stable when
        // messages are added. We use `bounds_for_item` (guarded) to detect whether the last item is
        // currently visible within the viewport.
        let prev_offset = self.messages_list_state.scroll_px_offset_for_scrollbar();
        let viewport = self.messages_list_state.viewport_bounds();
        let viewport_ready = viewport.size.height > gpui::Pixels::ZERO;

        let last_item_bounds = new_count.checked_sub(1).and_then(|idx| {
            self.messages_list_state.bounds_for_item(idx).or_else(|| {
                idx.checked_sub(1)
                    .and_then(|idx| self.messages_list_state.bounds_for_item(idx))
            })
        });

        let at_bottom = if !viewport_ready {
            true
        } else if let Some(bounds) = last_item_bounds {
            let item_bottom = bounds.origin.y + bounds.size.height;
            let viewport_bottom = viewport.origin.y + viewport.size.height;
            item_bottom <= viewport_bottom + px(2.0)
        } else {
            false
        };

        self.user_scrolled_away = !at_bottom;

        self.messages_list_state.reset(new_count);

        if self.user_scrolled_away {
            // User has scrolled up - preserve their position
            let max_offset = self.messages_list_state.max_offset_for_scrollbar();
            let clamped_y = prev_offset.y.clamp(-max_offset.height, gpui::Pixels::ZERO);
            self.messages_list_state
                .set_offset_from_scrollbar(gpui::point(prev_offset.x, clamped_y));
        } else if self.is_generating {
            // At bottom during streaming - immediately set to bottom, then animate
            // This prevents visual jump to top when reset() is called
            let max_offset = self.messages_list_state.max_offset_for_scrollbar();
            self.messages_list_state
                .set_offset_from_scrollbar(gpui::point(prev_offset.x, -max_offset.height));
            self.start_smooth_scroll_to_bottom();
        } else {
            // At bottom, not generating (streaming just ended or static view)
            // Restore scroll to bottom since reset() moved us to top
            let max_offset = self.messages_list_state.max_offset_for_scrollbar();
            self.messages_list_state
                .set_offset_from_scrollbar(gpui::point(prev_offset.x, -max_offset.height));
        }
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
        use stockpot_core::mcp::{McpConfig, McpServerEntry};

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
            // Main content area with optional sidebar
            .child(self.render_main_content_area(cx))
            // Overlay dialogs
            .child(self.render_settings(cx))
            .child(self.render_approval_dialog(cx))
            .child(self.render_add_model_dialog(cx))
            .child(self.render_api_keys_dialog(cx))
            .child(self.render_key_pool_dialog(cx))
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
