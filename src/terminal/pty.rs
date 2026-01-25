//! PTY spawning and management using portable-pty.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Configuration for spawning a PTY
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Command to execute
    pub command: String,
    /// Working directory
    pub cwd: Option<PathBuf>,
    /// Initial terminal size
    pub size: PtySize,
    /// Environment variable overrides
    pub env: HashMap<String, String>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            cwd: None,
            size: PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            },
            env: HashMap::new(),
        }
    }
}

/// Create headless environment variables to prevent interactive pagers.
/// Used for LLM-spawned commands that shouldn't block on user input.
pub fn headless_env() -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Disable interactive terminal features
    env.insert("TERM".to_string(), "dumb".to_string());

    // Disable pagers
    env.insert("PAGER".to_string(), "cat".to_string());
    env.insert("GIT_PAGER".to_string(), "cat".to_string());
    env.insert("BAT_PAGER".to_string(), String::new());
    env.insert("SYSTEMD_PAGER".to_string(), String::new());

    // Configure less to not wait for input
    env.insert("LESS".to_string(), "-FRX".to_string());

    // Disable git terminal prompts
    env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());

    // Force color output (many tools disable color when TERM=dumb)
    env.insert("FORCE_COLOR".to_string(), "1".to_string());
    env.insert("CLICOLOR_FORCE".to_string(), "1".to_string());

    env
}

/// Create environment for interactive user terminals.
/// Supports TUI apps like htop, vim, etc.
pub fn interactive_env() -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Set proper terminal type for TUI apps
    env.insert("TERM".to_string(), "xterm-256color".to_string());

    // Enable color output
    env.insert("COLORTERM".to_string(), "truecolor".to_string());
    env.insert("FORCE_COLOR".to_string(), "1".to_string());
    env.insert("CLICOLOR".to_string(), "1".to_string());

    env
}

/// Result of spawning a PTY process
pub struct SpawnedPty {
    /// Channel to send bytes to the PTY
    pub writer_tx: mpsc::UnboundedSender<Vec<u8>>,
    /// Channel to receive output from the PTY
    pub output_rx: mpsc::UnboundedReceiver<PtyEvent>,
    /// Handle to resize the PTY
    pub resize_tx: mpsc::UnboundedSender<PtySize>,
}

/// Events from the PTY
#[derive(Debug)]
pub enum PtyEvent {
    /// Output bytes received
    Output(Vec<u8>),
    /// Process exited with code
    Exit(Option<i32>),
    /// Error occurred
    Error(String),
}

