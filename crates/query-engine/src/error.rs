#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Postgres(#[from] postgres::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("SQLite: {0}")]
    SQLite(#[from] rusqlite::Error),
    #[error("DuckDB: {0}")]
    DuckDB(#[from] duckdb::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
