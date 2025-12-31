//! CLI components.

pub mod add_model;
pub mod bridge;
pub mod commands;
pub mod completion_reedline;
pub mod model_picker;
pub mod repl;
pub mod runner;
pub mod streaming_markdown;

pub use add_model::{list_extra_models, run_add_model};
pub use completion_reedline::{create_reedline, SpotCompleter, SpotPrompt, COMMANDS};
pub use model_picker::{edit_model_settings, pick_agent, pick_model, show_model_settings};
pub use repl::Repl;
pub use runner::{run_interactive, run_single_prompt};
