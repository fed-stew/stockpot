//! Models settings tab
//!
//! OAuth account status, model list grouped by provider, and model management.

use std::collections::BTreeMap;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::ModelSettingsField;
use crate::tui::app::TuiApp;
use crate::tui::hit_test::{ClickTarget, HitTestRegistry};
use crate::tui::theme::Theme;
use stockpot_core::config::Settings;
use stockpot_core::models::utils::has_oauth_tokens;
use stockpot_core::models::ModelType;

/// Render the Models settings tab content
pub fn render_models_tab(
    frame: &mut Frame,
    area: Rect,
    app: &TuiApp,
    hit_registry: &mut HitTestRegistry,
) {
    let settings = Settings::new(&app.db);
    let default_model = settings.model();
    let available_models = app.model_registry.list_available(&app.db);

    // Group models by provider type
    let by_type = group_models_by_type(app, &available_models);

    // Get current state
    let selected_index = app.settings_state.models_selected_index;
    let in_oauth_section = app.settings_state.models_in_oauth_section;
    let expanded = &app.settings_state.models_expanded_providers;

    // Layout: OAuth section at top, then model list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // OAuth Accounts section
            Constraint::Min(10),   // Available Models section
        ])
        .split(area);

    // ─────────────────────────────────────────────────────────────────────────
    // OAuth Accounts Section
    // ─────────────────────────────────────────────────────────────────────────
    render_oauth_section(frame, chunks[0], app, in_oauth_section);

    // ─────────────────────────────────────────────────────────────────────────
    // Available Models Section
    // ─────────────────────────────────────────────────────────────────────────
    render_models_section(
        frame,
        chunks[1],
        &by_type,
        &default_model,
        selected_index,
        !in_oauth_section,
        expanded,
        app.settings_state.expanded_model.as_deref(),
        &app.settings_state.model_temp_value,
        &app.settings_state.model_top_p_value,
        app.settings_state.model_settings_field,
        app.settings_state.model_settings_editing,
    );

    // ─────────────────────────────────────────────────────────────────────────
    // Register hit targets for models section
    // ─────────────────────────────────────────────────────────────────────────
    // The models section has a 1-cell border, then a 1-line header, then the list
    let models_inner = inner_rect(chunks[1]);
    let list_start_y = models_inner.y + 2; // Skip header line and blank
    let list_width = models_inner.width;
    let list_max_y = models_inner.y + models_inner.height;

    let mut current_y = list_start_y;

    for (type_label, models) in by_type.iter() {
        // Register provider group header
        if current_y < list_max_y {
            hit_registry.register(
                Rect::new(models_inner.x, current_y, list_width, 1),
                ClickTarget::ModelsProvider(type_label.clone()),
            );
        }
        current_y += 1;

        // If expanded, register each model
        if expanded.contains(type_label) {
            for model in models {
                if current_y < list_max_y {
                    hit_registry.register(
                        Rect::new(models_inner.x, current_y, list_width, 1),
                        ClickTarget::ModelsItem(model.name.clone()),
                    );
                }
                current_y += 1;
            }
        }
    }
}

/// Calculate inner rect (inside 1-cell border)
fn inner_rect(area: Rect) -> Rect {
    Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    )
}

