//! Chat view component - displays the message list

use crate::gui::state::Conversation;
use gpui::{div, prelude::*, px, Entity, Styled};

/// Chat view component for displaying messages
pub struct ChatView {
    pub conversation: Conversation,
}

impl ChatView {
    pub fn new() -> Self {
        Self {
            conversation: Conversation::new(),
        }
    }
}

impl Default for ChatView {
    fn default() -> Self {
        Self::new()
    }
}
