//! Tool execution context for terminal integration.
//!
//! Provides shared state for tools that need to spawn processes
//! and interact with the terminal system.

use std::sync::Arc;
use tokio::sync::mpsc;

use crate::terminal::{SystemExecRequest, SystemExecResponse, SystemExecStore};

/// Context for tool execution with terminal integration.
///
/// This provides tools with access to:
/// - Process store for tracking running terminals
/// - Channel to request terminal spawning from UI
#[derive(Clone)]
pub struct ToolContext {
    /// Process store for tracking terminals
    pub store: Arc<SystemExecStore>,
    /// Channel to send execution requests to UI
    pub request_tx: mpsc::UnboundedSender<SystemExecRequest>,
    /// Counter for generating request IDs
    next_request_id: Arc<std::sync::atomic::AtomicU64>,
}

impl ToolContext {
    /// Create a new tool context.
    pub fn new(
        store: Arc<SystemExecStore>,
        request_tx: mpsc::UnboundedSender<SystemExecRequest>,
    ) -> Self {
        Self {
            store,
            request_tx,
            next_request_id: Arc::new(std::sync::atomic::AtomicU64::new(1)),
        }
    }

    /// Generate a unique request ID.
    pub fn next_request_id(&self) -> u64 {
        self.next_request_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Send an execution request and register for response.
    pub fn send_request(
        &self,
        request: SystemExecRequest,
    ) -> Result<tokio::sync::oneshot::Receiver<SystemExecResponse>, String> {
        let request_id = match &request {
            SystemExecRequest::ExecuteShell { request_id, .. } => *request_id,
            SystemExecRequest::KillProcess { request_id, .. } => *request_id,
        };

        let (tx, rx) = tokio::sync::oneshot::channel();
        self.store.register_pending(request_id, tx);

        self.request_tx
            .send(request)
            .map_err(|e| format!("Failed to send request: {}", e))?;

        Ok(rx)
    }

    /// Execute a shell command and wait for process ID.
    pub async fn execute_shell(
        &self,
        command: String,
        cwd: Option<String>,
    ) -> Result<String, String> {
        let request_id = self.next_request_id();
        let request = SystemExecRequest::ExecuteShell {
            request_id,
            command,
            cwd,
        };

        let rx = self.send_request(request)?;

        match rx.await {
            Ok(SystemExecResponse::Started { process_id }) => Ok(process_id),
            Ok(SystemExecResponse::Error { message }) => Err(message),
            Ok(SystemExecResponse::Killed { .. }) => Err("Unexpected kill response".to_string()),
            Err(_) => Err("Response channel closed".to_string()),
        }
    }

    /// Kill a process by ID.
    pub async fn kill_process(&self, process_id: String) -> Result<(), String> {
        let request_id = self.next_request_id();
        let request = SystemExecRequest::KillProcess {
            request_id,
            process_id: process_id.clone(),
        };

        let rx = self.send_request(request)?;

        match rx.await {
            Ok(SystemExecResponse::Killed { .. }) => Ok(()),
            Ok(SystemExecResponse::Error { message }) => Err(message),
            Ok(SystemExecResponse::Started { .. }) => Err("Unexpected start response".to_string()),
            Err(_) => Err("Response channel closed".to_string()),
        }
    }

    /// Wait for a process to complete with timeout.
    ///
    /// Returns the output if process completes within timeout,
    /// or None if still running.
    pub async fn wait_for_completion(
        &self,
        process_id: &str,
        timeout: std::time::Duration,
    ) -> Option<(String, Option<i32>)> {
        let deadline = std::time::Instant::now() + timeout;

        loop {
            // Check if process has exited
            if let Some(snapshot) = self.store.snapshot(process_id) {
                if snapshot.exit_code.is_some() {
                    return Some((snapshot.output, snapshot.exit_code));
                }
            } else {
                // Process not found
                return None;
            }

            // Check timeout
            if std::time::Instant::now() >= deadline {
                return None;
            }

            // Wait for update or short sleep
            tokio::select! {
                _ = self.store.wait_for_update(process_id) => {}
                _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
            }
        }
    }
}

impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("next_request_id", &self.next_request_id)
            .finish_non_exhaustive()
    }
}

/// Global tool context holder.
///
/// Tools can access this via thread-local storage or pass it through RunContext.
static TOOL_CONTEXT: std::sync::OnceLock<ToolContext> = std::sync::OnceLock::new();

/// Set the global tool context.
pub fn set_global_context(ctx: ToolContext) -> Result<(), ToolContext> {
    TOOL_CONTEXT.set(ctx)
}

/// Get the global tool context.
pub fn get_global_context() -> Option<&'static ToolContext> {
    TOOL_CONTEXT.get()
}
