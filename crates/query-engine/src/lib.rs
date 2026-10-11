//! PostgreSQL schema discovery and a serializable, nested relational query model.
//! All filtering, sorting and joins execute on the server in one SQL statement.
pub mod catalog;
pub mod editing;
pub mod error;
pub mod executor;
pub mod expression;
pub mod query;
pub mod schema;
pub use error::{Error, Result};
pub use query::{Filter, Formula, Node, Operator, Page, Query, Sort};
pub use schema::{Catalog, Column, ForeignKey, Relationship, TableInfo, TableName};
