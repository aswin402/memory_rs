use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("HNSW index error: {0}")]
    Hnsw(String),

    #[error("Lock poisoned: {0}")]
    LockPoisoned(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("Entity not found: {0}")]
    EntityNotFound(String),

    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<anyhow::Error> for MemoryError {
    fn from(err: anyhow::Error) -> Self {
        let err_str = err.to_string();
        if err_str.contains("HNSW") || err_str.contains("World") || err_str.contains("hnsw") {
            MemoryError::Hnsw(err_str)
        } else if err_str.contains("embedding") || err_str.contains("Fastembed") || err_str.contains("ONNX") || err_str.contains("model") {
            MemoryError::Embedding(err_str)
        } else {
            MemoryError::Other(err_str)
        }
    }
}

pub type Result<T> = std::result::Result<T, MemoryError>;
