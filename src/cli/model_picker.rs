//! Interactive model selection using dialoguer.
//!
//! Provides fuzzy-search model picking, agent selection, and
//! interactive model settings editing.

use dialoguer::{theme::ColorfulTheme, Confirm, FuzzySelect, Input, Select};

use crate::db::Database;
use crate::models::{ModelRegistry, ModelSettings};

/// Show an interactive model picker, returns selected model name.
///
/// Uses fuzzy search to filter through available models.
pub fn pick_model(db: &Database, registry: &ModelRegistry, current: &str) -> Option<String> {
    // Only show models that have valid provider configuration (API keys or OAuth tokens)
    let models: Vec<String> = registry.list_available(db);

    if models.is_empty() {
        println!("No models configured.");
        return None;
    }

    let default = models.iter().position(|m| m == current).unwrap_or(0);

    // Show with current marker
    let display: Vec<String> = models
        .iter()
        .map(|m| {
            if m == current {
                format!("{} (current)", m)
            } else {
                m.clone()
            }
        })
        .collect();

    let selection = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select model (type to filter)")
        .items(&display)
        .default(default)
        .interact_opt();

    match selection {
        Ok(Some(idx)) => models.get(idx).cloned(),
        _ => None,
    }
}

/// Show an interactive agent picker.
///
/// Takes a list of (name, display_name) tuples.
pub fn pick_agent(agents: &[(String, String)], current: &str) -> Option<String> {
    if agents.is_empty() {
        println!("No agents available.");
        return None;
    }

    let default = agents.iter().position(|(n, _)| n == current).unwrap_or(0);

    let display: Vec<String> = agents
        .iter()
        .map(|(name, display)| {
            if name == current {
                format!("{} - {} (current)", name, display)
            } else {
                format!("{} - {}", name, display)
            }
        })
        .collect();

    match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agent")
        .items(&display)
        .default(default)
        .interact_opt()
    {
        Ok(Some(idx)) => Some(agents[idx].0.clone()),
        _ => None,
    }
}

