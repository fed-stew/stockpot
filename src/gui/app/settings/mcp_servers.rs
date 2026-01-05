//! MCP servers settings tab
//!
//! Manages MCP server configuration and agent-to-MCP attachments.

use gpui::{div, prelude::*, px, rgb, rgba, Context, MouseButton, SharedString, Styled};

use crate::config::Settings;
use crate::gui::app::ChatApp;

impl ChatApp {
    pub(crate) fn render_settings_mcp_servers(&self, cx: &Context<Self>) -> impl IntoElement {
        use crate::mcp::McpConfig;

        let theme = self.theme.clone();
        let agents = self.agents.list();
        let selected_agent = self.mcp_settings_selected_agent.clone();

        // Load MCP config
        let mcp_config = McpConfig::load_or_default();
        let mut servers: Vec<(String, bool, Option<String>, String)> = mcp_config
            .servers
            .iter()
            .map(|(name, entry)| {
                let cmd_preview = format!("{} {}", entry.command, entry.args.join(" "));
                (
                    name.clone(),
                    entry.enabled,
                    entry.description.clone(),
                    cmd_preview,
                )
            })
            .collect();
        servers.sort_by(|a, b| a.0.cmp(&b.0));

        // Load agent MCP attachments
        let settings = Settings::new(&self.db);
        let all_attachments = settings.get_all_agent_mcps().unwrap_or_default();
        let agent_mcps = settings.get_agent_mcps(&selected_agent);

        // Top section: Import button
        let import_section = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .mb(px(16.))
            .pb(px(16.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .id("import-mcp-json")
                    .px(px(16.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .bg(theme.accent)
                    .text_color(rgb(0xffffff))
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|s| s.opacity(0.9))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.show_mcp_import_dialog = true;
                            this.mcp_import_json.clear();
                            this.mcp_import_error = None;
                            cx.notify();
                        }),
                    )
                    .child("📋 Import from JSON"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .child("Paste Claude Desktop / standard MCP config format"),
            );

        // Left panel: MCP Servers list
        let left_panel = div()
            .flex()
            .flex_col()
            .w(px(380.))
            .min_h(px(0.))
            .pr(px(20.))
            .border_r_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mb(px(12.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text)
                            .child("🔌 MCP Servers"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.text_muted)
                            .child(format!("{} defined", servers.len())),
                    ),
            )
            .child(
                div()
                    .id("mcp-servers-list")
                    .flex_1()
                    .min_h(px(0.))
                    .max_h(px(400.))
                    .overflow_y_scroll()
                    .scrollbar_width(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .when(servers.is_empty(), |d| {
                        d.child(
                            div()
                                .px(px(16.))
                                .py(px(24.))
                                .rounded(px(8.))
                                .bg(theme.tool_card)
                                .text_size(px(13.))
                                .text_color(theme.text_muted)
                                .text_center()
                                .child("No MCP servers defined.\nClick 'Import from JSON' to add servers."),
                        )
                    })
                    .children(servers.iter().map(|(name, enabled, desc, cmd_preview)| {
                        let server_name = name.clone();
                        let server_name_del = name.clone();
                        let is_enabled = *enabled;
                        let description = desc.clone();
                        let cmd = cmd_preview.clone();

                        div()
                            .id(SharedString::from(format!("mcp-server-{}", server_name)))
                            .p(px(12.))
                            .rounded(px(8.))
                            .bg(theme.tool_card)
                            .border_l_2()
                            .border_color(if is_enabled { rgb(0x4ade80) } else { theme.border })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .text_size(px(14.))
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .text_color(theme.text)
                                                    .child(name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .px(px(6.))
                                                    .py(px(2.))
                                                    .rounded(px(4.))
                                                    .text_size(px(10.))
                                                    .bg(if is_enabled { rgba(0x4ade8033) } else { theme.background })
                                                    .text_color(if is_enabled { rgb(0x4ade80) } else { theme.text_muted })
                                                    .child(if is_enabled { "enabled" } else { "disabled" }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.))
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!("toggle-mcp-{}", server_name)))
                                                    .px(px(8.))
                                                    .py(px(4.))
                                                    .rounded(px(4.))
                                                    .text_size(px(11.))
                                                    .text_color(theme.text_muted)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(theme.background).text_color(theme.text))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |_this, _, _, cx| {
                                                            let mut config = McpConfig::load_or_default();
                                                            if let Some(entry) = config.servers.get_mut(&server_name) {
                                                                entry.enabled = !entry.enabled;
                                                                let _ = config.save_default();
                                                            }
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .child(if is_enabled { "disable" } else { "enable" }),
                                            )
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!("delete-mcp-{}", server_name_del)))
                                                    .px(px(8.))
                                                    .py(px(4.))
                                                    .rounded(px(4.))
                                                    .text_size(px(11.))
                                                    .text_color(theme.text_muted)
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(rgba(0xff6b6b22)).text_color(rgb(0xff6b6b)))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |_this, _, _, cx| {
                                                            let mut config = McpConfig::load_or_default();
                                                            config.remove_server(&server_name_del);
                                                            let _ = config.save_default();
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .child("×"),
                                            ),
                                    ),
                            )
                            .when_some(description, |d, desc| {
                                d.child(
                                    div()
                                        .mt(px(6.))
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child(desc),
                                )
                            })
                            .child(
                                div()
                                    .mt(px(8.))
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(4.))
                                    .bg(theme.background)
                                    .text_size(px(11.))
                                    .text_color(theme.text_muted)
                                    .overflow_hidden()
                                    .child(Self::truncate_text(&cmd, 50)),
                            )
                    })),
            );

        // Right panel: Agent MCP assignments
        let right_panel = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .pl(px(20.))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.text)
                    .mb(px(12.))
                    .child("🤖 Agent → MCP Attachments"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.text_muted)
                    .mb(px(12.))
                    .child("Select an agent, then check which MCPs it should use."),
            )
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .flex_1()
                    .min_h(px(0.))
                    // Agent list
                    .child(
                        div()
                            .w(px(200.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.text_muted)
                                    .mb(px(4.))
                                    .child("Agents"),
                            )
                            .child(
                                div()
                                    .id("mcp-agents-scroll")
                                    .flex_1()
                                    .min_h(px(0.))
                                    .max_h(px(300.))
                                    .overflow_y_scroll()
                                    .scrollbar_width(px(6.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .children(agents.iter().map(|info| {
                                        let is_selected = info.name == selected_agent;
                                        let agent_mcp_list = all_attachments.get(&info.name);
                                        let mcp_count =
                                            agent_mcp_list.map(|m| m.len()).unwrap_or(0);

                                        let agent_name = info.name.clone();
                                        div()
                                            .id(SharedString::from(format!(
                                                "mcp-agent-{}",
                                                agent_name
                                            )))
                                            .px(px(10.))
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
                                            .cursor_pointer()
                                            .hover(|s| s.opacity(0.9))
                                            .on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.mcp_settings_selected_agent =
                                                        agent_name.clone();
                                                    cx.notify();
                                                }),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .justify_between()
                                                    .child(
                                                        div()
                                                            .text_size(px(12.))
                                                            .child(info.display_name.clone()),
                                                    )
                                                    .when(mcp_count > 0, |d| {
                                                        d.child(
                                                            div()
                                                                .px(px(6.))
                                                                .py(px(2.))
                                                                .rounded(px(10.))
                                                                .bg(if is_selected {
                                                                    rgba(0xffffff33)
                                                                } else {
                                                                    theme.background
                                                                })
                                                                .text_size(px(10.))
                                                                .text_color(if is_selected {
                                                                    rgb(0xffffff)
                                                                } else {
                                                                    theme.text_muted
                                                                })
                                                                .child(format!("{}", mcp_count)),
                                                        )
                                                    }),
                                            )
                                    })),
                            ),
                    )
                    // MCP checkboxes for selected agent
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.text_muted)
                                    .mb(px(4.))
                                    .child(format!("MCPs for {}", selected_agent)),
                            )
                            .when(servers.is_empty(), |d| {
                                d.child(
                                    div()
                                        .py(px(16.))
                                        .text_size(px(12.))
                                        .text_color(theme.text_muted)
                                        .child("No MCPs available yet."),
                                )
                            })
                            .child(
                                div()
                                    .id("mcp-checkboxes-scroll")
                                    .flex_1()
                                    .min_h(px(0.))
                                    .max_h(px(300.))
                                    .overflow_y_scroll()
                                    .scrollbar_width(px(6.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .children(
                                        servers.iter().filter(|(_, enabled, _, _)| *enabled).map(
                                            |(name, _, _, _)| {
                                                let mcp_name = name.clone();
                                                let is_attached = agent_mcps.contains(&mcp_name);
                                                let selected_agent = selected_agent.clone();

                                                div()
                                                    .id(SharedString::from(format!(
                                                        "attach-mcp-{}",
                                                        mcp_name
                                                    )))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(10.))
                                                    .px(px(10.))
                                                    .py(px(8.))
                                                    .rounded(px(6.))
                                                    .bg(if is_attached {
                                                        theme.accent
                                                    } else {
                                                        theme.tool_card
                                                    })
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.9))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(move |this, _, _, cx| {
                                                            let settings = Settings::new(&this.db);
                                                            if is_attached {
                                                                let _ = settings.remove_agent_mcp(
                                                                    &selected_agent,
                                                                    &mcp_name,
                                                                );
                                                            } else {
                                                                let _ = settings.add_agent_mcp(
                                                                    &selected_agent,
                                                                    &mcp_name,
                                                                );
                                                            }
                                                            cx.notify();
                                                        }),
                                                    )
                                                    .child(
                                                        div()
                                                            .w(px(18.))
                                                            .h(px(18.))
                                                            .rounded(px(4.))
                                                            .border_2()
                                                            .border_color(if is_attached {
                                                                rgb(0xffffff)
                                                            } else {
                                                                theme.border
                                                            })
                                                            .bg(if is_attached {
                                                                rgb(0xffffff)
                                                            } else {
                                                                theme.background
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_size(px(12.))
                                                            .font_weight(gpui::FontWeight::BOLD)
                                                            .text_color(theme.accent)
                                                            .when(is_attached, |d| d.child("✓")),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(13.))
                                                            .text_color(if is_attached {
                                                                rgb(0xffffff)
                                                            } else {
                                                                theme.text
                                                            })
                                                            .child(name.clone()),
                                                    )
                                            },
                                        ),
                                    ),
                            ),
                    ),
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

    fn truncate_text(s: &str, max_len: usize) -> String {
        if s.len() <= max_len {
            s.to_string()
        } else {
            format!("{}...", &s[..max_len])
        }
    }

    pub(crate) fn render_mcp_import_dialog(&self, cx: &Context<Self>) -> impl IntoElement {
        #[allow(unused_imports)]
        use crate::mcp::{McpConfig, McpServerEntry};

        let theme = self.theme.clone();
        let show = self.show_mcp_import_dialog;

        div().when(show, |d| {
            d.absolute()
                .top_0()
                .left_0()
                .size_full()
                .bg(rgba(0x000000aa))
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                })
                .child(
                    div()
                        .w(px(600.))
                        .max_h(px(500.))
                        .bg(theme.panel_background)
                        .border_1()
                        .border_color(theme.border)
                        .rounded(px(12.))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                        })
                        // Header
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px(px(20.))
                                .py(px(14.))
                                .border_b_1()
                                .border_color(theme.border)
                                .child(
                                    div()
                                        .text_size(px(15.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.text)
                                        .child("📋 Import MCP Config from JSON"),
                                )
                                .child(
                                    div()
                                        .id("close-mcp-import")
                                        .px(px(8.))
                                        .py(px(4.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .hover(|s| s.bg(theme.tool_card))
                                        .text_color(theme.text_muted)
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(|this, _, _, cx| {
                                                this.show_mcp_import_dialog = false;
                                                this.mcp_import_json.clear();
                                                this.mcp_import_error = None;
                                                cx.notify();
                                            }),
                                        )
                                        .child("✕"),
                                ),
                        )
                        // Content
                        .child(
                            div()
                                .flex_1()
                                .p(px(20.))
                                .flex()
                                .flex_col()
                                .gap(px(12.))
                                .child(
                                    div().text_size(px(12.)).text_color(theme.text_muted).child(
                                        "Paste your MCP config JSON (Claude Desktop format):",
                                    ),
                                )
                                .child(
                                    div()
                                        .id("mcp-json-preview")
                                        .flex_1()
                                        .min_h(px(150.))
                                        .p(px(12.))
                                        .rounded(px(8.))
                                        .bg(theme.background)
                                        .border_1()
                                        .border_color(theme.border)
                                        .overflow_y_scroll()
                                        .scrollbar_width(px(6.))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .font_family("monospace")
                                                .text_color(if self.mcp_import_json.is_empty() {
                                                    theme.text_muted
                                                } else {
                                                    theme.text
                                                })
                                                .child(if self.mcp_import_json.is_empty() {
                                                    SharedString::from(
                                                        r#"{
  "mcpServers": {
    "playwright": {
      "command": "npx",
      "args": ["@playwright/mcp@latest"]
    }
  }
}"#,
                                                    )
                                                } else {
                                                    SharedString::from(self.mcp_import_json.clone())
                                                }),
                                        ),
                                )
                                .when_some(self.mcp_import_error.as_ref(), |d, err| {
                                    d.child(
                                        div()
                                            .px(px(12.))
                                            .py(px(8.))
                                            .rounded(px(6.))
                                            .bg(rgba(0xff6b6b22))
                                            .text_size(px(12.))
                                            .text_color(rgb(0xff6b6b))
                                            .child(err.clone()),
                                    )
                                })
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(8.))
                                        .child(
                                            div()
                                                .id("paste-mcp-json")
                                                .px(px(16.))
                                                .py(px(10.))
                                                .rounded(px(6.))
                                                .bg(theme.tool_card)
                                                .text_color(theme.text)
                                                .text_size(px(13.))
                                                .cursor_pointer()
                                                .hover(|s| s.opacity(0.9))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, cx| {
                                                        if let Some(text) = cx
                                                            .read_from_clipboard()
                                                            .and_then(|i| i.text())
                                                        {
                                                            this.mcp_import_json = text.to_string();
                                                            this.mcp_import_error = None;
                                                            cx.notify();
                                                        }
                                                    }),
                                                )
                                                .child("📋 Paste from Clipboard"),
                                        )
                                        .child(
                                            div()
                                                .id("clear-mcp-json")
                                                .px(px(16.))
                                                .py(px(10.))
                                                .rounded(px(6.))
                                                .bg(theme.background)
                                                .text_color(theme.text_muted)
                                                .text_size(px(13.))
                                                .cursor_pointer()
                                                .hover(|s| s.opacity(0.9))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, cx| {
                                                        this.mcp_import_json.clear();
                                                        this.mcp_import_error = None;
                                                        cx.notify();
                                                    }),
                                                )
                                                .child("Clear"),
                                        )
                                        .child(div().flex_1()) // spacer
                                        .when(!self.mcp_import_json.is_empty(), |d| {
                                            d.child(
                                                div()
                                                    .id("import-mcp-btn")
                                                    .px(px(20.))
                                                    .py(px(10.))
                                                    .rounded(px(6.))
                                                    .bg(theme.accent)
                                                    .text_color(rgb(0xffffff))
                                                    .text_size(px(13.))
                                                    .font_weight(gpui::FontWeight::MEDIUM)
                                                    .cursor_pointer()
                                                    .hover(|s| s.opacity(0.9))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(|this, _, _, cx| {
                                                            this.do_mcp_import(cx);
                                                        }),
                                                    )
                                                    .child("Import Servers"),
                                            )
                                        }),
                                ),
                        ),
                )
        })
    }
}
