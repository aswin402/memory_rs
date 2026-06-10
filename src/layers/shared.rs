use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

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
            "CREATE TABLE IF NOT EXISTS shared_agent_memory (
                memory_key TEXT PRIMARY KEY,
                memory_value TEXT NOT NULL,
                source_agent TEXT NOT NULL,
                target_agents TEXT NOT NULL, -- JSON array of target agent IDs
                importance REAL NOT NULL DEFAULT 1.0,
                timestamp TEXT NOT NULL
            );"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn store_shared_memory(&self, item: SharedMemoryItem) -> Result<()> {
        let conn = self.conn.lock();
        let targets_json = serde_json::to_string(&item.target_agents)?;
        conn.execute(
            "INSERT OR REPLACE INTO shared_agent_memory 
             (memory_key, memory_value, source_agent, target_agents, importance, timestamp) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                item.key,
                item.value,
                item.source_agent,
                targets_json,
                item.importance,
                item.timestamp
            ],
        )?;
        Ok(())
    }

    pub fn retrieve_shared_memory(&self, agent_id: &str) -> Result<Vec<SharedMemoryItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT memory_key, memory_value, source_agent, target_agents, importance, timestamp 
             FROM shared_agent_memory"
        )?;
        let mut rows = stmt.query([])?;
        let mut results = Vec::new();

        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;
            let source_agent: String = row.get(2)?;
            let targets_json: String = row.get(3)?;
            let importance: f64 = row.get(4)?;
            let timestamp: String = row.get(5)?;

            let target_agents: Vec<String> = serde_json::from_str(&targets_json).unwrap_or_default();
            
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

    pub fn delete_shared_memory(&self, key: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM shared_agent_memory WHERE memory_key = ?1", params![key])?;
        Ok(())
     }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        *self.conn.lock() = Connection::open(db_path)?;
        Ok(())
    }
}
