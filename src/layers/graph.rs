use crate::error::{Result, MemoryError};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub name: String,
    pub entity_type: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub from: String,
    pub to: String,
    pub relation_type: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RelationHistoryItem {
    pub from: String,
    pub to: String,
    pub relation_type: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub superseded_by: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnowledgeGraph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}

pub struct GraphMemory {
    pub(crate) conn: Mutex<Connection>,
}

impl GraphMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS graph_nodes (
                name TEXT,
                entity_type TEXT NOT NULL,
                observations TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (name, user_id, session_id, agent_id)
            );
            CREATE INDEX IF NOT EXISTS idx_graph_nodes_scope ON graph_nodes (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS graph_edges (
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                valid_until TEXT,
                superseded_by TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                PRIMARY KEY (from_name, to_name, relation_type, user_id, session_id, agent_id, valid_from)
            );
            CREATE INDEX IF NOT EXISTS idx_graph_edges_scope ON graph_edges (user_id, session_id, agent_id);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE graph_nodes ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_nodes ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_nodes ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_graph_nodes_scope ON graph_nodes (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN valid_until TEXT", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN superseded_by TEXT", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN confidence REAL NOT NULL DEFAULT 1.0", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_graph_edges_scope ON graph_edges (user_id, session_id, agent_id)", []);

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn create_entities(&self, entities: Vec<Entity>, scope: &crate::layers::MemoryScope) -> Result<Vec<Entity>> {
        let conn = self.conn.lock();
        let mut created = Vec::new();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        for entity in entities {
            // Check if entity already exists
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4)",
                params![entity.name, user_id, session_id, agent_id],
                |row| row.get(0),
            )?;
            if !exists {
                let obs_str = serde_json::to_string(&entity.observations)?;
                conn.execute(
                    "INSERT INTO graph_nodes (name, entity_type, observations, user_id, session_id, agent_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![entity.name, entity.entity_type, obs_str, user_id, session_id, agent_id],
                )?;
                created.push(entity);
            }
        }
        Ok(created)
    }

    pub fn create_relations(&self, relations: Vec<Relation>, scope: &crate::layers::MemoryScope) -> Result<Vec<Relation>> {
        let conn = self.conn.lock();
        let mut created = Vec::new();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        for relation in relations {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_edges WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL)",
                params![relation.from, relation.to, relation.relation_type, user_id, session_id, agent_id],
                |row| row.get(0),
            )?;
            if !exists {
                conn.execute(
                    "INSERT INTO graph_edges (from_name, to_name, relation_type, user_id, session_id, agent_id, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1.0)",
                    params![relation.from, relation.to, relation.relation_type, user_id, session_id, agent_id],
                )?;
                created.push(relation);
            }
        }
        Ok(created)
    }

    pub fn add_observations(
        &self,
        observations: Vec<AddObservationsInput>,
        scope: &crate::layers::MemoryScope,
    ) -> Result<Vec<AddObservationsOutput>> {
        let conn = self.conn.lock();
        let mut results = Vec::new();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        for obs in observations {
            let current_obs_str: Option<String> = conn
                .query_row(
                    "SELECT observations FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                    params![obs.entity_name, user_id, session_id, agent_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(obs_json) = current_obs_str {
                let mut current_obs: Vec<String> = serde_json::from_str(&obs_json)?;
                let mut added = Vec::new();
                for content in obs.contents {
                    if !current_obs.contains(&content) {
                        current_obs.push(content.clone());
                        added.push(content);
                    }
                }
                let new_obs_json = serde_json::to_string(&current_obs)?;
                conn.execute(
                    "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                    params![new_obs_json, obs.entity_name, user_id, session_id, agent_id],
                )?;
                results.push(AddObservationsOutput {
                    entity_name: obs.entity_name,
                    added_observations: added,
                });
            } else {
                return Err(MemoryError::EntityNotFound(format!("Entity with name {} not found in scope", obs.entity_name)));
            }
        }
        Ok(results)
    }

    pub fn delete_entities(&self, names: Vec<String>, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        for name in names {
            conn.execute("DELETE FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4", params![name, user_id, session_id, agent_id])?;
            conn.execute(
                "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE (from_name = ?1 OR to_name = ?1) AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4 AND valid_until IS NULL",
                params![name, user_id, session_id, agent_id],
            )?;
        }
        Ok(())
    }

    pub fn delete_observations(&self, deletions: Vec<DeleteObservationsInput>, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        for del in deletions {
            let current_obs_str: Option<String> = conn
                .query_row(
                    "SELECT observations FROM graph_nodes WHERE name = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4",
                    params![del.entity_name, user_id, session_id, agent_id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(obs_json) = current_obs_str {
                let current_obs: Vec<String> = serde_json::from_str(&obs_json)?;
                let filtered_obs: Vec<String> = current_obs
                    .into_iter()
                    .filter(|o| !del.observations.contains(o))
                    .collect();
                let new_obs_json = serde_json::to_string(&filtered_obs)?;
                conn.execute(
                    "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2 AND user_id = ?3 AND session_id = ?4 AND agent_id = ?5",
                    params![new_obs_json, del.entity_name, user_id, session_id, agent_id],
                )?;
            }
        }
        Ok(())
    }

    pub fn delete_relations(&self, relations: Vec<Relation>, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        for rel in relations {
            conn.execute(
                "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL",
                params![rel.from, rel.to, rel.relation_type, user_id, session_id, agent_id],
            )?;
        }
        Ok(())
    }

    pub fn read_graph(&self, scope: &crate::layers::MemoryScope) -> Result<KnowledgeGraph> {
        let conn = self.conn.lock();

        let mut stmt_nodes =
            conn.prepare("SELECT name, entity_type, observations FROM graph_nodes WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')")?;
        let mut node_rows = stmt_nodes.query(params![scope.user_id, scope.session_id, scope.agent_id])?;
        let mut entities = Vec::new();
        while let Some(row) = node_rows.next()? {
            let name: String = row.get(0)?;
            let entity_type: String = row.get(1)?;
            let obs_json: String = row.get(2)?;
            let observations: Vec<String> = serde_json::from_str(&obs_json)?;
            entities.push(Entity {
                name,
                entity_type,
                observations,
            });
        }

        let mut stmt_edges =
            conn.prepare("SELECT from_name, to_name, relation_type FROM graph_edges WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*') AND valid_until IS NULL")?;
        let mut edge_rows = stmt_edges.query(params![scope.user_id, scope.session_id, scope.agent_id])?;
        let mut relations = Vec::new();
        while let Some(row) = edge_rows.next()? {
            relations.push(Relation {
                from: row.get(0)?,
                to: row.get(1)?,
                relation_type: row.get(2)?,
            });
        }

        Ok(KnowledgeGraph {
            entities,
            relations,
        })
    }

    pub fn search_nodes(&self, query: &str, scope: &crate::layers::MemoryScope) -> Result<KnowledgeGraph> {
        let conn = self.conn.lock();
        let query_pattern = format!("%{}%", query.to_lowercase());

        let mut stmt_nodes = conn.prepare(
            "SELECT name, entity_type, observations FROM graph_nodes 
             WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1)
               AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
               AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
               AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
        )?;
        let mut node_rows = stmt_nodes.query(params![query_pattern, scope.user_id, scope.session_id, scope.agent_id])?;
        let mut entities = Vec::new();

        while let Some(row) = node_rows.next()? {
            let name: String = row.get(0)?;
            let entity_type: String = row.get(1)?;
            let obs_json: String = row.get(2)?;
            let observations: Vec<String> = serde_json::from_str(&obs_json)?;
            entities.push(Entity {
                name,
                entity_type,
                observations,
            });
        }

        // Return relations where at least one endpoint matches query using optimized SQL Subqueries
        let mut stmt_edges = conn.prepare(
            "SELECT DISTINCT from_name, to_name, relation_type FROM graph_edges 
             WHERE (from_name IN (
                 SELECT name FROM graph_nodes 
                 WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1)
             ) OR to_name IN (
                 SELECT name FROM graph_nodes 
                 WHERE (LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1)
             ))
             AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
             AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
             AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')
             AND valid_until IS NULL"
        )?;
        let mut edge_rows = stmt_edges.query(params![query_pattern, scope.user_id, scope.session_id, scope.agent_id])?;
        let mut relations = Vec::new();
        while let Some(row) = edge_rows.next()? {
            relations.push(Relation {
                from: row.get(0)?,
                to: row.get(1)?,
                relation_type: row.get(2)?,
            });
        }

        Ok(KnowledgeGraph {
            entities,
            relations,
        })
    }

    pub fn open_nodes(&self, names: Vec<String>, scope: &crate::layers::MemoryScope) -> Result<KnowledgeGraph> {
        let conn = self.conn.lock();
        let mut entities = Vec::new();

        for name in &names {
            let mut stmt =
                conn.prepare("SELECT entity_type, observations FROM graph_nodes WHERE name = ?1 AND (?2 IS NULL OR user_id = ?2 OR user_id = '*') AND (?3 IS NULL OR session_id = ?3 OR session_id = '*') AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')")?;
            let mut rows = stmt.query(params![name, scope.user_id, scope.session_id, scope.agent_id])?;
            if let Some(row) = rows.next()? {
                let entity_type: String = row.get(0)?;
                let obs_json: String = row.get(1)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json)?;
                entities.push(Entity {
                    name: name.clone(),
                    entity_type,
                    observations,
                });
            }
        }

        let mut relations = Vec::new();
        if !names.is_empty() {
            let n = names.len();
            let placeholders_from = (4..=3+n)
                .map(|i| format!("?{}", i))
                .collect::<Vec<_>>()
                .join(", ");
            let placeholders_to = (4+n..=3+2*n)
                .map(|i| format!("?{}", i))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT DISTINCT from_name, to_name, relation_type FROM graph_edges 
                 WHERE (from_name IN ({0}) OR to_name IN ({1}))
                   AND (?1 IS NULL OR user_id = ?1 OR user_id = '*')
                   AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
                   AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')
                   AND valid_until IS NULL",
                placeholders_from, placeholders_to
            );
            let mut stmt_edges = conn.prepare(&sql)?;
            let mut params = Vec::new();
            params.push(scope.user_id.clone());
            params.push(scope.session_id.clone());
            params.push(scope.agent_id.clone());
            for name in &names {
                params.push(Some(name.clone()));
            }
            for name in &names {
                params.push(Some(name.clone()));
            }
            let mut edge_rows = stmt_edges.query(rusqlite::params_from_iter(params))?;
            while let Some(row) = edge_rows.next()? {
                relations.push(Relation {
                    from: row.get(0)?,
                    to: row.get(1)?,
                    relation_type: row.get(2)?,
                });
            }
        }

        Ok(KnowledgeGraph {
            entities,
            relations,
        })
    }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS graph_nodes (
                name TEXT,
                entity_type TEXT NOT NULL,
                observations TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                PRIMARY KEY (name, user_id, session_id, agent_id)
            );
            CREATE INDEX IF NOT EXISTS idx_graph_nodes_scope ON graph_nodes (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS graph_edges (
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                valid_until TEXT,
                superseded_by TEXT,
                confidence REAL NOT NULL DEFAULT 1.0,
                PRIMARY KEY (from_name, to_name, relation_type, user_id, session_id, agent_id, valid_from)
            );
            CREATE INDEX IF NOT EXISTS idx_graph_edges_scope ON graph_edges (user_id, session_id, agent_id);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE graph_nodes ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_nodes ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_nodes ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_graph_nodes_scope ON graph_nodes (user_id, session_id, agent_id)", []);

        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN valid_until TEXT", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN superseded_by TEXT", []);
        let _ = conn.execute("ALTER TABLE graph_edges ADD COLUMN confidence REAL NOT NULL DEFAULT 1.0", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_graph_edges_scope ON graph_edges (user_id, session_id, agent_id)", []);

        *self.conn.lock() = conn;
        Ok(())
    }

    pub fn invalidate_edge(&self, from_name: &str, to_name: &str, relation_type: &str, scope: &crate::layers::MemoryScope) -> Result<bool> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        let rows_updated = conn.execute(
            "UPDATE graph_edges SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') 
             WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3 AND user_id = ?4 AND session_id = ?5 AND agent_id = ?6 AND valid_until IS NULL",
            params![from_name, to_name, relation_type, user_id, session_id, agent_id],
        )?;
        Ok(rows_updated > 0)
    }

    pub fn query_fact_history(
        &self,
        entity_name: &str,
        relation_type: Option<String>,
        scope: &crate::layers::MemoryScope,
    ) -> Result<Vec<RelationHistoryItem>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT from_name, to_name, relation_type, valid_from, valid_until, superseded_by, confidence
             FROM graph_edges
             WHERE (from_name = ?1 OR to_name = ?1)
               AND (?2 IS NULL OR relation_type = ?2)
               AND (?3 IS NULL OR user_id = ?3 OR user_id = '*')
               AND (?4 IS NULL OR session_id = ?4 OR session_id = '*')
               AND (?5 IS NULL OR agent_id = ?5 OR agent_id = '*')
             ORDER BY valid_from ASC"
        )?;
        let mut rows = stmt.query(params![
            entity_name,
            relation_type,
            scope.user_id,
            scope.session_id,
            scope.agent_id,
        ])?;
        let mut history = Vec::new();
        while let Some(row) = rows.next()? {
            history.push(RelationHistoryItem {
                from: row.get(0)?,
                to: row.get(1)?,
                relation_type: row.get(2)?,
                valid_from: row.get(3)?,
                valid_until: row.get(4)?,
                superseded_by: row.get(5)?,
                confidence: row.get(6)?,
            });
        }
        Ok(history)
    }

    pub fn query_as_of(&self, as_of: &str, scope: &crate::layers::MemoryScope) -> Result<KnowledgeGraph> {
        let conn = self.conn.lock();

        let mut stmt_nodes =
            conn.prepare("SELECT name, entity_type, observations FROM graph_nodes WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')")?;
        let mut node_rows = stmt_nodes.query(params![scope.user_id, scope.session_id, scope.agent_id])?;
        let mut entities = Vec::new();
        while let Some(row) = node_rows.next()? {
            let name: String = row.get(0)?;
            let entity_type: String = row.get(1)?;
            let obs_json: String = row.get(2)?;
            let observations: Vec<String> = serde_json::from_str(&obs_json)?;
            entities.push(Entity {
                name,
                entity_type,
                observations,
            });
        }

        let mut stmt_edges =
            conn.prepare("SELECT from_name, to_name, relation_type FROM graph_edges WHERE (?1 IS NULL OR user_id = ?1 OR user_id = '*') AND (?2 IS NULL OR session_id = ?2 OR session_id = '*') AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*') AND valid_from <= ?4 AND (valid_until IS NULL OR valid_until > ?4)")?;
        let mut edge_rows = stmt_edges.query(params![scope.user_id, scope.session_id, scope.agent_id, as_of])?;
        let mut relations = Vec::new();
        while let Some(row) = edge_rows.next()? {
            relations.push(Relation {
                from: row.get(0)?,
                to: row.get(1)?,
                relation_type: row.get(2)?,
            });
        }

        Ok(KnowledgeGraph {
            entities,
            relations,
        })
    }

    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AddObservationsInput {
    pub entity_name: String,
    pub contents: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AddObservationsOutput {
    pub entity_name: String,
    pub added_observations: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeleteObservationsInput {
    pub entity_name: String,
    pub observations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::MemoryScope;
    use std::fs;

    #[test]
    fn test_graph_edges_bitemporal_behavior() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_graph_edges_bitemporal.db");
        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        let graph = GraphMemory::new(&db_path)?;
        let scope = MemoryScope {
            user_id: Some("test_user".to_string()),
            session_id: Some("test_session".to_string()),
            agent_id: Some("test_agent".to_string()),
        };

        // 1. Create entities
        let entity_a = Entity {
            name: "A".to_string(),
            entity_type: "Person".to_string(),
            observations: vec!["Obs 1".to_string()],
        };
        let entity_b = Entity {
            name: "B".to_string(),
            entity_type: "Person".to_string(),
            observations: vec!["Obs 2".to_string()],
        };
        graph.create_entities(vec![entity_a, entity_b], &scope)?;

        // 2. Create a relation (edge)
        let rel = Relation {
            from: "A".to_string(),
            to: "B".to_string(),
            relation_type: "friends_with".to_string(),
        };
        let created = graph.create_relations(vec![rel.clone()], &scope)?;
        assert_eq!(created.len(), 1);

        // Verify it is returned in read_graph
        let kg = graph.read_graph(&scope)?;
        assert_eq!(kg.relations.len(), 1);
        assert_eq!(kg.relations[0].from, "A");
        assert_eq!(kg.relations[0].to, "B");

        // Verify duplicating creation is skipped (exists check works)
        let created_dup = graph.create_relations(vec![rel.clone()], &scope)?;
        assert_eq!(created_dup.len(), 0);

        // 3. Delete the relation (should soft-delete)
        graph.delete_relations(vec![rel.clone()], &scope)?;

        // Verify read_graph now filters it out (valid_until is not null)
        let kg_after_delete = graph.read_graph(&scope)?;
        assert_eq!(kg_after_delete.relations.len(), 0);

        // Verify search_nodes also filters it out
        let search_kg = graph.search_nodes("A", &scope)?;
        assert_eq!(search_kg.relations.len(), 0);

        // Verify open_nodes also filters it out
        let open_kg = graph.open_nodes(vec!["A".to_string(), "B".to_string()], &scope)?;
        assert_eq!(open_kg.relations.len(), 0);

        // Check SQL directly to ensure valid_until has been set and confidence is 1.0
        let conn = Connection::open(&db_path)?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE valid_until IS NOT NULL",
            [],
            |r| r.get(0)
        )?;
        assert_eq!(count, 1);

        let confidence: f64 = conn.query_row(
            "SELECT confidence FROM graph_edges WHERE from_name = 'A'",
            [],
            |r| r.get(0)
        )?;
        assert_eq!(confidence, 1.0);

        // Cleanup
        let _ = fs::remove_file(&db_path);
        Ok(())
    }

    #[test]
    fn test_graph_history_and_temporal_queries() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join("test_graph_history_temporal.db");
        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        let graph = GraphMemory::new(&db_path)?;
        let scope = MemoryScope {
            user_id: Some("test_user".to_string()),
            session_id: Some("test_session".to_string()),
            agent_id: Some("test_agent".to_string()),
        };

        // 1. Create entities
        let entity_a = Entity {
            name: "A".to_string(),
            entity_type: "Person".to_string(),
            observations: vec!["Obs 1".to_string()],
        };
        let entity_b = Entity {
            name: "B".to_string(),
            entity_type: "Person".to_string(),
            observations: vec!["Obs 2".to_string()],
        };
        graph.create_entities(vec![entity_a, entity_b], &scope)?;

        // Capture a timestamp before adding relation
        let before_creation = chrono::Utc::now().to_rfc3339();
        std::thread::sleep(std::time::Duration::from_secs(1));

        // 2. Create a relation
        let rel = Relation {
            from: "A".to_string(),
            to: "B".to_string(),
            relation_type: "colleagues".to_string(),
        };
        graph.create_relations(vec![rel.clone()], &scope)?;

        std::thread::sleep(std::time::Duration::from_secs(1));
        let after_creation = chrono::Utc::now().to_rfc3339();
        std::thread::sleep(std::time::Duration::from_secs(1));

        // 3. Invalidate/Delete the relation via invalidate_edge
        graph.invalidate_edge("A", "B", "colleagues", &scope)?;

        std::thread::sleep(std::time::Duration::from_secs(1));
        let after_invalidation = chrono::Utc::now().to_rfc3339();

        // Check history
        let history = graph.query_fact_history("A", Some("colleagues".to_string()), &scope)?;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].from, "A");
        assert_eq!(history[0].to, "B");
        assert!(history[0].valid_until.is_some());

        // Check query_as_of before creation -> should have no relations
        let kg_before = graph.query_as_of(&before_creation, &scope)?;
        assert_eq!(kg_before.relations.len(), 0);

        // Check query_as_of after creation -> should have 1 relation
        let kg_after = graph.query_as_of(&after_creation, &scope)?;
        assert_eq!(kg_after.relations.len(), 1);
        assert_eq!(kg_after.relations[0].from, "A");

        // Check query_as_of after invalidation -> should have 0 relations
        let kg_final = graph.query_as_of(&after_invalidation, &scope)?;
        assert_eq!(kg_final.relations.len(), 0);

        // Cleanup
        let _ = fs::remove_file(&db_path);
        Ok(())
    }
}
