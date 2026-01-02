//! Stockpot GUI - GPUI-based graphical interface for the stockpot agent framework

use gpui::{
    prelude::*, px, size, App, Application, Bounds, SharedString, WindowBounds, WindowOptions,
};
use gpui_component::{Root, Theme, ThemeMode};

use stockpot::gui::{register_keybindings, ChatApp};

fn main() {
    // Create a Tokio runtime for async operations (HTTP clients, etc.)
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    // Create GPUI application with gpui-component assets
    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(|cx: &mut App| {
            // Initialize gpui-component (REQUIRED - sets up themes, icons, etc.)
            gpui_component::init(cx);

            // Set dark theme
            Theme::change(ThemeMode::Dark, None, cx);

            // Register keybindings
            register_keybindings(cx);

            // Activate the application
            cx.activate(true);

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
                |window, cx| {
                    // Create the main app view
                    let app_view = cx.new(|cx| ChatApp::new(window, cx));
                    // Wrap in Root (required by gpui-component)
                    cx.new(|cx| Root::new(app_view, window, cx))
                },
            )
            .expect("Failed to open window");
        });
}
