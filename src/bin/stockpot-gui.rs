//! Stockpot GUI - GPUI-based graphical interface for the stockpot agent framework

use gpui::{prelude::*, px, size, App, Application, Bounds, SharedString, WindowBounds, WindowOptions};

use stockpot::gui::{register_keybindings, ChatApp};

fn main() {
    // Create a Tokio runtime for async operations (HTTP clients, etc.)
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    Application::new().run(|cx: &mut App| {
        // Register keybindings
        register_keybindings(cx);

        // Create main window
        let bounds = Bounds::centered(None, size(px(1000.), px(750.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(SharedString::from("Stockpot")),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| ChatApp::new(window, cx)),
        )
        .expect("Failed to open window");

        cx.activate(true);
    });
}
