//! Stockpot - AI-powered coding assistant
//!
//! A unified application with GUI (default), CLI, and TUI modes.

#![allow(dead_code)]
#![allow(unused_imports)]

mod agents;
mod auth;
mod cli;
mod config;
mod db;
mod mcp;
mod messaging;
mod models;
mod session;
mod tokens;
mod tools;
mod version_check;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Stockpot - Your AI coding companion 🍲
#[derive(Parser, Debug)]
#[command(name = "spot")]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    /// Run in CLI/REPL mode instead of GUI
    #[arg(long)]
    cli: bool,

    /// Run in TUI mode (terminal UI)
    #[arg(long)]
    tui: bool,

    /// Execute a single prompt and exit (implies --cli)
    #[arg(short, long)]
    prompt: Option<String>,

    /// Specify which agent to use
    #[arg(short, long)]
    agent: Option<String>,

    /// Specify which model to use
    #[arg(short, long)]
    model: Option<String>,

    /// Run in bridge mode for external UI (implies --cli)
    #[arg(long)]
    bridge: bool,

    /// Working directory (like git -C)
    #[arg(short = 'C', long, visible_alias = "directory")]
    cwd: Option<String>,

    /// Enable debug logging (equivalent to RUST_LOG=debug)
    #[arg(short = 'd', long)]
    debug: bool,

    /// Enable verbose logging (equivalent to RUST_LOG=trace)
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Skip checking for new versions
    #[arg(long)]
    skip_update_check: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Change working directory if specified (do this early)
    if let Some(cwd) = &args.cwd {
        std::env::set_current_dir(cwd)?;
    }

    // Determine the mode to run
    // GUI is default, but --cli, --tui, --prompt, or --bridge override it
    let use_cli = args.cli || args.prompt.is_some() || args.bridge;
    let use_tui = args.tui;

    #[cfg(feature = "gui")]
    if !use_cli && !use_tui {
        return run_gui(args);
    }

    #[cfg(not(feature = "gui"))]
    if !use_cli && !use_tui {
        eprintln!("GUI mode not available (compiled without 'gui' feature)");
        eprintln!("Use --cli for command-line mode or --tui for terminal UI");
        std::process::exit(1);
    }

    if use_tui {
        // TUI mode - placeholder for now
        eprintln!("TUI mode not yet implemented. Use --cli for now.");
        std::process::exit(1);
    }

    // CLI mode
    run_cli(args)
}

/// Run the GUI application
#[cfg(feature = "gui")]
fn run_gui(args: Args) -> anyhow::Result<()> {
    use gpui::{
        prelude::*, px, size, App, Application, Bounds, SharedString, WindowBounds, WindowOptions,
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
                if let Some(release) = version_check::check_for_update().await {
                    // In GUI mode, we could show a notification instead
                    // For now, just log it
                    tracing::info!(
                        "Update available: {} -> {}",
                        version_check::CURRENT_VERSION,
                        release.version
                    );
                }
            });
        });
    }

    // Create GPUI application with gpui-component assets
    Application::new()
        .with_assets(gpui_component_assets::Assets)
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

/// Run the CLI/REPL application
fn run_cli(args: Args) -> anyhow::Result<()> {
    // Build tokio runtime for CLI mode
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        // Determine log level from args or env
        let default_filter = if args.verbose {
            "trace"
        } else if args.debug {
            "debug,gpui_component=warn"
        } else {
            // Suppress noisy gpui_component markdown warnings while keeping other warnings
            "warn,gpui_component::text::format::markdown=error"
        };

        // Initialize tracing with stderr output
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
            tracing::info!("Debug logging enabled");
        }

        // Initialize database
        let db = db::Database::open()?;
        db.migrate()?;

        // Check for updates in background (non-blocking)
        if !args.bridge && !args.skip_update_check {
            tokio::spawn(async {
                if let Some(release) = version_check::check_for_update().await {
                    version_check::print_update_message(&release);
                }
            });
        }

        // Handle different CLI modes
        if args.bridge {
            cli::bridge::run_bridge_mode().await?;
        } else if let Some(prompt) = args.prompt {
            cli::runner::run_single_prompt(
                &db,
                &prompt,
                args.agent.as_deref(),
                args.model.as_deref(),
            )
            .await?;
        } else {
            cli::runner::run_interactive(&db, args.agent.as_deref(), args.model.as_deref()).await?;
        }

        Ok(())
    })
}
