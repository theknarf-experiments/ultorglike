//! A hierarchical view — the complete state for rendering and navigating data.
//!
//! The `HierarchicalView` combines:
//! - **Schema**: column names, types, and nesting metadata
//! - **Data**: the actual nested relational rows
//! - **State**: cursor position, active filters, sort order
//!
//! This is the primary data structure passed to the UI renderers.

use crate::cursor::{Cursor, Direction, Position};
use crate::nested::{NestedRelation, NestedRow, Value};
use arrow::datatypes::DataType;

/// Metadata about a column in the hierarchical view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub data_type: DataType,
    pub is_nested: bool,            // is this a 1-to-N relation field?
    pub child_columns: Vec<String>, // if nested, what are the child column names
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Filter on a column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub column_name: String,
    pub operator: FilterOperator,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOperator {
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    Contains,
    StartsWith,
    EndsWith,
}

/// The complete state of a hierarchical view.
///
/// This struct is the primary interface between the data model and the UI:
/// - It holds the schema, data, and cursor
/// - It provides methods for navigating and querying
/// - It's passed to renderers to produce terminal output
#[derive(Debug, Clone)]
pub struct HierarchicalView {
    /// Column metadata (name, type, nesting info)
    pub columns: Vec<ColumnMeta>,
    /// The data rows
    pub rows: Vec<NestedRow>,
    /// The current cursor position
    pub cursor: Cursor,
    /// Active filters
    pub filters: Vec<Filter>,
    /// Active sort (column index, direction)
    pub sort: Option<(usize, SortDirection)>,
    /// Title for display (e.g., table name)
    pub title: String,
    /// Whether this is a nested view (child relation)
    pub is_nested: bool,
}

impl HierarchicalView {
    /// Create a new flat (root-level) view.
    pub fn new(title: impl Into<String>, columns: Vec<String>, data_types: Vec<DataType>) -> Self {
        let metas: Vec<ColumnMeta> = columns
            .iter()
            .zip(data_types)
            .map(|(name, dt)| ColumnMeta {
                name: name.clone(),
                data_type: dt,
                is_nested: false,
                child_columns: vec![],
            })
            .collect();

        Self {
            columns: metas,
            rows: vec![],
            cursor: Cursor::new(),
            filters: vec![],
            sort: None,
            title: title.into(),
            is_nested: false,
        }
    }

    /// Create a new nested view (child relation).
    pub fn nested(
        title: impl Into<String>,
        parent_row: usize,
        expanded_column: impl Into<String>,
        columns: Vec<String>,
        data_types: Vec<DataType>,
    ) -> Self {
        let title_str = title.into();
        let expanded_str = expanded_column.into();
        let metas: Vec<ColumnMeta> = columns
            .iter()
            .zip(data_types)
            .map(|(name, dt)| ColumnMeta {
                name: name.clone(),
                data_type: dt,
                is_nested: false,
                child_columns: vec![],
            })
            .collect();

        Self {
            columns: metas,
            rows: vec![],
            cursor: Cursor::nested(parent_row, expanded_str.clone()),
            filters: vec![],
            sort: None,
            title: format!("{} > {}", title_str, expanded_str),
            is_nested: true,
        }
    }

    /// Add a row to this view.
    pub fn add_row(&mut self, row: NestedRow) {
        self.rows.push(row);
    }

    /// Add multiple rows.
    pub fn add_rows(&mut self, rows: Vec<NestedRow>) {
        self.rows.extend(rows);
    }

    /// Get the current row under the cursor.
    pub fn current_row(&self) -> Option<&NestedRow> {
        self.rows.get(self.cursor.row)
    }

    /// Get the current cell value under the cursor.
    pub fn current_value(&self) -> Option<&Value> {
        self.current_row()
            .and_then(|r| r.values.get(self.cursor.col))
    }

    /// Maximum row index in the view.
    pub fn max_row(&self) -> usize {
        self.rows.len().saturating_sub(1)
    }

    /// Maximum column index in the view.
    pub fn max_col(&self) -> usize {
        self.columns.len().saturating_sub(1)
    }

