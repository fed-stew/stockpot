//! State management for the GUI

mod conversation;
mod message;
mod sections;
mod tool_display;

pub use conversation::Conversation;
pub use message::{ChatMessage, MessageRole, ToolCall, ToolCallState};
pub use sections::{AgentSection, MessageSection};
pub use tool_display::format_tool_call_display;
