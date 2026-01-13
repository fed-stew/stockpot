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
    /// Agent dropdown item
    AgentItem(String),
    /// Model dropdown item
    ModelItem(String),
    /// Collapse/expand toggle for a section
    SectionToggle(String), // section_id
    /// Message content (for text selection)
    MessageContent,
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
