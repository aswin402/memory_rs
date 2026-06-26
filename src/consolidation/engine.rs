use crate::layers::{GraphMemory, SemanticMemory, MemoryScope};
use crate::layers::graph::Relation;
use crate::search::dedup::SemanticDedup;
use anyhow::Result;
use rusqlite::params;

pub struct DecisionEngine;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DecisionReport {
    pub action: String, // "no-op", "update", "delete_add", "add"
    pub layer: String,  // "semantic", "graph"
    pub message: String,
    pub winner_id: Option<String>,
    pub loser_id: Option<String>,
}

impl DecisionEngine {
    pub fn decide_and_store(
        graph: &GraphMemory,
        semantic: &SemanticMemory,
        text: Option<&str>,
        relation: Option<&Relation>,
        scope: &MemoryScope,
    ) -> Result<DecisionReport> {
        // 1. Process relation input (Graph Layer)
        if let Some(rel) = relation {
            let exclusive_relations = vec![
                "lives_in".to_string(),
                "current_job".to_string(),
                "spouse".to_string(),
                "has_status".to_string(),
                "is_born_in".to_string(),
                "located_in".to_string(),
            ];

            if exclusive_relations.contains(&rel.relation_type) {
                let conn_g = graph.conn.lock();
                let user_id = scope.user_id.as_deref().unwrap_or("*");
                let session_id = scope.session_id.as_deref().unwrap_or("*");
                let agent_id = scope.agent_id.as_deref().unwrap_or("*");

                struct MatchingEdge {
                    to_name: String,
                }

                let mut matching = None;
                {
                    let mut stmt = conn_g.prepare(
                        "SELECT to_name 
                         FROM graph_edges 
                         WHERE from_name = ?1 
                           AND relation_type = ?2 
                           AND valid_until IS NULL
                           AND (user_id = ?3 OR user_id = '*')
                           AND (session_id = ?4 OR session_id = '*')
                           AND (agent_id = ?5 OR agent_id = '*')"
                    )?;
                    let mut rows = stmt.query(params![rel.from, rel.relation_type, user_id, session_id, agent_id])?;
                    if let Some(row) = rows.next()? {
                        matching = Some(MatchingEdge {
                            to_name: row.get(0)?,
                        });
                    }
                }

                if let Some(old_edge) = matching {
                    if old_edge.to_name != rel.to {
                        conn_g.execute(
                            "UPDATE graph_edges 
                             SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 
                                 superseded_by = ?1 
                             WHERE from_name = ?2 
                               AND to_name = ?3 
                               AND relation_type = ?4 
                               AND valid_until IS NULL
                               AND user_id = ?5
                               AND session_id = ?6
                               AND agent_id = ?7",
                            params![rel.to, rel.from, old_edge.to_name, rel.relation_type, user_id, session_id, agent_id],
                        )?;

                        drop(conn_g);

                        graph.create_relations(vec![rel.clone()], scope)?;

                        return Ok(DecisionReport {
                            action: "delete_add".to_string(),
                            layer: "graph".to_string(),
                            message: format!(
                                "Superseded old graph relation '{} {} {}' with '{} {} {}'",
                                rel.from, rel.relation_type, old_edge.to_name, rel.from, rel.relation_type, rel.to
                            ),
                            winner_id: Some(format!("{}-{}-{}", rel.from, rel.to, rel.relation_type)),
                            loser_id: Some(format!("{}-{}-{}", rel.from, old_edge.to_name, rel.relation_type)),
                        });
                    } else {
                        return Ok(DecisionReport {
                            action: "no-op".to_string(),
                            layer: "graph".to_string(),
                            message: format!("Graph relation '{} {} {}' already exists.", rel.from, rel.relation_type, rel.to),
                            winner_id: Some(format!("{}-{}-{}", rel.from, rel.to, rel.relation_type)),
                            loser_id: None,
                        });
                    }
                }
            }

            graph.create_relations(vec![rel.clone()], scope)?;
            return Ok(DecisionReport {
                action: "add".to_string(),
                layer: "graph".to_string(),
                message: format!("Created new graph relation '{}->{} ({})'", rel.from, rel.to, rel.relation_type),
                winner_id: Some(format!("{}-{}-{}", rel.from, rel.to, rel.relation_type)),
                loser_id: None,
            });
        }

        // 2. Process text statement (Semantic Layer)
        if let Some(t) = text {
            let duplicates = SemanticDedup::find_duplicates(semantic, t, 0.92, scope)?;

            if !duplicates.is_empty() {
                let best = duplicates.iter().max_by(|a, b| a.2.partial_cmp(&b.2).unwrap()).unwrap();
                let (best_id, best_text, similarity) = best;

                if *similarity >= 0.98 {
                    return Ok(DecisionReport {
                        action: "no-op".to_string(),
                        layer: "semantic".to_string(),
                        message: format!("Exact duplicate found in semantic memory (sim: {:.3}).", similarity),
                        winner_id: Some(best_id.clone()),
                        loser_id: None,
                    });
                } else {
                    let merged = SemanticDedup::merge_facts(best_text, t);
                    semantic.update_fact(best_id, &merged, 0.8, scope)?;

                    return Ok(DecisionReport {
                        action: "update".to_string(),
                        layer: "semantic".to_string(),
                        message: format!(
                            "Enriched existing semantic fact '{}' with merged text: '{}'",
                            best_id, merged
                        ),
                        winner_id: Some(best_id.clone()),
                        loser_id: None,
                    });
                }
            }

            let fact_id = format!("fact-{}", uuid::Uuid::new_v4());
            semantic.add_fact(&fact_id, t, 0.8, scope)?;

            return Ok(DecisionReport {
                action: "add".to_string(),
                layer: "semantic".to_string(),
                message: format!("Added new fact '{}' to semantic memory.", fact_id),
                winner_id: Some(fact_id),
                loser_id: None,
            });
        }

        anyhow::bail!("Either text or relation must be provided to DecisionEngine")
    }
}
