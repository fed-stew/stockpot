//! Main TUI application state and logic

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
use tui_textarea::{CursorMove, Input, TextArea};

use super::activity::{Activity, ActivityConverter, RenderedLine};
use super::attachments::AttachmentManager;
use super::event::{AppEvent, ClipboardManager, EventHandler};
use super::execution::execute_agent;
use super::hit_test::{ClickTarget, HitTestRegistry};
use super::selection::{Position, SelectionState};
use super::settings::SettingsState;
use super::state::TuiConversation;
use super::theme::Theme;
use super::ui;
use super::widgets::{self, ActivityFeedState};
use crate::agents::AgentManager;
use crate::config::Settings;
use crate::db::Database;
use crate::mcp::McpManager;
use crate::messaging::{AgentEvent, Message, MessageBus, ToolStatus};
use crate::models::ModelRegistry;
use crate::tools::SpotToolRegistry;

/// Main TUI application
pub struct TuiApp {
    /// Terminal instance
    terminal: Terminal<CrosstermBackend<Stdout>>,
    /// Event handler (optional so we can take it out in run loop)
    events: Option<EventHandler>,
    /// Whether the app should quit
    should_quit: bool,
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

    /// Throughput samples (count, time)
    pub throughput_samples: Vec<(usize, Instant)>,
    /// Current throughput (chars/sec)
    pub current_throughput_cps: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Activity feed state (rustpuppy-style)
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

