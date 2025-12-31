//! Context and model pin management commands.

use crate::config::Settings;
use crate::db::Database;
use serdes_ai_core::ModelRequest;

/// Truncate message history to last N messages.
pub fn truncate(messages: &mut Vec<ModelRequest>, args: &str) {
    let n: usize = args.parse().unwrap_or(10);
    
    if messages.len() <= n {
        println!("  Context has {} messages (no truncation needed)", messages.len());
        return;
    }
    
    let original_len = messages.len();
    let removed = original_len - n;
    
    // Keep last n messages
    *messages = messages.split_off(removed);
    
    println!("✂️  Truncated context: {} → {} messages ({} removed)", 
        original_len, messages.len(), removed);
}

/// Show context information.
pub fn show(
    db: &Database,
    messages: &[ModelRequest],
    current_session: Option<&str>,
    agent_name: &str,
) {
    let msg_count = messages.len();
    let token_estimate = estimate_tokens(messages);
    
    println!("\n\x1b[1m📊 Context Info:\x1b[0m\n");
    println!("  Messages: {}", msg_count);
    println!("  Estimated tokens: ~{}", token_estimate);
    
    if let Some(session) = current_session {
        println!("  Current session: {}", session);
    }
    
    // Check for pinned model in database
    let settings = Settings::new(db);
    if let Some(pinned) = settings.get_agent_pinned_model(agent_name) {
        println!("  Pinned model: {}", pinned);
    }
    
    // Show part breakdown
    if msg_count > 0 {
        let total_parts: usize = messages.iter()
            .map(|msg| msg.parts.len())
            .sum();
        println!("  Total parts: {}", total_parts);
    }
    
    println!();
}

/// Pin a model to an agent (persisted to database).
/// 
/// If `is_current_agent` is true, also updates the current_model reference.
pub fn pin_model(
    db: &Database,
    current_model: &mut String,
    target_agent: &str,
    model: &str,
    is_current_agent: bool,
) {
    if model.is_empty() {
        println!("❌ Please specify a model: /pin <model>");
        println!("   Or: /pin <agent> <model>");
        println!("   Example: /pin gpt-4o");
        println!("   Example: /pin reviewer gpt-4o");
        return;
    }
    
    let settings = Settings::new(db);
    if let Err(e) = settings.set_agent_pinned_model(target_agent, model) {
        println!("❌ Failed to pin model: {}", e);
        return;
    }
    
    // Only update current_model if we're pinning to the current agent
    if is_current_agent {
        *current_model = model.to_string();
    }
    
    println!("📌 Pinned \x1b[1;33m{}\x1b[0m to agent \x1b[1;36m{}\x1b[0m", model, target_agent);
}

/// Unpin the model from an agent (removes from database).
/// 
/// If `is_current_agent` is true, also resets current_model to default.
pub fn unpin_model(
    db: &Database,
    current_model: &mut String,
    target_agent: &str,
    is_current_agent: bool,
) {
    let settings = Settings::new(db);
    
    // Check if there's actually a pin to remove
    if settings.get_agent_pinned_model(target_agent).is_none() {
        println!("  No model pinned for agent {}", target_agent);
        return;
    }
    
    if let Err(e) = settings.clear_agent_pinned_model(target_agent) {
        println!("❌ Failed to unpin model: {}", e);
        return;
    }
    
    println!("📌 Unpinned model from agent \x1b[1;36m{}\x1b[0m", target_agent);
    
    // Only reset current_model if we're unpinning the current agent
    if is_current_agent {
        *current_model = settings.model();
        println!("   Now using default: \x1b[1;33m{}\x1b[0m", current_model);
    }
}

/// List all agent model pins.
pub fn list_pins(db: &Database) {
    let settings = Settings::new(db);
    
    match settings.get_all_agent_pinned_models() {
        Ok(pins) if pins.is_empty() => {
            println!("\n  No model pins configured.");
            println!("  Use /pin <model> to pin a model to the current agent.");
            println!();
        }
        Ok(pins) => {
            println!("\n\x1b[1m📌 Agent Model Pins\x1b[0m\n");
            
            // Find max agent name length for alignment
            let max_len = pins.keys().map(|k| k.len()).max().unwrap_or(10);
            
            for (agent, model) in &pins {
                println!("  \x1b[36m{:width$}\x1b[0m → \x1b[33m{}\x1b[0m", 
                    agent, model, width = max_len);
            }
            println!();
        }
        Err(e) => {
            println!("❌ Failed to list pins: {}", e);
        }
    }
}

/// Get the effective model for an agent (pinned or default).
pub fn get_effective_model(
    db: &Database,
    current_model: &str,
    agent_name: &str,
) -> String {
    let settings = Settings::new(db);
    settings.get_agent_pinned_model(agent_name)
        .unwrap_or_else(|| current_model.to_string())
}

/// Estimate tokens in messages.
fn estimate_tokens(messages: &[ModelRequest]) -> usize {
    let mut total = 0;
    for msg in messages {
        total += serde_json::to_string(msg)
            .map(|s| s.len() / 4)
            .unwrap_or(25);
    }
    total
}
