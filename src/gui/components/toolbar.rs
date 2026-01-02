//! Toolbar component - agent/model selection and actions

use gpui::{div, prelude::*, px, Styled};

/// Toolbar with agent/model selection and action buttons
pub struct Toolbar {
    pub current_agent: String,
    pub current_model: String,
}

impl Toolbar {
    pub fn new() -> Self {
        Self {
            current_agent: "default".to_string(),
            current_model: "claude-sonnet-4-20250514".to_string(),
        }
    }
}

impl Default for Toolbar {
    fn default() -> Self {
        Self::new()
    }
}
