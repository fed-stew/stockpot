//! Hit testing for clickable TUI elements
//!
//! Tracks rendered widget bounds and maps mouse coordinates to actions

use ratatui::layout::Rect;

/// Identifies a clickable element
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClickTarget {
    /// Agent dropdown trigger in toolbar
    AgentDropdown,
    /// Model dropdown trigger in toolbar
    ModelDropdown,
    /// Folder dropdown trigger in toolbar
    FolderDropdown,
    /// Agent dropdown item
    AgentItem(String),
    /// Model dropdown item
    ModelItem(String),
    /// Folder dropdown item (index in list)
    FolderItem(usize),
    /// Collapse/expand toggle for a section
    SectionToggle(String), // section_id
    /// Message content (for text selection)
    MessageContent,
    /// Settings button in header
    SettingsButton,

    // ─────────────────────────────────────────────────────────────────────────
    // Settings Dialog Click Targets
    // ─────────────────────────────────────────────────────────────────────────
    /// Settings tab clicked (tab index)
    SettingsTab(usize),
    /// Settings close area (clicking outside the dialog)
    SettingsClose,
    /// General tab: toggle switch clicked (setting ID)
    SettingsToggle(String),
    /// General tab: radio option clicked (setting ID, option index)
    SettingsRadio(String, usize),
    /// Models tab: provider row clicked to expand/collapse
    ModelsProvider(String),
    /// Models tab: model row clicked to set as default
    ModelsItem(String),
    /// Pinned Agents tab: agent selected
    PinnedAgentItem(usize),
    /// Pinned Agents tab: model selected for pinning
    PinnedModelItem(usize),
    /// MCP tab: server selected
    McpServerItem(usize),
    /// MCP tab: agent selected
    McpAgentItem(usize),
    /// MCP tab: checkbox toggled (server index)
    McpCheckbox(usize),
}

/// Tracks clickable regions for hit testing
#[derive(Debug, Default)]
pub struct HitTestRegistry {
    /// Map of regions to their click targets
    /// We use a vector and iterate in reverse (last rendered on top) for hit testing
    regions: Vec<(Rect, ClickTarget)>,
}

impl HitTestRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn register(&mut self, rect: Rect, target: ClickTarget) {
        self.regions.push((rect, target));
    }

    pub fn hit_test(&self, x: u16, y: u16) -> Option<&ClickTarget> {
        // Search in reverse order to respect Z-order (later registrations are "on top")
        for (rect, target) in self.regions.iter().rev() {
            if x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height {
                return Some(target);
            }
        }
        None
    }
}
