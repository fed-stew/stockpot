//! Message view component - renders individual messages

use gpui::{div, prelude::*, px, Styled};
use crate::gui::state::ChatMessage;

/// Individual message view
pub struct MessageView {
    pub message: ChatMessage,
}
