use crate::error::{Result, MemoryError};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

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

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImpactReport {
    pub affected_symbols: Vec<String>,
    pub max_depth: u32,
    pub risk_score: f64,
    pub details: String,
}

pub struct CodebaseMemory {
    conn: Mutex<Connection>,
}

impl CodebaseMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;",
        )?;
        crate::db::run_migrations(&conn)?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn index_element(&self, el: CodeElement, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        conn.execute(
            "INSERT OR REPLACE INTO code_elements 
             (element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line, user_id, session_id, agent_id) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                el.id,
                el.file_path,
                el.element_type,
                el.name,
                el.signature,
                el.ast_json,
                el.parent_id,
                el.start_line,
                el.end_line,
                user_id,
                session_id,
                agent_id
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

    pub fn query_elements(&self, file_path: &str, query: &str, scope: &crate::layers::MemoryScope) -> Result<Vec<CodeElement>> {
        let conn = self.conn.lock();
        let mut results = Vec::new();

        let mut conditions = vec![
            "(?1 IS NULL OR user_id = ?1 OR user_id = '*')".to_string(),
            "(?2 IS NULL OR session_id = ?2 OR session_id = '*')".to_string(),
            "(?3 IS NULL OR agent_id = ?3 OR agent_id = '*')".to_string(),
        ];

        let mut params = vec![
            scope.user_id.clone(),
            scope.session_id.clone(),
            scope.agent_id.clone(),
        ];

        if !file_path.is_empty() {
            conditions.push(format!("file_path = ?{}", params.len() + 1));
            params.push(Some(file_path.to_string()));
        }

        if !query.is_empty() {
            conditions.push(format!("(name LIKE ?{0} OR element_type LIKE ?{0})", params.len() + 1));
            params.push(Some(format!("%{}%", query)));
        }

        let sql = format!(
            "SELECT element_id, file_path, element_type, name, signature, ast_json, parent_id, start_line, end_line 
             FROM code_elements WHERE {}",
            conditions.join(" AND ")
        );

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query(rusqlite::params_from_iter(params))?;

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
            conn.prepare(
                "SELECT caller_id, callee_id, call_site FROM code_calls WHERE caller_id = ?1",
            )?
        } else if !callee_id.is_empty() {
            conn.prepare(
                "SELECT caller_id, callee_id, call_site FROM code_calls WHERE callee_id = ?1",
            )?
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
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;",
        )?;

        *self.conn.lock() = conn;
        Ok(())
    }

    pub fn impact_analysis(&self, target_symbol: &str, scope: &crate::layers::MemoryScope) -> Result<ImpactReport> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        // 1. Fetch active code elements in scope
        let mut stmt_n = conn.prepare(
            "SELECT element_id FROM code_elements 
             WHERE (user_id = ?1 OR user_id = '*')
               AND (session_id = ?2 OR session_id = '*')
               AND (agent_id = ?3 OR agent_id = '*')"
        )?;
        let rows_n = stmt_n.query_map(params![user_id, session_id, agent_id], |r| r.get::<_, String>(0))?;
        let mut node_set = std::collections::HashSet::new();
        for r in rows_n {
            node_set.insert(r?);
        }

        // 2. Fetch calls
        let mut stmt_e = conn.prepare("SELECT caller_id, callee_id FROM code_calls")?;
        let rows_e = stmt_e.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;

        let mut pet_graph = petgraph::graph::DiGraph::new();
        let mut node_map = std::collections::HashMap::new();

        for el_id in &node_set {
            let idx = pet_graph.add_node(el_id.clone());
            node_map.insert(el_id.clone(), idx);
        }

        for r in rows_e {
            let (caller, callee) = r?;
            if let (Some(&c_from), Some(&c_to)) = (node_map.get(&caller), node_map.get(&callee)) {
                // Reverse direction: points from callee to caller to follow impact propagation path
                pet_graph.add_edge(c_to, c_from, ());
            }
        }

        let start_idx = match node_map.get(target_symbol) {
            Some(&idx) => idx,
            None => return Err(MemoryError::SymbolNotFound(format!("Symbol '{}' not found in indexed codebase", target_symbol))),
        };

        // Manual BFS on reversed callgraph
        let mut queue = std::collections::VecDeque::new();
        let mut depths = std::collections::HashMap::new();

        queue.push_back(start_idx);
        depths.insert(start_idx, 0);

        let mut affected = Vec::new();
        let mut max_depth = 0;

        while let Some(node) = queue.pop_front() {
            let d = depths[&node];
            if d > 0 {
                affected.push(pet_graph.node_weight(node).unwrap().clone());
                if d > max_depth {
                    max_depth = d;
                }
            }

            for neighbor in pet_graph.neighbors(node) {
                if !depths.contains_key(&neighbor) {
                    depths.insert(neighbor, d + 1);
                    queue.push_back(neighbor);
                }
            }
        }

        // Risk Heuristic
        let direct_callers = pet_graph.neighbors(start_idx).count();
        let transitive_callers = affected.len().saturating_sub(direct_callers);
        let raw_score = 0.1 * (direct_callers as f64) + 0.05 * (transitive_callers as f64) + 0.1 * (max_depth as f64);
        let risk_score = raw_score.min(1.0);

        let details = format!(
            "Target '{}' has {} direct callers and {} transitive callers. Maximum propagation depth: {}.",
            target_symbol, direct_callers, transitive_callers, max_depth
        );

        Ok(ImpactReport {
            affected_symbols: affected,
            max_depth,
            risk_score,
            details,
        })
    }

    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_impact_analysis() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_code_{}.db", uuid::Uuid::new_v4()));
        let codebase = CodebaseMemory::new(&db_path)?;
        let scope = crate::layers::MemoryScope::default();

        codebase.index_element(CodeElement {
            id: "fn_a".to_string(),
            file_path: "src/a.rs".to_string(),
            element_type: "Function".to_string(),
            name: "a".to_string(),
            signature: "fn a()".to_string(),
            ast_json: None,
            parent_id: None,
            start_line: 1,
            end_line: 10,
        }, &scope)?;

        codebase.index_element(CodeElement {
            id: "fn_b".to_string(),
            file_path: "src/b.rs".to_string(),
            element_type: "Function".to_string(),
            name: "b".to_string(),
            signature: "fn b()".to_string(),
            ast_json: None,
            parent_id: None,
            start_line: 1,
            end_line: 10,
        }, &scope)?;

        codebase.index_call(CodeCall {
            caller_id: "fn_b".to_string(),
            callee_id: "fn_a".to_string(),
            call_site: None,
        })?;

        let report = codebase.impact_analysis("fn_a", &scope)?;
        assert_eq!(report.affected_symbols, vec!["fn_b"]);
        assert_eq!(report.max_depth, 1);
        assert!(report.risk_score > 0.0);

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