/// Edit model settings interactively.
///
/// Shows different options based on model type (OpenAI reasoning, Claude, etc.).
/// Returns true if settings were modified.
pub fn edit_model_settings(db: &Database, model_name: &str, _registry: &ModelRegistry) -> bool {
    let current = ModelSettings::load(db, model_name).unwrap_or_default();

    println!("\n⚙️  Settings for: \x1b[1m{}\x1b[0m\n", model_name);

    // Temperature (universal)
    let temp_str = current
        .temperature
        .map(|t| format!("{:.1}", t))
        .unwrap_or_default();
    let new_temp: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Temperature (0.0-2.0, empty=default)")
        .default(temp_str)
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    if !new_temp.is_empty() {
        if let Ok(t) = new_temp.parse::<f32>() {
            let clamped: f32 = t.clamp(0.0, 2.0);
            let value = format!("{:.2}", clamped);
            let _ = ModelSettings::save_setting(db, model_name, "temperature", &value);
        }
    } else {
        let _ = ModelSettings::clear_setting(db, model_name, "temperature");
    }

    // Max tokens
    let max_str: String = current
        .max_tokens
        .map(|t: i32| t.to_string())
        .unwrap_or_default();
    let new_max: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Max tokens (empty=default)")
        .default(max_str)
        .allow_empty(true)
        .interact_text()
        .unwrap_or_default();

    if !new_max.is_empty() {
        if let Ok(tokens) = new_max.parse::<i32>() {
            let value: String = tokens.to_string();
            let _ = ModelSettings::save_setting(db, model_name, "max_tokens", &value);
        }
    } else {
        let _ = ModelSettings::clear_setting(db, model_name, "max_tokens");
    }

    // Check model type for specific settings
    let is_openai_reasoning =
        model_name.contains("gpt-5") || model_name.contains("o1") || model_name.contains("o3");
    let is_claude = model_name.contains("claude");

    // OpenAI reasoning models
    if is_openai_reasoning {
        let efforts = ["minimal", "low", "medium", "high", "xhigh"];
        let current_idx = current
            .reasoning_effort
            .as_ref()
            .and_then(|r| efforts.iter().position(|e| *e == r))
            .unwrap_or(2); // default to medium

        if let Ok(idx) = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Reasoning effort")
            .items(&efforts)
            .default(current_idx)
            .interact()
        {
            let _ = ModelSettings::save_setting(db, model_name, "reasoning_effort", efforts[idx]);
        }

        // Verbosity (not for codex models)
        if !model_name.contains("codex") {
            let verbosities = ["low", "medium", "high"];
            let verbosity_values = [0, 1, 2];
            let current_idx = current
                .verbosity
                .map(|v| match v {
                    0 => 0,
                    1 => 1,
                    _ => 2,
                })
                .unwrap_or(1); // default to medium

            if let Ok(idx) = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Verbosity")
                .items(&verbosities)
                .default(current_idx)
                .interact()
            {
                let _ = ModelSettings::save_setting(
                    db,
                    model_name,
                    "verbosity",
                    &verbosity_values[idx].to_string(),
                );
            }
        }
    }

    // Claude models
    if is_claude {
        let extended = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Enable extended thinking?")
            .default(current.extended_thinking.unwrap_or(true))
            .interact()
            .unwrap_or(true);

        let _ = ModelSettings::save_setting(
            db,
            model_name,
            "extended_thinking",
            if extended { "true" } else { "false" },
        );

        if extended {
            let budget_str: String = current.budget_tokens.unwrap_or(10000).to_string();
            let new_budget: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Thinking budget (tokens)")
                .default(budget_str)
                .interact_text()
                .unwrap_or_else(|_| "10000".to_string());

            if let Ok(budget) = new_budget.parse::<i32>() {
                let value: String = budget.to_string();
                let _ = ModelSettings::save_setting(db, model_name, "budget_tokens", &value);
            }
        }

        // Interleaved thinking (Claude 4 only)
        if model_name.contains("claude-4")
            || model_name.contains("sonnet-4")
            || model_name.contains("opus-4")
        {
            let interleaved = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Enable interleaved thinking? (Claude 4 only)")
                .default(current.interleaved_thinking.unwrap_or(false))
                .interact()
                .unwrap_or(false);

            let _ = ModelSettings::save_setting(
                db,
                model_name,
                "interleaved_thinking",
                if interleaved { "true" } else { "false" },
            );
        }
    }

    println!("\n✅ Settings saved\n");
    true
}

/// Show current model settings (non-interactive).
pub fn show_model_settings(db: &Database, model_name: &str) {
    let settings = ModelSettings::load(db, model_name).unwrap_or_default();

    println!("\n⚙️  Settings for: \x1b[1m{}\x1b[0m\n", model_name);

    if let Some(t) = settings.temperature {
        println!("  temperature:          {:.2}", t);
    }
    if let Some(t) = settings.max_tokens {
        println!("  max_tokens:           {}", t);
    }
    if let Some(t) = settings.seed {
        println!("  seed:                 {}", t);
    }
    if let Some(ref t) = settings.reasoning_effort {
        println!("  reasoning_effort:     {}", t);
    }
    if let Some(t) = settings.verbosity {
        let label = match t {
            0 => "low",
            1 => "medium",
            _ => "high",
        };
        println!("  verbosity:            {} ({})", t, label);
    }
    if let Some(t) = settings.extended_thinking {
        println!("  extended_thinking:    {}", t);
    }
    if let Some(t) = settings.budget_tokens {
        println!("  budget_tokens:        {}", t);
    }
    if let Some(t) = settings.interleaved_thinking {
        println!("  interleaved_thinking: {}", t);
    }

    if settings.is_empty() {
        println!("  (using defaults)");
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_settings_is_empty() {
        let settings = ModelSettings::default();
        assert!(settings.is_empty());

        let settings = ModelSettings {
            temperature: Some(0.5),
            ..Default::default()
        };
        assert!(!settings.is_empty());
    }
}