        // Initialize components
        let model_registry = Arc::new(ModelRegistry::load_from_db(&db).unwrap_or_default());
        let agents = Arc::new(AgentManager::new());
        let current_agent = agents.current_name();
        let tool_registry = Arc::new(SpotToolRegistry::new());
        let mcp_manager = Arc::new(McpManager::new());
        let message_bus = MessageBus::new();

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
        })
    }

    pub fn update_throughput(&mut self, chars: usize) {
        let now = Instant::now();
        self.throughput_samples.push((chars, now));

        // Remove samples older than 2 seconds
        self.throughput_samples
            .retain(|(_, t)| now.duration_since(*t).as_secs_f64() < 2.0);

        self.tick_throughput();
    }

    pub fn tick_throughput(&mut self) {
        let now = Instant::now();
        // Recalculate based on current samples
        let total_chars: usize = self.throughput_samples.iter().map(|(c, _)| c).sum();

        if let Some((_, first_time)) = self.throughput_samples.first() {
            let duration = now.duration_since(*first_time).as_secs_f64();
            if duration > 0.1 {
                self.current_throughput_cps = total_chars as f64 / duration;
            }
        }
    }

    pub fn reset_throughput(&mut self) {
        self.throughput_samples.clear();
        self.current_throughput_cps = 0.0;
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

        // Adjust for scroll offset - selection is in screen coords
        let scroll = self.activity_state.scroll_offset;
        let abs_start = start_line + scroll;
        let abs_end = end_line + scroll;

        let mut result = String::new();

        for (i, line) in self.rendered_lines.iter().enumerate() {
            if i < abs_start || i > abs_end {
                continue;
            }

            let text = &line.copyable_text;
            let chars: Vec<char> = text.chars().collect();

            if i == abs_start && i == abs_end {
                // Single line selection
                let s = start_col.min(chars.len());
                let e = (end_col + 1).min(chars.len());
                result.push_str(&chars[s..e].iter().collect::<String>());
            } else if i == abs_start {
                // First line
                let s = start_col.min(chars.len());
                result.push_str(&chars[s..].iter().collect::<String>());
                result.push('\n');
            } else if i == abs_end {
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

    /// Add a user message to activities
    fn add_user_activity(&mut self, content: &str) {
        self.activities.push(Activity::user_message(content));
        self.activity_scroll_to_bottom();
    }

    /// Get context usage as percentage
    pub fn context_percentage(&self) -> u8 {
        if self.context_window_size == 0 {
            return 0;
        }
        ((self.context_tokens_used as f64 / self.context_window_size as f64) * 100.0) as u8
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Settings interaction helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Handle Tab key in settings (switch tabs or panels)
    fn handle_settings_tab_key(&mut self, shift: bool) {
        use super::settings::{PinnedAgentsPanel, SettingsTab};

        match self.settings_state.active_tab {
            SettingsTab::PinnedAgents => {
                // In Pinned Agents tab, Tab switches between panels
                if shift {
                    self.settings_state.pinned_panel = self.settings_state.pinned_panel.prev();
                } else {
                    self.settings_state.pinned_panel = self.settings_state.pinned_panel.next();
                }
            }
            _ => {
                // In other tabs, Tab switches tabs
                if shift {
                    self.settings_state.prev_tab();
                } else {
                    self.settings_state.next_tab();
                }
            }
        }
    }

    /// Handle Up key in settings
    fn handle_settings_up_key(&mut self) {
        use super::settings::{PinnedAgentsPanel, SettingsTab};

        match self.settings_state.active_tab {
            SettingsTab::General => {
                if self.settings_state.selected_index > 0 {
                    self.settings_state.selected_index -= 1;
                }
            }
            SettingsTab::PinnedAgents => match self.settings_state.pinned_panel {
                PinnedAgentsPanel::DefaultModel => {
                    // No navigation in default model section for now
                }
                PinnedAgentsPanel::Agents => {
                    if self.settings_state.agent_list_index > 0 {
                        self.settings_state.agent_list_index -= 1;
                        // Reset model list selection when agent changes
                        self.settings_state.model_list_index = 0;
                    }
                }
                PinnedAgentsPanel::Models => {
                    if self.settings_state.model_list_index > 0 {
                        self.settings_state.model_list_index -= 1;
                    }
                }
            },
            SettingsTab::Models => {
                if self.settings_state.models_in_oauth_section {
                    // Can't go up from OAuth section, stay there
                } else if self.settings_state.models_selected_index > 0 {
                    self.settings_state.models_selected_index -= 1;
                } else {
                    // At top of models list, go back to OAuth section
                    self.settings_state.models_in_oauth_section = true;
                }
            }
            SettingsTab::McpServers => {
                use super::settings::McpPanel;
                match self.settings_state.mcp_panel {
                    McpPanel::Servers => {
                        if self.settings_state.mcp_server_index > 0 {
                            self.settings_state.mcp_server_index -= 1;
                        }
                    }
                    McpPanel::Agents => {
                        if self.settings_state.mcp_agent_index > 0 {
                            self.settings_state.mcp_agent_index -= 1;
                            self.settings_state.mcp_checkbox_index = 0;
                        }
                    }
                    McpPanel::McpCheckboxes => {
                        if self.settings_state.mcp_checkbox_index > 0 {
                            self.settings_state.mcp_checkbox_index -= 1;
                        }
                    }
                }
            }
        }
    }

    /// Handle Down key in settings
    fn handle_settings_down_key(&mut self) {
        use super::settings::{PinnedAgentsPanel, SettingsTab};

        match self.settings_state.active_tab {
            SettingsTab::General => {
                if self.settings_state.selected_index < 3 {
                    self.settings_state.selected_index += 1;
                }
            }
            SettingsTab::PinnedAgents => match self.settings_state.pinned_panel {
                PinnedAgentsPanel::DefaultModel => {
                    // No navigation in default model section for now
                }
                PinnedAgentsPanel::Agents => {
                    let agent_count = self.agents.list().len();
                    if agent_count > 0
                        && self.settings_state.agent_list_index < agent_count - 1
                    {
                        self.settings_state.agent_list_index += 1;
                        // Reset model list selection when agent changes
                        self.settings_state.model_list_index = 0;
                    }
                }
                PinnedAgentsPanel::Models => {
                    let model_count = self.model_registry.list_available(&self.db).len() + 1; // +1 for "Use Default"
                    if self.settings_state.model_list_index < model_count - 1 {
                        self.settings_state.model_list_index += 1;
                    }
                }
            },
            SettingsTab::Models => {
                if self.settings_state.models_in_oauth_section {
                    // Go from OAuth section to models list
                    self.settings_state.models_in_oauth_section = false;
                    self.settings_state.models_selected_index = 0;
                } else {
                    let available_models = self.model_registry.list_available(&self.db);
                    let max_index = super::settings::models::count_models_items(self, &available_models);
                    if max_index > 0 && self.settings_state.models_selected_index < max_index - 1 {
                        self.settings_state.models_selected_index += 1;
                    }
                }
            }
            SettingsTab::McpServers => {
                use super::settings::McpPanel;
                match self.settings_state.mcp_panel {
                    McpPanel::Servers => {
                        let server_count = super::settings::mcp_servers::server_count();
                        if server_count > 0 && self.settings_state.mcp_server_index < server_count - 1 {
                            self.settings_state.mcp_server_index += 1;
                        }
                    }
                    McpPanel::Agents => {
                        let agent_count = self.agents.list().len();
                        if agent_count > 0 && self.settings_state.mcp_agent_index < agent_count - 1 {
                            self.settings_state.mcp_agent_index += 1;
                            self.settings_state.mcp_checkbox_index = 0;
                        }
                    }
                    McpPanel::McpCheckboxes => {
                        let enabled_count = super::settings::mcp_servers::enabled_server_count();
                        if enabled_count > 0 && self.settings_state.mcp_checkbox_index < enabled_count - 1 {
                            self.settings_state.mcp_checkbox_index += 1;
                        }
                    }
                }
            }
        }
    }

    /// Handle Enter key in settings
    fn handle_settings_enter_key(&mut self) {
        use super::settings::{PinnedAgentsPanel, SettingsTab};
        use crate::agents::UserMode;
        use crate::config::{PdfMode, Settings};

        let settings = Settings::new(&self.db);

        match self.settings_state.active_tab {
            SettingsTab::General => {
                let idx = self.settings_state.selected_index;
                match idx {
                    0 => {
                        // PDF Mode - toggle between Image and Text
                        let current = settings.pdf_mode();
                        let new_mode = match current {
                            PdfMode::Image => PdfMode::TextExtract,
                            PdfMode::TextExtract => PdfMode::Image,
                        };
                        let _ = settings.set_pdf_mode(new_mode);
                    }
                    1 => {
                        // User Mode - cycle through Normal -> Expert -> Developer
                        let current = settings.user_mode();
                        let new_mode = match current {
                            UserMode::Normal => UserMode::Expert,
                            UserMode::Expert => UserMode::Developer,
                            UserMode::Developer => UserMode::Normal,
                        };
                        let _ = settings.set_user_mode(new_mode);
                    }
                    2 => {
                        // Show Reasoning - toggle
                        let current = settings.get_bool("show_reasoning").unwrap_or(true);
                        let _ = settings.set("show_reasoning", if current { "false" } else { "true" });
                    }
                    3 => {
                        // YOLO Mode - toggle
                        let current = settings.yolo_mode();
                        let _ = settings.set_yolo_mode(!current);
                    }
                    _ => {}
                }
            }
            SettingsTab::PinnedAgents => {
                match self.settings_state.pinned_panel {
                    PinnedAgentsPanel::DefaultModel => {
                        // Could open a dropdown in the future
                    }
                    PinnedAgentsPanel::Agents => {
                        // Selecting an agent - switch focus to models panel
                        self.settings_state.pinned_panel = PinnedAgentsPanel::Models;
                    }
                    PinnedAgentsPanel::Models => {
                        // Pin/unpin the selected model
                        let agents = self.agents.list();
                        if let Some(agent_info) = agents.get(self.settings_state.agent_list_index) {
                            let agent_name = &agent_info.name;
                            let model_idx = self.settings_state.model_list_index;

                            if model_idx == 0 {
                                // "Use Default" - clear the pin
                                let _ = settings.clear_agent_pinned_model(agent_name);
                            } else {
                                // Pin the selected model
                                let available_models = self.model_registry.list_available(&self.db);
                                if let Some(model_name) = available_models.get(model_idx - 1) {
                                    let _ = settings.set_agent_pinned_model(agent_name, model_name);
                                }
                            }
                        }
                    }
                }
            }
            SettingsTab::Models => {
                if self.settings_state.models_in_oauth_section {
                    // Can't interact with OAuth section in TUI
                    return;
                }
                
                let available_models = self.model_registry.list_available(&self.db);
                let selected_index = self.settings_state.models_selected_index;
                
                // Check if it's a group header (to expand/collapse)
                if let Some(type_label) = super::settings::models::is_group_header(self, &available_models, selected_index) {
                    // Toggle expanded state
                    if self.settings_state.models_expanded_providers.contains(&type_label) {
                        self.settings_state.models_expanded_providers.remove(&type_label);
                    } else {
                        self.settings_state.models_expanded_providers.insert(type_label);
                    }
                } else if let Some(model_name) = super::settings::models::get_model_at_index(self, &available_models, selected_index) {
                    // Set as default model
                    self.current_model = model_name.clone();
                    let _ = settings.set("model", &model_name);
                    self.update_context_usage();
                }
            }
            SettingsTab::McpServers => {
                use super::settings::McpPanel;
                match self.settings_state.mcp_panel {
                    McpPanel::Servers => {
                        // Toggle enable/disable of selected server
                        super::settings::mcp_servers::toggle_server_enabled(
                            self.settings_state.mcp_server_index
                        );
                    }
                    McpPanel::Agents => {
                        // Selecting an agent - switch focus to MCP checkboxes
                        self.settings_state.mcp_panel = McpPanel::McpCheckboxes;
                    }
                    McpPanel::McpCheckboxes => {
                        // Toggle MCP attachment for the selected agent
                        let agents = self.agents.list();
                        if let Some(agent_info) = agents.get(self.settings_state.mcp_agent_index) {
                            let agent_name = &agent_info.name;
                            let checkbox_idx = self.settings_state.mcp_checkbox_index;
                            
                            if let Some(mcp_name) = super::settings::mcp_servers::get_enabled_server_name(checkbox_idx) {
                                let current_mcps = settings.get_agent_mcps(agent_name);
                                if current_mcps.contains(&mcp_name) {
                                    // Remove attachment
                                    let _ = settings.remove_agent_mcp(agent_name, &mcp_name);
                                } else {
                                    // Add attachment
                                    let _ = settings.add_agent_mcp(agent_name, &mcp_name);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Handle Left key in settings
    fn handle_settings_left_key(&mut self) {
        use super::settings::{PinnedAgentsPanel, SettingsTab};
        use crate::agents::UserMode;
        use crate::config::{PdfMode, Settings};

        match self.settings_state.active_tab {
            SettingsTab::General => {
                // Cycle radio options backwards
                let settings = Settings::new(&self.db);
                let idx = self.settings_state.selected_index;
                match idx {
                    0 => {
                        let current = settings.pdf_mode();
                        let new_mode = match current {
                            PdfMode::Image => PdfMode::TextExtract,
                            PdfMode::TextExtract => PdfMode::Image,
                        };
                        let _ = settings.set_pdf_mode(new_mode);
                    }
                    1 => {
                        let current = settings.user_mode();
                        let new_mode = match current {
                            UserMode::Normal => UserMode::Developer,
                            UserMode::Expert => UserMode::Normal,
                            UserMode::Developer => UserMode::Expert,
                        };
                        let _ = settings.set_user_mode(new_mode);
                    }
                    _ => {}
                }
            }
            SettingsTab::PinnedAgents => {
                // Switch to previous panel
                self.settings_state.pinned_panel = self.settings_state.pinned_panel.prev();
            }
            SettingsTab::McpServers => {
                // Switch to previous panel
                self.settings_state.mcp_panel = self.settings_state.mcp_panel.prev();
            }
            _ => {}
        }
    }

    /// Handle Right key in settings
    fn handle_settings_right_key(&mut self) {
        use super::settings::{PinnedAgentsPanel, SettingsTab};
        use crate::agents::UserMode;
        use crate::config::{PdfMode, Settings};

        match self.settings_state.active_tab {
            SettingsTab::General => {
                // Cycle radio options forwards
                let settings = Settings::new(&self.db);
                let idx = self.settings_state.selected_index;
                match idx {
                    0 => {
                        let current = settings.pdf_mode();
                        let new_mode = match current {
                            PdfMode::Image => PdfMode::TextExtract,
                            PdfMode::TextExtract => PdfMode::Image,
                        };
                        let _ = settings.set_pdf_mode(new_mode);
                    }
                    1 => {
                        let current = settings.user_mode();
                        let new_mode = match current {
                            UserMode::Normal => UserMode::Expert,
                            UserMode::Expert => UserMode::Developer,
                            UserMode::Developer => UserMode::Normal,
                        };
                        let _ = settings.set_user_mode(new_mode);
                    }
                    _ => {}
                }
            }
            SettingsTab::PinnedAgents => {
                // Switch to next panel
                self.settings_state.pinned_panel = self.settings_state.pinned_panel.next();
            }
            SettingsTab::McpServers => {
                // Switch to next panel
                self.settings_state.mcp_panel = self.settings_state.mcp_panel.next();
            }
            _ => {}
        }
    }
    /// Handle Delete key in settings
    fn handle_settings_delete_key(&mut self) {
        use super::settings::{McpPanel, SettingsTab};
        
        match self.settings_state.active_tab {
            SettingsTab::McpServers => {
                if self.settings_state.mcp_panel == McpPanel::Servers {
                    // Remove the selected MCP server
                    let server_count = super::settings::mcp_servers::server_count();
                    if server_count > 0 {
                        super::settings::mcp_servers::remove_server(
                            self.settings_state.mcp_server_index
                        );
                        // Adjust selection if needed
                        if self.settings_state.mcp_server_index >= server_count.saturating_sub(1) {
                            self.settings_state.mcp_server_index = server_count.saturating_sub(2);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Folder Modal Methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Open the folder modal and load directory entries
    pub fn open_folder_modal(&mut self) {
        self.show_folder_modal = true;
        self.folder_modal_selected = 0;
        self.folder_modal_scroll = 0;
        self.load_folder_entries();
    }

    /// Close the folder modal
    pub fn close_folder_modal(&mut self) {
        self.show_folder_modal = false;
        self.folder_modal_entries.clear();
        self.folder_modal_selected = 0;
        self.folder_modal_scroll = 0;
    }

    /// Load directory entries for the current working directory
    pub fn load_folder_entries(&mut self) {
        self.folder_modal_entries.clear();
        
        if let Ok(entries) = std::fs::read_dir(&self.current_working_dir) {
            let mut dirs: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter(|p| {
                    // Filter out hidden directories (except ..)
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| !n.starts_with('.') || n == "..")
                        .unwrap_or(false)
                })
                .collect();
            
            // Sort alphabetically
            dirs.sort_by(|a, b| {
                a.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .cmp(
                        &b.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_lowercase(),
                    )
            });
            
            self.folder_modal_entries = dirs;
        }
    }

    /// Navigate to a directory in the folder modal
    pub fn folder_modal_navigate(&mut self, index: usize) {
        // Index 0 is ".." (parent directory)
        if index == 0 {
            // Go to parent
            if let Some(parent) = self.current_working_dir.parent() {
                self.current_working_dir = parent.to_path_buf();
                self.folder_modal_selected = 0;
                self.folder_modal_scroll = 0;
                self.load_folder_entries();
            }
        } else if let Some(path) = self.folder_modal_entries.get(index - 1) {
            // Navigate into the selected directory
            self.current_working_dir = path.clone();
            self.folder_modal_selected = 0;
            self.folder_modal_scroll = 0;
            self.load_folder_entries();
        }
    }

    /// Ensure the selected item is visible in the folder modal
    pub fn folder_modal_ensure_visible(&mut self, visible_height: usize) {
        if self.folder_modal_selected < self.folder_modal_scroll {
            self.folder_modal_scroll = self.folder_modal_selected;
        } else if self.folder_modal_selected >= self.folder_modal_scroll + visible_height {
            self.folder_modal_scroll = self.folder_modal_selected - visible_height + 1;
        }
    }

    /// Confirm the current folder selection and close modal
    pub fn folder_modal_confirm(&mut self) {
        // Set the working directory
        if std::env::set_current_dir(&self.current_working_dir).is_ok() {
            tracing::info!("Changed working directory to: {:?}", self.current_working_dir);
        }
        self.close_folder_modal();
    }

    /// Get total items in folder modal (parent + entries)
    pub fn folder_modal_item_count(&self) -> usize {
        1 + self.folder_modal_entries.len() // 1 for ".." parent
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

        while !self.should_quit {
            // Process auto-scroll if dragging selection outside visible area
            if self.selection.is_dragging() && self.auto_scroll_direction.is_some() {
                if self.last_auto_scroll.elapsed() >= auto_scroll_interval {
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
                        }
                        _ => {}
                    }
                }
            }

            // Prepare state for rendering (avoids mutation during render)
            self.prepare_for_render();

            // Draw UI
            let app_ptr: *mut TuiApp = self;
            self.terminal
                .draw(|frame| unsafe { ui::render(frame, &mut *app_ptr) })?;

            // Handle events and messages
            tokio::select! {
                biased;  // Prefer bus messages over UI events
                
                Ok(msg) = bus_receiver.recv() => {
                    tracing::debug!("BUS RECV: {:?}", std::mem::discriminant(&msg));
                    self.handle_bus_message(msg);
                }
                maybe_event = events.next() => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await?;
                    }
                }
            }
        }

        // Put events back (though we're dropping anyway)
        self.events = Some(events);

        Ok(())
    }

    /// Update context usage tracking
    pub fn update_context_usage(&mut self) {
        // Use the existing token estimation utility
        self.context_tokens_used = crate::tokens::estimate_tokens(&self.message_history);

        // Update window size from current model
        if let Some(model) = self.model_registry.get(&self.current_model) {
            self.context_window_size = model.context_length;
        }
    }

    /// Handle an application event
    async fn handle_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::Key(key) => {
                use crossterm::event::{KeyCode, KeyModifiers};

                // Global shortcuts that bypass textarea
                match (key.modifiers, key.code) {
                    (KeyModifiers::CONTROL, KeyCode::Char('q')) => {
                        self.should_quit = true;
                        return Ok(());
                    }
                    (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                        if self.is_generating {
                            // Cancel generation?
                            // TODO: implement cancellation
                        } else if let Some(selected) = self.get_selected_text_from_activities() {
                            self.clipboard.copy(&selected);
                            self.copy_feedback = Some((Instant::now(), "Copied!".to_string()));
                            self.selection.clear();
                        } else if let Some(selected) = self.get_selected_text() {
                            self.clipboard.copy(&selected);
                            self.copy_feedback = Some((Instant::now(), "Copied!".to_string()));
                        }
                        return Ok(());
                    }
                    // 'y' for vim-style yank when selection is active
                    (KeyModifiers::NONE, KeyCode::Char('y')) => {
                        if self.selection.is_active() {
                            if let Some(selected) = self.get_selected_text_from_activities() {
                                self.clipboard.copy(&selected);
                                self.copy_feedback = Some((Instant::now(), "Yanked!".to_string()));
                                self.selection.clear();
                                return Ok(());
                            }
                        }
                        // If no selection, let 'y' pass through to textarea
                    }
                    (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
                        if let Some(text) = self.clipboard.paste() {
                            self.input.insert_str(&text);
                        }
                        return Ok(());
                    }
                    (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                        // Clear everything for new conversation
                        self.conversation.clear();
                        self.activities.clear();
                        self.rendered_lines.clear();
                        self.message_history.clear();
                        self.input = Self::build_input();
                        self.selection.clear();
                        self.activity_state = widgets::ActivityFeedState::default();
                        // Force full terminal clear to remove any rendering artifacts
                        // from wide Unicode chars that may leave ghost characters
                        self.terminal.clear()?;
                        return Ok(());
                    }
                    (_, KeyCode::Esc) => {
                        // Settings takes priority, then folder modal, then dropdowns, then help
                        if self.show_settings {
                            self.show_settings = false;
                        } else if self.show_folder_modal {
                            self.close_folder_modal();
                        } else {
                            self.show_agent_dropdown = false;
                            self.show_model_dropdown = false;
                            self.show_help = false;
                        }
                        return Ok(());
                    }
                    // F1 toggles help (don't use '?' - that should just type a question mark)
                    (_, KeyCode::F(1)) => {
                        self.show_help = !self.show_help;
                        return Ok(());
                    }
                    // F2 or Ctrl+, toggles settings
                    (_, KeyCode::F(2)) | (KeyModifiers::CONTROL, KeyCode::Char(',')) => {
                        self.show_settings = !self.show_settings;
                        return Ok(());
                    }
                    // Settings navigation when settings is open
                    (KeyModifiers::NONE, KeyCode::Tab) if self.show_settings => {
                        self.handle_settings_tab_key(false);
                        return Ok(());
                    }
                    (KeyModifiers::SHIFT, KeyCode::BackTab) if self.show_settings => {
                        self.handle_settings_tab_key(true);
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Up) if self.show_settings => {
                        self.handle_settings_up_key();
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Down) if self.show_settings => {
                        self.handle_settings_down_key();
                        return Ok(());
                    }
                    // Enter or Space toggles/cycles the selected setting
                    (KeyModifiers::NONE, KeyCode::Enter) if self.show_settings => {
                        self.handle_settings_enter_key();
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Char(' ')) if self.show_settings => {
                        self.handle_settings_enter_key();
                        return Ok(());
                    }
                    // Left/Right for cycling radio options or panel navigation
                    (KeyModifiers::NONE, KeyCode::Left) if self.show_settings => {
                        self.handle_settings_left_key();
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Right) if self.show_settings => {
                        self.handle_settings_right_key();
                        return Ok(());
                    }
                    // Delete key for removing items in settings
                    (KeyModifiers::NONE, KeyCode::Delete) if self.show_settings => {
                        self.handle_settings_delete_key();
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Backspace) if self.show_settings => {
                        // Backspace also works as delete in settings
                        self.handle_settings_delete_key();
                        return Ok(());
                    }
                    // Folder modal navigation
                    (KeyModifiers::NONE, KeyCode::Up) if self.show_folder_modal => {
                        if self.folder_modal_selected > 0 {
                            self.folder_modal_selected -= 1;
                            // Auto-scroll to keep selection visible (8 visible items)
                            self.folder_modal_ensure_visible(8);
                        }
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Down) if self.show_folder_modal => {
                        let max_index = self.folder_modal_item_count().saturating_sub(1);
                        if self.folder_modal_selected < max_index {
                            self.folder_modal_selected += 1;
                            // Auto-scroll to keep selection visible (8 visible items)
                            self.folder_modal_ensure_visible(8);
                        }
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Enter) if self.show_folder_modal => {
                        if self.folder_modal_selected == 0 {
                            // Parent directory - navigate to it
                            self.folder_modal_navigate(0);
                        } else {
                            // Subdirectory - navigate into it
                            self.folder_modal_navigate(self.folder_modal_selected);
                        }
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Backspace) if self.show_folder_modal => {
                        // Backspace goes to parent directory
                        self.folder_modal_navigate(0);
                        return Ok(());
                    }
                    // Ctrl+Enter confirms and closes folder modal
                    (KeyModifiers::CONTROL, KeyCode::Enter) if self.show_folder_modal => {
                        self.folder_modal_confirm();
                        return Ok(());
                    }
                    // Absorb all other keys when folder modal is open
                    _ if self.show_folder_modal => {
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Enter)
                    | (KeyModifiers::NONE, KeyCode::Char('\n')) => {
                        if !self.input.is_empty() && !self.is_generating {
                            self.send_message().await?;
                        }
                        return Ok(());
                    }
                    (KeyModifiers::SHIFT, KeyCode::Enter)
                    | (KeyModifiers::ALT, KeyCode::Enter) => {
                        // Shift+Enter or Alt+Enter inserts newline
                        // (Alt+Enter works as fallback when terminal doesn't support
                        // keyboard enhancement for detecting Shift+Enter)
                        self.input.insert_newline();
                        return Ok(());
                    }
                    // Absorb all other keys when settings is open
                    _ if self.show_settings => {
                        return Ok(());
                    }
                    _ => {}
                }

                // Pass other keys to textarea (only if settings not open)
                if !self.show_settings {
                    self.input.input(Input::from(key));
                }
            }
            AppEvent::SelectionStart { row, col } => {
                self.last_mouse_pos = Some((col, row));
                
                // ONLY start selection if click is inside the activity area
                // This prevents selection logic from running on dropdown clicks, etc.
                let area = self.cached_activity_area;
                if row >= area.y && row < area.y + area.height 
                    && col >= area.x && col < area.x + area.width 
                {
                    self.selection.start_selection(Position::new(row, col));
                    self.auto_scroll_direction = None;
                }
                // If click is outside activity area, don't start selection
                // (let the Click event handle dropdowns, etc.)
            }
            AppEvent::SelectionUpdate { row, col } => {
                self.last_mouse_pos = Some((col, row));

                // Only process if selection is active (was started in activity area)
                if !self.selection.is_dragging() {
                    return Ok(());
                }

                // Check if mouse is above or below activity area for auto-scroll
                let area = self.cached_activity_area;
                let area_top = area.y;
                let area_bottom = area.y + area.height;

                if row < area_top {
                    // Mouse above visible area - scroll up
                    self.auto_scroll_direction = Some(-1);
                    let content_line = self.activity_state.scroll_offset;
                    self.selection.update_to(content_line, col as usize);
                } else if row >= area_bottom {
                    // Mouse below visible area - scroll down
                    self.auto_scroll_direction = Some(1);
                    let content_line = self.activity_state.scroll_offset
                        + self.activity_state.viewport_height.saturating_sub(1);
                    let max_line = self.rendered_lines.len().saturating_sub(1);
                    self.selection.update_to(content_line.min(max_line), col as usize);
                } else {
                    // Mouse in visible area - no auto-scroll
                    self.auto_scroll_direction = None;
                    let screen_line = (row - area_top) as usize;
                    let content_line = self.activity_state.scroll_offset + screen_line;
                    self.selection.update_to(content_line, col as usize);
                }
            }
            AppEvent::SelectionEnd => {
                self.selection.end_selection();
                self.auto_scroll_direction = None;
            }
            AppEvent::Click {
                row,
                col,
                button: _,
            } => {
                self.last_mouse_pos = Some((col, row));

                // Check hit test first
                // Note: col, row order in hit_test(x, y)
                let target = self.hit_registry.hit_test(col, row).cloned();

                // Close dropdowns on outside click (GUI-style behavior)
                // If a dropdown is open and the click is NOT on a related target, close it
                if self.show_agent_dropdown {
                    match &target {
                        Some(ClickTarget::AgentDropdown) | Some(ClickTarget::AgentItem(_)) => {
                            // Let normal handling proceed
                        }
                        _ => {
                            // Click outside agent dropdown - close without changing
                            self.show_agent_dropdown = false;
                            return Ok(());
                        }
                    }
                }
                // Note: Model dropdown removed from header - model pinning is in Settings > Pinned Agents
                if self.show_folder_modal {
                    match &target {
                        Some(ClickTarget::FolderDropdown) | Some(ClickTarget::FolderItem(_)) => {
                            // Let normal handling proceed
                        }
                        _ => {
                            // Click outside folder modal - close without changing
                            self.close_folder_modal();
                            return Ok(());
                        }
                    }
                }

                // Handle the click on a specific target
                if let Some(target) = target {
                    match target {
                        ClickTarget::AgentDropdown => {
                            self.show_agent_dropdown = !self.show_agent_dropdown;
                            self.show_folder_modal = false;
                        }
                        // Note: ModelDropdown target kept for potential future use, but not in header
                        ClickTarget::ModelDropdown => {
                            // Model selection is now in Settings > Pinned Agents
                            // This target is not registered in the header
                        }
                        ClickTarget::FolderDropdown => {
                            if self.show_folder_modal {
                                self.close_folder_modal();
                            } else {
                                self.open_folder_modal();
                            }
                            self.show_agent_dropdown = false;
                        }
                        ClickTarget::AgentItem(name) => {
                            self.current_agent = name;
                            self.show_agent_dropdown = false;
                        }
                        ClickTarget::ModelItem(name) => {
                            self.current_model = name.clone();
                            self.show_model_dropdown = false;

                            // Save settings
                            let settings = Settings::new(&self.db);
                            if let Err(e) = settings.set("model", &name) {
                                tracing::error!("Failed to save model setting: {}", e);
                            }
                        }
                        ClickTarget::SectionToggle(id) => {
                            self.conversation.toggle_section_collapsed(&id);
                            self.conversation.toggle_thinking_collapsed(&id);
                        }
                        ClickTarget::FolderItem(index) => {
                            self.folder_modal_navigate(index);
                        }
                        _ => {}
                    }
                    // If we hit a UI element, stop processing click
                    return Ok(());
                }

                // If it's a simple click (not a drag-selection), clear selection
                if let Some((start, end)) = self.selection.get_selection() {
                    if start == end {
                        self.selection.clear();
                    }
                } else {
                    self.selection.clear();
                }
            }
            AppEvent::Mouse(mouse) => {
                self.last_mouse_pos = Some((mouse.column, mouse.row));
                use crossterm::event::MouseEventKind;
                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        // Scroll both for now (activity feed takes precedence)
                        self.activity_scroll_up(3);
                        self.message_list_state.scroll_up(3);
                    }
                    MouseEventKind::ScrollDown => {
                        self.activity_scroll_down(3);
                        self.message_list_state.scroll_down(3);
                    }
                    _ => {}
                }
            }
            AppEvent::Resize(_, _) => {
                // Terminal will auto-resize
            }
            AppEvent::Tick => {
                // Animation updates if needed
            }
            AppEvent::Paste(text) => {
                self.input.insert_str(&text);
            }
            AppEvent::Error(e) => {
                tracing::error!("TUI error: {}", e);
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle incoming messages from the message bus
    fn handle_bus_message(&mut self, msg: Message) {
        match msg {
            Message::TextDelta(delta) => {
                // Check if this delta is from a nested agent
                if let Some(agent_name) = &delta.agent_name {
                    // Route to the nested agent's section
                    if let Some(section_id) = self.active_section_ids.get(agent_name).cloned() {
                        self.conversation
                            .append_to_nested_agent(&section_id, &delta.text);
                    } else {
                        // Fallback: append to main content if section not found
                        self.conversation.append_to_current(&delta.text);
                    }
                } else {
                    // No agent attribution - append to current (handles main agent)
                    self.conversation.append_to_current(&delta.text);
                }

                // Activity feed: update or create Streaming activity
                if let Some(Activity::Streaming { content, elapsed, .. }) = self.activities.last_mut() {
                    content.push_str(&delta.text);
                    *elapsed = self.stream_start.map(|s| s.elapsed()).unwrap_or_default();
                } else {
                    self.activities.push(Activity::streaming("Responding", true));
                    if let Some(Activity::Streaming { content, .. }) = self.activities.last_mut() {
                        content.push_str(&delta.text);
                    }
                }

                self.update_throughput(delta.text.len());
                // Scroll to bottom on new content
                self.message_list_state.scroll_to_bottom();
                self.activity_scroll_to_bottom();
            }
            Message::Thinking(thinking) => {
                tracing::info!("TUI: Received Thinking message, text_len={}, agent={:?}", 
                    thinking.text.len(), thinking.agent_name);
                if let Some(agent_name) = &thinking.agent_name {
                    if let Some(section_id) = self.active_section_ids.get(agent_name).cloned() {
                        self.conversation
                            .append_thinking_in_section(&section_id, &thinking.text);
                    } else {
                        self.conversation.append_thinking(&thinking.text);
                    }
                } else {
                    self.conversation.append_thinking(&thinking.text);
                }

                // Activity feed: append to existing Thinking or create new
                if let Some(Activity::Thinking { content, .. }) = self.activities.last_mut() {
                    // Append to existing thinking block
                    content.push_str(&thinking.text);
                } else {
                    // Create new thinking activity
                    self.activities.push(Activity::thinking(&thinking.text));
                }

                self.message_list_state.scroll_to_bottom();
                self.activity_scroll_to_bottom();
            }
            Message::Tool(tool) => {
                tracing::info!("TOOL MSG: name='{}' status={:?} args={:?}", 
                    tool.tool_name, tool.status, tool.args);

                // Existing conversation-based tool tracking
                match tool.status {
                    ToolStatus::Executing => {
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) =
                                self.active_section_ids.get(agent_name).cloned()
                            {
                                self.conversation.append_tool_call_to_section(
                                    &section_id,
                                    &tool.tool_name,
                                    tool.args.clone(),
                                );
                            } else {
                                self.conversation
                                    .append_tool_call(&tool.tool_name, tool.args.clone());
                            }
                        } else {
                            self.conversation
                                .append_tool_call(&tool.tool_name, tool.args.clone());
                        }
                    }
                    ToolStatus::Completed => {
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) =
                                self.active_section_ids.get(agent_name).cloned()
                            {
                                self.conversation.complete_tool_call_in_section(
                                    &section_id,
                                    &tool.tool_name,
                                    true,
                                );
                            } else {
                                self.conversation.complete_tool_call(&tool.tool_name, true);
                            }
                        } else {
                            self.conversation.complete_tool_call(&tool.tool_name, true);
                        }
                    }
                    ToolStatus::Failed => {
                        if let Some(agent_name) = &tool.agent_name {
                            if let Some(section_id) =
                                self.active_section_ids.get(agent_name).cloned()
                            {
                                self.conversation.complete_tool_call_in_section(
                                    &section_id,
                                    &tool.tool_name,
                                    false,
                                );
                            } else {
                                self.conversation.complete_tool_call(&tool.tool_name, false);
                            }
                        } else {
                            self.conversation.complete_tool_call(&tool.tool_name, false);
                        }
                    }
                    _ => {}
                }

                // Activity feed: convert tool to activities
                let new_activities = self.activity_converter.process_tool(&tool);
                tracing::info!("ACTIVITIES: got {} from converter for '{}' status={:?}", 
                    new_activities.len(), tool.tool_name, tool.status);
                for activity in new_activities {
                    tracing::info!("ACTIVITY: processing {:?}", std::mem::discriminant(&activity));
                    // Merge consecutive Explored activities (file reads/lists)
                    if let (
                        Some(Activity::Explored { actions, .. }),
                        Activity::Explored { actions: new_actions, .. },
                    ) = (self.activities.last_mut(), &activity)
                    {
                        tracing::info!("ACTIVITY: merging into existing Explored");
                        actions.extend(new_actions.clone());
                    } else {
                        tracing::info!("ACTIVITY: pushing new activity, total now {}", self.activities.len() + 1);
                        self.activities.push(activity);
                    }
                }
                if !self.activities.is_empty() {
                    self.activity_scroll_to_bottom();
                }

                self.message_list_state.scroll_to_bottom();
            }
            Message::Agent(agent) => match agent.event {
                AgentEvent::Started => {
                    if self.active_agent_stack.is_empty() {
                        // Main agent starting
                        self.conversation.start_assistant_message();
                        self.is_generating = true;
                        self.reset_throughput();
                        self.stream_start = Some(Instant::now());
                    } else {
                        // Sub-agent starting
                        if let Some(section_id) = self
                            .conversation
                            .start_nested_agent(&agent.agent_name, &agent.display_name)
                        {
                            self.active_section_ids
                                .insert(agent.agent_name.clone(), section_id);
                        }
                        // Activity feed: add nested agent activity
                        self.activities.push(Activity::nested_agent(
                            &agent.agent_name,
                            &agent.display_name,
                        ));
                    }
                    self.active_agent_stack.push(agent.agent_name.clone());
                    self.message_list_state.scroll_to_bottom();
                    self.activity_scroll_to_bottom();
                }
                AgentEvent::Completed { .. } => {
                    if let Some(completed_agent) = self.active_agent_stack.pop() {
                        if let Some(section_id) = self.active_section_ids.remove(&completed_agent) {
                            self.conversation.finish_nested_agent(&section_id);
                            self.conversation.set_section_collapsed(&section_id, true);
                        }
                    }
                    if self.active_agent_stack.is_empty() {
                        self.conversation.finish_current_message();
                        self.is_generating = false;
                        self.stream_start = None;
                        self.update_context_usage();
                    }
                }
                AgentEvent::Error { message } => {
                    // Pop all agents down to (and including) the errored one
                    while let Some(agent_name) = self.active_agent_stack.pop() {
                        if let Some(section_id) = self.active_section_ids.remove(&agent_name) {
                            self.conversation.append_to_nested_agent(
                                &section_id,
                                &format!("\n\n❌ Error: {}", message),
                            );
                            self.conversation.finish_nested_agent(&section_id);
                        }
                        if agent_name == agent.agent_name {
                            break;
                        }
                    }

                    if self.active_agent_stack.is_empty() {
                        self.conversation
                            .append_to_current(&format!("\n\n❌ Error: {}", message));
                        self.conversation.finish_current_message();
                        self.is_generating = false;
                        self.stream_start = None;
                    }

                    // Activity feed: add error as a failed task
                    let mut error_task = Activity::task(format!("❌ Error: {}", message));
                    if let Activity::Task { completed, .. } = &mut error_task {
                        *completed = false;
                    }
                    self.activities.push(error_task);
                }
            },
            Message::HistoryUpdate(history_update) => {
                // Update message history from executor result
                if !history_update.messages.is_empty() {
                    self.message_history = history_update.messages;
                    self.update_context_usage();
                    tracing::debug!(
                        history_len = self.message_history.len(),
                        "Updated message history from executor"
                    );
                }
            }
            _ => {}
        }
    }

    /// Send the current input as a message
    async fn send_message(&mut self) -> Result<()> {
        let content: String = self.input.lines().join("\n");
        if content.trim().is_empty() {
            return Ok(());
        }

        // Handle slash commands
        if content.starts_with('/') {
            let parts: Vec<&str> = content.split_whitespace().collect();
            match parts[0] {
                "/attach" => {
                    if let Some(path_str) = parts.get(1) {
                        let path = std::path::PathBuf::from(path_str);
                        if let Err(e) = self.attachments.add_file(path) {
                            tracing::error!("Attach error: {}", e);
                        } else {
                            // Clear input
                            self.input = Self::build_input();
                        }
                    }
                    return Ok(());
                }
                "/clear" => {
                    self.attachments.clear();
                    self.input = Self::build_input();
                    return Ok(());
                }
                _ => {} // Continue as message
            }
        }

        // Clear input
        self.input = Self::build_input();
        self.selection.clear();

        // Prepare content with attachments
        let mut final_content = content.clone();
        if !self.attachments.is_empty() {
            final_content.push_str("\n\n--- Attachments ---\n");
            for attachment in &self.attachments.pending {
                let crate::tui::attachments::TuiAttachment::File { path, .. } = attachment;
                if let Ok(text) = std::fs::read_to_string(path) {
                    final_content.push_str(&format!(
                        "File: {}\n```\n{}\n```\n\n",
                        path.display(),
                        text
                    ));
                }
            }
            self.attachments.clear();
        }

        // Add user message to conversation and activities
        self.conversation.add_user_message(final_content.clone());
        self.add_user_activity(&final_content);
        self.message_list_state.scroll_to_bottom();

        self.update_context_usage();

        // Prepare for execution
        let agent_name = self.current_agent.clone();
        let prompt = final_content;
        let history = self.message_history.clone();
        let model_name = self.current_model.clone();
        let db = self.db.clone();
        let agent_manager = self.agents.clone();
        let model_registry = self.model_registry.clone();
        let tool_registry = self.tool_registry.clone();
        let mcp_manager = self.mcp_manager.clone();
        let sender = self.message_bus.sender();

        // Mark as generating
        self.is_generating = true;
        self.stream_start = Some(Instant::now());

        // Spawn agent execution as local task (not Send-safe due to Database)
        // Results come back via MessageBus which the main loop processes
        tokio::task::spawn_local(async move {
            execute_agent(
                agent_name,
                prompt,
                history,
                model_name,
                db,
                agent_manager,
                model_registry,
                tool_registry,
                mcp_manager,
                sender,
            )
            .await;
        });

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

    fn build_input() -> TextArea<'static> {
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
