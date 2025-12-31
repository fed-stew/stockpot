//! Messaging system for Stockpot.
//!
//! Provides:
//! - [`Message`] types for agent-UI communication
//! - [`MessageBus`] for bidirectional messaging
//! - Terminal rendering with syntax highlighting
//! - Animated spinner for activity indication

mod types;
mod bus;
mod renderer;
mod spinner;

pub use types::*;
pub use bus::{MessageBus, MessageReceiver, MessageSender};
pub use renderer::{TerminalRenderer, RenderStyle};
pub use spinner::{Spinner, SpinnerHandle, SpinnerConfig};
