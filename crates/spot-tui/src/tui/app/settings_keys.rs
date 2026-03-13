//! Settings panel keyboard and mouse interaction handlers.

use super::TuiApp;
use crate::tui::settings::ModelSettingsField;
use spot_core::config::Settings;

impl TuiApp {
    /// Handle Tab key in settings (switch tabs or panels)
    pub(super) fn handle_settings_tab_key(&mut self, shift: bool) {
        use crate::tui::settings::SettingsTab;

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
    pub(super) fn handle_settings_up_key(&mut self) {
        use crate::tui::settings::{PinnedAgentsPanel, SettingsTab};

        match self.settings_state.active_tab {
            SettingsTab::General => {
                if self.settings_state.selected_index > 0 {
                    self.settings_state.selected_index -= 1;
                }
            }
            SettingsTab::PinnedAgents => match self.settings_state.pinned_panel {
                PinnedAgentsPanel::DefaultModel => {
                    if self.settings_state.default_model_dropdown_open {
                        // Navigate within dropdown
                        if self.settings_state.default_model_index > 0 {
                            self.settings_state.default_model_index -= 1;
                        }
                    }
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
                // If a model is expanded, navigate within its settings
                if self.settings_state.expanded_model.is_some() {
                    self.settings_state.model_settings_field =
                        self.settings_state.model_settings_field.next();
                    return;
                }

                if self.settings_state.models_in_oauth_section {
                    // Navigate within OAuth section (3 providers)
                    if self.settings_state.oauth_selected_index > 0 {
                        self.settings_state.oauth_selected_index -= 1;
                    }
                } else if self.settings_state.models_selected_index > 0 {
                    self.settings_state.models_selected_index -= 1;
                } else {
                    // At top of models list, go back to OAuth section
                    self.settings_state.models_in_oauth_section = true;
                }
            }
            SettingsTab::McpServers => {
                use crate::tui::settings::McpPanel;
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
    pub(super) fn handle_settings_down_key(&mut self) {
        use crate::tui::settings::{PinnedAgentsPanel, SettingsTab};

        match self.settings_state.active_tab {
            SettingsTab::General => {
                // Max index depends on whether compression is enabled
                // Indices: 0=PDF, 1=UserMode, 2=Reasoning, 3=YOLO, 4=CompressionToggle
                // If compression enabled: 5=Strategy, 6=Threshold, 7=Target
                let settings = Settings::new(&self.db);
                let max_index = if settings.get_compression_enabled() {
                    7
                } else {
                    4
                };
                if self.settings_state.selected_index < max_index {
                    self.settings_state.selected_index += 1;
                }
            }
            SettingsTab::PinnedAgents => match self.settings_state.pinned_panel {
                PinnedAgentsPanel::DefaultModel => {
                    if self.settings_state.default_model_dropdown_open {
                        // Navigate within dropdown
                        let available_models = self.model_registry.list_available(&self.db);
                        let max_idx = available_models.len().saturating_sub(1);
                        if self.settings_state.default_model_index < max_idx {
                            self.settings_state.default_model_index += 1;
                        }
                    }
                }
                PinnedAgentsPanel::Agents => {
                    let agent_count = self.agents.list().len();
                    if agent_count > 0 && self.settings_state.agent_list_index < agent_count - 1 {
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
                // If a model is expanded, navigate within its settings
                if self.settings_state.expanded_model.is_some() {
                    self.settings_state.model_settings_field =
                        self.settings_state.model_settings_field.next();
                    return;
                }

                if self.settings_state.models_in_oauth_section {
                    // Navigate within OAuth section (3 providers: 0=Claude, 1=ChatGPT, 2=Google)
                    if self.settings_state.oauth_selected_index < 2 {
                        self.settings_state.oauth_selected_index += 1;
                    } else {
                        // At bottom of OAuth section, go to models list
                        self.settings_state.models_in_oauth_section = false;
                        self.settings_state.models_selected_index = 0;
                    }
                } else {
                    let available_models = self.model_registry.list_available(&self.db);
                    let max_index =
                        crate::tui::settings::models::count_models_items(self, &available_models);
                    if max_index > 0 && self.settings_state.models_selected_index < max_index - 1 {
                        self.settings_state.models_selected_index += 1;
                    }
                }
            }
            SettingsTab::McpServers => {
                use crate::tui::settings::McpPanel;
                match self.settings_state.mcp_panel {
                    McpPanel::Servers => {
                        let server_count = crate::tui::settings::mcp_servers::server_count();
                        if server_count > 0
                            && self.settings_state.mcp_server_index < server_count - 1
                        {
                            self.settings_state.mcp_server_index += 1;
                        }
                    }
                    McpPanel::Agents => {
                        let agent_count = self.agents.list().len();
                        if agent_count > 0 && self.settings_state.mcp_agent_index < agent_count - 1
                        {
                            self.settings_state.mcp_agent_index += 1;
                            self.settings_state.mcp_checkbox_index = 0;
                        }
                    }
                    McpPanel::McpCheckboxes => {
                        let enabled_count =
                            crate::tui::settings::mcp_servers::enabled_server_count();
                        if enabled_count > 0
                            && self.settings_state.mcp_checkbox_index < enabled_count - 1
                        {
                            self.settings_state.mcp_checkbox_index += 1;
                        }
                    }
                }
            }
        }
    }

    /// Handle Enter key in settings
    pub(super) fn handle_settings_enter_key(&mut self) {
        use crate::tui::settings::{PinnedAgentsPanel, SettingsTab};
        use spot_core::agents::UserMode;
        use spot_core::config::PdfMode;

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
                        self.user_mode = new_mode;
                    }
                    2 => {
                        // Show Reasoning - toggle
                        let current = settings.get_bool("show_reasoning").unwrap_or(true);
                        let _ =
                            settings.set("show_reasoning", if current { "false" } else { "true" });
                    }
                    3 => {
                        // YOLO Mode - toggle
                        let current = settings.yolo_mode();
                        let _ = settings.set_yolo_mode(!current);
                    }
                    4 => {
                        // Compression Enabled - toggle
                        let current = settings.get_compression_enabled();
                        settings.set_compression_enabled(!current);
                    }
                    5 => {
                        // Compression Strategy - toggle between truncate/summarize
                        let current = settings.get_compression_strategy();
                        let new_strategy = if current == "truncate" {
                            "summarize"
                        } else {
                            "truncate"
                        };
                        settings.set_compression_strategy(new_strategy);
                    }
                    6 => {
                        // Compression Threshold - cycle through options
                        let thresholds = [0.50, 0.65, 0.75, 0.85, 0.95];
                        let current = settings.get_compression_threshold();
                        let idx = thresholds
                            .iter()
                            .position(|&t| (t - current).abs() < 0.01)
                            .unwrap_or(2);
                        let new_idx = (idx + 1) % thresholds.len();
                        settings.set_compression_threshold(thresholds[new_idx]);
                    }
                    7 => {
                        // Compression Target - cycle through options
                        let targets = [10_000usize, 20_000, 30_000, 50_000, 75_000];
                        let current = settings.get_compression_target_tokens();
                        let idx = targets.iter().position(|&t| t == current).unwrap_or(2);
                        let new_idx = (idx + 1) % targets.len();
                        settings.set_compression_target_tokens(targets[new_idx]);
                    }
                    _ => {}
                }
            }
            SettingsTab::PinnedAgents => {
                match self.settings_state.pinned_panel {
                    PinnedAgentsPanel::DefaultModel => {
                        if self.settings_state.default_model_dropdown_open {
                            // Select the model and close dropdown
                            let available_models = self.model_registry.list_available(&self.db);
                            if let Some(model_name) =
                                available_models.get(self.settings_state.default_model_index)
                            {
                                self.current_model = model_name.clone();
                                let _ = settings.set(spot_core::config::keys::MODEL, model_name);
                                self.update_context_usage();
                            }
                            self.settings_state.default_model_dropdown_open = false;
                        } else {
                            // Open the dropdown
                            self.settings_state.default_model_dropdown_open = true;
                        }
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
                    // Start OAuth flow for selected provider
                    self.start_oauth_flow();
                    return;
                }

                let available_models = self.model_registry.list_available(&self.db);
                let selected_index = self.settings_state.models_selected_index;

                // If we're in an expanded model, handle field editing
                if self.settings_state.expanded_model.is_some() {
                    if self.settings_state.model_settings_editing {
                        // Exit edit mode and save
                        self.settings_state.model_settings_editing = false;
                        self.save_current_model_settings();
                    } else {
                        // Enter edit mode for current field
                        self.settings_state.model_settings_editing = true;
                    }
                    return;
                }

                // Check if it's a group header (to expand/collapse)
                if let Some(type_label) = crate::tui::settings::models::is_group_header(
                    self,
                    &available_models,
                    selected_index,
                ) {
                    // Toggle expanded state
                    if self
                        .settings_state
                        .models_expanded_providers
                        .contains(&type_label)
                    {
                        self.settings_state
                            .models_expanded_providers
                            .remove(&type_label);
                    } else {
                        self.settings_state
                            .models_expanded_providers
                            .insert(type_label);
                    }
                } else if let Some(model_name) = crate::tui::settings::models::get_model_at_index(
                    self,
                    &available_models,
                    selected_index,
                ) {
                    // Check if model is expandable (non-OAuth)
                    if crate::tui::settings::models::is_model_expandable(
                        self,
                        &available_models,
                        selected_index,
                    ) {
                        // Toggle model expansion
                        if self.settings_state.expanded_model.as_deref() == Some(&model_name) {
                            // Collapse and save any changes
                            self.save_current_model_settings();
                            self.settings_state.expanded_model = None;
                            self.settings_state.model_settings_editing = false;
                        } else {
                            // Expand and load settings
                            self.load_model_settings(&model_name);
                            self.settings_state.expanded_model = Some(model_name);
                            self.settings_state.model_settings_field =
                                ModelSettingsField::Temperature;
                            self.settings_state.model_settings_editing = false;
                        }
                    }
                }
            }
            SettingsTab::McpServers => {
                use crate::tui::settings::McpPanel;
                match self.settings_state.mcp_panel {
                    McpPanel::Servers => {
                        // Toggle enable/disable of selected server
                        crate::tui::settings::mcp_servers::toggle_server_enabled(
                            self.settings_state.mcp_server_index,
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

                            if let Some(mcp_name) =
                                crate::tui::settings::mcp_servers::get_enabled_server_name(
                                    checkbox_idx,
                                )
                            {
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

    /// Handle toggle click in General settings tab
    pub(super) fn handle_settings_toggle_click(&mut self, id: &str) {
        let settings = Settings::new(&self.db);
        match id {
            "show_reasoning" => {
                let current = settings.get_bool("show_reasoning").unwrap_or(true);
                let _ = settings.set("show_reasoning", if current { "false" } else { "true" });
            }
            "yolo_mode" => {
                let current = settings.yolo_mode();
                let _ = settings.set_yolo_mode(!current);
            }
            "compression.enabled" => {
                let current = settings.get_compression_enabled();
                settings.set_compression_enabled(!current);
            }
            _ => {}
        }
    }

    /// Handle radio button click in General settings tab
    pub(super) fn handle_settings_radio_click(&mut self, id: &str, option: usize) {
        use spot_core::agents::UserMode;
        use spot_core::config::PdfMode;

        let settings = Settings::new(&self.db);
        match id {
            "pdf_mode" => {
                let mode = if option == 0 {
                    PdfMode::Image
                } else {
                    PdfMode::TextExtract
                };
                let _ = settings.set_pdf_mode(mode);
            }
            "user_mode" => {
                let mode = match option {
                    0 => UserMode::Normal,
                    1 => UserMode::Expert,
                    2 => UserMode::Developer,
                    _ => UserMode::Normal,
                };
                let _ = settings.set_user_mode(mode);
                self.user_mode = mode;
            }
            "compression.strategy" => {
                let strategy = if option == 0 { "truncate" } else { "summarize" };
                settings.set_compression_strategy(strategy);
            }
            "compression.threshold" => {
                let thresholds = [0.50, 0.65, 0.75, 0.85, 0.95];
                if let Some(&threshold) = thresholds.get(option) {
                    settings.set_compression_threshold(threshold);
                }
            }
            "compression.target_tokens" => {
                let targets = [10_000, 20_000, 30_000, 50_000, 75_000];
                if let Some(&tokens) = targets.get(option) {
                    settings.set_compression_target_tokens(tokens);
                }
            }
            _ => {}
        }
    }

    /// Handle model click in Models settings tab
    pub(super) fn handle_settings_model_click(&mut self, model_name: &str) {
        let settings = Settings::new(&self.db);
        let _ = settings.set(spot_core::config::keys::MODEL, model_name);
        self.current_model = model_name.to_string();
        self.update_context_usage();
    }

    /// Update selected agent from current index in Pinned Agents tab
    pub(super) fn update_selected_agent_from_index(&mut self) {
        let agents = self.agents.list();
        if let Some(agent) = agents.get(self.settings_state.agent_list_index) {
            self.settings_state.selected_agent = Some(agent.name.clone());
        }
    }

    /// Handle Left key in settings
    pub(super) fn handle_settings_left_key(&mut self) {
        use crate::tui::settings::SettingsTab;
        use spot_core::agents::UserMode;
        use spot_core::config::PdfMode;

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
                        self.user_mode = new_mode;
                    }
                    // Index 2 = Show Reasoning (toggle), Index 3 = YOLO Mode (toggle)
                    // Index 4 = Compression Toggle
                    5 => {
                        // Compression Strategy: toggle between truncate/summarize
                        let current = settings.get_compression_strategy();
                        let new_strategy = if current == "truncate" {
                            "summarize"
                        } else {
                            "truncate"
                        };
                        settings.set_compression_strategy(new_strategy);
                    }
                    6 => {
                        // Compression Threshold: cycle backwards
                        let thresholds = [0.50, 0.65, 0.75, 0.85, 0.95];
                        let current = settings.get_compression_threshold();
                        let idx = thresholds
                            .iter()
                            .position(|&t| (t - current).abs() < 0.01)
                            .unwrap_or(2);
                        let new_idx = if idx == 0 {
                            thresholds.len() - 1
                        } else {
                            idx - 1
                        };
                        settings.set_compression_threshold(thresholds[new_idx]);
                    }
                    7 => {
                        // Compression Target: cycle backwards
                        let targets = [10_000usize, 20_000, 30_000, 50_000, 75_000];
                        let current = settings.get_compression_target_tokens();
                        let idx = targets.iter().position(|&t| t == current).unwrap_or(2);
                        let new_idx = if idx == 0 { targets.len() - 1 } else { idx - 1 };
                        settings.set_compression_target_tokens(targets[new_idx]);
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
    pub(super) fn handle_settings_right_key(&mut self) {
        use crate::tui::settings::SettingsTab;
        use spot_core::agents::UserMode;
        use spot_core::config::PdfMode;

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
                        self.user_mode = new_mode;
                    }
                    // Index 2 = Show Reasoning (toggle), Index 3 = YOLO Mode (toggle)
                    // Index 4 = Compression Toggle
                    5 => {
                        // Compression Strategy: toggle between truncate/summarize
                        let current = settings.get_compression_strategy();
                        let new_strategy = if current == "truncate" {
                            "summarize"
                        } else {
                            "truncate"
                        };
                        settings.set_compression_strategy(new_strategy);
                    }
                    6 => {
                        // Compression Threshold: cycle forwards
                        let thresholds = [0.50, 0.65, 0.75, 0.85, 0.95];
                        let current = settings.get_compression_threshold();
                        let idx = thresholds
                            .iter()
                            .position(|&t| (t - current).abs() < 0.01)
                            .unwrap_or(2);
                        let new_idx = (idx + 1) % thresholds.len();
                        settings.set_compression_threshold(thresholds[new_idx]);
                    }
                    7 => {
                        // Compression Target: cycle forwards
                        let targets = [10_000usize, 20_000, 30_000, 50_000, 75_000];
                        let current = settings.get_compression_target_tokens();
                        let idx = targets.iter().position(|&t| t == current).unwrap_or(2);
                        let new_idx = (idx + 1) % targets.len();
                        settings.set_compression_target_tokens(targets[new_idx]);
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
    pub(super) fn handle_settings_delete_key(&mut self) {
        use crate::tui::settings::{McpPanel, SettingsTab};

        match self.settings_state.active_tab {
            SettingsTab::McpServers if self.settings_state.mcp_panel == McpPanel::Servers => {
                // Remove the selected MCP server
                let server_count = crate::tui::settings::mcp_servers::server_count();
                if server_count > 0 {
                    crate::tui::settings::mcp_servers::remove_server(
                        self.settings_state.mcp_server_index,
                    );
                    // Adjust selection if needed
                    if self.settings_state.mcp_server_index >= server_count.saturating_sub(1) {
                        self.settings_state.mcp_server_index = server_count.saturating_sub(2);
                    }
                }
            }
            SettingsTab::Models if !self.settings_state.models_in_oauth_section => {
                // Delete the selected model (if it's a model, not a group header)
                let available_models = self.model_registry.list_available(&self.db);
                let selected_index = self.settings_state.models_selected_index;

                if let Some(model_name) = crate::tui::settings::models::get_model_at_index(
                    self,
                    &available_models,
                    selected_index,
                ) {
                    // Check if it's an OAuth model (can't delete those)
                    let is_oauth = self
                        .model_registry
                        .get(&model_name)
                        .map(|c| c.is_oauth())
                        .unwrap_or(false);

                    if !is_oauth {
                        // Delete the model
                        if let Err(e) = spot_core::models::ModelRegistry::remove_model_from_db(
                            &self.db,
                            &model_name,
                        ) {
                            tracing::warn!("Failed to delete model {}: {}", model_name, e);
                            return;
                        }

                        // Clean up any agent pins referencing this model
                        let settings = Settings::new(&self.db);
                        if let Err(e) = settings.clear_pins_for_model(&model_name) {
                            tracing::warn!(
                                "Failed to clear pins for deleted model {}: {}",
                                model_name,
                                e
                            );
                        }

                        // Reload model registry
                        if let Ok(registry) =
                            spot_core::models::ModelRegistry::load_from_db(&self.db)
                        {
                            self.model_registry = std::sync::Arc::new(registry);
                        }

                        // Adjust selection if needed
                        let new_count = crate::tui::settings::models::count_models_items(
                            self,
                            &self.model_registry.list_available(&self.db),
                        );
                        if self.settings_state.models_selected_index >= new_count && new_count > 0 {
                            self.settings_state.models_selected_index = new_count - 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Handle 'k' key to open key pool management in Models tab
    pub(super) fn handle_settings_key_pool_open(&mut self) {
        use crate::tui::settings::SettingsTab;

        // Only works in Models tab
        if self.settings_state.active_tab != SettingsTab::Models {
            return;
        }

        // If a model is expanded, use that model's provider
        if let Some(ref expanded_model) = self.settings_state.expanded_model.clone() {
            // Get the model config to find its provider
            if let Some(config) = self.model_registry.get(expanded_model) {
                // Check for api_key environment variable in custom endpoint
                let api_key_env = config
                    .custom_endpoint
                    .as_ref()
                    .and_then(|e| e.api_key.as_ref())
                    .and_then(|k| {
                        if k.starts_with('$') {
                            Some(
                                k.trim_start_matches('$')
                                    .trim_matches(|c| c == '{' || c == '}')
                                    .to_string(),
                            )
                        } else {
                            None
                        }
                    });

                if let Some(env_var) = api_key_env {
                    // Load keys from database
                    let keys = self.db.get_pool_keys(&env_var).unwrap_or_default();
                    // Open the key pool overlay
                    self.settings_state.key_pool.open(&env_var, &env_var, keys);
                    return;
                }

                // Fallback: try to get provider from model type
                let provider = match config.model_type {
                    spot_core::models::ModelType::Openai => Some(("openai", "OpenAI")),
                    spot_core::models::ModelType::Anthropic => Some(("anthropic", "Anthropic")),
                    spot_core::models::ModelType::Gemini => Some(("gemini", "Google Gemini")),
                    spot_core::models::ModelType::AzureOpenai => {
                        Some(("azure_openai", "Azure OpenAI"))
                    }
                    spot_core::models::ModelType::Openrouter => Some(("openrouter", "OpenRouter")),
                    _ => None, // OAuth and custom providers without api_key_env
                };

                if let Some((provider_key, display_name)) = provider {
                    let keys = self.db.get_pool_keys(provider_key).unwrap_or_default();
                    self.settings_state
                        .key_pool
                        .open(provider_key, display_name, keys);
                }
            }
            return;
        }

        // Fallback to original behavior when no model is expanded
        if self.settings_state.models_in_oauth_section {
            return;
        }

        // Get the available models
        let available_models = self.model_registry.list_available(&self.db);
        let selected_index = self.settings_state.models_selected_index;

        // Get the type label for the current selection
        if let Some(type_label) = crate::tui::settings::models::get_current_type_label(
            self,
            &available_models,
            selected_index,
        ) {
            // Check if this provider supports API key pools
            if let Some((provider, display_name)) =
                crate::tui::settings::models::provider_for_type_label(&type_label)
            {
                // Load keys from database
                let keys = self.db.get_pool_keys(provider).unwrap_or_default();

                // Open the key pool overlay
                self.settings_state
                    .key_pool
                    .open(provider, display_name, keys);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Model Settings Methods
    // ─────────────────────────────────────────────────────────────────────────

    /// Load model settings into edit state
    pub(super) fn load_model_settings(&mut self, model_name: &str) {
        use spot_core::models::settings::ModelSettings;

        let settings = ModelSettings::load(&self.db, model_name).unwrap_or_default();

        self.settings_state.model_temp_value = settings
            .temperature
            .map(|t| format!("{:.1}", t))
            .unwrap_or_else(|| "0.7".to_string());

        self.settings_state.model_top_p_value = settings
            .top_p
            .map(|t| {
                let s = format!("{:.3}", t);
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            })
            .unwrap_or_else(|| "1.0".to_string());
    }

    /// Save current model settings from edit state
    pub(super) fn save_current_model_settings(&mut self) {
        use spot_core::models::settings::ModelSettings;

        let Some(model_name) = self.settings_state.expanded_model.clone() else {
            return;
        };

        // Save temperature
        if !self.settings_state.model_temp_value.is_empty() {
            if let Err(e) = ModelSettings::save_setting(
                &self.db,
                &model_name,
                "temperature",
                &self.settings_state.model_temp_value,
            ) {
                tracing::warn!("Failed to save temperature: {}", e);
            }
        }

        // Save top_p
        if !self.settings_state.model_top_p_value.is_empty() {
            if let Err(e) = ModelSettings::save_setting(
                &self.db,
                &model_name,
                "top_p",
                &self.settings_state.model_top_p_value,
            ) {
                tracing::warn!("Failed to save top_p: {}", e);
            }
        }
    }

    /// Handle character input for model settings editing
    pub(super) fn handle_model_settings_char(&mut self, c: char) {
        if !self.settings_state.model_settings_editing {
            return;
        }

        // Only allow digits and decimal point
        if !c.is_ascii_digit() && c != '.' {
            return;
        }

        match self.settings_state.model_settings_field {
            ModelSettingsField::Temperature => {
                // Validate: max 3 chars (e.g., "2.0")
                if self.settings_state.model_temp_value.len() < 4 {
                    self.settings_state.model_temp_value.push(c);
                }
            }
            ModelSettingsField::TopP => {
                // Validate: max 5 chars (e.g., "0.951")
                if self.settings_state.model_top_p_value.len() < 5 {
                    self.settings_state.model_top_p_value.push(c);
                }
            }
        }
    }

    /// Handle backspace for model settings editing
    pub(super) fn handle_model_settings_backspace(&mut self) {
        if !self.settings_state.model_settings_editing {
            return;
        }

        match self.settings_state.model_settings_field {
            ModelSettingsField::Temperature => {
                self.settings_state.model_temp_value.pop();
            }
            ModelSettingsField::TopP => {
                self.settings_state.model_top_p_value.pop();
            }
        }
    }

    /// Handle Tab key in model settings to cycle fields
    pub(super) fn handle_model_settings_tab(&mut self) {
        if self.settings_state.expanded_model.is_some() {
            // Save current field before switching
            if self.settings_state.model_settings_editing {
                self.settings_state.model_settings_editing = false;
            }
            self.settings_state.model_settings_field =
                self.settings_state.model_settings_field.next();
        }
    }
}
