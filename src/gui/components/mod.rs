//! UI Components for the GUI

mod attachment_preview;
mod chat_view;
mod collapsible;
mod input_field;
mod markdown_text;
mod message;
mod scrollbar;
mod selectable_text;
mod spinner;
// Note: text_input.rs is no longer used - now using gpui_component::input::Input
// mod text_input;
mod throughput_chart;
mod toolbar;
mod tooltip;

pub use chat_view::ChatView;
pub use input_field::InputField;
pub use message::MessageView;
pub use scrollbar::{scroll_ratio, scrollbar, ScrollbarDragState};
pub use selectable_text::{
    Copy as SelectableCopy, SelectAll as SelectableSelectAll, SelectableText,
};
// Note: TextInput actions are no longer needed - gpui_component Input handles them internally
// pub use text_input::{
//     Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft,
//     SelectRight, Submit, TextElement, TextInput,
// };
pub use toolbar::Toolbar;

pub use attachment_preview::{render_attachment_preview, render_attachments_row};
pub use spinner::{current_spinner_frame, spinner, Spinner};

pub use collapsible::{collapsible, collapsible_display, CollapsibleProps};
pub use throughput_chart::{throughput_chart, ThroughputChartProps};
pub use tooltip::SimpleTooltip;
