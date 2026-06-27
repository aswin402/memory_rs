use crate::error::Result;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SharedMemoryItem {
    pub key: String,
    pub value: String,
    pub source_agent: String,
    pub target_agents: Vec<String>,
    pub importance: f64,
    pub timestamp: String,
}

pub struct SharedMemory {
    conn: Mutex<Connection>,
}

impl SharedMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS shared_agent_memory (
                memory_key TEXT,
                memory_value TEXT NOT NULL,
                source_agent TEXT NOT NULL,
                target_agents TEXT NOT NULL, -- JSON array of target agent IDs
                importance REAL NOT NULL DEFAULT 1.0,
                timestamp TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (memory_key, user_id, session_id, agent_id)
            );
            CREATE INDEX IF NOT EXISTS idx_shared_agent_memory_scope ON shared_agent_memory (user_id, session_id, agent_id);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE shared_agent_memory ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE shared_agent_memory ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE shared_agent_memory ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_shared_agent_memory_scope ON shared_agent_memory (user_id, session_id, agent_id)", []);

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn store_shared_memory(&self, item: SharedMemoryItem, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let targets_json = serde_json::to_string(&item.target_agents)?;
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        conn.execute(
            "INSERT OR REPLACE INTO shared_agent_memory 
             (memory_key, memory_value, source_agent, target_agents, importance, timestamp, user_id, session_id, agent_id) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                item.key,
                item.value,
                item.source_agent,
                targets_json,
                item.importance,
                item.timestamp,
                user_id,
                session_id,
                agent_id
            ],
        )?;
        Ok(())
    }

    pub fn retrieve_shared_memory(&self, agent_id: &str, scope: &crate::layers::MemoryScope) -> Result<Vec<SharedMemoryItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT memory_key, memory_value, source_agent, target_agents, importance, timestamp 
             FROM shared_agent_memory
             WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*')
               AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
               AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')",
        )?;
        let mut rows = stmt.query(params![scope.user_id, scope.session_id, scope.agent_id])?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            let source_agent: String = row.get(2)?;
            let targets_json: String = row.get(3)?;
            let importance: f64 = row.get(4)?;
            let timestamp: String = row.get(5)?;

            let target_agents: Vec<String> =
                serde_json::from_str(&targets_json).unwrap_or_default();

            // Filter: if agent_id is empty, return all. Otherwise, check if agent_id is in target_agents list or if it's wildcard "*"
            if agent_id.is_empty()
                || target_agents.contains(&agent_id.to_string())
                || target_agents.contains(&"*".to_string())
                || source_agent == agent_id
            {
                results.push(SharedMemoryItem {
                    key,
                    value,
                    source_agent,
                    target_agents,
                    importance,
                    timestamp,
                });
            }
        }
        Ok(results)
    }

    pub fn delete_shared_memory(&self, key: &str, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        conn.execute(
            "DELETE FROM shared_agent_memory WHERE memory_key = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
            params![key, user_id, session_id, agent_id],
        )?;
        Ok(())
    }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS shared_agent_memory (
                memory_key TEXT,
                memory_value TEXT NOT NULL,
                source_agent TEXT NOT NULL,
                target_agents TEXT NOT NULL, -- JSON array of target agent IDs
                importance REAL NOT NULL DEFAULT 1.0,
                timestamp TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (memory_key, user_id, session_id, agent_id)
            );
            CREATE INDEX IF NOT EXISTS idx_shared_agent_memory_scope ON shared_agent_memory (user_id, session_id, agent_id);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE shared_agent_memory ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE shared_agent_memory ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE shared_agent_memory ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_shared_agent_memory_scope ON shared_agent_memory (user_id, session_id, agent_id)", []);

        *self.conn.lock() = conn;
        Ok(())
    }

    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}
