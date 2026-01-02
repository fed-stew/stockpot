//! Input field component for message entry

use gpui::{div, prelude::*, px, Styled};

/// Text input field for composing messages
pub struct InputField {
    pub text: String,
    pub placeholder: String,
}

impl InputField {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            placeholder: "Type a message...".to_string(),
        }
    }
}

impl Default for InputField {
    fn default() -> Self {
        Self::new()
    }
}
