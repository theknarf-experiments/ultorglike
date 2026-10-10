//! Cursor for navigating hierarchical views.
//!
//! The cursor tracks position within a nested relational structure, supporting:
//! - **Vertical navigation** (`j`/`k`): move between rows at the current level
//! - **Horizontal navigation** (`h`/`l`): move between columns (fields)
//! - **Expand/collapse** (`l` on nested relations): enter/exit child relations
//!
//! This follows the "spreadsheet-like cursor" described in the InfoVis 2013 paper,
//! where cursor movement follows the physical location of cells in the visual layout.

/// Direction of cursor movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// The current position of the cursor within a hierarchical view.
///
/// The cursor can be at different "levels" of nesting:
/// - **Root level**: navigating the main table (flat list of entities)
/// - **Nested level**: viewing a child relation (expanded via `l`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    /// The current row within the visible table
    pub row: usize,
    /// The current column (field index) within the visible table
    pub col: usize,
    /// The parent context, if we're in a nested relation
    /// `None` means we're at the root level
    pub parent: Option<ParentContext>,
}

/// Context for returning to the parent level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentContext {
    /// The parent row we came from
    pub parent_row: usize,
    /// The column in the parent that was expanded (the nested relation's field name)
    pub expanded_column: String,
}

impl Cursor {
    /// Create a new cursor at the top-left of a view.
    pub fn new() -> Self {
        Self {
            row: 0,
            col: 0,
            parent: None,
        }
    }

    /// Create a new cursor in a nested context.
    pub fn nested(parent_row: usize, expanded_column: String) -> Self {
        Self {
            row: 0,
            col: 0,
            parent: Some(ParentContext {
                parent_row,
                expanded_column,
            }),
        }
    }

    /// Get the parent context if we're in a nested view.
    pub fn parent_context(&self) -> Option<&ParentContext> {
        self.parent.as_ref()
    }

    /// Check if we're currently in a nested (child) view.
    pub fn is_nested(&self) -> bool {
        self.parent.is_some()
    }

    /// Create a new cursor at a specific row and column (root level).
    pub fn at(row: usize, col: usize) -> Self {
        Self {
            row,
            col,
            parent: None,
        }
    }

    /// Create a new cursor in a nested context at a specific row.
    pub fn at_nested(parent_row: usize, expanded_column: String, row: usize, col: usize) -> Self {
        Self {
            row,
            col,
            parent: Some(ParentContext {
                parent_row,
                expanded_column,
            }),
        }
    }

    /// Go back to the parent level (collapse nested view).
    /// Returns (parent_row, parent_col) to reposition at.
    pub fn collapse(&self) -> (usize, usize) {
        if let Some(ref parent) = self.parent {
            (parent.parent_row, 0)
        } else {
            (self.row, self.col)
        }
    }

    /// Expand into a nested relation.
    pub fn expand(&mut self, parent_row: usize, expanded_column: String) {
        self.row = 0;
        self.col = 0;
        self.parent = Some(ParentContext {
            parent_row,
            expanded_column,
        });
    }

    /// Clear any nested context (go back to root level).
    pub fn clear_nested(&mut self) {
        self.parent = None;
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

/// The target position for cursor movement.
///
/// Can represent either a position within the current view
/// or a request to expand into a nested relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Position {
    /// Position within the current (flat) view
    Flat { row: usize, col: usize },
    /// Position within a nested relation
    Nested {
        parent_row: usize,
        expanded_column: String,
    },
}

impl Position {
    /// Create a flat position.
    pub fn flat(row: usize, col: usize) -> Self {
        Self::Flat { row, col }
    }

    /// Create a nested position (expand).
    pub fn nested(parent_row: usize, expanded_column: String) -> Self {
        Self::Nested {
            parent_row,
            expanded_column,
        }
    }

    /// Returns true if this position represents an expansion into a nested view.
    pub fn is_nested(&self) -> bool {
        matches!(self, Position::Nested { .. })
    }

    /// Returns the row index for this position.
    pub fn row(&self) -> usize {
        match self {
            Position::Flat { row, .. } => *row,
            Position::Nested { parent_row, .. } => *parent_row,
        }
    }

    /// Returns the column index for this position (0 for nested).
    pub fn col(&self) -> usize {
        match self {
            Position::Flat { col, .. } => *col,
            Position::Nested { .. } => 0,
        }
    }
}
