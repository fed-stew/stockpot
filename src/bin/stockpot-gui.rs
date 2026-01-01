//! Stockpot GUI - GPUI-based graphical interface for the stockpot agent framework

use std::sync::Arc;

use gpui::{prelude::*, px, size, App, Application, Bounds, SharedString, WindowBounds, WindowOptions};

use stockpot::gui::{register_keybindings, ChatApp, GlobalLanguageRegistry};
use zed_theme::{ActiveTheme as _, LoadThemes};

fn main() {
    // Create a Tokio runtime for async operations (HTTP clients, etc.)
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    Application::new().run(|cx: &mut App| {
        // Zed markdown depends on Zed's settings + theme globals.
        zed_settings::init(cx);
        zed_theme::init(LoadThemes::JustBase, cx);

        // Enable syntax highlighting in fenced code blocks.
        let language_registry = Arc::new(zed_language::LanguageRegistry::new(
            cx.background_executor().clone(),
        ));
        language_registry.set_theme(cx.theme().clone());

        let fs: Arc<dyn zed_fs::Fs> = Arc::new(zed_fs::RealFs::new(
            None,
            cx.background_executor().clone(),
        ));
        let node_runtime = zed_node_runtime::NodeRuntime::unavailable();

        zed_languages::init(language_registry.clone(), fs, node_runtime, cx);
        cx.set_global(GlobalLanguageRegistry(language_registry));

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
