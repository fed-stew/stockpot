//! Text selection state management

/// Represents a position in the terminal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub row: u16,
    pub col: u16,
}

impl Position {
    pub fn new(row: u16, col: u16) -> Self {
        Self { row, col }
    }
}

/// Manages text selection state
#[derive(Debug, Default, Clone)]
pub struct SelectionState {
    /// Starting anchor of selection (where drag began)
    anchor: Option<Position>,
    /// Current extent of selection (where cursor is now)
    extent: Option<Position>,
    /// Whether we're actively selecting
    is_selecting: bool,
}

impl SelectionState {
    /// Begin new selection
    pub fn start_selection(&mut self, pos: Position) {
        self.anchor = Some(pos);
        self.extent = Some(pos);
        self.is_selecting = true;
    }

    /// Update extent during drag
    pub fn update_selection(&mut self, pos: Position) {
        if self.is_selecting {
            self.extent = Some(pos);
        }
    }

    /// Finish selecting
    pub fn end_selection(&mut self) {
        self.is_selecting = false;
    }

    /// Clear selection
    pub fn clear(&mut self) {
        self.anchor = None;
        self.extent = None;
        self.is_selecting = false;
    }

    /// Get normalized start/end (start always before end)
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

    /// Whether selection is in progress (mouse drag active)
    pub fn is_active(&self) -> bool {
        self.is_selecting
    }

    /// Check if position is within selection (for highlighting)
    pub fn contains(&self, row: u16, col: u16) -> bool {
        if let Some((start, end)) = self.get_selection() {
            // Check rows
            if row < start.row || row > end.row {
                return false;
            }

            // Single line selection
            if start.row == end.row {
                return col >= start.col && col <= end.col;
            }

            // Multi-line selection
            if row == start.row {
                // First line: from start col to end of line
                col >= start.col
            } else if row == end.row {
                // Last line: from start of line to end col
                col <= end.col
            } else {
                // Middle lines: all selected
                true
            }
        } else {
            false
        }
    }
}
