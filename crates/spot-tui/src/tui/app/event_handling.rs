//! Application event handling (keyboard, mouse, system events).

use std::time::Instant;

use anyhow::Result;
use tui_textarea::Input;

use super::TuiApp;
use crate::tui::activity::Activity;
use crate::tui::event::AppEvent;
use crate::tui::execution::execute_agent;
use crate::tui::hit_test::ClickTarget;
use crate::tui::widgets;
use spot_core::config::Settings;

impl TuiApp {
    /// Handle an application event
    pub(super) async fn handle_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::Key(key) => {
                use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

                // IMPORTANT: Only process Press events to avoid double-firing on Windows.
                // Windows fires Press + Release (and sometimes Repeat) for arrow keys,
                // while macOS typically only fires Press.
                if key.kind != KeyEventKind::Press {
                    return Ok(());
                }

                // Clear any error message on keypress
                self.error_message = None;

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
                    // 'y' key opens key pool management in Models tab
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
                        // Default model dropdown takes highest priority
                        if self.show_settings && self.settings_state.default_model_dropdown_open {
                            self.settings_state.default_model_dropdown_open = false;
                        } else if self.show_settings
                            && self.settings_state.active_tab
                                == crate::tui::settings::SettingsTab::Models
                            && self.settings_state.expanded_model.is_some()
                        {
                            // Model settings expanded
                            if self.settings_state.model_settings_editing {
                                self.settings_state.model_settings_editing = false;
                            } else {
                                self.save_current_model_settings();
                                self.settings_state.expanded_model = None;
                            }
                        } else if self.show_settings {
                            // Settings takes priority, then folder modal, then dropdowns, then help
                            self.show_settings = false;
                        } else if self.show_folder_modal {
                            self.close_folder_modal();
                        } else if self.show_oauth_dialog {
                            // Cancel OAuth - just close dialog (flow continues in background)
                            self.show_oauth_dialog = false;
                            self.oauth_dialog_url = None;
                            self.oauth_dialog_port = None;
                            self.oauth_dialog_provider = None;
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
                    // ─────────────────────────────────────────────────────────────
                    // Key Pool overlay takes priority when active
                    // ─────────────────────────────────────────────────────────────
                    _ if self.settings_state.key_pool.active => {
                        let clipboard_text = self.clipboard.paste();
                        let _result = crate::tui::settings::handle_key_pool_event(
                            &mut self.settings_state.key_pool,
                            &self.db,
                            key,
                            clipboard_text.as_deref(),
                        );
                        // Key pool absorbs all events when active
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
                    // Absorb all other keys when OAuth dialog is open
                    _ if self.show_oauth_dialog => {
                        return Ok(());
                    }
                    (KeyModifiers::NONE, KeyCode::Enter)
                    | (KeyModifiers::NONE, KeyCode::Char('\n')) => {
                        if !self.input.is_empty() && !self.is_generating {
                            self.send_message().await?;
                        }
                        return Ok(());
                    }
                    (KeyModifiers::SHIFT, KeyCode::Enter) | (KeyModifiers::ALT, KeyCode::Enter) => {
                        // Shift+Enter or Alt+Enter inserts newline
                        // (Alt+Enter works as fallback when terminal doesn't support
                        // keyboard enhancement for detecting Shift+Enter)
                        self.input.insert_newline();
                        return Ok(());
                    }
                    // 'k' key opens key pool management in Models tab
                    (KeyModifiers::NONE, KeyCode::Char('k')) if self.show_settings => {
                        self.handle_settings_key_pool_open();
                        return Ok(());
                    }
                    // ─────────────────────────────────────────────────────────────
                    // Model settings editing handlers
                    // ─────────────────────────────────────────────────────────────
                    // Tab key in Models tab with expanded model - cycle fields
                    (KeyModifiers::NONE, KeyCode::Tab)
                        if self.show_settings
                            && self.settings_state.active_tab
                                == crate::tui::settings::SettingsTab::Models
                            && self.settings_state.expanded_model.is_some() =>
                    {
                        self.handle_model_settings_tab();
                        return Ok(());
                    }
                    // Character input for model settings
                    (KeyModifiers::NONE, KeyCode::Char(c))
                        if self.show_settings
                            && self.settings_state.active_tab
                                == crate::tui::settings::SettingsTab::Models
                            && self.settings_state.model_settings_editing =>
                    {
                        self.handle_model_settings_char(c);
                        return Ok(());
                    }
                    // Backspace for model settings editing
                    (KeyModifiers::NONE, KeyCode::Backspace)
                        if self.show_settings
                            && self.settings_state.active_tab
                                == crate::tui::settings::SettingsTab::Models
                            && self.settings_state.model_settings_editing =>
                    {
                        self.handle_model_settings_backspace();
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
                // Don't start selection when dialogs/overlays are open
                if self.show_settings
                    || self.show_agent_dropdown
                    || self.show_folder_modal
                    || self.show_help
                {
                    return Ok(());
                }

                self.last_mouse_pos = Some((col, row));

                // ONLY start selection if click is inside the activity area
                // This prevents selection logic from running on dropdown clicks, etc.
                let area = self.cached_activity_area;
                if row >= area.y
                    && row < area.y + area.height
                    && col >= area.x
                    && col < area.x + area.width
                {
                    // Convert screen coordinates to content coordinates:
                    // - screen_line: row offset from top of activity area
                    // - content_line: actual line in the content (accounting for scroll)
                    let screen_line = (row - area.y) as usize;
                    let content_line = self.activity_state.scroll_offset + screen_line;
                    // Note: col stays as screen X since rendering also uses screen X
                    self.selection.start_at(content_line, col as usize);
                    self.auto_scroll_direction = None;
                }
                // If click is outside activity area, don't start selection
                // (let the Click event handle dropdowns, etc.)
            }
            AppEvent::SelectionUpdate { row, col } => {
                // Don't update selection when dialogs/overlays are open
                if self.show_settings
                    || self.show_agent_dropdown
                    || self.show_folder_modal
                    || self.show_help
                {
                    return Ok(());
                }

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
                    self.selection
                        .update_to(content_line.min(max_line), col as usize);
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

                // Auto-copy to clipboard on mouse release if there's a selection
                if self.selection.is_active() {
                    if let Some(selected) = self.get_selected_text_from_activities() {
                        if !selected.is_empty() {
                            self.clipboard.copy(&selected);
                            self.copy_feedback = Some((Instant::now(), "Copied!".to_string()));
                            self.selection.clear();
                        }
                    }
                }
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

                // Handle settings clicks when settings is open
                if self.show_settings {
                    use crate::tui::settings::{McpPanel, PinnedAgentsPanel, SettingsTab};

                    if let Some(target) = &target {
                        match target {
                            ClickTarget::SettingsClose => {
                                self.show_settings = false;
                                return Ok(());
                            }
                            ClickTarget::SettingsTab(idx) => {
                                self.settings_state.active_tab = SettingsTab::from_index(*idx);
                                self.settings_state.reset_tab_state();
                                return Ok(());
                            }
                            ClickTarget::SettingsToggle(id) => {
                                self.handle_settings_toggle_click(id);
                                return Ok(());
                            }
                            ClickTarget::SettingsRadio(id, option_idx) => {
                                self.handle_settings_radio_click(id, *option_idx);
                                return Ok(());
                            }
                            ClickTarget::ModelsProvider(provider) => {
                                // Toggle provider expansion
                                if self
                                    .settings_state
                                    .models_expanded_providers
                                    .contains(provider)
                                {
                                    self.settings_state
                                        .models_expanded_providers
                                        .remove(provider);
                                } else {
                                    self.settings_state
                                        .models_expanded_providers
                                        .insert(provider.clone());
                                }
                                return Ok(());
                            }
                            ClickTarget::ModelsItem(model_name) => {
                                self.handle_settings_model_click(model_name);
                                return Ok(());
                            }
                            ClickTarget::DefaultModelItem(idx) => {
                                // Set the model as default and close dropdown
                                let available_models = self.model_registry.list_available(&self.db);
                                if let Some(model_name) = available_models.get(*idx) {
                                    self.current_model = model_name.clone();
                                    let settings = Settings::new(&self.db);
                                    let _ =
                                        settings.set(spot_core::config::keys::MODEL, model_name);
                                    self.update_context_usage();
                                }
                                self.settings_state.default_model_dropdown_open = false;
                                self.settings_state.default_model_index = *idx;
                                return Ok(());
                            }
                            ClickTarget::PinnedAgentItem(idx) => {
                                self.settings_state.agent_list_index = *idx;
                                self.settings_state.pinned_panel = PinnedAgentsPanel::Agents;
                                self.update_selected_agent_from_index();
                                return Ok(());
                            }
                            ClickTarget::PinnedModelItem(idx) => {
                                self.settings_state.model_list_index = *idx;
                                self.settings_state.pinned_panel = PinnedAgentsPanel::Models;
                                // Trigger model pinning
                                self.handle_settings_enter_key();
                                return Ok(());
                            }
                            ClickTarget::McpServerItem(idx) => {
                                self.settings_state.mcp_server_index = *idx;
                                self.settings_state.mcp_panel = McpPanel::Servers;
                                return Ok(());
                            }
                            ClickTarget::McpAgentItem(idx) => {
                                self.settings_state.mcp_agent_index = *idx;
                                self.settings_state.mcp_panel = McpPanel::Agents;
                                return Ok(());
                            }
                            ClickTarget::McpCheckbox(idx) => {
                                self.settings_state.mcp_checkbox_index = *idx;
                                self.settings_state.mcp_panel = McpPanel::McpCheckboxes;
                                // Trigger checkbox toggle
                                self.handle_settings_enter_key();
                                return Ok(());
                            }
                            _ => {} // Other targets fall through
                        }
                    }
                    // If settings is open but no settings target hit, absorb the click
                    return Ok(());
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
                            if let Err(e) = settings.set(spot_core::config::keys::MODEL, &name) {
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
                        ClickTarget::SettingsButton => {
                            self.show_settings = !self.show_settings;
                            self.show_agent_dropdown = false;
                            self.show_folder_modal = false;
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
                        if self.show_settings {
                            // Scroll up in settings - decrease selection index
                            self.handle_settings_up_key();
                        } else {
                            // Scroll both for now (activity feed takes precedence)
                            self.activity_scroll_up(3);
                            self.message_list_state.scroll_up(3);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if self.show_settings {
                            // Scroll down in settings - increase selection index
                            self.handle_settings_down_key();
                        } else {
                            self.activity_scroll_down(3);
                            self.message_list_state.scroll_down(3);
                        }
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

    /// Send the current input as a message
    pub(super) async fn send_message(&mut self) -> Result<()> {
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

        // Prepare for execution
        let agent_name = self.current_agent.clone();
        let prompt = final_content;
        let history = self.message_history.clone();
        let model_name = self.effective_model_for_agent(&self.current_agent);
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

    /// Add a user message to activities
    pub(super) fn add_user_activity(&mut self, content: &str) {
        self.activities.push(Activity::user_message(content));
        self.activity_scroll_to_bottom();
    }
}
