//! Input field component for message entry

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