    /// Check if a column at a given row is a nested relation.
    pub fn is_nested_relation(&self, row: usize, col: usize) -> bool {
        if let Some(row_data) = self.rows.get(row) {
            if let Some(value) = row_data.values.get(col) {
                matches!(value, Value::Relation(_))
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get the nested relation at a specific row and column.
    pub fn get_nested_relation(&self, row: usize, col: usize) -> Option<&NestedRelation> {
        self.rows.get(row)?.values.get(col)?.get_relation()
    }

    /// Find the column index by name.
    pub fn find_column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// Navigate in the given direction.
    /// Returns the new Position (Flat or Nested).
    pub fn navigate(&mut self, direction: Direction) -> Position {
        match direction {
            Direction::Up => self.navigate_up(),
            Direction::Down => self.navigate_down(),
            Direction::Left => self.navigate_left(),
            Direction::Right => self.navigate_right(),
        }
    }

    fn navigate_up(&mut self) -> Position {
        if self.cursor.row == 0 {
            return Position::flat(0, self.cursor.col);
        }
        self.cursor.row -= 1;
        Position::flat(self.cursor.row, self.cursor.col)
    }

    fn navigate_down(&mut self) -> Position {
        if self.cursor.row >= self.max_row() {
            return Position::flat(self.cursor.row, self.cursor.col);
        }
        self.cursor.row += 1;
        Position::flat(self.cursor.row, self.cursor.col)
    }

    fn navigate_left(&mut self) -> Position {
        if self.cursor.col == 0 {
            // At first column — if nested, go back to parent
            if let Some(ref parent) = self.cursor.parent {
                self.cursor.col = self.find_column_index(&parent.expanded_column).unwrap_or(0);
                self.cursor.clear_nested();
            }
            Position::flat(self.cursor.row, self.cursor.col)
        } else {
            self.cursor.col -= 1;
            Position::flat(self.cursor.row, self.cursor.col)
        }
    }

    fn navigate_right(&mut self) -> Position {
        let max_col = self.max_col();
        if self.cursor.col >= max_col {
            return Position::flat(self.cursor.row, self.cursor.col);
        }

        self.cursor.col += 1;

        // Check if the next column is a nested relation
        if self.is_nested_relation(self.cursor.row, self.cursor.col) {
            let col_name = self.columns[self.cursor.col].name.clone();
            // Expand into nested view
            self.cursor.expand(self.cursor.row, col_name.clone());
            Position::nested(self.cursor.row, col_name)
        } else {
            Position::flat(self.cursor.row, self.cursor.col)
        }
    }

    /// Jump to the first row.
    pub fn jump_to_first(&mut self) -> Position {
        self.cursor.row = 0;
        Position::flat(0, self.cursor.col)
    }

    /// Jump to the last row.
    pub fn jump_to_last(&mut self) -> Position {
        let max = self.max_row();
        self.cursor.row = max;
        Position::flat(max, self.cursor.col)
    }

    /// Render the visible portion of this view as a text table.
    ///
    /// This is a simplified representation — the full TUI rendering
    /// uses the `tui-widgets` crate for proper rendering.
    pub fn to_text_display(&self) -> String {
        let mut output = format!(" {} \n", self.title);

        // Header
        for (i, col) in self.columns.iter().enumerate() {
            let is_nested_marker = if col.is_nested { " ▼" } else { "" };
            let is_current = if i == self.cursor.col && !self.cursor.is_nested() {
                " >>"
            } else {
                ""
            };
            output.push_str(&format!("{:<20}{}", col.name, is_nested_marker));
            if is_current.is_empty() {
                output.push(' ');
            }
        }
        output.push('\n');

        // Separator
        output.push_str(&"─".repeat(40 * self.columns.len().min(10)));
        output.push('\n');

        // Show 10 rows starting from cursor
        let start = self.cursor.row.saturating_sub(5);
        let end = (self.cursor.row + 10).min(self.rows.len());

        for row_idx in start..end {
            let is_current_row = row_idx == self.cursor.row;
            let marker = if is_current_row { ">>" } else { "  " };

            if let Some(row) = self.rows.get(row_idx) {
                output.push_str(marker);
                for (col_idx, _col_meta) in self.columns.iter().enumerate().take(10) {
                    let display = if col_idx < row.values.len() {
                        match &row.values[col_idx] {
                            Value::Primitive(pv) => pv.value.to_display_string(),
                            Value::Relation(nr) => {
                                if nr.is_empty() {
                                    "[none]".to_string()
                                } else {
                                    format!("[{} items]", nr.len())
                                }
                            }
                        }
                    } else {
                        "".to_string()
                    };
                    let is_current_col =
                        col_idx == self.cursor.col && is_current_row && !self.cursor.is_nested();
                    if is_current_col {
                        output.push_str(&format!(" ▸{:<18} ", display));
                    } else {
                        output.push_str(&format!(" {:<18}", display));
                    }
                }
            }
            output.push('\n');
        }

        output
    }
}
