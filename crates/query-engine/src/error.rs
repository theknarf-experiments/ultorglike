#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Postgres(#[from] postgres::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, Error>;
