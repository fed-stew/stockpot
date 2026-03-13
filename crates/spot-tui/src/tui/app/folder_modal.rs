//! Folder modal dialog for changing working directory.

use super::TuiApp;

impl TuiApp {
    /// Open the folder modal and load directory entries
    pub fn open_folder_modal(&mut self) {
        // Sync with actual current directory before opening
        if let Ok(actual) = std::env::current_dir() {
            self.current_working_dir = actual;
        }
        self.show_folder_modal = true;
        self.folder_modal_selected = 0;
        self.folder_modal_scroll = 0;
        self.load_folder_entries();
    }

    /// Close the folder modal
    pub fn close_folder_modal(&mut self) {
        self.show_folder_modal = false;
        self.folder_modal_entries.clear();
        self.folder_modal_selected = 0;
        self.folder_modal_scroll = 0;
    }

    /// Load directory entries for the current working directory
    pub fn load_folder_entries(&mut self) {
        self.folder_modal_entries.clear();

        if let Ok(entries) = std::fs::read_dir(&self.current_working_dir) {
            let mut dirs: Vec<std::path::PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter(|p| {
                    // Filter out hidden directories (except ..)
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| !n.starts_with('.') || n == "..")
                        .unwrap_or(false)
                })
                .collect();

            // Sort alphabetically
            dirs.sort_by(|a, b| {
                a.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_lowercase()
                    .cmp(
                        &b.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_lowercase(),
                    )
            });

            self.folder_modal_entries = dirs;
        }
    }

    /// Navigate to a directory in the folder modal
    pub fn folder_modal_navigate(&mut self, index: usize) {
        // Index 0 is ".." (parent directory)
        let new_path = if index == 0 {
            // Go to parent
            self.current_working_dir.parent().map(|p| p.to_path_buf())
        } else {
            // Navigate into the selected directory
            self.folder_modal_entries.get(index - 1).cloned()
        };

        if let Some(path) = new_path {
            // Actually change the working directory
            match std::env::set_current_dir(&path) {
                Ok(()) => {
                    self.current_working_dir = path;
                    self.error_message = None;
                    tracing::info!(
                        "Changed working directory to: {:?}",
                        self.current_working_dir
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to navigate to {:?}: {}", path, e);
                    self.error_message = Some(format!("Cannot access: {}", e));
                    // Don't update current_working_dir - stay where we are
                    return;
                }
            }
            self.folder_modal_selected = 0;
            self.folder_modal_scroll = 0;
            self.load_folder_entries();
        }
    }

    /// Ensure the selected item is visible in the folder modal
    pub fn folder_modal_ensure_visible(&mut self, visible_height: usize) {
        if self.folder_modal_selected < self.folder_modal_scroll {
            self.folder_modal_scroll = self.folder_modal_selected;
        } else if self.folder_modal_selected >= self.folder_modal_scroll + visible_height {
            self.folder_modal_scroll = self.folder_modal_selected - visible_height + 1;
        }
    }

    /// Confirm the current folder selection and close modal
    pub fn folder_modal_confirm(&mut self) {
        // Directory is already set during navigation, just close the modal
        self.close_folder_modal();
    }

    /// Get total items in folder modal (parent + entries)
    pub fn folder_modal_item_count(&self) -> usize {
        1 + self.folder_modal_entries.len() // 1 for ".." parent
    }
}
