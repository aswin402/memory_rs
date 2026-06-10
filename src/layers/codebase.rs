use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeElement {
    pub id: String,
    pub file_path: String,
    pub element_type: String, // "Function", "Struct", "Method", "Class", "Module"
    pub name: String,
    pub signature: String,
    pub ast_json: Option<String>,
    pub parent_id: Option<String>,
    pub start_line: i64,
    pub end_line: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CodeCall {
    pub caller_id: String,
    pub callee_id: String,
    pub call_site: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RepositoryEvolution {
    pub file_path: String,
    pub version: String,
    pub commit_hash: Option<String>,
    pub author: Option<String>,
    pub change_type: String, // "Added", "Modified", "Deleted"
    pub summary_of_changes: String,
    pub bug_introduced: bool,
    pub bug_fixed: bool,
    pub timestamp: String,
}

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
            );
            CREATE TABLE IF NOT EXISTS code_elements (
                element_id TEXT PRIMARY KEY,
                file_path TEXT NOT NULL,
                element_type TEXT NOT NULL,
                name TEXT NOT NULL,
                signature TEXT NOT NULL,
                ast_json TEXT,
                parent_id TEXT,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS code_calls (
                caller_id TEXT NOT NULL,
                callee_id TEXT NOT NULL,
                call_site TEXT,
                PRIMARY KEY (caller_id, callee_id)
            );
            CREATE TABLE IF NOT EXISTS repository_evolution (
                file_path TEXT NOT NULL,
                version TEXT NOT NULL,
                commit_hash TEXT,
                author TEXT,
                change_type TEXT NOT NULL,
                summary_of_changes TEXT NOT NULL,
                bug_introduced INTEGER NOT NULL DEFAULT 0,
                bug_fixed INTEGER NOT NULL DEFAULT 0,
                timestamp TEXT NOT NULL,
                PRIMARY KEY (file_path, version)
            );"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn index_element(&self, el: CodeElement) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO code_elements 
             (element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                el.id,
                el.file_path,
                el.element_type,
                el.name,
                el.signature,
                el.ast_json,
                el.parent_id,
                el.start_line,
                el.end_line
            ],
        )?;
        Ok(())
    }

    pub fn index_call(&self, call: CodeCall) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO code_calls (caller_id, callee_id, call_site) VALUES (?1, ?2, ?3)",
            params![call.caller_id, call.callee_id, call.call_site],
        )?;
        Ok(())
    }

    pub fn log_evolution(&self, evo: RepositoryEvolution) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO repository_evolution 
             (file_path, version, commit_hash, author, change_type, summary_of_changes, bug_introduced, bug_fixed, timestamp) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                evo.file_path,
                evo.version,
                evo.commit_hash,
                evo.author,
                evo.change_type,
                evo.summary_of_changes,
                if evo.bug_introduced { 1 } else { 0 },
                if evo.bug_fixed { 1 } else { 0 },
                evo.timestamp
            ],
        )?;
        Ok(())
    }

    pub fn query_evolution(&self, file_path: &str) -> Result<Vec<RepositoryEvolution>> {
        let conn = self.conn.lock();
        let mut stmt = if file_path.is_empty() {
            conn.prepare(
                "SELECT file_path, version, commit_hash, author, change_type, summary_of_changes, bug_introduced, bug_fixed, timestamp 
                 FROM repository_evolution ORDER BY timestamp DESC"
            )?
        } else {
            conn.prepare(
                "SELECT file_path, version, commit_hash, author, change_type, summary_of_changes, bug_introduced, bug_fixed, timestamp 
                 FROM repository_evolution WHERE file_path = ?1 ORDER BY timestamp DESC"
            )?
        };

        let mut rows = if file_path.is_empty() {
            stmt.query([])?
        } else {
            stmt.query(params![file_path])?
        };

        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(RepositoryEvolution {
                file_path: row.get(0)?,
                version: row.get(1)?,
                commit_hash: row.get(2)?,
                author: row.get(3)?,
                change_type: row.get(4)?,
                summary_of_changes: row.get(5)?,
                bug_introduced: row.get::<_, i64>(6)? != 0,
                bug_fixed: row.get::<_, i64>(7)? != 0,
                timestamp: row.get(8)?,
            });
        }
        Ok(results)
    }

    pub fn query_elements(&self, file_path: &str, query: &str) -> Result<Vec<CodeElement>> {
        let conn = self.conn.lock();
        let mut results = Vec::new();

        let mut stmt = if !file_path.is_empty() && !query.is_empty() {
            conn.prepare(
                "SELECT element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line 
                 FROM code_elements WHERE file_path = ?1 AND name LIKE ?2"
            )?
        } else if !file_path.is_empty() {
            conn.prepare(
                "SELECT element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line 
                 FROM code_elements WHERE file_path = ?1"
            )?
        } else if !query.is_empty() {
            conn.prepare(
                "SELECT element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line 
                 FROM code_elements WHERE name LIKE ?1 OR element_type LIKE ?1"
            )?
        } else {
            conn.prepare(
                "SELECT element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line 
                 FROM code_elements"
            )?
        };

        let mut rows = if !file_path.is_empty() && !query.is_empty() {
            let pattern = format!("%{}%", query);
            stmt.query(params![file_path, pattern])?
        } else if !file_path.is_empty() {
            stmt.query(params![file_path])?
        } else if !query.is_empty() {
            let pattern = format!("%{}%", query);
            stmt.query(params![pattern])?
        } else {
            stmt.query([])?
        };

        while let Some(row) = rows.next()? {
            results.push(CodeElement {
                id: row.get(0)?,
                file_path: row.get(1)?,
                element_type: row.get(2)?,
                name: row.get(3)?,
                signature: row.get(4)?,
                ast_json: row.get(5)?,
                parent_id: row.get(6)?,
                start_line: row.get(7)?,
                end_line: row.get(8)?,
            });
        }
        Ok(results)
    }

    pub fn query_calls(&self, caller_id: &str, callee_id: &str) -> Result<Vec<CodeCall>> {
        let conn = self.conn.lock();
        let mut results = Vec::new();

        let mut stmt = if !caller_id.is_empty() && !callee_id.is_empty() {
            conn.prepare("SELECT caller_id, callee_id, call_site FROM code_calls WHERE caller_id = ?1 AND callee_id = ?2")?
        } else if !caller_id.is_empty() {
            conn.prepare("SELECT caller_id, callee_id, call_site FROM code_calls WHERE caller_id = ?1")?
        } else if !callee_id.is_empty() {
            conn.prepare("SELECT caller_id, callee_id, call_site FROM code_calls WHERE callee_id = ?1")?
        } else {
            conn.prepare("SELECT caller_id, callee_id, call_site FROM code_calls")?
        };

        let mut rows = if !caller_id.is_empty() && !callee_id.is_empty() {
            stmt.query(params![caller_id, callee_id])?
        } else if !caller_id.is_empty() {
            stmt.query(params![caller_id])?
        } else if !callee_id.is_empty() {
            stmt.query(params![callee_id])?
        } else {
            stmt.query([])?
        };

        while let Some(row) = rows.next()? {
            results.push(CodeCall {
                caller_id: row.get(0)?,
                callee_id: row.get(1)?,
                call_site: row.get(2)?,
            });
        }
        Ok(results)
    }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        *self.conn.lock() = Connection::open(db_path)?;
        Ok(())
    }
}