/// Render the OAuth accounts status section
fn render_oauth_section(frame: &mut Frame, area: Rect, app: &TuiApp, is_focused: bool) {
    let border_color = if is_focused {
        Theme::ACCENT
    } else {
        Theme::BORDER
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            " OAuth Accounts ",
            Style::default().fg(if is_focused {
                Theme::ACCENT
            } else {
                Theme::HEADER
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Check OAuth status for each provider
    let claude_connected = has_oauth_tokens(&app.db, "claude-code");
    let chatgpt_connected = has_oauth_tokens(&app.db, "chatgpt");
    let google_connected = has_oauth_tokens(&app.db, "google");

    // Get selection state
    let selected_index = app.settings_state.oauth_selected_index;
    let in_progress = app.settings_state.oauth_in_progress.as_deref();

    let lines = vec![
        render_oauth_line("Claude Code", "claude-code", claude_connected, is_focused && selected_index == 0, in_progress),
        render_oauth_line("ChatGPT", "chatgpt", chatgpt_connected, is_focused && selected_index == 1, in_progress),
        render_oauth_line("Google", "google", google_connected, is_focused && selected_index == 2, in_progress),
        Line::from(""),
        Line::from(Span::styled(
            if is_focused { "  ↵ Enter to connect  •  ↑↓ Navigate" } else { "  Press Enter to connect" },
            Style::default().fg(Theme::MUTED),
        )),
    ];

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Render a single OAuth status line
fn render_oauth_line(
    name: &str,
    provider_id: &str,
    connected: bool,
    is_selected: bool,
    in_progress: Option<&str>,
) -> Line<'static> {
    // Check if this provider is currently authenticating
    let is_in_progress = in_progress == Some(provider_id);

    let (status_icon, status_text, status_color) = if is_in_progress {
        ("⟳", "Connecting...", Theme::ACCENT)
    } else if connected {
        ("✓", "Connected", Theme::GREEN)
    } else {
        ("○", "Not connected", Theme::MUTED)
    };

    // Selection indicator
    let prefix = if is_selected { "▸ " } else { "  " };
    let name_style = if is_selected {
        Style::default().fg(Theme::ACCENT)
    } else {
        Style::default().fg(Theme::TEXT)
    };

    Line::from(vec![
        Span::styled(prefix.to_string(), Style::default().fg(Theme::ACCENT)),
        Span::styled(format!("{:<14}", name), name_style),
        Span::styled(
            format!("{} ", status_icon),
            Style::default().fg(status_color),
        ),
        Span::styled(status_text.to_string(), Style::default().fg(status_color)),
    ])
}

/// Render the models list section
fn render_models_section(
    frame: &mut Frame,
    area: Rect,
    by_type: &BTreeMap<String, Vec<ModelInfo>>,
    default_model: &str,
    selected_index: usize,
    is_focused: bool,
    expanded: &std::collections::HashSet<String>,
    expanded_model: Option<&str>,
    model_temp_value: &str,
    model_top_p_value: &str,
    model_settings_field: ModelSettingsField,
    model_settings_editing: bool,
) {
    let border_color = if is_focused {
        Theme::ACCENT
    } else {
        Theme::BORDER
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            " Available Models ",
            Style::default().fg(if is_focused {
                Theme::ACCENT
            } else {
                Theme::HEADER
            }),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Header with action hints
    let header_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let list_area = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(2),
    );

    let header = Line::from(vec![
        Span::styled("  Enter", Style::default().fg(Theme::ACCENT)),
        Span::styled(": expand/set default  ", Style::default().fg(Theme::MUTED)),
        Span::styled("Del", Style::default().fg(Theme::ACCENT)),
        Span::styled(": remove model", Style::default().fg(Theme::MUTED)),
    ]);
    frame.render_widget(Paragraph::new(header), header_area);

    // Build list items
    let mut items: Vec<ListItem> = Vec::new();
    let mut item_index = 0;

    for (type_label, models) in by_type.iter() {
        let is_expanded = expanded.contains(type_label);
        let is_group_selected = selected_index == item_index && is_focused;

        // Provider group header
        let chevron = if is_expanded { "▾" } else { "▸" };
        let selector = if is_group_selected { "▶ " } else { "  " };

        let header_style = if is_group_selected {
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::HEADER)
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(selector, Style::default().fg(Theme::ACCENT)),
            Span::styled(format!("{} ", chevron), header_style),
            Span::styled(type_label.clone(), header_style),
            Span::styled(
                format!(" ({})", models.len()),
                Style::default().fg(Theme::MUTED),
            ),
        ])));

        item_index += 1;

        // Model items (if expanded)
        if is_expanded {
            for model in models {
                let is_model_selected = selected_index == item_index && is_focused;
                let is_default = model.name == default_model;
                let is_model_expanded = expanded_model == Some(model.name.as_str());

                let selector = if is_model_selected { "▶ " } else { "  " };

                // Chevron for expandable models (non-OAuth only)
                let chevron = if !model.is_oauth {
                    if is_model_expanded {
                        "▼ "
                    } else {
                        "▸ "
                    }
                } else {
                    "  "
                };

                let name_style = if is_model_selected {
                    Style::default()
                        .fg(Theme::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else if is_default {
                    Style::default().fg(Theme::GREEN)
                } else {
                    Style::default().fg(Theme::TEXT)
                };

                let mut spans = vec![
                    Span::styled(selector, Style::default().fg(Theme::ACCENT)),
                    Span::raw("  "), // Indent under provider
                    Span::styled(chevron, Style::default().fg(Theme::MUTED)),
                    Span::styled(truncate_model_name(&model.name, 35), name_style),
                ];

                if is_default {
                    spans.push(Span::styled(
                        " ✓ default",
                        Style::default().fg(Theme::GREEN),
                    ));
                }

                items.push(ListItem::new(Line::from(spans)));
                item_index += 1;

                // Render expanded settings panel for non-OAuth models
                if is_model_expanded && !model.is_oauth {
                    // Temperature row
                    let temp_focused = model_settings_field == ModelSettingsField::Temperature;
                    let temp_style = if temp_focused && is_focused {
                        Style::default().fg(Theme::ACCENT)
                    } else {
                        Style::default().fg(Theme::MUTED)
                    };

                    let temp_value_display = if model_settings_editing && temp_focused {
                        format!("[{}▏]", model_temp_value)
                    } else {
                        format!("[{}]", model_temp_value)
                    };

                    items.push(ListItem::new(Line::from(vec![
                        Span::raw("        "), // Deep indent
                        Span::styled("Temperature: ", temp_style),
                        Span::styled(temp_value_display, temp_style),
                        Span::styled(" (0.0-2.0)", Style::default().fg(Theme::MUTED)),
                    ])));

                    // Top P row
                    let top_p_focused = model_settings_field == ModelSettingsField::TopP;
                    let top_p_style = if top_p_focused && is_focused {
                        Style::default().fg(Theme::ACCENT)
                    } else {
                        Style::default().fg(Theme::MUTED)
                    };

                    let top_p_value_display = if model_settings_editing && top_p_focused {
                        format!("[{}▏]", model_top_p_value)
                    } else {
                        format!("[{}]", model_top_p_value)
                    };

                    items.push(ListItem::new(Line::from(vec![
                        Span::raw("        "),
                        Span::styled("Top P:       ", top_p_style),
                        Span::styled(top_p_value_display, top_p_style),
                        Span::styled(" (0.0-1.0)", Style::default().fg(Theme::MUTED)),
                    ])));

                    // API Keys hint row
                    items.push(ListItem::new(Line::from(vec![
                        Span::raw("        "),
                        Span::styled("Press ", Style::default().fg(Theme::MUTED)),
                        Span::styled("k", Style::default().fg(Theme::ACCENT)),
                        Span::styled(" to manage API keys", Style::default().fg(Theme::MUTED)),
                    ])));

                    // Spacer
                    items.push(ListItem::new(Line::from("")));
                }
            }
        }
    }

    if items.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "  No models available. Add API keys or login via OAuth.",
            Style::default().fg(Theme::MUTED),
        ));
        frame.render_widget(msg, list_area);
    } else {
        let list = List::new(items);
        let mut list_state = ListState::default();
        list_state.select(Some(selected_index));
        frame.render_stateful_widget(list, list_area, &mut list_state);
    }
}

