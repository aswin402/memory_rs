use serde::{Serialize, Deserialize};
use schemars::JsonSchema;
use crate::layers::{GraphMemory, SemanticMemory, MemoryScope};
use crate::layers::semantic::{rebuild_hnsw_index, calculate_cosine_similarity};
use crate::error::Result;
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
    pub conflicts: Vec<ConflictPair>,
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
        let mut conflicts = Vec::new();
        let mut resolutions_applied = Vec::new();

        // 1. Graph Conflicts
        let conn_g = graph.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        struct EdgeRecord {
            from_name: String,
            to_name: String,
            relation_type: String,
            valid_from: String,
            confidence: f64,
            user_id: String,
            session_id: String,
            agent_id: String,
        }

        let mut edges = Vec::new();
        {
            let mut stmt = conn_g.prepare(
                "SELECT from_name, to_name, relation_type, valid_from, confidence, user_id, session_id, agent_id 
                 FROM graph_edges 
                 WHERE valid_until IS NULL 
                   AND (user_id = ?1 OR user_id = '*')
                   AND (session_id = ?2 OR session_id = '*')
                   AND (agent_id = ?3 OR agent_id = '*')"
            )?;
            let mut rows = stmt.query(params![user_id, session_id, agent_id])?;
            while let Some(row) = rows.next()? {
                edges.push(EdgeRecord {
                    from_name: row.get(0)?,
                    to_name: row.get(1)?,
                    relation_type: row.get(2)?,
                    valid_from: row.get(3)?,
                    confidence: row.get(4)?,
                    user_id: row.get(5)?,
                    session_id: row.get(6)?,
                    agent_id: row.get(7)?,
                });
            }
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

                            let (existing, new) = if r1.valid_from <= r2.valid_from {
                                (r1, r2)
                            } else {
                                (r2, r1)
                            };

                            let existing_edge_key = format!("{}-{}-{}", existing.from_name, existing.to_name, existing.relation_type);
                            let new_edge_key = format!("{}-{}-{}", new.from_name, new.to_name, new.relation_type);

                            let details = ConflictDetails::Graph {
                                from_name: from.clone(),
                                relation_type: rel.clone(),
                                existing_target: existing.to_name.clone(),
                                new_target: new.to_name.clone(),
                                existing_edge_key,
                                new_edge_key,
                            };

                            conflicts.push(ConflictPair {
                                id: conflict_id.clone(),
                                details,
                            });

                            let action_taken = format!(
                                "Resolved graph conflict for {}({}): {} (valid: {}) wins over {} (valid: {})",
                                from, rel, winner.to_name, winner.valid_from, loser.to_name, loser.valid_from
                            );

                            if !dry_run {
                                conn_g.execute(
                                    "UPDATE graph_edges 
                                     SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 
                                         superseded_by = ?1 
                                     WHERE from_name = ?2 
                                       AND to_name = ?3 
                                       AND relation_type = ?4 
                                       AND valid_from = ?5
                                       AND user_id = ?6
                                       AND session_id = ?7
                                       AND agent_id = ?8",
                                    params![
                                        winner.to_name,
                                        loser.from_name,
                                        loser.to_name,
                                        loser.relation_type,
                                        loser.valid_from,
                                        loser.user_id,
                                        loser.session_id,
                                        loser.agent_id
                                    ],
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

        // Drop lock on conn_g
        drop(conn_g);

        // 2. Semantic Conflicts
        let conn_s = semantic.conn.lock();
        struct SemanticRecord {
            node_id: String,
            raw_text: String,
            embedding: Vec<f32>,
            valid_from: String,
            importance: f64,
            user_id: String,
            session_id: String,
            agent_id: String,
        }

        let mut facts = Vec::new();
        {
            let mut stmt_s = conn_s.prepare(
                "SELECT node_id, raw_text, embedding, valid_from, importance, user_id, session_id, agent_id 
                 FROM semantic_metadata 
                 WHERE valid_until IS NULL 
                   AND (user_id = ?1 OR user_id = '*')
                   AND (session_id = ?2 OR session_id = '*')
                   AND (agent_id = ?3 OR agent_id = '*')"
            )?;
            let mut rows_s = stmt_s.query(params![user_id, session_id, agent_id])?;
            while let Some(row) = rows_s.next()? {
                let node_id: String = row.get(0)?;
                let raw_text: String = row.get(1)?;
                let blob: Vec<u8> = row.get(2)?;
                let valid_from: String = row.get(3)?;
                let importance: f64 = row.get(4)?;
                let u_id: String = row.get(5)?;
                let s_id: String = row.get(6)?;
                let a_id: String = row.get(7)?;

                // Deserialize embedding
                let embedding: Vec<f32> = blob
                    .chunks_exact(4)
                    .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
                    .collect();

                facts.push(SemanticRecord {
                    node_id,
                    raw_text,
                    embedding,
                    valid_from,
                    importance,
                    user_id: u_id,
                    session_id: s_id,
                    agent_id: a_id,
                });
            }
        }

        let mut resolved_loser_ids = std::collections::HashSet::new();

        // Pairwise similarity comparison
        for i in 0..facts.len() {
            for j in (i + 1)..facts.len() {
                let f1 = &facts[i];
                let f2 = &facts[j];

                // Skip if either has already been resolved as a loser in this pass
                if resolved_loser_ids.contains(&f1.node_id) || resolved_loser_ids.contains(&f2.node_id) {
                    continue;
                }

                let sim = calculate_cosine_similarity(&f1.embedding, &f2.embedding);
                if sim >= semantic_threshold {
                    conflicts_found += 1;
                    let conflict_id = uuid::Uuid::new_v4().to_string();

                    let (winner, loser) = match strategy {
                        "confidence" | "importance" => {
                            if f1.importance > f2.importance {
                                (f1, f2)
                            } else if f2.importance > f1.importance {
                                (f2, f1)
                            } else {
                                if f1.valid_from > f2.valid_from { (f1, f2) } else { (f2, f1) }
                            }
                        }
                        _ => {
                            if f1.valid_from > f2.valid_from { (f1, f2) } else { (f2, f1) }
                        }
                    };

                    resolved_loser_ids.insert(loser.node_id.clone());

                    let details = ConflictDetails::Semantic {
                        fact_id_a: f1.node_id.clone(),
                        fact_id_b: f2.node_id.clone(),
                        text_a: f1.raw_text.clone(),
                        text_b: f2.raw_text.clone(),
                        similarity: sim,
                    };

                    conflicts.push(ConflictPair {
                        id: conflict_id.clone(),
                        details,
                    });

                    let action_taken = format!(
                        "Resolved semantic conflict (sim={:.3}): \"{}\" (valid: {}) wins over \"{}\" (valid: {})",
                        sim, winner.raw_text, winner.valid_from, loser.raw_text, loser.valid_from
                    );

                    if !dry_run {
                        conn_s.execute(
                            "UPDATE semantic_metadata 
                             SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 
                                 superseded_by = ?1 
                             WHERE node_id = ?2 
                               AND valid_from = ?3
                               AND user_id = ?4
                               AND session_id = ?5
                               AND agent_id = ?6",
                            params![
                                winner.node_id,
                                loser.node_id,
                                loser.valid_from,
                                loser.user_id,
                                loser.session_id,
                                loser.agent_id
                            ],
                        )?;
                    }

                    resolutions_applied.push(ResolutionAction {
                        conflict_id,
                        resolved: !dry_run,
                        action_taken,
                        winner_id: winner.node_id.clone(),
                        loser_id: loser.node_id.clone(),
                    });
                }
            }
        }

        // Rebuild HNSW if anything mutated
        if !dry_run && !resolved_loser_ids.is_empty() {
            let dimensions = 384;
            let world = rebuild_hnsw_index(&conn_s, dimensions)?;
            *semantic.hnsw_index.lock() = world;
        }

        Ok(ConflictResolutionReport {
            conflicts_found,
            conflicts,
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
            conflicts: vec![],
            resolutions_applied: vec![],
            dry_run: true,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("conflictsFound"));
        assert!(json.contains("conflicts"));
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
        assert_eq!(report_dry.conflicts.len(), 1);

        // Resolve conflicts (live run)
        let report_live = ConflictResolver::run(&graph, &semantic, &exclusive, 0.85, "recency", false, &scope)?;
        assert_eq!(report_live.conflicts_found, 1);
        assert!(report_live.resolutions_applied[0].resolved);
        assert_eq!(report_live.conflicts.len(), 1);

        // Verify that LA wins (it was created later)
        let active = graph.read_graph(&scope)?;
        assert_eq!(active.relations.len(), 1);
        assert_eq!(active.relations[0].to, "LA");

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn test_semantic_conflict_detection_and_resolution() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_semantic_conflict_{}.db", uuid::Uuid::new_v4()));
        let graph = GraphMemory::new(&db_path)?;
        let semantic = SemanticMemory::new(&db_path)?;
        let scope = MemoryScope::default();

        // Add two semantically identical facts
        // fact-1 (earlier)
        semantic.add_fact("fact-1", "Aswin lives in San Francisco", 0.9, &scope)?;

        std::thread::sleep(std::time::Duration::from_secs(1));

        // fact-2 (later)
        semantic.add_fact("fact-2", "Aswin resides in San Francisco city", 0.9, &scope)?;

        // Detect conflicts
        let report_dry = ConflictResolver::run(&graph, &semantic, &[], 0.85, "recency", true, &scope)?;
        assert_eq!(report_dry.conflicts_found, 1);
        assert_eq!(report_dry.resolutions_applied.len(), 1);
        assert!(!report_dry.resolutions_applied[0].resolved);
        assert_eq!(report_dry.conflicts.len(), 1);

        // Resolve live
        let report_live = ConflictResolver::run(&graph, &semantic, &[], 0.85, "recency", false, &scope)?;
        assert_eq!(report_live.conflicts_found, 1);
        assert!(report_live.resolutions_applied[0].resolved);
        assert_eq!(report_live.conflicts.len(), 1);

        // Verify fact-1 is invalidated and only fact-2 is active
        let active = semantic.query_as_of(&chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(), &scope)?;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].node_id, "fact-2");

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
