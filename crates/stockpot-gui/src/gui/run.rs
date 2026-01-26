//! GUI Application Runner
//!
//! Provides the entry point for launching the GUI application.

use anyhow::Result;
use gpui::{
    prelude::*, px, size, App, Application, Bounds, QuitMode, SharedString, WindowBounds,
    WindowOptions,
};
use gpui_component::{Root, Theme, ThemeMode};
use stockpot_core::runner::AppConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use super::{register_keybindings, ChatApp};

/// Run the GUI application.
///
/// # Errors
///
/// Returns an error if the GUI fails to start.
pub fn run_gui(config: AppConfig) -> Result<()> {
    let default_filter = if config.verbose {
        "trace"
    } else if config.debug {
        "debug,gpui_component=warn"
    } else {
        "warn,gpui_component::text::format::markdown=error"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_writer(std::io::stderr),
        )
        .init();

    // Enable debug stream event logging if --debug flag is set
    if config.debug {
        stockpot_core::enable_debug_stream_events();
    }

    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    if !config.skip_update_check {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Some(release) = stockpot_core::version_check::check_for_update().await {
                    tracing::info!(
                        "Update available: {} -> {}",
                        stockpot_core::version_check::CURRENT_VERSION,
                        release.version
                    );
                }
            });
        });
    }

    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            register_keybindings(cx);
            cx.activate(true);

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
                    let app_view = cx.new(|cx| ChatApp::new(window, cx));
                    cx.new(|cx| Root::new(app_view, window, cx))
                },
            )
            .expect("Failed to open window");
        });

    Ok(())
}

/// Run the render performance test.
///
/// This is for testing markdown rendering performance.
pub fn run_render_test() -> Result<()> {
    use super::render_test::standard_test_cases;
    use super::render_test_app::RenderTestApp;
    use std::collections::VecDeque;

    let filter = tracing_subscriber::EnvFilter::new("warn,gpui_component=error");
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║         STOCKPOT RENDER PERFORMANCE TEST                      ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();

    let test_cases = standard_test_cases();
    println!(
        "Running {} test cases (300 frames each, with markdown)...\n",
        test_cases.len()
    );

    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);

            let bounds = Bounds::centered(None, size(px(900.), px(700.)), cx);
            let mut cases: VecDeque<_> = test_cases.into_iter().collect();
            let first_case = cases.pop_front().expect("No test cases");

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    ..Default::default()
                },
                move |window, cx| {
                    cx.new(|cx| {
                        let mut app = RenderTestApp::new(&first_case, window, cx);
                        app.set_remaining_cases(cases);
                        app
                    })
                },
            )
            .expect("Failed to open test window");
        });

    Ok(())
}
