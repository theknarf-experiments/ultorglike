//! PostgreSQL, SQLite and DuckDB discovery, nested relational queries and staged writes.
//! Filtering, sorting, joins and formulas execute in the selected database.
pub mod catalog;
pub mod editing;
pub mod embedded;
pub mod error;
pub mod executor;
pub mod expression;
pub mod query;
pub mod schema;
pub use error::{Error, Result};
pub use query::{Filter, Formula, Node, Operator, Page, Query, Sort};
pub use schema::{Backend, Catalog, Column, ForeignKey, Relationship, TableInfo, TableName};
