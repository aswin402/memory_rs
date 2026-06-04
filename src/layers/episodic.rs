use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

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
            );
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
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tool_performance (
                tool_name TEXT NOT NULL,
                model_name TEXT NOT NULL,
                task_type TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                average_latency REAL NOT NULL DEFAULT 0.0,
                last_used TEXT NOT NULL,
                PRIMARY KEY (tool_name, model_name, task_type)
            );"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn log_episode(&self, ep: EpisodeLog) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO episodic_logs 
             (id, task_description, execution_status, steps_taken, error_message, reflection, created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                ep.id,
                ep.task_description,
                ep.execution_status,
                ep.steps_taken,
                ep.error_message,
                ep.reflection,
                ep.created_at
            ],
        )?;
        Ok(())
    }

    pub fn log_reflection(&self, item: ReflectionItem) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO reflection_memory 
             (id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
                item.created_at
            ],
        )?;
        Ok(())
    }

    pub fn get_reflections(&self, query: &str) -> Result<Vec<ReflectionItem>> {
        let conn = self.conn.lock();
        let mut stmt = if query.is_empty() {
            conn.prepare(
                "SELECT id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at 
                 FROM reflection_memory ORDER BY created_at DESC"
            )?
        } else {
            conn.prepare(
                "SELECT id, task_description, status, attempt_number, steps_taken, error_encountered, root_cause, solution_applied, reflection, created_at 
                 FROM reflection_memory WHERE task_description LIKE ?1 OR reflection LIKE ?1 OR root_cause LIKE ?1 
                 ORDER BY created_at DESC"
            )?
        };

        let mut rows = if query.is_empty() {
            stmt.query([])?
        } else {
            let pattern = format!("%{}%", query);
            stmt.query(params![pattern])?
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

    pub fn record_tool_performance(&self, rec: ToolPerformanceRecord) -> Result<()> {
        let conn = self.conn.lock();
        // Check if record exists
        let existing: Option<(i64, i64, f64)> = conn.query_row(
            "SELECT success_count, failure_count, average_latency FROM tool_performance 
             WHERE tool_name = ?1 AND model_name = ?2 AND task_type = ?3",
            params![rec.tool_name, rec.model_name, rec.task_type],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).ok();

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
                 WHERE tool_name = ?5 AND model_name = ?6 AND task_type = ?7",
                params![new_s, new_f, new_lat, rec.last_used, rec.tool_name, rec.model_name, rec.task_type],
            )?;
        } else {
            conn.execute(
                "INSERT INTO tool_performance 
                 (tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    rec.tool_name,
                    rec.model_name,
                    rec.task_type,
                    rec.success_count,
                    rec.failure_count,
                    rec.average_latency,
                    rec.last_used
                ],
            )?;
        }
        Ok(())
    }

    pub fn query_tool_performance(&self, task_type: &str) -> Result<Vec<ToolPerformanceRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT tool_name, model_name, task_type, success_count, failure_count, average_latency, last_used 
             FROM tool_performance WHERE task_type = ?1 ORDER BY success_count DESC, average_latency ASC"
        )?;
        let mut rows = stmt.query(params![task_type])?;
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
}
