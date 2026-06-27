use crate::coordinator::MemoryCoordinator;
use crate::layers::MemoryScope;
use crate::search::importance::ImportanceScorer;
use crate::search::dedup::SemanticDedup;
use crate::layers::semantic::calculate_cosine_similarity;
use crate::error::Result;
use rusqlite::params;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionReport {
    pub removed_count: u32,
    pub merged_count: u32,
    pub details: String,
}

pub struct MemoryCompactor;

impl MemoryCompactor {
    pub fn run_compaction(
        coordinator: &MemoryCoordinator,
        strategy: &str,
        dry_run: bool,
        min_importance: f64,
        max_age_hours: f64,
        cluster_threshold: f64,
        scope: &MemoryScope,
    ) -> Result<CompactionReport> {
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        let conn = coordinator.semantic.conn.lock();

        // 1. Fetch active facts in scope
        struct RawFact {
            node_id: String,
            raw_text: String,
            embedding_blob: Vec<u8>,
            timestamp: String,
        }

        let mut facts = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT node_id, raw_text, embedding, timestamp 
                 FROM semantic_metadata 
                 WHERE valid_until IS NULL
                   AND (user_id = ?1 OR user_id = '*')
                   AND (session_id = ?2 OR session_id = '*')
                   AND (agent_id = ?3 OR agent_id = '*')"
            )?;
            let rows = stmt.query_map(params![user_id, session_id, agent_id], |r| {
                Ok(RawFact {
                    node_id: r.get(0)?,
                    raw_text: r.get(1)?,
                    embedding_blob: r.get(2)?,
                    timestamp: r.get(3)?,
                })
            })?;

