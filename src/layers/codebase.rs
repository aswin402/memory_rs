use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::Connection;

pub struct CodebaseMemory {
    conn: Mutex<Connection>,
}

impl CodebaseMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS codebase_signatures (
                id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                item_name TEXT NOT NULL,
                item_type TEXT NOT NULL,
                signature TEXT NOT NULL,
                dependencies TEXT
            );"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}
