use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::Connection;

pub struct SemanticMemory {
    conn: Mutex<Connection>,
}

impl SemanticMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS semantic_metadata (
                node_id TEXT PRIMARY KEY,
                raw_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0
            );"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}
