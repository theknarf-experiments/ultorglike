//! # Data Model
//!
//! Nested relational data model for spreadsheet-style terminal navigation.
//!
//! This crate provides the core data structures that represent hierarchical
//! relational data — the kind of structure described in Eirik Bakke's research
//! (SIEUFERD, Related Worksheets).
//!
//! ## Core Concepts
//!
//! - [`NestedRow`] — represents a single row with primitive and nested relation values
//! - [`Cursor`] — tracks position within a hierarchical data structure
//! - [`HierarchicalView`] — a complete view combining schema metadata with data
//! - [`recordbatch_to_view`] — converts Arrow RecordBatch to HierarchicalView
//!
//! ## Design Principles
//!
//! From the research papers, the key requirements are:
//!
//! 1. **Hierarchical views** — one-to-many relationships appear as nested tables, not flat joins
//! 2. **Schema-independent** — the same structures work for any database schema
//! 3. **Spreadsheet-like cursor** — keyboard navigation (j/k/h/l) moves through the visual layout
//! 4. **Form + table hybrid** — combine outline (form-style) with nested table layouts

pub mod cursor;
pub mod from_arrow;
pub mod nested;
pub mod view;

#[cfg(test)]
mod tests;

pub use cursor::{Cursor, Direction, ParentContext, Position};
pub use from_arrow::{demo_customers_and_orders, recordbatch_to_view};
pub use nested::{CellValue, NestedRelation, NestedRow, PrimitiveValue, Value};
pub use view::HierarchicalView;

// Re-export arrow types for convenience
pub use arrow::datatypes::DataType;

pub mod document;
