//! UI Components for the GUI

mod chat_view;
mod input_field;
mod markdown_text;
mod message;
mod scrollbar;
mod selectable_text;
mod text_input;
mod toolbar;
mod zed_markdown;

pub use chat_view::ChatView;
pub use input_field::InputField;
pub use message::MessageView;
pub use scrollbar::{scrollbar, ScrollbarDragState};
pub use selectable_text::{
    Copy as SelectableCopy, SelectAll as SelectableSelectAll, SelectableText,
};
pub use text_input::{
    Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft,
    SelectRight, Submit, TextElement, TextInput,
};
pub use toolbar::Toolbar;
pub use zed_markdown::ZedMarkdownText;
