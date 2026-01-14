//! Main TUI application state and logic

use std::collections::HashMap;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};
use ratatui::Terminal;
use serdes_ai_core::ModelRequest;
use tui_textarea::{CursorMove, Input, TextArea};

use super::attachments::AttachmentManager;
use super::event::{AppEvent, ClipboardManager, EventHandler};
use super::execution::execute_agent;
use super::hit_test::{ClickTarget, HitTestRegistry};
use super::selection::{Position, SelectionState};
use super::state::TuiConversation;
use super::theme::Theme;
use super::ui;
use super::widgets;
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

    /// Throughput samples (count, time)
    pub throughput_samples: Vec<(usize, Instant)>,
    /// Current throughput (chars/sec)
    pub current_throughput_cps: f64,
}

impl TuiApp {
    /// Create a new TUI application
    pub async fn new() -> Result<Self> {
        // Initialize terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
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
            throughput_samples: Vec::new(),
            current_throughput_cps: 0.0,
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

        while !self.should_quit {
            // Draw UI
            let app_ptr: *mut TuiApp = self;
            self.terminal
                .draw(|frame| unsafe { ui::render(frame, &mut *app_ptr) })?;

            // Handle events and messages
            tokio::select! {
                maybe_event = events.next() => {
                    if let Some(event) = maybe_event {
                        self.handle_event(event).await?;
                    }
                }
                Ok(msg) = bus_receiver.recv() => {
                    self.handle_bus_message(msg);
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
                        } else if let Some(selected) = self.get_selected_text() {
                            self.clipboard.copy(&selected);
                        }
                        return Ok(());
                    }
                    (KeyModifiers::CONTROL, KeyCode::Char('v')) => {
                        if let Some(text) = self.clipboard.paste() {
                            self.input.insert_str(&text);
                        }
                        return Ok(());
                    }
                    (KeyModifiers::CONTROL, KeyCode::Char('n')) => {
                        self.conversation.clear();
                        self.message_history.clear();
                        self.input = Self::build_input();
                        self.selection.clear();
                        return Ok(());
                    }
                    (_, KeyCode::Esc) => {
                        self.show_agent_dropdown = false;
                        self.show_model_dropdown = false;
                        self.show_help = false;
                        return Ok(());
                    }
                    // F1 toggles help (don't use '?' - that should just type a question mark)
                    (_, KeyCode::F(1)) => {
                        self.show_help = !self.show_help;
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Enter)
                    | (KeyModifiers::NONE, KeyCode::Char('\n')) => {
                        if !self.input.is_empty() && !self.is_generating {
                            self.send_message().await?;
                        }
                        return Ok(());
                    }
                    (KeyModifiers::SHIFT, KeyCode::Enter) => {
                        // Shift+Enter inserts newline
                        self.input.insert_newline();
                        return Ok(());
                    }
                    _ => {}
                }

                // Pass other keys to textarea
                self.input.input(Input::from(key));
            }
            AppEvent::SelectionStart { row, col } => {
                self.last_mouse_pos = Some((col, row));
                self.selection.start_selection(Position::new(row, col));
            }
            AppEvent::SelectionUpdate { row, col } => {
                self.last_mouse_pos = Some((col, row));
                self.selection.update_selection(Position::new(row, col));
            }
            AppEvent::SelectionEnd => {
                self.selection.end_selection();
            }
            AppEvent::Click {
                row,
                col,
                button: _,
            } => {
                self.last_mouse_pos = Some((col, row));

                // Check hit test first
                // Note: col, row order in hit_test(x, y)
                if let Some(target) = self.hit_registry.hit_test(col, row).cloned() {
                    match target {
                        ClickTarget::AgentDropdown => {
                            self.show_agent_dropdown = !self.show_agent_dropdown;
                            self.show_model_dropdown = false;
                        }
                        ClickTarget::ModelDropdown => {
                            self.show_model_dropdown = !self.show_model_dropdown;
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
                        self.message_list_state.scroll_up(3);
                    }
                    MouseEventKind::ScrollDown => {
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
                self.update_throughput(delta.text.len());
                // Scroll to bottom on new content
                self.message_list_state.scroll_to_bottom();
            }
            Message::Thinking(thinking) => {
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
                self.message_list_state.scroll_to_bottom();
            }
            Message::Tool(tool) => {
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
                self.message_list_state.scroll_to_bottom();
            }
            Message::Agent(agent) => match agent.event {
                AgentEvent::Started => {
                    if self.active_agent_stack.is_empty() {
                        // Main agent starting
                        self.conversation.start_assistant_message();
                        self.is_generating = true;
                        self.reset_throughput();
                    } else {
                        // Sub-agent starting
                        if let Some(section_id) = self
                            .conversation
                            .start_nested_agent(&agent.agent_name, &agent.display_name)
                        {
                            self.active_section_ids
                                .insert(agent.agent_name.clone(), section_id);
                        }
                    }
                    self.active_agent_stack.push(agent.agent_name.clone());
                    self.message_list_state.scroll_to_bottom();
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
                    }
                }
            },
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

        // Add user message
        self.conversation.add_user_message(final_content.clone());
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

        // Execute agent (blocking UI for now to avoid Send issues)
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
        // Create text input
        let mut input = TextArea::default();
        input.set_cursor_line_style(Style::default());
        input.set_placeholder_text("Type a message...");
        input.set_block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(" Message "),
        );
        input.move_cursor(CursorMove::End);
        input
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // Restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}
