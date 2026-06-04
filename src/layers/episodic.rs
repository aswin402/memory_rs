use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::Connection;

pub struct EpisodicMemory {
    conn: Mutex<Connection>,
}

impl EpisodicMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS episodic_logs (
                id TEXT PRIMARY KEY,
                task_description TEXT NOT NULL,
                execution_status TEXT NOT NULL,
                steps_taken TEXT NOT NULL,
                error_message TEXT,
                reflection TEXT,
                created_at TEXT NOT NULL
            );"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}
