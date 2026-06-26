use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use crate::layers::{GraphMemory, SemanticMemory, MemoryScope};
use anyhow::Result;
use rusqlite::params;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConflictDetails {
    Graph {
        from_name: String,
        relation_type: String,
        existing_target: String,
        new_target: String,
        existing_edge_key: String,
        new_edge_key: String,
    },
    Semantic {
        fact_id_a: String,
        fact_id_b: String,
        text_a: String,
        text_b: String,
        similarity: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictPair {
    pub id: String,
    pub details: ConflictDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionAction {
    pub conflict_id: String,
    pub resolved: bool,
    pub action_taken: String,
    pub winner_id: String,
    pub loser_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolutionReport {
    pub conflicts_found: usize,
    pub resolutions_applied: Vec<ResolutionAction>,
    pub dry_run: bool,
}

pub struct ConflictResolver;

impl ConflictResolver {
    pub fn run(
        graph: &GraphMemory,
        semantic: &SemanticMemory,
        exclusive_relations: &[String],
        semantic_threshold: f64,
        strategy: &str,
        dry_run: bool,
        scope: &MemoryScope,
    ) -> Result<ConflictResolutionReport> {
        let mut conflicts_found = 0;
        let mut resolutions_applied = Vec::new();

        // 1. Graph Conflicts
        let conn = graph.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        struct EdgeRecord {
            from_name: String,
            to_name: String,
            relation_type: String,
            valid_from: String,
            confidence: f64,
        }

        let mut stmt = conn.prepare(
            "SELECT from_name, to_name, relation_type, valid_from, confidence 
             FROM graph_edges 
             WHERE valid_until IS NULL 
               AND (user_id = ?1 OR user_id = '*')
               AND (session_id = ?2 OR session_id = '*')
               AND (agent_id = ?3 OR agent_id = '*')"
        )?;
        let mut rows = stmt.query(params![user_id, session_id, agent_id])?;
        let mut edges = Vec::new();
        while let Some(row) = rows.next()? {
            edges.push(EdgeRecord {
                from_name: row.get(0)?,
                to_name: row.get(1)?,
                relation_type: row.get(2)?,
                valid_from: row.get(3)?,
                confidence: row.get(4)?,
            });
        }

        // Group by (from_name, relation_type)
        use std::collections::HashMap;
        let mut groups: HashMap<(String, String), Vec<EdgeRecord>> = HashMap::new();
        for edge in edges {
            if exclusive_relations.contains(&edge.relation_type) {
                groups.entry((edge.from_name.clone(), edge.relation_type.clone()))
                    .or_default()
                    .push(edge);
            }
        }

        for ((from, rel), records) in groups {
            if records.len() > 1 {
                // Pairwise conflict comparison
                for i in 0..records.len() {
                    for j in (i + 1)..records.len() {
                        let r1 = &records[i];
                        let r2 = &records[j];
                        if r1.to_name != r2.to_name {
                            conflicts_found += 1;
                            let conflict_id = uuid::Uuid::new_v4().to_string();

                            let (winner, loser) = match strategy {
                                "confidence" => {
                                    if r1.confidence > r2.confidence {
                                        (r1, r2)
                                    } else if r2.confidence > r1.confidence {
                                        (r2, r1)
                                    } else {
                                        // fallback to recency
                                        if r1.valid_from > r2.valid_from { (r1, r2) } else { (r2, r1) }
                                    }
                                }
                                _ => {
                                    // Default: recency
                                    if r1.valid_from > r2.valid_from { (r1, r2) } else { (r2, r1) }
                                }
                            };

                            let action_taken = format!(
                                "Resolved graph conflict for {}({}): {} (valid: {}) wins over {} (valid: {})",
                                from, rel, winner.to_name, winner.valid_from, loser.to_name, loser.valid_from
                            );

                            if !dry_run {
                                conn.execute(
                                    "UPDATE graph_edges 
                                     SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 
                                         superseded_by = ?1 
                                     WHERE from_name = ?2 AND to_name = ?3 AND relation_type = ?4 AND valid_from = ?5",
                                    params![winner.to_name, loser.from_name, loser.to_name, loser.relation_type, loser.valid_from],
                                )?;
                            }

                            resolutions_applied.push(ResolutionAction {
                                conflict_id,
                                resolved: !dry_run,
                                action_taken,
                                winner_id: format!("{}-{}-{}", winner.from_name, winner.to_name, winner.relation_type),
                                loser_id: format!("{}-{}-{}", loser.from_name, loser.to_name, loser.relation_type),
                            });
                        }
                    }
                }
            }
        }

        Ok(ConflictResolutionReport {
            conflicts_found,
            resolutions_applied,
            dry_run,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization() {
        let report = ConflictResolutionReport {
            conflicts_found: 0,
            resolutions_applied: vec![],
            dry_run: true,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("conflictsFound"));
    }

    #[tokio::test]
    async fn test_graph_conflict_detection_and_resolution() -> Result<()> {
        use crate::layers::graph::{Entity, Relation};

        let db_path = std::env::temp_dir().join(format!("test_graph_conflict_{}.db", uuid::Uuid::new_v4()));
        let graph = GraphMemory::new(&db_path)?;
        let semantic = SemanticMemory::new(&db_path)?;
        let scope = MemoryScope::default();

        // Seed A lives_in NY (valid_from = earlier)
        let entity_a = Entity {
            name: "A".to_string(),
            entity_type: "Person".to_string(),
            observations: vec![],
        };
        let entity_ny = Entity {
            name: "NY".to_string(),
            entity_type: "City".to_string(),
            observations: vec![],
        };
        let entity_la = Entity {
            name: "LA".to_string(),
            entity_type: "City".to_string(),
            observations: vec![],
        };
        graph.create_entities(vec![entity_a, entity_ny, entity_la], &scope)?;

        // Add NY edge
        graph.create_relations(vec![Relation {
            from: "A".to_string(),
            to: "NY".to_string(),
            relation_type: "lives_in".to_string(),
        }], &scope)?;

        std::thread::sleep(std::time::Duration::from_secs(1));

        // Add LA edge
        graph.create_relations(vec![Relation {
            from: "A".to_string(),
            to: "LA".to_string(),
            relation_type: "lives_in".to_string(),
        }], &scope)?;

        let exclusive = vec!["lives_in".to_string()];

        // Detect conflicts (dry run)
        let report_dry = ConflictResolver::run(&graph, &semantic, &exclusive, 0.85, "recency", true, &scope)?;
        assert_eq!(report_dry.conflicts_found, 1);
        assert_eq!(report_dry.resolutions_applied.len(), 1);
        assert!(!report_dry.resolutions_applied[0].resolved); // not resolved in dry run

        // Resolve conflicts (live run)
        let report_live = ConflictResolver::run(&graph, &semantic, &exclusive, 0.85, "recency", false, &scope)?;
        assert_eq!(report_live.conflicts_found, 1);
        assert!(report_live.resolutions_applied[0].resolved);

        // Verify that LA wins (it was created later)
        let active = graph.read_graph(&scope)?;
        assert_eq!(active.relations.len(), 1);
        assert_eq!(active.relations[0].to, "LA");

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
