use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpisodeLog {
    pub id: String,
    pub task_description: String,
    pub execution_status: String,
    pub steps_taken: String,
    pub error_message: Option<String>,
    pub reflection: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReflectionItem {
    pub id: String,
    pub task_description: String,
    pub status: String, // "Success" or "Failed"
    pub attempt_number: i64,
    pub steps_taken: String,
    pub error_encountered: Option<String>,
    pub root_cause: Option<String>,
    pub solution_applied: Option<String>,
    pub reflection: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolPerformanceRecord {
    pub tool_name: String,
    pub model_name: String,
    pub task_type: String,
    pub success_count: i64,
    pub failure_count: i64,
    pub average_latency: f64,
    pub last_used: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryAccessStat {
    pub memory_id: String,
    pub layer: String,
    pub access_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemoryStats {
    pub total_records: std::collections::HashMap<String, i64>,
    pub db_size_bytes: u64,
    pub most_accessed: Vec<MemoryAccessStat>,
}

pub struct EpisodicMemory {
    pub(crate) conn: Mutex<Connection>,
}

impl EpisodicMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS episodic_logs (
                id TEXT PRIMARY KEY,
                task_description TEXT NOT NULL,
                execution_status TEXT NOT NULL,
                steps_taken TEXT NOT NULL,
                error_message TEXT,
                reflection TEXT,
                created_at TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*'
            );
            CREATE INDEX IF NOT EXISTS idx_episodic_logs_scope ON episodic_logs (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS reflection_memory (
                id TEXT PRIMARY KEY,
                task_description TEXT NOT NULL,
                status TEXT NOT NULL,
                attempt_number INTEGER NOT NULL,
                steps_taken TEXT NOT NULL,
                error_encountered TEXT,
                root_cause TEXT,
                solution_applied TEXT,
                reflection TEXT NOT NULL,
                created_at TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*'
            );
            CREATE INDEX IF NOT EXISTS idx_reflection_memory_scope ON reflection_memory (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS tool_performance (
                tool_name TEXT NOT NULL,
                model_name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                average_latency REAL NOT NULL DEFAULT 0.0,
                last_used TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (tool_name, model_name, task_type, user_id, session_id, agent_id)
            );
            CREATE INDEX IF NOT EXISTS idx_tool_performance_scope ON tool_performance (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS memory_access_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL,
                layer TEXT NOT NULL,
                accessed_at TEXT NOT NULL,
                accessed_by TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_access_log_mem_id ON memory_access_log (memory_id);
            CREATE INDEX IF NOT EXISTS idx_memory_access_log_layer ON memory_access_log (layer);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE episodic_logs ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE episodic_logs ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE episodic_logs ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_episodic_logs_scope ON episodic_logs (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE reflection_memory ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE reflection_memory ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE reflection_memory ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_reflection_memory_scope ON reflection_memory (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE tool_performance ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE tool_performance ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE tool_performance ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_tool_performance_scope ON tool_performance (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE memory_access_log ADD COLUMN accessed_by TEXT NOT NULL DEFAULT 'unknown'", []);

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn log_episode(&self, ep: EpisodeLog, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        conn.execute(
            "INSERT OR REPLACE INTO episodic_logs 
             (id, task_description, execution_status, steps_taken, error_message, reflection, created_at, user_id, session_id, agent_id) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                ep.id,
                ep.task_description,
                ep.execution_status,
                ep.steps_taken,
                ep.error_message,
                ep.reflection,
                ep.created_at,
                user_id,
                session_id,
                agent_id
            ],
        )?;
        Ok(())
    }

    pub fn log_reflection(&self, item: ReflectionItem, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        conn.execute(
            "INSERT OR REPLACE INTO reflection_memory 
             (id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at, user_id, session_id, agent_id) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                item.id,
                item.task_description,
                item.status,
                item.attempt_number,
                item.steps_taken,
                item.error_encountered,
                item.root_cause,
                item.solution_applied,
                item.reflection,
                item.created_at,
                user_id,
                session_id,
                agent_id
            ],
        )?;
        Ok(())
    }

    pub fn get_reflections(&self, query: &str, scope: &crate::layers::MemoryScope) -> Result<Vec<ReflectionItem>> {
        let conn = self.conn.lock();
        let mut stmt = if query.is_empty() {
            conn.prepare(
                "SELECT id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at 
                 FROM reflection_memory 
                 WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')
                 ORDER BY created_at DESC"
            )?
        } else {
            conn.prepare(
                "SELECT id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at 
                 FROM reflection_memory 
                 WHERE (task_description LIKE ?1 OR reflection LIKE ?1 OR root_cause LIKE ?1)
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')
                 ORDER BY created_at DESC"
            )?
        };

        let mut rows = if query.is_empty() {
            stmt.query(params![scope.user_id, scope.session_id, scope.agent_id])?
        } else {
            let pattern = format!("%{}%", query);
            stmt.query(params![pattern, scope.user_id, scope.session_id, scope.agent_id])?
        };

        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(ReflectionItem {
                id: row.get(0)?,
                task_description: row.get(1)?,
                status: row.get(2)?,
                attempt_number: row.get(3)?,
                steps_taken: row.get(4)?,
                error_encountered: row.get(5)?,
                root_cause: row.get(6)?,
                solution_applied: row.get(7)?,
                reflection: row.get(8)?,
                created_at: row.get(9)?,
            });
        }
        Ok(results)
    }

    pub fn record_tool_performance(&self, rec: ToolPerformanceRecord, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        // Check if record exists
        let existing: Option<(i64, i64, f64)> = conn
            .query_row(
                "SELECT success_count, failure_count, average_latency FROM tool_performance 
              WHERE tool_name = ?1 AND model_name = ?2 AND task_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6",
                params![rec.tool_name, rec.model_name, rec.task_type, user_id, session_id, agent_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        if let Some((s_count, f_count, avg_lat)) = existing {
            let new_s = s_count + rec.success_count;
            let new_f = f_count + rec.failure_count;
            let total_runs = new_s + new_f;
            let run_lat = rec.average_latency;

            // Calculate new average running latency
            let new_lat = if total_runs > 0 {
                let current_total_lat = (s_count + f_count) as f64 * avg_lat;
                (current_total_lat + run_lat) / total_runs as f64
            } else {
                0.0
            };

            conn.execute(
                "UPDATE tool_performance 
                 SET success_count = ?1, failure_count = ?2, average_latency = ?3, last_used = ?4 
                 WHERE tool_name = ?5 AND model_name = ?6 AND task_type = ?7 AND user_id = ?8 AND session_id = ?9 AND agent_id = ?10",
                params![
                    new_s,
                    new_f,
                    new_lat,
                    rec.last_used,
                    rec.tool_name,
                    rec.model_name,
                    rec.task_type,
                    user_id,
                    session_id,
                    agent_id
                ],
            )?;
        } else {
            conn.execute(
                "INSERT INTO tool_performance 
                 (tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used, user_id, session_id, agent_id) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    rec.tool_name,
                    rec.model_name,
                    rec.task_type,
                    rec.success_count,
                    rec.failure_count,
                    rec.average_latency,
                    rec.last_used,
                    user_id,
                    session_id,
                    agent_id
                ],
            )?;
        }
        Ok(())
    }

    pub fn query_tool_performance(&self, task_type: &str, scope: &crate::layers::MemoryScope) -> Result<Vec<ToolPerformanceRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used 
             FROM tool_performance 
             WHERE task_type = ?1
               AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
               AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
               AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')
             ORDER BY success_count DESC, average_latency ASC"
        )?;
        let mut rows = stmt.query(params![task_type, scope.user_id, scope.session_id, scope.agent_id])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(ToolPerformanceRecord {
                tool_name: row.get(0)?,
                model_name: row.get(1)?,
                task_type: row.get(2)?,
                success_count: row.get(3)?,
                failure_count: row.get(4)?,
                average_latency: row.get(5)?,
                last_used: row.get(6)?,
            });
        }
        Ok(results)
    }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS episodic_logs (
                id TEXT PRIMARY KEY,
                task_description TEXT NOT NULL,
                execution_status TEXT NOT NULL,
                steps_taken TEXT NOT NULL,
                error_message TEXT,
                reflection TEXT,
                created_at TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*'
            );
            CREATE INDEX IF NOT EXISTS idx_episodic_logs_scope ON episodic_logs (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS reflection_memory (
                id TEXT PRIMARY KEY,
                task_description TEXT NOT NULL,
                status TEXT NOT NULL,
                attempt_number INTEGER NOT NULL,
                steps_taken TEXT NOT NULL,
                error_encountered TEXT,
                root_cause TEXT,
                solution_applied TEXT,
                reflection TEXT NOT NULL,
                created_at TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*'
            );
            CREATE INDEX IF NOT EXISTS idx_reflection_memory_scope ON reflection_memory (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS tool_performance (
                tool_name TEXT NOT NULL,
                model_name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                average_latency REAL NOT NULL DEFAULT 0.0,
                last_used TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (tool_name, model_name, task_type, user_id, session_id, agent_id)
            );
            CREATE INDEX IF NOT EXISTS idx_tool_performance_scope ON tool_performance (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS memory_access_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                memory_id TEXT NOT NULL,
                layer TEXT NOT NULL,
                accessed_at TEXT NOT NULL,
                accessed_by TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_access_log_mem_id ON memory_access_log (memory_id);
            CREATE INDEX IF NOT EXISTS idx_memory_access_log_layer ON memory_access_log (layer);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE episodic_logs ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE episodic_logs ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE episodic_logs ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_episodic_logs_scope ON episodic_logs (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE reflection_memory ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE reflection_memory ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE reflection_memory ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_reflection_memory_scope ON reflection_memory (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE tool_performance ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE tool_performance ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE tool_performance ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_tool_performance_scope ON tool_performance (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE memory_access_log ADD COLUMN accessed_by TEXT NOT NULL DEFAULT 'unknown'", []);

        *self.conn.lock() = conn;
        Ok(())
    }

    pub fn log_access(&self, memory_id: &str, layer: &str, accessed_by: &str) -> Result<()> {
        let conn = self.conn.lock();
        let timestamp = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO memory_access_log (memory_id, layer, accessed_at, accessed_by) 
             VALUES (?1, ?2, ?3, ?4)",
            params![memory_id, layer, timestamp, accessed_by],
        )?;
        Ok(())
    }

    pub fn get_memory_stats(&self) -> Result<MemoryStats> {
        let conn = self.conn.lock();
        
        // 1. Get database size via pragmas
        let page_count: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        let page_size: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        let db_size_bytes = (page_count * page_size) as u64;

        // 2. Count records in each table
        let mut total_records = std::collections::HashMap::new();
        let tables = vec![
            "semantic_metadata",
            "graph_nodes",
            "graph_edges",
            "episodic_logs",
            "reflection_memory",
            "tool_performance",
            "code_elements",
            "shared_agent_memory",
            "memory_access_log",
        ];
        for table in tables {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
                .unwrap_or(0);
            total_records.insert(table.to_string(), count);
        }

        // 3. Get top 10 most accessed memories
        let mut stmt = conn.prepare(
            "SELECT memory_id, layer, COUNT(*) as access_count 
             FROM memory_access_log 
             GROUP BY memory_id, layer 
             ORDER BY access_count DESC 
             LIMIT 10"
        )?;
        let mut rows = stmt.query([])?;
        let mut most_accessed = Vec::new();
        while let Some(row) = rows.next()? {
            most_accessed.push(MemoryAccessStat {
                memory_id: row.get(0)?,
                layer: row.get(1)?,
                access_count: row.get(2)?,
            });
        }

        Ok(MemoryStats {
            total_records,
            db_size_bytes,
            most_accessed,
        })
    }

    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}
