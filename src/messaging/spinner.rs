//! Animated spinner for showing activity during LLM calls.
//!
//! Provides a simple terminal spinner that shows progress and context info.

use crossterm::{
    cursor::{Hide, MoveToColumn, Show},
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    ExecutableCommand,
};
use std::io::{stdout, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// Spinner animation frames.
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Alternative spinner styles.
pub const DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const LINE: &[&str] = &["-", "\\", "|", "/"];
pub const CIRCLE: &[&str] = &["◐", "◓", "◑", "◒"];
pub const BOUNCE: &[&str] = &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"];

/// Spinner configuration.
#[derive(Clone)]
pub struct SpinnerConfig {
    /// Animation frames.
    pub frames: Vec<&'static str>,
    /// Frame duration in milliseconds.
    pub interval_ms: u64,
    /// Spinner color.
    pub color: Color,
    /// Whether to show token count.
    pub show_tokens: bool,
}

impl Default for SpinnerConfig {
    fn default() -> Self {
        Self {
            frames: SPINNER_FRAMES.to_vec(),
            interval_ms: 80,
            color: Color::Cyan,
            show_tokens: true,
        }
    }
}

/// A spinner handle for controlling the animation.
pub struct SpinnerHandle {
    stop_tx: watch::Sender<bool>,
    task: Option<tokio::task::JoinHandle<()>>,
    tokens: Arc<AtomicUsize>,
    max_tokens: Arc<AtomicUsize>,
    is_paused: Arc<AtomicBool>,
}

impl SpinnerHandle {
    /// Update the token count display.
    pub fn set_tokens(&self, current: usize, max: usize) {
        self.tokens.store(current, Ordering::Relaxed);
        self.max_tokens.store(max, Ordering::Relaxed);
    }

    /// Pause the spinner (e.g., during tool output).
    pub fn pause(&self) {
        self.is_paused.store(true, Ordering::Relaxed);
    }

    /// Resume the spinner.
    pub fn resume(&self) {
        self.is_paused.store(false, Ordering::Relaxed);
    }

    /// Stop the spinner.
    pub async fn stop(mut self) {
        let _ = self.stop_tx.send(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
        // Clear the spinner line
        let mut stdout = stdout();
        let _ = stdout.execute(MoveToColumn(0));
        let _ = stdout.execute(Clear(ClearType::CurrentLine));
        let _ = stdout.execute(Show);
    }

    /// Stop the spinner synchronously (non-async).
    pub fn stop_sync(&mut self) {
        let _ = self.stop_tx.send(true);
        // Clear the spinner line
        let mut stdout = stdout();
        let _ = stdout.execute(MoveToColumn(0));
        let _ = stdout.execute(Clear(ClearType::CurrentLine));
        let _ = stdout.execute(Show);
    }
}

impl Drop for SpinnerHandle {
    fn drop(&mut self) {
        let _ = self.stop_tx.send(true);
        // Ensure cursor is shown
        let mut stdout = stdout();
        let _ = stdout.execute(Show);
    }
}

/// Spinner for showing activity.
pub struct Spinner {
    config: SpinnerConfig,
}

impl Spinner {
    /// Create a new spinner with default config.
    pub fn new() -> Self {
        Self {
            config: SpinnerConfig::default(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: SpinnerConfig) -> Self {
        Self { config }
    }

    /// Start the spinner with a message.
    pub fn start(&self, message: impl Into<String>) -> SpinnerHandle {
        let config = self.config.clone();
        let message = message.into();
        let (stop_tx, mut stop_rx) = watch::channel(false);
        let tokens = Arc::new(AtomicUsize::new(0));
        let max_tokens = Arc::new(AtomicUsize::new(0));
        let is_paused = Arc::new(AtomicBool::new(false));

        let tokens_clone = tokens.clone();
        let max_tokens_clone = max_tokens.clone();
        let is_paused_clone = is_paused.clone();

        let task = tokio::spawn(async move {
            let mut frame_idx = 0;
            let mut stdout = stdout();

            // Hide cursor
            let _ = stdout.execute(Hide);

            loop {
                // Check for stop signal
                if *stop_rx.borrow() {
                    break;
                }

                // Skip rendering if paused
                if is_paused_clone.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }

                // Get current token count
                let current_tokens = tokens_clone.load(Ordering::Relaxed);
                let max = max_tokens_clone.load(Ordering::Relaxed);

                // Build the status line
                let frame = config.frames[frame_idx % config.frames.len()];
                let token_info = if config.show_tokens && max > 0 {
                    format!(
                        " [{}/{}]",
                        format_tokens(current_tokens),
                        format_tokens(max)
                    )
                } else {
                    String::new()
                };

                // Render
                let _ = stdout.execute(MoveToColumn(0));
                let _ = stdout.execute(Clear(ClearType::CurrentLine));
                let _ = stdout.execute(SetForegroundColor(config.color));
                let _ = stdout.execute(Print(format!("{} {}{}", frame, message, token_info)));
                let _ = stdout.execute(ResetColor);
                let _ = stdout.flush();

                frame_idx += 1;

                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(config.interval_ms)) => {}
                    _ = stop_rx.changed() => { break; }
                }
            }

            // Show cursor
            let _ = stdout.execute(Show);
        });

        SpinnerHandle {
            stop_tx,
            task: Some(task),
            tokens,
            max_tokens,
            is_paused,
        }
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

/// Format token count for display with space as thousands separator.
fn format_tokens(count: usize) -> String {
    crate::tokens::format_tokens_with_separator(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1 500");
        assert_eq!(format_tokens(128000), "128 000");
        assert_eq!(format_tokens(1500000), "1 500 000");
    }

    #[test]
    fn test_spinner_config_default() {
        let config = SpinnerConfig::default();
        assert_eq!(config.interval_ms, 80);
        assert!(config.show_tokens);
    }

    #[tokio::test]
    async fn test_spinner_lifecycle() {
        let spinner = Spinner::new();
        let handle = spinner.start("Testing...");

        // Let it spin briefly
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Update tokens
        handle.set_tokens(1000, 128000);

        // Stop it
        handle.stop().await;
    }
}
