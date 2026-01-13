//! System execution store for tracking terminal processes.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{oneshot, Notify};

use super::types::{ProcessSnapshot, SystemExecResponse};

#[cfg(test)]
use super::types::ProcessKind;

/// Entry in the process store
struct ProcessEntry {
    snapshot: ProcessSnapshot,
    notify: Arc<Notify>,
}

/// Inner state protected by mutex
#[derive(Default)]
struct Inner {
    processes: HashMap<String, ProcessEntry>,
    pending: HashMap<u64, oneshot::Sender<SystemExecResponse>>,
}

/// Store for tracking all terminal processes
#[derive(Default)]
pub struct SystemExecStore {
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for SystemExecStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemExecStore").finish_non_exhaustive()
    }
}

impl SystemExecStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pending request that's waiting for a response
    pub fn register_pending(
        &self,
        request_id: u64,
        responder: oneshot::Sender<SystemExecResponse>,
    ) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.pending.insert(request_id, responder);
    }

    /// Send response to a pending request
    pub fn respond(&self, request_id: u64, response: SystemExecResponse) {
        let responder = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.pending.remove(&request_id)
        };
        if let Some(tx) = responder {
            let _ = tx.send(response);
        }
    }

    /// Insert or update a process
    pub fn upsert_process(&self, snapshot: ProcessSnapshot) {
        let notify = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let notify = inner
                .processes
                .get(&snapshot.process_id)
                .map(|p| p.notify.clone())
                .unwrap_or_else(|| Arc::new(Notify::new()));
            inner.processes.insert(
                snapshot.process_id.clone(),
                ProcessEntry {
                    snapshot,
                    notify: notify.clone(),
                },
            );
            notify
        };
        notify.notify_waiters();
    }

    /// Update output for a process
    pub fn set_output(&self, process_id: &str, output: String) {
        let notify = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(entry) = inner.processes.get_mut(process_id) else {
                return;
            };
            entry.snapshot.output = output;
            entry.notify.clone()
        };
        notify.notify_waiters();
    }

    /// Mark a process as finished
    pub fn mark_finished(&self, process_id: &str, exit_code: Option<i32>) {
        let now_ms = now_ms();
        let notify = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(entry) = inner.processes.get_mut(process_id) else {
                return;
            };
            entry.snapshot.exit_code = exit_code;
            entry.snapshot.finished_at_ms = Some(now_ms);
            entry.notify.clone()
        };
        notify.notify_waiters();
    }

    /// Set visibility of a process
    pub fn set_visible(&self, process_id: &str, visible: bool) {
        let notify = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let Some(entry) = inner.processes.get_mut(process_id) else {
                return;
            };
            entry.snapshot.visible = visible;
            entry.notify.clone()
        };
        notify.notify_waiters();
    }

    /// Remove a process from the store
    pub fn remove_process(&self, process_id: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.processes.remove(process_id);
    }

    /// List all visible process IDs
    pub fn list_visible(&self) -> Vec<String> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut ids: Vec<String> = inner
            .processes
            .values()
            .filter(|p| p.snapshot.visible)
            .map(|p| p.snapshot.process_id.clone())
            .collect();
        ids.sort();
        ids
    }

    /// Get snapshot of a process
    pub fn snapshot(&self, process_id: &str) -> Option<ProcessSnapshot> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.processes.get(process_id).map(|p| p.snapshot.clone())
    }

    /// Get output of a process
    pub fn output(&self, process_id: &str) -> Option<String> {
        self.snapshot(process_id).map(|s| s.output)
    }

    /// Get exit code of a process
    pub fn exit_code(&self, process_id: &str) -> Option<Option<i32>> {
        self.snapshot(process_id).map(|s| s.exit_code)
    }

    /// Wait for an update to a process
    pub async fn wait_for_update(&self, process_id: &str) -> Result<(), String> {
        let notify = {
            let inner = self
                .inner
                .lock()
                .map_err(|e| format!("Lock poisoned: {}", e))?;
            inner
                .processes
                .get(process_id)
                .map(|p| p.notify.clone())
                .ok_or_else(|| format!("Unknown process: {}", process_id))?
        };
        notify.notified().await;
        Ok(())
    }

    /// Get all process snapshots (for UI listing)
    pub fn all_snapshots(&self) -> Vec<ProcessSnapshot> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .processes
            .values()
            .map(|p| p.snapshot.clone())
            .collect()
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_basic_operations() {
        let store = SystemExecStore::new();

        let snapshot = ProcessSnapshot {
            process_id: "test-1".to_string(),
            kind: ProcessKind::Llm,
            visible: true,
            output: String::new(),
            exit_code: None,
            started_at_ms: 1000,
            finished_at_ms: None,
        };

        store.upsert_process(snapshot);

        assert!(store.snapshot("test-1").is_some());
        assert_eq!(store.list_visible(), vec!["test-1"]);

        store.set_output("test-1", "Hello, World!".to_string());
        assert_eq!(store.output("test-1"), Some("Hello, World!".to_string()));

        store.mark_finished("test-1", Some(0));
        assert_eq!(store.exit_code("test-1"), Some(Some(0)));
    }

    #[test]
    fn test_store_visibility() {
        let store = SystemExecStore::new();

        let snapshot = ProcessSnapshot {
            process_id: "test-2".to_string(),
            kind: ProcessKind::User,
            visible: true,
            output: String::new(),
            exit_code: None,
            started_at_ms: 2000,
            finished_at_ms: None,
        };

        store.upsert_process(snapshot);
        assert_eq!(store.list_visible().len(), 1);

        store.set_visible("test-2", false);
        assert_eq!(store.list_visible().len(), 0);
    }
}
