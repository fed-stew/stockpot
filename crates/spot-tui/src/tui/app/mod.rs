//! Main TUI application state and logic

mod bus_handler;
mod context;
mod event_handling;
mod folder_modal;
pub mod oauth;
mod settings_keys;
mod throughput;

use std::collections::HashMap;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::Terminal;
use serdes_ai_core::ModelRequest;
use tui_textarea::{CursorMove, TextArea};

use super::activity::{Activity, ActivityConverter, RenderedLine};
use super::attachments::AttachmentManager;
use super::event::{ClipboardManager, EventHandler};
use super::hit_test::HitTestRegistry;
use super::selection::SelectionState;
use super::settings::SettingsState;
use super::state::TuiConversation;
use super::theme::Theme;
use super::ui;
use super::widgets::{self, ActivityFeedState};
use spot_core::agents::{AgentManager, UserMode};
use spot_core::config::Settings;
use spot_core::db::Database;
use spot_core::mcp::McpManager;
use spot_core::messaging::MessageBus;
use spot_core::models::ModelRegistry;
use spot_core::tools::SpotToolRegistry;

/// Main TUI application
pub struct TuiApp {
    /// Terminal instance
    pub(crate) terminal: Terminal<CrosstermBackend<Stdout>>,
    /// Event handler (optional so we can take it out in run loop)
    events: Option<EventHandler>,
    /// Whether the app should quit
    pub(super) should_quit: bool,
    /// Color theme
    pub theme: Theme,
    /// Database connection
    pub db: Arc<Database>,
    /// Agent manager
    pub agents: Arc<AgentManager>,
    /// Model registry
    pub model_registry: Arc<ModelRegistry>,
    /// Tool registry
    pub tool_registry: Arc<SpotToolRegistry>,
    /// MCP manager
    pub mcp_manager: Arc<McpManager>,
    /// Message bus for agent communication
    pub message_bus: MessageBus,
    /// Clipboard manager
    pub clipboard: ClipboardManager,
    /// Current agent name
    pub current_agent: String,
    /// Current model name
    pub current_model: String,
    /// Current user mode (affects agent visibility)
    pub user_mode: UserMode,
    /// Whether we're currently generating
    pub is_generating: bool,
    /// Text input area
    pub input: TextArea<'static>,
    /// Selected text state
    pub selection: SelectionState,
    /// Conversation state
    pub conversation: TuiConversation,
    /// Message list scroll state
    pub message_list_state: widgets::MessageListState,
    /// Show agent dropdown
    pub show_agent_dropdown: bool,
    /// Show model dropdown
    pub show_model_dropdown: bool,

    /// Hit test registry for mouse interaction
    pub hit_registry: HitTestRegistry,
    /// Last known mouse position
    pub last_mouse_pos: Option<(u16, u16)>,

    /// Active agent stack (for nested agents)
    pub active_agent_stack: Vec<String>,
    /// Active section IDs for nested agents
    pub active_section_ids: HashMap<String, String>,
    /// Request history for context
    pub message_history: Vec<ModelRequest>,

    /// File attachments
    pub attachments: AttachmentManager,
    /// Context tokens used
    pub context_tokens_used: usize,
    /// Context window size
    pub context_window_size: usize,
    /// Show help overlay
    pub show_help: bool,
    /// Whether settings panel is visible
    pub show_settings: bool,
    /// Settings panel state
    pub settings_state: SettingsState,

    // ─────────────────────────────────────────────────────────────────────────
    // Folder modal state
    // ─────────────────────────────────────────────────────────────────────────
    /// Whether folder modal is visible
    pub show_folder_modal: bool,
    /// Current working directory
    pub current_working_dir: std::path::PathBuf,
    /// Directory entries in folder modal
    pub folder_modal_entries: Vec<std::path::PathBuf>,
    /// Selected index in folder modal
    pub folder_modal_selected: usize,
    /// Scroll offset for folder modal
    pub folder_modal_scroll: usize,

    // ─────────────────────────────────────────────────────────────────────────
    // OAuth dialog state
    // ─────────────────────────────────────────────────────────────────────────
    /// Whether OAuth dialog is visible
    pub show_oauth_dialog: bool,
    /// OAuth provider being authenticated
    pub oauth_dialog_provider: Option<String>,
    /// OAuth auth URL to display
    pub oauth_dialog_url: Option<String>,
    /// OAuth callback port
    pub oauth_dialog_port: Option<u16>,
    /// OAuth status message
    pub oauth_dialog_status: String,

