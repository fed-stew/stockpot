//! Settings integration for TUI

use crate::config::Settings;
use crate::db::Database;
use anyhow::Result;

pub fn save_current_model(db: &Database, model: &str) -> Result<()> {
    let settings = Settings::new(db);
    settings.set("model", model)?;
    Ok(())
}

pub fn save_current_agent(db: &Database, agent: &str) -> Result<()> {
    let settings = Settings::new(db);
    settings.set("last_agent", agent)?;
    Ok(())
}

pub fn get_agent_pinned_model(db: &Database, agent: &str) -> Option<String> {
    let settings = Settings::new(db);
    settings.get_agent_pinned_model(agent)
}
