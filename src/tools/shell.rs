//! Shell command execution.

use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShellError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Command not found: {0}")]
    NotFound(String),
    #[error("Command failed with exit code {0}")]
    ExitCode(i32),
    #[error("Timeout after {0} seconds")]
    Timeout(u64),
}

/// Result of running a command.
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

/// Command runner with configuration.
pub struct CommandRunner {
    working_dir: Option<String>,
    timeout_secs: Option<u64>,
    env: Vec<(String, String)>,
}

impl CommandRunner {
    /// Create a new command runner.
    pub fn new() -> Self {
        Self {
            working_dir: None,
            timeout_secs: None,
            env: Vec::new(),
        }
    }

    /// Set working directory.
    pub fn working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Set timeout.
    pub fn timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// Add environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
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

        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        let output = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        let exit_code = output.status.code().unwrap_or(-1);

        Ok(CommandResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code,
            success: output.status.success(),
        })
    }

    /// Run a command with streaming output (callback for each line).
    pub fn run_streaming<F>(&self, command: &str, mut on_line: F) -> Result<CommandResult, ShellError>
    where
        F: FnMut(&str, bool), // (line, is_stderr)
    {
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg).arg(command);
        
        if let Some(dir) = &self.working_dir {
            cmd.current_dir(dir);
        }

        for (key, value) in &self.env {
            cmd.env(key, value);
        }

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let mut stdout_content = String::new();
        let mut stderr_content = String::new();

        // Read stdout
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                on_line(&line, false);
                stdout_content.push_str(&line);
                stdout_content.push('\n');
            }
        }

        // Read stderr
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                on_line(&line, true);
                stderr_content.push_str(&line);
                stderr_content.push('\n');
            }
        }

        let status = child.wait()?;
        let exit_code = status.code().unwrap_or(-1);

        Ok(CommandResult {
            stdout: stdout_content,
            stderr: stderr_content,
            exit_code,
            success: status.success(),
        })
    }
}

impl Default for CommandRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to run a simple command.
pub fn run_command(command: &str) -> Result<CommandResult, ShellError> {
    CommandRunner::new().run(command)
}