    /// Throughput samples (count, time)
    pub throughput_samples: Vec<(usize, Instant)>,
    /// Current throughput (chars/sec)
    pub current_throughput_cps: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Activity feed state
    // ─────────────────────────────────────────────────────────────────────────
    /// Activities for the feed
    pub activities: Vec<Activity>,
    /// Activity feed scroll state
    pub activity_state: ActivityFeedState,
    /// Activity converter for tool→activity mapping
    pub activity_converter: ActivityConverter,
    /// Rendered lines cache for copy support
    pub rendered_lines: Vec<RenderedLine>,
    /// Auto-scroll direction during selection (-1 up, 1 down, None = off)
    pub auto_scroll_direction: Option<i32>,
    /// Last auto-scroll tick
    pub last_auto_scroll: Instant,
    /// Cached activity area for mouse calculations
    pub cached_activity_area: Rect,
    /// Copy feedback message with expiry timestamp
    pub copy_feedback: Option<(Instant, String)>,
    /// Stream start time for elapsed calculation
    pub stream_start: Option<Instant>,
    /// Error message to display (e.g., folder change failures)
    pub error_message: Option<String>,
    /// OAuth completion receiver - receives (provider, Ok(()) | Err(msg)) when OAuth finishes
    pub oauth_completion_rx: tokio::sync::mpsc::UnboundedReceiver<(String, Result<(), String>)>,
    /// OAuth completion sender - cloned and passed to OAuth tasks
    pub(super) oauth_completion_tx:
        tokio::sync::mpsc::UnboundedSender<(String, Result<(), String>)>,
    /// OAuth dialog info receiver - receives (provider, url, port) to show dialog
    pub oauth_dialog_rx: tokio::sync::mpsc::UnboundedReceiver<(String, String, u16)>,
    /// OAuth dialog info sender - cloned and passed to OAuth tasks
    pub(super) oauth_dialog_tx: tokio::sync::mpsc::UnboundedSender<(String, String, u16)>,
}

