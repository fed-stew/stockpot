//! Factory function for creating Reedline with completion menu.

use nu_ansi_term::Color;
use nu_ansi_term::Style;
use reedline::{
    ColumnarMenu, Emacs, KeyCode, KeyModifiers, MenuBuilder, Reedline, ReedlineEvent, ReedlineMenu,
};

use super::completer::SpotCompleter;
use super::prompt::SpotHighlighter;

/// Create reedline with Tab-triggered completion menu
pub fn create_reedline(completer: SpotCompleter) -> Reedline {
    // Clean menu style - no heavy borders
    let completion_menu = Box::new(
        ColumnarMenu::default()
            .with_name("completion_menu")
            .with_columns(1)
            .with_column_padding(2)
            .with_text_style(Style::new().fg(Color::Default))
            .with_selected_text_style(Style::new().fg(Color::Black).on(Color::Cyan))
            .with_description_text_style(Style::new().fg(Color::DarkGray)),
    );

    let mut keybindings = reedline::default_emacs_keybindings();

    // Tab to show/navigate menu
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );

    // Shift+Tab to go back
    keybindings.add_binding(
        KeyModifiers::SHIFT,
        KeyCode::BackTab,
        ReedlineEvent::MenuPrevious,
    );

    Reedline::create()
        .with_completer(Box::new(completer))
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_quick_completions(true)
        .with_partial_completions(true)
        .with_highlighter(Box::new(SpotHighlighter))
        .with_edit_mode(Box::new(Emacs::new(keybindings)))
}
