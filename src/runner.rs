//! Application Runner Module
//!
//! Provides shared entry point functions for GUI, TUI, and render test modes.
//! These functions are called by the various binary entry points.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Shared application configuration.
///
/// This struct contains the runtime configuration options that are shared
/// across all binary entry points. Routing flags (like --tui) are handled
/// by the individual binaries.
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    /// Enable debug logging
    pub debug: bool,
    /// Enable verbose (trace-level) logging
    pub verbose: bool,
    /// Skip the automatic update check on startup
    pub skip_update_check: bool,
}

/// Run the GUI application.
///
/// # Errors
///
/// Returns an error if the GUI feature is not enabled or if the GUI fails to start.
#[cfg(feature = "gui")]
pub fn run_gui(config: AppConfig) -> anyhow::Result<()> {
    use crate::gui;
    use gpui::{
        prelude::*, px, size, App, Application, Bounds, QuitMode, SharedString, WindowBounds,
        WindowOptions,
    };
    use gpui_component::{Root, Theme, ThemeMode};

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
        crate::enable_debug_stream_events();
    }

    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    if !config.skip_update_check {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Some(release) = crate::version_check::check_for_update().await {
                    tracing::info!(
                        "Update available: {} -> {}",
                        crate::version_check::CURRENT_VERSION,
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
            gui::register_keybindings(cx);
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
                    let app_view = cx.new(|cx| gui::ChatApp::new(window, cx));
                    cx.new(|cx| Root::new(app_view, window, cx))
                },
            )
            .expect("Failed to open window");
        });

    Ok(())
}

#[cfg(not(feature = "gui"))]
pub fn run_gui(_config: AppConfig) -> anyhow::Result<()> {
    anyhow::bail!("GUI feature not enabled. Recompile with --features gui")
}

/// Run the TUI application.
///
/// # Errors
///
/// Returns an error if the TUI feature is not enabled or if the TUI fails to start.
#[cfg(feature = "tui")]
pub fn run_tui(config: AppConfig) -> anyhow::Result<()> {
    use std::fs::File;

    // Set up file logging for TUI debugging
    let log_file = File::create("/tmp/stockpot-tui.log").expect("Failed to create log file");
    let default_filter = if config.verbose {
        "trace"
    } else if config.debug {
        "debug"
    } else {
        "info,stockpot=debug"
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(log_file)),
        )
        .init();

    // Enable debug stream event logging if --debug flag is set
    if config.debug {
        crate::enable_debug_stream_events();
    }

    // Use LocalSet to allow spawn_local for non-Send futures (Database uses RefCell)
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async { crate::tui::run().await })
}

#[cfg(not(feature = "tui"))]
pub fn run_tui(_config: AppConfig) -> anyhow::Result<()> {
    anyhow::bail!("TUI feature not enabled. Recompile with --features tui")
}

/// Run the render performance test.
///
/// This is a GUI-only feature for testing markdown rendering performance.
///
/// # Errors
///
/// Returns an error if the GUI feature is not enabled.
#[cfg(feature = "gui")]
pub fn run_render_test() -> anyhow::Result<()> {
    use crate::gui::render_test::standard_test_cases;
    use crate::gui::render_test_app::RenderTestApp;
    use gpui::{prelude::*, px, size, App, Application, Bounds, WindowBounds, WindowOptions};
    use gpui_component::{Theme, ThemeMode};
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
            // Initialize gpui_component (REQUIRED for TextView::markdown)
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

#[cfg(not(feature = "gui"))]
pub fn run_render_test() -> anyhow::Result<()> {
    anyhow::bail!("Render test requires GUI feature. Recompile with --features gui")
}
