//! Text selection state management
//!
//! Provides rustpuppy-compatible text selection with:
//! - Drag selection tracking
//! - Normalized start/end positions
//! - Position containment checking for highlighting

/// Represents a position in the terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub row: u16,
    pub col: u16,
}

impl Position {
    pub fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }

    /// Convert to (line, col) tuple with usize
    pub fn as_tuple(&self) -> (usize, usize) {
        (self.row as usize, self.col as usize)
    }
}

/// Manages text selection state
///
/// Compatible with rustpuppy's TextSelection API.
#[derive(Debug, Default, Clone)]
pub struct SelectionState {
    /// Starting anchor of selection (where drag began)
    anchor: Option<Position>,
    /// Current extent of selection (where cursor is now)
    extent: Option<Position>,
    /// Whether we're actively selecting (mouse drag in progress)
    pub is_selecting: bool,
}

impl SelectionState {
    /// Create a new empty selection state
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin new selection at position
    pub fn start_selection(&mut self, pos: Position) {
        self.anchor = Some(pos);
        self.extent = Some(pos);
        self.is_selecting = true;
    }

    /// Begin selection with line/col coordinates
    pub fn start_at(&mut self, line: usize, col: usize) {
        self.start_selection(Position::new(line as u16, col as u16));
    }

    /// Update extent during drag
    pub fn update_selection(&mut self, pos: Position) {
        if self.is_selecting {
            self.extent = Some(pos);
        }
    }

    /// Update extent with line/col coordinates
    pub fn update_to(&mut self, line: usize, col: usize) {
        self.update_selection(Position::new(line as u16, col as u16));
    }

    /// Finish selecting (mouse released)
    pub fn end_selection(&mut self) {
        self.is_selecting = false;
    }

    /// Clear selection entirely
    pub fn clear(&mut self) {
        self.anchor = None;
        self.extent = None;
        self.is_selecting = false;
    }

    /// Whether a selection exists (has both start and end)
    pub fn is_active(&self) -> bool {
        self.anchor.is_some() && self.extent.is_some()
    }

    /// Whether currently dragging to select
    pub fn is_dragging(&self) -> bool {
        self.is_selecting
    }

    /// Get normalized selection as Position pair (start always before end)
    /// Returns None if no selection active
    pub fn get_selection(&self) -> Option<(Position, Position)> {
        match (self.anchor, self.extent) {
            (Some(start), Some(end)) => {
                if start.row < end.row || (start.row == end.row && start.col <= end.col) {
                    Some((start, end))
                } else {
                    Some((end, start))
                }
            }
            _ => None,
        }
    }

    /// Get normalized selection as ((line, col), (line, col)) tuples
    /// Matches rustpuppy's TextSelection::normalized() API
    pub fn normalized(&self) -> Option<((usize, usize), (usize, usize))> {
        self.get_selection()
            .map(|(start, end)| (start.as_tuple(), end.as_tuple()))
    }

    /// Check if position is within selection (for highlighting)
    /// Uses u16 coordinates (terminal cells)
    pub fn contains(&self, row: u16, col: u16) -> bool {
        self.contains_usize(row as usize, col as usize)
    }

    /// Check if position is within selection using usize coordinates
    /// Matches rustpuppy's TextSelection::contains() API
    pub fn contains_usize(&self, line: usize, col: usize) -> bool {
        if let Some(((start_line, start_col), (end_line, end_col))) = self.normalized() {
            if line < start_line || line > end_line {
                return false;
            }
            if line == start_line && line == end_line {
                return col >= start_col && col <= end_col;
            }
            if line == start_line {
                return col >= start_col;
            }
            if line == end_line {
                return col <= end_col;
            }
            true
        } else {
            false
        }
    }

    /// Get the raw start position (anchor)
    pub fn start(&self) -> Option<(usize, usize)> {
        self.anchor.map(|p| p.as_tuple())
    }

    /// Get the raw end position (extent)
    pub fn end(&self) -> Option<(usize, usize)> {
        self.extent.map(|p| p.as_tuple())
    }

    /// Get selection bounds as line range (for scrolling calculations)
    pub fn line_range(&self) -> Option<(usize, usize)> {
        self.normalized().map(|((start_line, _), (end_line, _))| (start_line, end_line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_selection() {
        let sel = SelectionState::new();
        assert!(!sel.is_active());
        assert!(!sel.is_dragging());
        assert!(sel.normalized().is_none());
        assert!(!sel.contains(0, 0));
    }

    #[test]
    fn test_single_line_selection() {
        let mut sel = SelectionState::new();
        sel.start_at(5, 10);
        sel.update_to(5, 20);
        sel.end_selection();

        assert!(sel.is_active());
        assert!(!sel.is_dragging());

        let norm = sel.normalized().unwrap();
        assert_eq!(norm, ((5, 10), (5, 20)));

        assert!(!sel.contains_usize(5, 9));
        assert!(sel.contains_usize(5, 10));
        assert!(sel.contains_usize(5, 15));
        assert!(sel.contains_usize(5, 20));
        assert!(!sel.contains_usize(5, 21));
    }

    #[test]
    fn test_multi_line_selection() {
        let mut sel = SelectionState::new();
        sel.start_at(5, 10);
        sel.update_to(8, 5);
        sel.end_selection();

        let norm = sel.normalized().unwrap();
        assert_eq!(norm, ((5, 10), (8, 5)));

        // Line 5: from col 10 onwards
        assert!(!sel.contains_usize(5, 9));
        assert!(sel.contains_usize(5, 10));
        assert!(sel.contains_usize(5, 100));

        // Lines 6-7: entire lines
        assert!(sel.contains_usize(6, 0));
        assert!(sel.contains_usize(6, 50));
        assert!(sel.contains_usize(7, 0));

        // Line 8: up to col 5
        assert!(sel.contains_usize(8, 0));
        assert!(sel.contains_usize(8, 5));
        assert!(!sel.contains_usize(8, 6));
    }

    #[test]
    fn test_backwards_selection() {
        let mut sel = SelectionState::new();
        sel.start_at(10, 20);  // Start lower-right
        sel.update_to(5, 10);  // Drag to upper-left
        sel.end_selection();

        // Should normalize to start before end
        let norm = sel.normalized().unwrap();
        assert_eq!(norm, ((5, 10), (10, 20)));
    }

    #[test]
    fn test_clear() {
        let mut sel = SelectionState::new();
        sel.start_at(5, 10);
        sel.update_to(5, 20);

        assert!(sel.is_active());

        sel.clear();

        assert!(!sel.is_active());
        assert!(sel.normalized().is_none());
    }
}
