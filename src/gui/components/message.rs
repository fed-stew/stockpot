//! Message view component - renders individual messages

use crate::gui::state::ChatMessage;
use gpui::{div, prelude::*, px, Styled};

/// Individual message view
pub struct MessageView {
    pub message: ChatMessage,
}
