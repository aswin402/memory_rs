pub struct Config {
    pub db_path: String,
    pub embedding_model: String,
    pub default_ttl: u64,
}

impl Config {
    pub fn from_env() -> Self {
        let db_path = std::env::var("MEMORY_DB_PATH").unwrap_or_else(|_| "memory.db".to_string());
        let embedding_model = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "all-MiniLM-L6-v2".to_string());
        let default_ttl = std::env::var("WORKING_MEMORY_TTL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300);

        Self {
            db_path,
            embedding_model,
            default_ttl,
        }
    }
}
