//! Shell command execution.

use std::process::{Command, Stdio};
use thiserror::Error;

/// Maximum characters in shell command output to protect context window
const SHELL_MAX_OUTPUT_CHARS: usize = 50_000;

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result of running a command.
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

/// Truncate output to a maximum character limit, cutting at line boundaries.
fn truncate_output(output: String, max_chars: usize) -> (String, bool) {
    if output.len() <= max_chars {
        return (output, false);
    }

    let mut truncated: String = output.chars().take(max_chars).collect();

    // Try to cut at a newline boundary for cleaner output
    if let Some(last_newline) = truncated.rfind('\n') {
        truncated.truncate(last_newline);
    }

    truncated.push_str(&format!(
        "\n\n[OUTPUT TRUNCATED: {} chars exceeded {} char limit]",
        output.len(),
        max_chars
    ));

    (truncated, true)
}

/// Command runner with configuration.
pub struct CommandRunner {
    working_dir: Option<String>,
}

impl CommandRunner {
    /// Create a new command runner.
    pub fn new() -> Self {
        Self { working_dir: None }
    }

    /// Set working directory.
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set timeout (note: timeout is handled by the shell tool, not here).
    pub fn timeout(self, _secs: u64) -> Self {
        // Timeout is not implemented in CommandRunner - it's handled at a higher level
        self
    }

    /// Run a command.
    pub fn run(&self, command: &str) -> Result<CommandResult, ShellError> {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg).arg(command);

        if let Some(dir) = &self.working_dir {
            cmd.current_dir(dir);
        }

        let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output()?;

        let exit_code = output.status.code().unwrap_or(-1);

        let (stdout, stdout_truncated) = truncate_output(
            String::from_utf8_lossy(&output.stdout).to_string(),
            SHELL_MAX_OUTPUT_CHARS,
        );
        let (stderr, stderr_truncated) = truncate_output(
            String::from_utf8_lossy(&output.stderr).to_string(),
            SHELL_MAX_OUTPUT_CHARS,
        );

        Ok(CommandResult {
            stdout,
            stderr,
            exit_code,
            success: output.status.success(),
            stdout_truncated,
            stderr_truncated,
        })
    }
}

impl Default for CommandRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_output_no_truncation() {
        let small = "hello world".to_string();
        let (result, truncated) = truncate_output(small.clone(), 1000);
        assert!(!truncated);
        assert_eq!(result, small);
    }

    #[test]
    fn test_truncate_output_with_truncation() {
        // Create output larger than limit
        let large = "x".repeat(60_000);
        let (result, truncated) = truncate_output(large, SHELL_MAX_OUTPUT_CHARS);
        assert!(truncated);
        assert!(result.len() < 60_000);
        assert!(result.contains("OUTPUT TRUNCATED"));
        assert!(result.contains("char limit"));
    }

    #[test]
    fn test_truncate_at_newline() {
        let content = "line1\nline2\nline3\nline4\nline5".to_string();
        // Set small limit that would cut in middle of line
        let (result, truncated) = truncate_output(content, 15);
        assert!(truncated);
        // Should cut at newline, not mid-line
        assert!(result.starts_with("line1\nline2"));
        assert!(result.contains("TRUNCATED"));
    }

    #[test]
    fn test_command_result_truncation_flags() {
        // This test verifies the CommandResult struct has truncation flags
        let result = CommandResult {
            stdout: "test".to_string(),
            stderr: "error".to_string(),
            exit_code: 0,
            success: true,
            stdout_truncated: true,
            stderr_truncated: false,
        };
        assert!(result.stdout_truncated);
        assert!(!result.stderr_truncated);
    }

    // =========================================================================
    // Additional CommandRunner Tests (from PR)
    // =========================================================================

    #[test]
    fn test_command_runner_new() {
        let runner = CommandRunner::new();
        assert!(runner.working_dir.is_none());
    }

    #[test]
    fn test_command_runner_default() {
        let runner = CommandRunner::default();
        assert!(runner.working_dir.is_none());
    }

    #[test]
    fn test_command_runner_builder_chain() {
        let runner = CommandRunner::new().working_dir("/tmp").timeout(30); // timeout is a no-op but should compile

        assert_eq!(runner.working_dir, Some("/tmp".to_string()));
    }

    #[test]
    fn test_run_simple_command() {
        let result = CommandRunner::new().run("echo hello").unwrap();
        assert!(result.success);
        assert!(result.stdout.contains("hello"));
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_shell_error_display() {
        let io_err = ShellError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert!(io_err.to_string().contains("IO error"));
    }

    #[test]
    fn test_empty_command() {
        // Empty command should still execute (shell handles it)
        let result = CommandRunner::new().run("");
        // Empty command typically succeeds with no output
        assert!(result.is_ok());
    }

    #[test]
    fn test_command_result_clone() {
        let result = CommandResult {
            stdout: "output".to_string(),
            stderr: "error".to_string(),
            exit_code: 42,
            success: false,
            stdout_truncated: false,
            stderr_truncated: false,
        };
        let cloned = result.clone();
        assert_eq!(result.stdout, cloned.stdout);
        assert_eq!(result.stderr, cloned.stderr);
        assert_eq!(result.exit_code, cloned.exit_code);
        assert_eq!(result.success, cloned.success);
    }
}
