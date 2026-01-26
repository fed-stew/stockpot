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
mod streaming_markdown_view;
mod terminal_view;
// Note: text_input.rs is no longer used - now using gpui_component::input::Input
// mod text_input;
mod radio_group;
mod segmented_control;
mod selectable_list_item;
mod settings_row;
mod throughput_chart;
mod toggle;
mod toolbar;
mod tooltip;

pub use chat_view::ChatView;
pub use input_field::InputField;
pub use message::MessageView;
pub use scrollbar::{
    list_scrollbar, scroll_ratio, scrollbar, ListScrollbarDragState, ScrollbarDragState,
};
pub use selectable_text::{
    Copy as SelectableCopy, SelectAll as SelectableSelectAll, SelectableText,
};
pub use streaming_markdown_view::StreamingMarkdownView;
// Note: TextInput actions are no longer needed - gpui_component Input handles them internally
// pub use text_input::{
//     Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft,
//     SelectRight, Submit, TextElement, TextInput,
// };
pub use toolbar::Toolbar;

pub use attachment_preview::{render_attachment_preview, render_attachments_row};
pub use spinner::{current_spinner_frame, spinner, Spinner};

pub use collapsible::{collapsible, collapsible_display, CollapsibleProps};
pub use radio_group::{labeled_radio_group, radio_group, RadioOption};
pub use segmented_control::{labeled_segmented_control, segmented_control};
pub use selectable_list_item::{
    selectable_list_item, selectable_list_item_with_icon, selectable_list_item_with_subtitle,
};
pub use settings_row::{settings_row, settings_row_dynamic, settings_row_no_border};
pub use terminal_view::TerminalView;
pub use throughput_chart::{throughput_chart, ThroughputChartProps};
pub use toggle::{labeled_toggle, toggle};
pub use tooltip::{MarkdownTooltip, SimpleTooltip};