impl TuiApp {
    /// Create a new TUI application
    pub async fn new() -> Result<Self> {
        // Initialize terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        // Try to enable keyboard enhancement to detect Shift+Enter and other modifier combos
        // This requires terminal support for the kitty keyboard protocol
        // If not supported, we fall back to Alt+Enter for newlines
        if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) {
            let _ = execute!(
                stdout,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            );
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Initialize database
        #[allow(clippy::arc_with_non_send_sync)]
        let db = Arc::new(Database::open()?);
        db.migrate()?;

        // Load settings
        let settings = Settings::new(&db);
        let current_model = settings.model();
        let user_mode = settings.user_mode();

        // Initialize components
        let model_registry = Arc::new(ModelRegistry::load_from_db(&db).unwrap_or_default());
        let agents = Arc::new(AgentManager::new());
        let current_agent = agents.current_name();
        let tool_registry = Arc::new(SpotToolRegistry::new());
        let mcp_manager = Arc::new(McpManager::new());
        let message_bus = MessageBus::new();

        // OAuth completion channel
        let (oauth_completion_tx, oauth_completion_rx) = tokio::sync::mpsc::unbounded_channel();
        let (oauth_dialog_tx, oauth_dialog_rx) = tokio::sync::mpsc::unbounded_channel();

        // Event handler with ~60 FPS tick rate
        let events = EventHandler::new(Duration::from_millis(16));

        let input = Self::build_input();

        Ok(Self {
            terminal,
            events: Some(events),
            should_quit: false,
            theme: Theme::dark(),
            db,
            agents,
            model_registry,
            tool_registry,
            mcp_manager,
            message_bus,
            clipboard: ClipboardManager::new(),
            current_agent,
            current_model,
            user_mode,
            is_generating: false,
            input,
            selection: SelectionState::default(),
            conversation: TuiConversation::new(),
            message_list_state: widgets::MessageListState::default(),
            show_agent_dropdown: false,
            show_model_dropdown: false,
            hit_registry: HitTestRegistry::new(),
            last_mouse_pos: None,
            active_agent_stack: Vec::new(),
            active_section_ids: HashMap::new(),
            message_history: Vec::new(),
            attachments: AttachmentManager::default(),
            context_tokens_used: 0,
            context_window_size: 128000, // Default for GPT-4o
            show_help: false,
            show_settings: false,
            settings_state: SettingsState::default(),
            show_folder_modal: false,
            current_working_dir: std::env::current_dir().unwrap_or_default(),
            folder_modal_entries: Vec::new(),
            folder_modal_selected: 0,
            folder_modal_scroll: 0,
            // OAuth dialog
            show_oauth_dialog: false,
            oauth_dialog_provider: None,
            oauth_dialog_url: None,
            oauth_dialog_port: None,
            oauth_dialog_status: String::new(),
            throughput_samples: Vec::new(),
            current_throughput_cps: 0.0,
            // Activity feed state
            activities: Vec::new(),
            activity_state: ActivityFeedState::default(),
            activity_converter: ActivityConverter::new(),
            rendered_lines: Vec::new(),
            auto_scroll_direction: None,
            last_auto_scroll: Instant::now(),
            cached_activity_area: Rect::default(),
            copy_feedback: None,
            stream_start: None,
            error_message: None,
            oauth_completion_rx,
            oauth_completion_tx,
            oauth_dialog_rx,
            oauth_dialog_tx,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Activity scroll helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Scroll activity feed to bottom
    pub fn activity_scroll_to_bottom(&mut self) {
        self.activity_state.scroll_to_bottom();
    }

    /// Scroll activity feed up by lines
    pub fn activity_scroll_up(&mut self, lines: usize) {
        self.activity_state.scroll_up(lines);
    }

    /// Scroll activity feed down by lines
    pub fn activity_scroll_down(&mut self, lines: usize) {
        self.activity_state.scroll_down(lines);
    }

    /// Get selected text from rendered activity lines
    pub fn get_selected_text_from_activities(&self) -> Option<String> {
        if !self.selection.is_active() {
            return None;
        }

        let ((start_line, start_col), (end_line, end_col)) = self.selection.normalized()?;

        // Selection coordinates are already in content/absolute coordinates
        // (scroll offset is added when selection starts/updates in mouse handlers)
        // So we use start_line and end_line directly without adding scroll again!

        let mut result = String::new();

        for (i, line) in self.rendered_lines.iter().enumerate() {
            if i < start_line || i > end_line {
                continue;
            }

            let text = &line.copyable_text;
            let chars: Vec<char> = text.chars().collect();

            if i == start_line && i == end_line {
                // Single line selection
                let s = start_col.min(chars.len());
                let e = (end_col + 1).min(chars.len());
                result.push_str(&chars[s..e].iter().collect::<String>());
            } else if i == start_line {
                // First line
                let s = start_col.min(chars.len());
                result.push_str(&chars[s..].iter().collect::<String>());
                result.push('\n');
            } else if i == end_line {
                // Last line
                let e = (end_col + 1).min(chars.len());
                result.push_str(&chars[..e].iter().collect::<String>());
            } else {
                // Middle lines - take whole line
                result.push_str(text);
                result.push('\n');
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Prepare app state before rendering.
    /// Called before terminal.draw() to avoid mutation during render.
    pub fn prepare_for_render(&mut self) {
        // Input styling is now minimal (no border) - see build_input()
        // The "› " prompt is rendered directly in ui.rs render_input()
    }

    /// Run the main event loop
    pub async fn run(&mut self) -> Result<()> {
        // Start MCP servers in background
        let mcp = self.mcp_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = mcp.start_all().await {
                tracing::error!("Failed to start MCP servers: {}", e);
            }
        });

        // Take events out of self to avoid borrow conflicts in select! loop
        let mut events = self.events.take().expect("Events not initialized");
        let mut bus_receiver = self.message_bus.subscribe();

        // Auto-scroll constants
        let auto_scroll_interval = Duration::from_millis(50);
        let auto_scroll_lines = 1;

        // Frame rate limiting: determine interval based on VDI mode.
        // VDI default = 66ms (~15fps), normal = 16ms (~60fps).
        let frame_interval = {
            let settings = Settings::new(&self.db);
            if spot_core::config::is_vdi_mode_active(&settings) {
                let ms = settings.get_vdi_frame_interval_ms();
                tracing::info!(
                    frame_interval_ms = ms,
                    "VDI mode active - using reduced frame rate"
                );
                Duration::from_millis(ms)
            } else {
                Duration::from_millis(16)
            }
        };
        let mut last_render = Instant::now();
        let mut needs_render = true; // Render once on startup

        while !self.should_quit {
            // Process auto-scroll if dragging selection outside visible area
            if self.selection.is_dragging()
                && self.auto_scroll_direction.is_some()
                && self.last_auto_scroll.elapsed() >= auto_scroll_interval
            {
                self.last_auto_scroll = Instant::now();
                match self.auto_scroll_direction {
                    Some(-1) => {
                        // Scroll up
                        self.activity_scroll_up(auto_scroll_lines);
                        // Update selection to stay with scroll
                        if let Some((_, col)) = self.selection.end() {
                            let new_line = self.activity_state.scroll_offset;
                            self.selection.update_to(new_line, col);
                        }
                        needs_render = true;
                    }
                    Some(1) => {
                        // Scroll down
                        self.activity_scroll_down(auto_scroll_lines);
                        if let Some((_, col)) = self.selection.end() {
                            let new_line = self.activity_state.scroll_offset
                                + self.activity_state.viewport_height.saturating_sub(1);
                            let max_line = self.rendered_lines.len().saturating_sub(1);
                            self.selection.update_to(new_line.min(max_line), col);
                        }
                        needs_render = true;
                    }
                    _ => {}
                }
            }

            // Only render when state changed AND enough time has passed (frame rate limiting)
            let since_last_render = last_render.elapsed();
            if needs_render && since_last_render >= frame_interval {
                self.prepare_for_render();
                let app_ptr: *mut TuiApp = self;
                self.terminal
                    .draw(|frame| unsafe { ui::render(frame, &mut *app_ptr) })?;
                last_render = Instant::now();
                needs_render = false;
            }

            // Check for OAuth dialog info (non-blocking)
            while let Ok((provider, url, port)) = self.oauth_dialog_rx.try_recv() {
                self.show_oauth_dialog = true;
                self.oauth_dialog_provider = Some(provider);
                self.oauth_dialog_url = Some(url);
                self.oauth_dialog_port = Some(port);
                self.oauth_dialog_status = "Waiting for authentication...".to_string();
                needs_render = true;
            }

            // Check for OAuth completion (non-blocking)
            while let Ok((provider, result)) = self.oauth_completion_rx.try_recv() {
                // Close dialog
                if self.oauth_dialog_provider.as_deref() == Some(&provider) {
                    self.show_oauth_dialog = false;
                    self.oauth_dialog_provider = None;
                    self.oauth_dialog_url = None;
                    self.oauth_dialog_port = None;
                }
                // Clear the in-progress state
                if self.settings_state.oauth_in_progress.as_deref() == Some(&provider) {
                    self.settings_state.oauth_in_progress = None;
                }
                // Refresh model registry on success
                if result.is_ok() {
                    self.refresh_model_registry();
                }
                needs_render = true;
            }

            // Calculate how long to wait: either until next frame or indefinitely if idle
            let wait_duration = if needs_render {
                // State is dirty - wait only until next frame is due
                frame_interval.saturating_sub(last_render.elapsed())
            } else {
                // Nothing to render - wait up to the frame interval for new events.
                // This keeps the loop responsive while not busy-spinning.
                frame_interval
            };

            // Handle events and messages with a timeout so we render on schedule
            tokio::select! {
                biased;  // Prefer bus messages over UI events

                Ok(msg) = bus_receiver.recv() => {
                    self.handle_bus_message(msg);
                    // Drain additional pending bus messages to batch updates.
                    // This prevents render-per-message during heavy streaming.
                    while let Ok(Some(msg)) = bus_receiver.try_recv() {
                        self.handle_bus_message(msg);
                    }
                    needs_render = true;
                }
                maybe_event = events.next() => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await?;
                        needs_render = true;
                    }
                }
                _ = tokio::time::sleep(wait_duration) => {
                    // Timer expired - loop back to render if needed
                }
            }
        }

        // Put events back (though we're dropping anyway)
        self.events = Some(events);

        Ok(())
    }

    fn get_selected_text(&mut self) -> Option<String> {
        if let Some((start, end)) = self.selection.get_selection() {
            let buffer = self.terminal.current_buffer_mut();
            let mut selected_text = String::new();

            for row in start.row..=end.row {
                let row_width = buffer.area.width;
                let start_col = if row == start.row { start.col } else { 0 };
                let end_col = if row == end.row {
                    end.col
                } else {
                    row_width - 1
                };

                for col in start_col..=end_col {
                    if col < row_width && row < buffer.area.height {
                        #[allow(deprecated)]
                        let cell = buffer.get(col, row);
                        selected_text.push_str(cell.symbol());
                    }
                }

                if row != end.row {
                    selected_text.push('\n');
                }
            }

            if !selected_text.is_empty() {
                return Some(selected_text);
            }
        }
        None
    }

    pub(crate) fn build_input() -> TextArea<'static> {
        // Create text input with minimal styling (no border)
        let mut input = TextArea::default();
        input.set_cursor_line_style(Style::default());
        input.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        input.set_placeholder_text("Type a message...");
        input.set_placeholder_style(Style::default().fg(Theme::MUTED));
        input.set_style(Style::default().fg(Theme::TEXT).bg(Theme::INPUT_BG));
        // No block/border - we render a simple "› " prompt in ui.rs instead
        input.move_cursor(CursorMove::End);
        input
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // Restore terminal - order matters!
        // Pop keyboard enhancement flags first (reverse order of setup)
        let _ = execute!(self.terminal.backend_mut(), PopKeyboardEnhancementFlags);
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}