/// Information about a model for display
#[derive(Debug, Clone)]
struct ModelInfo {
    name: String,
    #[allow(dead_code)]
    description: Option<String>,
    is_oauth: bool,
}

/// Group models by their provider type
fn group_models_by_type(
    app: &TuiApp,
    available_models: &[String],
) -> BTreeMap<String, Vec<ModelInfo>> {
    let mut by_type: BTreeMap<String, Vec<ModelInfo>> = BTreeMap::new();

    for name in available_models {
        if let Some(config) = app.model_registry.get(name) {
            let label = type_label_for(name, config.model_type);
            by_type.entry(label).or_default().push(ModelInfo {
                name: name.clone(),
                description: config.description.clone(),
                is_oauth: config.is_oauth(),
            });
        } else {
            by_type
                .entry("Unknown".to_string())
                .or_default()
                .push(ModelInfo {
                    name: name.clone(),
                    description: None,
                    is_oauth: false,
                });
        }
    }

    // Sort models within each type
    for models in by_type.values_mut() {
        models.sort_by(|a, b| a.name.cmp(&b.name));
    }

    by_type
}

/// Get a human-readable label for a model type
fn type_label_for(name: &str, model_type: ModelType) -> String {
    match model_type {
        ModelType::Openai => "OpenAI".to_string(),
        ModelType::Anthropic => "Anthropic".to_string(),
        ModelType::Gemini => "Google Gemini".to_string(),
        ModelType::ClaudeCode => "Claude Code (OAuth)".to_string(),
        ModelType::ChatgptOauth => "ChatGPT (OAuth)".to_string(),
        ModelType::GoogleVertex => "Google (OAuth)".to_string(),
        ModelType::AzureOpenai => "Azure OpenAI".to_string(),
        ModelType::Openrouter => "OpenRouter".to_string(),
        ModelType::RoundRobin => "Round Robin".to_string(),
        ModelType::CustomOpenai | ModelType::CustomAnthropic => {
            if let Some(idx) = name.find(':') {
                let provider = &name[..idx];
                let mut chars = provider.chars();
                match chars.next() {
                    Some(c) => format!(
                        "Custom: {}",
                        c.to_uppercase().chain(chars).collect::<String>()
                    ),
                    None => "Custom".to_string(),
                }
            } else {
                "Custom".to_string()
            }
        }
    }
}