/// Spawn a PTY with the given configuration.
///
/// Returns a SpawnedPty with channels for communication.
pub fn spawn_pty(config: PtyConfig) -> Result<SpawnedPty, String> {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(config.size)
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    // Build the command
    let mut cmd = if cfg!(windows) {
        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.args(["-NoLogo", "-NoProfile", "-Command", &config.command]);
        cmd
    } else {
        let mut cmd = CommandBuilder::new("sh");
        cmd.args(["-lc", &config.command]);
        cmd
    };

    // Set working directory - default to current process directory if not specified
    // This ensures commands run in the expected directory even when LLM doesn't specify one
    let effective_cwd = config.cwd.clone().or_else(|| std::env::current_dir().ok());
    if let Some(cwd) = effective_cwd {
        cmd.cwd(cwd);
    }

    // Set environment variables from config (caller decides headless vs interactive)
    for (key, value) in &config.env {
        cmd.env(key, value);
    }

    // Spawn the child process
    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn command: {}", e))?;

    // Get reader and writer handles
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

    // Create channels
    let (writer_tx, writer_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (output_tx, output_rx) = mpsc::unbounded_channel::<PtyEvent>();
    let (resize_tx, mut resize_rx) = mpsc::unbounded_channel::<PtySize>();

    // Clone master for resize handling
    let master = pair.master;

    // Spawn read thread (blocking I/O)
    let output_tx_clone = output_tx.clone();
    thread::spawn(move || {
        read_pty_output(reader, output_tx_clone);
    });

    // Spawn write thread
    thread::spawn(move || {
        write_to_pty(writer, writer_rx);
    });

    // Spawn resize handler thread
    thread::spawn(move || {
        while let Some(new_size) = resize_rx.blocking_recv() {
            if let Err(e) = master.resize(new_size) {
                warn!("Failed to resize PTY: {}", e);
            }
        }
    });

    // Spawn child waiter thread
    let output_tx_exit = output_tx;
    thread::spawn(move || match child.wait() {
        Ok(status) => {
            let code = status.exit_code();
            debug!("PTY child exited with code: {:?}", code);
            // Convert u32 to Option<i32> - treat as signed for standard exit codes
            let exit_code = Some(code as i32);
            let _ = output_tx_exit.send(PtyEvent::Exit(exit_code));
        }
        Err(e) => {
            error!("Failed to wait for PTY child: {}", e);
            let _ = output_tx_exit.send(PtyEvent::Error(format!("Wait failed: {}", e)));
        }
    });

    Ok(SpawnedPty {
        writer_tx,
        output_rx,
        resize_tx,
    })
}

/// Read from PTY in a loop, sending output to channel
fn read_pty_output(mut reader: Box<dyn Read + Send>, tx: mpsc::UnboundedSender<PtyEvent>) {
    let mut buf = [0u8; 4096];

    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                debug!("PTY reader got EOF");
                break;
            }
            Ok(n) => {
                if tx.send(PtyEvent::Output(buf[..n].to_vec())).is_err() {
                    debug!("PTY output channel closed");
                    break;
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::Interrupted {
                    error!("PTY read error: {}", e);
                    let _ = tx.send(PtyEvent::Error(format!("Read error: {}", e)));
                    break;
                }
            }
        }
    }
}

/// Write to PTY from channel
fn write_to_pty(mut writer: Box<dyn Write + Send>, mut rx: mpsc::UnboundedReceiver<Vec<u8>>) {
    while let Some(data) = rx.blocking_recv() {
        if let Err(e) = writer.write_all(&data) {
            error!("PTY write error: {}", e);
            break;
        }
        if let Err(e) = writer.flush() {
            warn!("PTY flush error: {}", e);
        }
    }
}

/// Spawn a user shell (interactive terminal)
pub fn spawn_user_shell(cwd: Option<PathBuf>, size: PtySize) -> Result<SpawnedPty, String> {
    let shell = if cfg!(windows) {
        "powershell.exe".to_string()
    } else {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    };

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(size)
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let mut cmd = CommandBuilder::new(&shell);
    if cfg!(windows) {
        cmd.arg("-NoLogo");
    }

    if let Some(cwd) = &cwd {
        cmd.cwd(cwd);
    }

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {}", e))?;

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

    let (writer_tx, writer_rx) = mpsc::unbounded_channel();
    let (output_tx, output_rx) = mpsc::unbounded_channel();
    let (resize_tx, mut resize_rx) = mpsc::unbounded_channel();

    let master = pair.master;

    let output_tx_clone = output_tx.clone();
    thread::spawn(move || {
        read_pty_output(reader, output_tx_clone);
    });

    thread::spawn(move || {
        write_to_pty(writer, writer_rx);
    });

    thread::spawn(move || {
        while let Some(new_size) = resize_rx.blocking_recv() {
            if let Err(e) = master.resize(new_size) {
                warn!("Failed to resize PTY: {}", e);
            }
        }
    });

    let output_tx_exit = output_tx;
    thread::spawn(move || match child.wait() {
        Ok(status) => {
            let exit_code = Some(status.exit_code() as i32);
            let _ = output_tx_exit.send(PtyEvent::Exit(exit_code));
        }
        Err(e) => {
            let _ = output_tx_exit.send(PtyEvent::Error(format!("Wait failed: {}", e)));
        }
    });

    Ok(SpawnedPty {
        writer_tx,
        output_rx,
        resize_tx,
    })
}
