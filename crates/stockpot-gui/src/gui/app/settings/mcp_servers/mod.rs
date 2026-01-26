//! MCP servers settings tab.
//!
//! Manages MCP server configuration and agent-to-MCP attachments.
//!
//! This module is organized into subcomponents:
//! - `import_section`: Top import button section
//! - `server_list`: Left panel showing MCP servers
//! - `agent_assignments`: Right panel for agent-to-MCP attachments
//! - `import_dialog`: Modal dialog for JSON import

mod agent_assignments;
mod import_dialog;
mod import_section;
mod server_list;

use gpui::{div, prelude::*, px, Context, Styled};

use crate::gui::app::ChatApp;
use stockpot_core::config::Settings;

impl ChatApp {
    /// Renders the MCP servers settings tab.
    ///
    /// This tab provides:
    /// - Import button to add servers from JSON
    /// - Left panel: List of defined MCP servers with controls
    /// - Right panel: Agent-to-MCP assignment interface
    pub(crate) fn render_settings_mcp_servers(&self, cx: &Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let agents = self.agents.list();
        let selected_agent = self.mcp_settings_selected_agent.clone();

        // Load server data
        let servers = server_list::load_servers();

        // Load agent MCP attachments
        let settings = Settings::new(&self.db);
        let all_attachments = settings.get_all_agent_mcps().unwrap_or_default();
        let agent_mcps = settings.get_agent_mcps(&selected_agent);

        // Build the UI
        let import_section = import_section::render_import_section(&theme, cx);
        let left_panel = server_list::render_server_list(&theme, cx, &servers);
        let right_panel = agent_assignments::render_agent_assignments(
            &theme,
            cx,
            &agents,
            &selected_agent,
            &servers,
            &agent_mcps,
            &all_attachments,
        );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .child(import_section)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(left_panel)
                    .child(right_panel),
            )
    }

    /// Renders the MCP import dialog overlay.
    ///
    /// Shows a modal dialog for pasting and importing MCP server
    /// configurations from JSON format (Claude Desktop compatible).
    pub(crate) fn render_mcp_import_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        import_dialog::render_import_dialog(
            &self.theme,
            cx,
            self.show_mcp_import_dialog,
            &self.mcp_import_json,
            self.mcp_import_error.as_ref(),
        )
    }
}