/// Truncate model name for display
fn truncate_model_name(name: &str, max_len: usize) -> String {
    if name.len() > max_len {
        format!("{}...", &name[..max_len - 3])
    } else {
        name.to_string()
    }
}

/// Check if a model at given index is expandable (non-OAuth)
pub fn is_model_expandable(
    app: &TuiApp,
    available_models: &[String],
    selected_index: usize,
) -> bool {
    if let Some(model_name) = get_model_at_index(app, available_models, selected_index) {
        if let Some(config) = app.model_registry.get(&model_name) {
            return !config.is_oauth();
        }
    }
    false
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions for counting items (for keyboard navigation)
// ─────────────────────────────────────────────────────────────────────────────

/// Count total selectable items in the models tab
pub fn count_models_items(app: &TuiApp, available_models: &[String]) -> usize {
    let by_type = group_models_by_type(app, available_models);
    let expanded = &app.settings_state.models_expanded_providers;

    let mut count = 0;
    for (type_label, models) in by_type.iter() {
        count += 1; // The group header
        if expanded.contains(type_label) {
            count += models.len(); // The models in the group
        }
    }
    count
}

/// Check if the selected index is a group header (vs a model)
pub fn is_group_header(
    app: &TuiApp,
    available_models: &[String],
    selected_index: usize,
) -> Option<String> {
    let by_type = group_models_by_type(app, available_models);
    let expanded = &app.settings_state.models_expanded_providers;

    let mut current_index = 0;
    for (type_label, models) in by_type.iter() {
        if current_index == selected_index {
            return Some(type_label.clone());
        }
        current_index += 1;

        if expanded.contains(type_label) {
            current_index += models.len();
        }
    }
    None
}

/// Get the model name at a given index (if it's a model, not a header)
pub fn get_model_at_index(
    app: &TuiApp,
    available_models: &[String],
    selected_index: usize,
) -> Option<String> {
    let by_type = group_models_by_type(app, available_models);
    let expanded = &app.settings_state.models_expanded_providers;

    let mut current_index = 0;
    for (type_label, models) in by_type.iter() {
        if current_index == selected_index {
            return None; // It's a group header
        }
        current_index += 1;

        if expanded.contains(type_label) {
            for model in models {
                if current_index == selected_index {
                    return Some(model.name.clone());
                }
                current_index += 1;
            }
        }
    }
    None
}

/// Get the provider info for a type label (for key pool management)
/// Returns (provider_name, display_name) if the provider supports API keys
pub fn provider_for_type_label(type_label: &str) -> Option<(&'static str, &'static str)> {
    match type_label {
        "OpenAI" => Some(("openai", "OpenAI")),
        "Anthropic" => Some(("anthropic", "Anthropic")),
        "Google Gemini" => Some(("gemini", "Google Gemini")),
        "Azure OpenAI" => Some(("azure_openai", "Azure OpenAI")),
        "OpenRouter" => Some(("openrouter", "OpenRouter")),
        _ => None, // OAuth and custom providers don't use key pools
    }
}

/// Get the type label for the currently selected item (or nearest group header)
pub fn get_current_type_label(
    app: &TuiApp,
    available_models: &[String],
    selected_index: usize,
) -> Option<String> {
    let by_type = group_models_by_type(app, available_models);
    let expanded = &app.settings_state.models_expanded_providers;

    let mut current_index = 0;
    let mut last_type_label: Option<String> = None;

    for (type_label, models) in by_type.iter() {
        last_type_label = Some(type_label.clone());

        if current_index == selected_index {
            return Some(type_label.clone());
        }
        current_index += 1;

        if expanded.contains(type_label) {
            for _ in models {
                if current_index == selected_index {
                    return Some(type_label.clone());
                }
                current_index += 1;
            }
        }
    }
    last_type_label
}
