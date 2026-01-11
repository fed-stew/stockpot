//! Stockpot - AI-powered coding assistant
//!
//! A GUI application for AI-assisted coding.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Stockpot - Your AI coding companion 🍲
#[derive(Parser, Debug)]
#[command(name = "spot")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Args {
    /// Working directory (like git -C)
    #[arg(short = 'C', long, visible_alias = "directory")]
    pub cwd: Option<String>,

    /// Enable debug logging (equivalent to RUST_LOG=debug)
    #[arg(short = 'd', long)]
    pub debug: bool,

    /// Enable verbose logging (equivalent to RUST_LOG=trace)
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Skip checking for new versions
    #[arg(long)]
    pub skip_update_check: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Change working directory if specified (do this early)
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
    }

    run_gui(args)
}

/// Run the GUI application
#[cfg(feature = "gui")]
fn run_gui(args: Args) -> anyhow::Result<()> {
    use gpui::{
        prelude::*, px, size, App, Application, Bounds, QuitMode, SharedString, WindowBounds,
        WindowOptions,
    };
    use gpui_component::{Root, Theme, ThemeMode};
    use stockpot::gui;

    // Initialize tracing for GUI mode
    let default_filter = if args.verbose {
        "trace"
    } else if args.debug {
        "debug,gpui_component=warn"
    } else {
        // Suppress noisy gpui_component markdown warnings while keeping other warnings
        "warn,gpui_component::text::format::markdown=error"
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_writer(std::io::stderr),
        )
        .init();

    if args.debug || args.verbose {
        tracing::info!("Debug logging enabled for GUI mode");
    }

    // Create a Tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _guard = runtime.enter();

    // Check for updates in background
    if !args.skip_update_check {
        std::thread::spawn(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Some(release) = stockpot::version_check::check_for_update().await {
                    // In GUI mode, we could show a notification instead
                    // For now, just log it
                    tracing::info!(
                        "Update available: {} -> {}",
                        stockpot::version_check::CURRENT_VERSION,
                        release.version
                    );
                }
            });
        });
    }

    // Create GPUI application with gpui-component assets
    // Use LastWindowClosed quit mode so closing the window terminates the app on macOS
    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx: &mut App| {
            // Initialize gpui-component (REQUIRED - sets up themes, icons, etc.)
            gpui_component::init(cx);

            // Set dark theme
            Theme::change(ThemeMode::Dark, None, cx);

            // Register keybindings
            gui::register_keybindings(cx);

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
                    let app_view = cx.new(|cx| gui::ChatApp::new(window, cx));
                    // Wrap in Root (required by gpui-component)
                    cx.new(|cx| Root::new(app_view, window, cx))
                },
            )
            .expect("Failed to open window");
        });

    Ok(())
}
