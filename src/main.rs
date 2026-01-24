//! Stockpot - AI-powered coding assistant

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Stockpot - Your AI coding companion 🍲
#[derive(Parser, Debug)]
#[command(name = "spot")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    #[arg(short = 'C', long, visible_alias = "directory")]
    pub cwd: Option<String>,
    #[arg(short = 'd', long)]
    pub debug: bool,
    #[arg(short = 'v', long)]
    pub verbose: bool,
    #[arg(long)]
    pub tui: bool,
    #[arg(long)]
    pub render_test: bool,
    #[arg(long)]
    pub skip_update_check: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
    }

    if args.render_test {
        run_render_test(args)
    } else if args.tui {
        run_tui(args)
    } else {
        run_gui(args)
    }
}

#[cfg(feature = "gui")]
fn run_render_test(_args: Args) -> anyhow::Result<()> {
    use gpui::{px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions};
    use gpui_component::{Theme, ThemeMode};
    use std::collections::VecDeque;
    use stockpot::gui::render_test::standard_test_cases;
    use stockpot::gui::render_test_app::RenderTestApp;

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
fn run_render_test(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("Render test requires GUI feature")
}

#[cfg(feature = "tui")]
fn run_tui(args: Args) -> anyhow::Result<()> {
    use std::fs::File;

    // Set up file logging for TUI debugging
    let log_file = File::create("/tmp/stockpot-tui.log").expect("Failed to create log file");
    let default_filter = if args.verbose {
        "trace"
    } else if args.debug {
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
    if args.debug {
        stockpot::enable_debug_stream_events();
    }

    // Use LocalSet to allow spawn_local for non-Send futures (Database uses RefCell)
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async { stockpot::tui::run().await })
}

#[cfg(not(feature = "tui"))]
fn run_tui(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("TUI feature not enabled")
}

#[cfg(not(feature = "gui"))]
fn run_gui(_args: Args) -> anyhow::Result<()> {
    anyhow::bail!("GUI feature not enabled")
}

#[cfg(feature = "gui")]
fn run_gui(args: Args) -> anyhow::Result<()> {
    use gpui::{
        prelude::*, px, size, App, Application, Bounds, QuitMode, SharedString, WindowBounds,
        WindowOptions,
    };
    use gpui_component::{Root, Theme, ThemeMode};
    use stockpot::gui;

    let default_filter = if args.verbose {
        "trace"
    } else if args.debug {
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
    if args.debug {
        stockpot::enable_debug_stream_events();
    }

    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    if !args.skip_update_check {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Some(release) = stockpot::version_check::check_for_update().await {
                    tracing::info!(
                        "Update available: {} -> {}",
                        stockpot::version_check::CURRENT_VERSION,
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
