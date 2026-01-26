//! File attachment support for TUI
//!
//! In TUI mode, we support:
//! - File paths (drag & drop not available in terminals)
//! - Paste file paths
//! - Basic attachment preview

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum TuiAttachment {
    File { path: PathBuf, filename: String },
    // Note: Images work but we can't preview them in terminal
    // They'll be sent as base64 to vision models
}

#[derive(Debug, Default)]
pub struct AttachmentManager {
    pub pending: Vec<TuiAttachment>,
}

impl AttachmentManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_file(&mut self, path: PathBuf) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("File does not exist: {}", path.display()));
        }

        let filename = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        self.pending.push(TuiAttachment::File { path, filename });
        Ok(())
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.pending.len() {
            self.pending.remove(index);
        }
    }

    pub fn clear(&mut self) {
        self.pending.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}
