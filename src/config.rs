pub struct Config {
    pub db_path: String,
    pub embedding_model: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            db_path: std::env::var("MEMORY_DB_PATH").unwrap_or_else(|_| "memory.db".to_string()),
            embedding_model: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string()),
        }
    }
}
