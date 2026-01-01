//! Chat view component - displays the message list

use gpui::{div, prelude::*, px, Entity, Styled};
use crate::gui::state::Conversation;

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