            for r in rows {
                facts.push(r?);
            }
        }

        let mut removed_count = 0;
        let mut merged_count = 0;
        let mut details = Vec::new();

        let now = chrono::Utc::now();

        // Helper to parse embedding vector
        let parse_vector = |blob: &[u8]| -> Vec<f32> {
            let mut vec = Vec::new();
            for chunk in blob.chunks_exact(4) {
                let array: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                vec.push(f32::from_ne_bytes(array));
            }
            vec
        };

        // --- STRATEGY: DECAY OR BOTH ---
        if strategy == "decay" || strategy == "both" {
            for fact in &facts {
                // Get access count
                let access_count: u32 = coordinator.episodic.conn.lock().query_row(
                    "SELECT COUNT(*) FROM memory_access_log WHERE memory_id = ?1 AND layer = 'semantic'",
                    params![fact.node_id],
                    |r| r.get(0)
                )?;

                // Get edge count (connections in graph where entity is mentioned)
                let mut edge_count = 0;
                {
                    let conn_g = coordinator.graph.conn.lock();
                    let mut stmt_nodes = conn_g.prepare(
                        "SELECT name FROM graph_nodes 
                         WHERE (user_id = ?1 OR user_id = '*')
                           AND (session_id = ?2 OR session_id = '*')
                           AND (agent_id = ?3 OR agent_id = '*')"
                    )?;
                    let nodes = stmt_nodes.query_map(params![user_id, session_id, agent_id], |r| r.get::<_, String>(0))?;
                    for node_name in nodes {
                        if let Ok(name) = node_name {
                            if fact.raw_text.to_lowercase().contains(&name.to_lowercase()) {
                                let count: u32 = conn_g.query_row(
                                    "SELECT COUNT(*) FROM graph_edges 
                                     WHERE (from_name = ?1 OR to_name = ?1) 
                                       AND valid_until IS NULL
                                       AND (user_id = ?2 OR user_id = '*')
                                       AND (session_id = ?3 OR session_id = '*')
                                       AND (agent_id = ?4 OR agent_id = '*')",
                                    params![name, user_id, session_id, agent_id],
                                    |r| r.get(0)
                                ).unwrap_or(0);
                                edge_count += count;
                            }
                        }
                    }
                }

                // Get age in hours
                let created = chrono::DateTime::parse_from_rfc3339(&fact.timestamp)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or(now);
                let age_hours = now.signed_duration_since(created).num_seconds() as f64 / 3600.0;

                // Calculate importance
                let computed = ImportanceScorer::calculate_importance(
                    access_count,
                    edge_count,
                    age_hours,
                    false,
                );

                // Update importance in DB
                if !dry_run {
                    conn.execute(
                        "UPDATE semantic_metadata SET importance = ?1 WHERE node_id = ?2 AND valid_until IS NULL",
                        params![computed, fact.node_id],
                    )?;
                }

                if computed < min_importance && age_hours > max_age_hours {
                    removed_count += 1;
                    if !dry_run {
                        conn.execute(
                            "UPDATE semantic_metadata SET valid_until = ?1 WHERE node_id = ?2 AND valid_until IS NULL",
                            params![now.to_rfc3339(), fact.node_id],
                        )?;
                    }
                    details.push(format!("Decayed & archived fact '{}' (importance: {:.3})", fact.node_id, computed));
                }
            }
        }

        // Re-fetch remaining active facts for clustering if needed
        let active_facts = if strategy == "cluster" || strategy == "both" {
            let mut stmt = conn.prepare(
                "SELECT node_id, raw_text, embedding, timestamp 
                 FROM semantic_metadata 
                 WHERE valid_until IS NULL
                   AND (user_id = ?1 OR user_id = '*')
                   AND (session_id = ?2 OR session_id = '*')
                   AND (agent_id = ?3 OR agent_id = '*')"
            )?;
            let rows = stmt.query_map(params![user_id, session_id, agent_id], |r| {
                Ok(RawFact {
                    node_id: r.get(0)?,
                    raw_text: r.get(1)?,
                    embedding_blob: r.get(2)?,
                    timestamp: r.get(3)?,
                })
            })?;
            let mut vec = Vec::new();
            for r in rows {
                vec.push(r?);
            }
            vec
        } else {
            Vec::new()
        };

        // --- STRATEGY: CLUSTER OR BOTH ---
        if (strategy == "cluster" || strategy == "both") && active_facts.len() >= 2 {
            let mut visited = std::collections::HashSet::new();
            for i in 0..active_facts.len() {
                if visited.contains(&active_facts[i].node_id) {
                    continue;
                }

                let vec_i = parse_vector(&active_facts[i].embedding_blob);
                let mut cluster_indices = vec![i];

                for j in (i + 1)..active_facts.len() {
                    if visited.contains(&active_facts[j].node_id) {
                        continue;
                    }
                    let vec_j = parse_vector(&active_facts[j].embedding_blob);
                    let similarity = calculate_cosine_similarity(&vec_i, &vec_j);

                    if similarity >= cluster_threshold {
                        cluster_indices.push(j);
                    }
                }

                if cluster_indices.len() >= 2 {
                    // Consolidate cluster: Pick winner with highest importance / recency
                    let mut winner_idx = cluster_indices[0];
                    let mut winner_importance = 0.0;

                    for &idx in &cluster_indices {
                        let imp: f64 = conn.query_row(
                            "SELECT importance FROM semantic_metadata WHERE node_id = ?1 AND valid_until IS NULL",
                            params![active_facts[idx].node_id],
                            |r| r.get(0)
                        )?;
                        if imp > winner_importance {
                            winner_importance = imp;
                            winner_idx = idx;
                        }
                    }

                    // Mark winner and merge texts
                    let winner = &active_facts[winner_idx];
                    let mut merged_text = winner.raw_text.clone();

                    for &idx in &cluster_indices {
                        if idx == winner_idx {
                            continue;
                        }
                        let loser = &active_facts[idx];
                        visited.insert(loser.node_id.clone());
                        merged_text = SemanticDedup::merge_facts(&merged_text, &loser.raw_text);

                        merged_count += 1;
                        if !dry_run {
                            conn.execute(
                                "UPDATE semantic_metadata 
                                 SET valid_until = ?1, superseded_by = ?2 
                                 WHERE node_id = ?3 AND valid_until IS NULL",
                                params![now.to_rfc3339(), winner.node_id, loser.node_id],
                            )?;
                        }
                        details.push(format!("Consolidated fact '{}' into winner '{}'", loser.node_id, winner.node_id));
                    }

                    // Update winner text and embedding
                    if !dry_run && merged_text != winner.raw_text {
                        drop(conn);
                        coordinator.semantic.update_fact(&winner.node_id, &merged_text, winner_importance, scope)?;
                        
                        let mut sub_report = Self::run_compaction(coordinator, strategy, dry_run, min_importance, max_age_hours, cluster_threshold, scope)?;
                        sub_report.removed_count += removed_count;
                        sub_report.merged_count += merged_count;
                        if !details.is_empty() {
                            if sub_report.details.is_empty() {
                                sub_report.details = details.join("; ");
                            } else {
                                sub_report.details = format!("{}; {}", details.join("; "), sub_report.details);
                            }
                        }
                        return Ok(sub_report);
                    }
                }
                visited.insert(active_facts[i].node_id.clone());
            }
        }

        // Rebuild HNSW if not dry_run and changes occurred
        if !dry_run && (removed_count > 0 || merged_count > 0) {
            // Rebuild index
            let dimensions = 384;
            if let Ok(new_index) = crate::layers::semantic::rebuild_hnsw_index(&conn, dimensions) {
                *coordinator.semantic.hnsw_index.lock() = new_index;
            }
            drop(conn);
            let _ = coordinator.semantic.query_similar_facts_vector("rebuild HNSW index trigger", 1, scope);
        } else {
            drop(conn);
        }

        Ok(CompactionReport {
            removed_count,
            merged_count,
            details: details.join("; "),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::MemoryScope;
    use std::sync::Arc;

    #[test]
    fn test_compactor_decay_and_cluster() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_compactor_{}.db", uuid::Uuid::new_v4()));
        let coordinator = Arc::new(MemoryCoordinator::new(db_path.to_str().unwrap(), 300)?);
        let scope = MemoryScope::default();

        // 1. Add decaying facts
        coordinator.semantic.add_fact("fact-1", "I hate vegetables", 0.01, &scope)?;
        coordinator.semantic.add_fact("fact-2", "I love clean coding", 0.9, &scope)?;

        // 2. Add highly similar facts for clustering
        coordinator.semantic.add_fact("fact-3", "Aswin is a Rust engineer", 0.8, &scope)?;
        coordinator.semantic.add_fact("fact-4", "Aswin works with Rust code", 0.8, &scope)?;

        // Set fact-1 to be very old
        {
            let conn = coordinator.semantic.conn.lock();
            let old_time = (chrono::Utc::now() - chrono::Duration::hours(48)).to_rfc3339();
            conn.execute("UPDATE semantic_metadata SET timestamp = ?1 WHERE node_id = 'fact-1'", params![old_time])?;
        }

        // Run compaction (both strategies)
        let report = MemoryCompactor::run_compaction(
            &coordinator,
            "both",
            false,
            0.15,
            24.0,
            0.75,
            &scope,
        )?;

        assert!(report.removed_count >= 1, "Should decay and remove fact-1");
        assert!(report.merged_count >= 1, "Should merge similar facts fact-3 and fact-4");

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
