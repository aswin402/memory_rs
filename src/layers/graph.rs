use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct Entity {
    pub name: String,
    pub entityType: String,
    pub observations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema, Clone)]
pub struct Relation {
    pub from: String,
    pub to: String,
    pub relationType: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KnowledgeGraph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}

pub struct GraphMemory {
    conn: Mutex<Connection>,
}

impl GraphMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS graph_nodes (
                name TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                observations TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS graph_edges (
                from_name TEXT NOT NULL,
                to_name TEXT NOT NULL,
                relation_type TEXT NOT NULL,
                PRIMARY KEY (from_name, to_name, relation_type)
            );"
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn create_entities(&self, entities: Vec<Entity>) -> Result<Vec<Entity>> {
        let conn = self.conn.lock();
        let mut created = Vec::new();
        for entity in entities {
            // Check if entity already exists
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_nodes WHERE name = ?1)",
                params![entity.name],
                |row| row.get(0),
            )?;
            if !exists {
                let obs_str = serde_json::to_string(&entity.observations)?;
                conn.execute(
                    "INSERT INTO graph_nodes (name, entity_type, observations) VALUES (?1, ?2, ?3)",
                    params![entity.name, entity.entityType, obs_str],
                )?;
                created.push(entity);
            }
        }
        Ok(created)
    }

    pub fn create_relations(&self, relations: Vec<Relation>) -> Result<Vec<Relation>> {
        let conn = self.conn.lock();
        let mut created = Vec::new();
        for relation in relations {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM graph_edges WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3)",
                params![relation.from, relation.to, relation.relationType],
                |row| row.get(0),
            )?;
            if !exists {
                conn.execute(
                    "INSERT INTO graph_edges (from_name, to_name, relation_type) VALUES (?1, ?2, ?3)",
                    params![relation.from, relation.to, relation.relationType],
                )?;
                created.push(relation);
            }
        }
        Ok(created)
    }

    pub fn add_observations(&self, observations: Vec<AddObservationsInput>) -> Result<Vec<AddObservationsOutput>> {
        let conn = self.conn.lock();
        let mut results = Vec::new();
        for obs in observations {
            let current_obs_str: Option<String> = conn.query_row(
                "SELECT observations FROM graph_nodes WHERE name = ?1",
                params![obs.entityName],
                |row| row.get(0),
            ).ok();

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
                    "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2",
                    params![new_obs_json, obs.entityName],
                )?;
                results.push(AddObservationsOutput {
                    entityName: obs.entityName,
                    addedObservations: added,
                });
            } else {
                anyhow::bail!("Entity with name {} not found", obs.entityName);
            }
        }
        Ok(results)
    }

    pub fn delete_entities(&self, names: Vec<String>) -> Result<()> {
        let conn = self.conn.lock();
        for name in names {
            conn.execute("DELETE FROM graph_nodes WHERE name = ?1", params![name])?;
            conn.execute("DELETE FROM graph_edges WHERE from_name = ?1 OR to_name = ?1", params![name])?;
        }
        Ok(())
    }

    pub fn delete_observations(&self, deletions: Vec<DeleteObservationsInput>) -> Result<()> {
        let conn = self.conn.lock();
        for del in deletions {
            let current_obs_str: Option<String> = conn.query_row(
                "SELECT observations FROM graph_nodes WHERE name = ?1",
                params![del.entityName],
                |row| row.get(0),
            ).ok();

            if let Some(obs_json) = current_obs_str {
                let current_obs: Vec<String> = serde_json::from_str(&obs_json)?;
                let filtered_obs: Vec<String> = current_obs.into_iter()
                    .filter(|o| !del.observations.contains(o))
                    .collect();
                let new_obs_json = serde_json::to_string(&filtered_obs)?;
                conn.execute(
                    "UPDATE graph_nodes SET observations = ?1 WHERE name = ?2",
                    params![new_obs_json, del.entityName],
                )?;
            }
        }
        Ok(())
    }

    pub fn delete_relations(&self, relations: Vec<Relation>) -> Result<()> {
        let conn = self.conn.lock();
        for rel in relations {
            conn.execute(
                "DELETE FROM graph_edges WHERE from_name = ?1 AND to_name = ?2 AND relation_type = ?3",
                params![rel.from, rel.to, rel.relationType],
            )?;
        }
        Ok(())
    }

    pub fn read_graph(&self) -> Result<KnowledgeGraph> {
        let conn = self.conn.lock();
        
        let mut stmt_nodes = conn.prepare("SELECT name, entity_type, observations FROM graph_nodes")?;
        let mut node_rows = stmt_nodes.query([])?;
        let mut entities = Vec::new();
        while let Some(row) = node_rows.next()? {
            let name: String = row.get(0)?;
            let entity_type: String = row.get(1)?;
            let obs_json: String = row.get(2)?;
            let observations: Vec<String> = serde_json::from_str(&obs_json)?;
            entities.push(Entity {
                name,
                entityType: entity_type,
                observations,
            });
        }

        let mut stmt_edges = conn.prepare("SELECT from_name, to_name, relation_type FROM graph_edges")?;
        let mut edge_rows = stmt_edges.query([])?;
        let mut relations = Vec::new();
        while let Some(row) = edge_rows.next()? {
            relations.push(Relation {
                from: row.get(0)?,
                to: row.get(1)?,
                relationType: row.get(2)?,
            });
        }

        Ok(KnowledgeGraph { entities, relations })
    }

    pub fn search_nodes(&self, query: &str) -> Result<KnowledgeGraph> {
        let conn = self.conn.lock();
        let query_pattern = format!("%{}%", query.to_lowercase());

        let mut stmt_nodes = conn.prepare(
            "SELECT name, entity_type, observations FROM graph_nodes 
             WHERE LOWER(name) LIKE ?1 OR LOWER(entity_type) LIKE ?1 OR LOWER(observations) LIKE ?1"
        )?;
        let mut node_rows = stmt_nodes.query(params![query_pattern])?;
        let mut entities = Vec::new();
        let mut entity_names = std::collections::HashSet::new();

        while let Some(row) = node_rows.next()? {
            let name: String = row.get(0)?;
            let entity_type: String = row.get(1)?;
            let obs_json: String = row.get(2)?;
            let observations: Vec<String> = serde_json::from_str(&obs_json)?;
            entity_names.insert(name.clone());
            entities.push(Entity {
                name,
                entityType: entity_type,
                observations,
            });
        }

        // Return relations where at least one endpoint matches
        let mut stmt_edges = conn.prepare("SELECT from_name, to_name, relation_type FROM graph_edges")?;
        let mut edge_rows = stmt_edges.query([])?;
        let mut relations = Vec::new();
        while let Some(row) = edge_rows.next()? {
            let from_name: String = row.get(0)?;
            let to_name: String = row.get(1)?;
            if entity_names.contains(&from_name) || entity_names.contains(&to_name) {
                relations.push(Relation {
                    from: from_name,
                    to: to_name,
                    relationType: row.get(2)?,
                });
            }
        }

        Ok(KnowledgeGraph { entities, relations })
    }

    pub fn open_nodes(&self, names: Vec<String>) -> Result<KnowledgeGraph> {
        let conn = self.conn.lock();
        let mut entities = Vec::new();
        let mut entity_names = std::collections::HashSet::new();

        for name in &names {
            let mut stmt = conn.prepare("SELECT entity_type, observations FROM graph_nodes WHERE name = ?1")?;
            let mut rows = stmt.query(params![name])?;
            if let Some(row) = rows.next()? {
                let entity_type: String = row.get(0)?;
                let obs_json: String = row.get(1)?;
                let observations: Vec<String> = serde_json::from_str(&obs_json)?;
                entity_names.insert(name.clone());
                entities.push(Entity {
                    name: name.clone(),
                    entityType: entity_type,
                    observations,
                });
            }
        }

        let mut stmt_edges = conn.prepare("SELECT from_name, to_name, relation_type FROM graph_edges")?;
        let mut edge_rows = stmt_edges.query([])?;
        let mut relations = Vec::new();
        while let Some(row) = edge_rows.next()? {
            let from_name: String = row.get(0)?;
            let to_name: String = row.get(1)?;
            if entity_names.contains(&from_name) || entity_names.contains(&to_name) {
                relations.push(Relation {
                    from: from_name,
                    to: to_name,
                    relationType: row.get(2)?,
                });
            }
        }

        Ok(KnowledgeGraph { entities, relations })
    }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        *self.conn.lock() = Connection::open(db_path)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct AddObservationsInput {
    pub entityName: String,
    pub contents: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AddObservationsOutput {
    pub entityName: String,
    pub addedObservations: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
pub struct DeleteObservationsInput {
    pub entityName: String,
    pub observations: Vec<String>,
}
