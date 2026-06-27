use crate::layers::MemoryScope;
use crate::layers::semantic::SemanticMemory;
use crate::layers::graph::GraphMemory;
use crate::layers::episodic::EpisodicMemory;
use anyhow::Result;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecallItem {
    pub layer: String,
    pub content: String,
    pub confidence: f64,
    pub metadata: Option<serde_json::Value>,
}

pub struct RecallEngine;

const STOP_WORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "anyone", "anything", "are", "as", "at",
    "be", "because", "been", "before", "being", "below", "between", "both", "but", "by",
    "can", "did", "do", "does", "doing", "don", "down", "during",
    "each",
    "few", "for", "from", "further",
    "had", "has", "have", "having", "he", "her", "here", "hers", "herself", "him", "himself", "his", "how",
    "i", "if", "in", "into", "is", "it", "its", "itself",
    "just",
    "me", "more", "most", "my", "myself",
    "no", "nor", "not", "now",
    "of", "off", "on", "once", "only", "or", "other", "our", "ours", "ourselves", "out", "over", "own",
    "s", "same", "she", "should", "so", "some", "someone", "something", "t", "than", "that", "the", "their", "theirs", "them", "themselves", "then", "there", "these", "they", "this", "those", "through", "to", "too",
    "under", "until", "up",
    "very",
    "was", "we", "were", "what", "when", "where", "which", "who", "whom", "why", "will", "with",
    "you", "your", "yours", "yourself", "yourselves",
];

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.binary_search(&word).is_ok()
}

fn extract_keywords(context: &str) -> Vec<String> {
    let clean = context
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>();
    let mut words = Vec::new();
    for word in clean.split_whitespace() {
        let lower = word.to_lowercase();
        if lower.len() >= 3 && !is_stop_word(&lower) {
            words.push(lower);
        }
    }
    let mut seen = std::collections::HashSet::new();
    words.retain(|w| seen.insert(w.clone()));
    words
}

impl RecallEngine {
    pub fn recall(
        semantic: &SemanticMemory,
        graph: &GraphMemory,
        episodic: &EpisodicMemory,
        current_context: &str,
        max_results: usize,
        scope: &MemoryScope,
    ) -> Result<Vec<RecallItem>> {
        let mut raw_items = Vec::new();
        let keywords = extract_keywords(current_context);

        // 1. Semantic Memory
        if !current_context.trim().is_empty() {
            match semantic.query_similar_facts(current_context, max_results, scope) {
                Ok(facts) => {
                    for fact in facts {
                        let confidence = fact.similarity.clamp(0.0, 1.0);
                        raw_items.push(RecallItem {
                            layer: "semantic".to_string(),
                            content: fact.raw_text.clone(),
                            confidence,
                            metadata: Some(serde_json::json!({
                                "nodeId": fact.node_id,
                                "timestamp": fact.timestamp,
                                "importance": fact.importance,
                            })),
                        });
                    }
                }
                Err(err) => {
                    log::error!("Proactive recall semantic query failed: {}", err);
                }
            }
        }

        // 2. Graph Memory
        let mut graph_results = Vec::new();
        if !current_context.trim().is_empty() {
            if let Ok(kg) = graph.search_nodes(current_context, scope) {
                graph_results.push(kg);
            }
        }
        for kw in &keywords {
            if let Ok(kg) = graph.search_nodes(kw, scope) {
                graph_results.push(kg);
            }
        }

        let mut entities = std::collections::HashMap::new();
        let mut relations = std::collections::HashSet::new();
        for kg in graph_results {
            for entity in kg.entities {
                entities.insert(entity.name.clone(), entity);
            }
            for relation in kg.relations {
                relations.insert((relation.from.clone(), relation.to.clone(), relation.relation_type.clone()));
            }
        }

        for (_, entity) in entities {
            let mut matched_keywords_count = 0;
            let lower_name = entity.name.to_lowercase();
            let lower_type = entity.entity_type.to_lowercase();
            for kw in &keywords {
                let mut found = false;
                if lower_name.contains(kw) || lower_type.contains(kw) {
                    found = true;
                } else {
                    for obs in &entity.observations {
                        if obs.to_lowercase().contains(kw) {
                            found = true;
                            break;
                        }
                    }
                }
                if found {
                    matched_keywords_count += 1;
                }
            }
            let base_confidence = 0.6;
            let confidence = if keywords.is_empty() {
                base_confidence
            } else {
                (base_confidence + 0.1 * (matched_keywords_count as f64)).min(1.0)
            };

            let content = if entity.observations.is_empty() {
                format!("Entity: {} ({})", entity.name, entity.entity_type)
            } else {
                format!(
                    "Entity: {} ({}) - Observations: {}",
                    entity.name,
                    entity.entity_type,
                    entity.observations.join(", ")
                )
            };

            raw_items.push(RecallItem {
                layer: "graph".to_string(),
                content,
                confidence,
                metadata: Some(serde_json::json!({
                    "name": entity.name,
                    "entityType": entity.entity_type,
                    "observations": entity.observations,
                })),
            });
        }

        for (from, to, relation_type) in relations {
            let mut matched_keywords_count = 0;
            let lower_from = from.to_lowercase();
            let lower_to = to.to_lowercase();
            let lower_type = relation_type.to_lowercase();
            for kw in &keywords {
                if lower_from.contains(kw) || lower_to.contains(kw) || lower_type.contains(kw) {
                    matched_keywords_count += 1;
                }
            }
            let base_confidence = 0.5;
            let confidence = if keywords.is_empty() {
                base_confidence
            } else {
                (base_confidence + 0.1 * (matched_keywords_count as f64)).min(1.0)
            };

            let content = format!("Relation: {} - {} - {}", from, relation_type, to);
            raw_items.push(RecallItem {
                layer: "graph".to_string(),
                content,
                confidence,
                metadata: Some(serde_json::json!({
                    "from": from,
                    "to": to,
                    "relationType": relation_type,
                })),
            });
        }

        // 3. Episodic Memory
        let mut reflection_items = std::collections::HashMap::new();
        if !current_context.trim().is_empty() {
            if let Ok(refs) = episodic.get_reflections(current_context, scope) {
                for r in refs {
                    reflection_items.insert(r.id.clone(), r);
                }
            }
        }
        for kw in &keywords {
            if let Ok(refs) = episodic.get_reflections(kw, scope) {
                for r in refs {
                    reflection_items.insert(r.id.clone(), r);
                }
            }
        }

        for (_, r) in reflection_items {
            let mut matched_keywords_count = 0;
            let text_to_check = format!(
                "{} {} {} {}",
                r.task_description,
                r.reflection,
                r.root_cause.as_deref().unwrap_or(""),
                r.solution_applied.as_deref().unwrap_or("")
            ).to_lowercase();

            for kw in &keywords {
                if text_to_check.contains(kw) {
                    matched_keywords_count += 1;
                }
            }
            let base_confidence = 0.6;
            let confidence = if keywords.is_empty() {
                base_confidence
            } else {
                (base_confidence + 0.1 * (matched_keywords_count as f64)).min(1.0)
            };

            let content = format!(
                "Reflection on '{}' (Status: {}): {} | Root Cause: {} | Solution: {}",
                r.task_description,
                r.status,
                r.reflection,
                r.root_cause.as_deref().unwrap_or("None"),
                r.solution_applied.as_deref().unwrap_or("None")
            );

            raw_items.push(RecallItem {
                layer: "episodic".to_string(),
                content,
                confidence,
                metadata: Some(serde_json::json!({
                    "id": r.id,
                    "taskDescription": r.task_description,
                    "status": r.status,
                    "attemptNumber": r.attempt_number,
                    "createdAt": r.created_at,
                })),
            });
        }

        // 4. Merge identical content and boost confidence
        let mut merged_items: Vec<RecallItem> = Vec::new();
        for item in raw_items {
            let lower_content = item.content.trim().to_lowercase();
            if let Some(existing) = merged_items.iter_mut().find(|x| x.content.trim().to_lowercase() == lower_content) {
                let max_conf = existing.confidence.max(item.confidence);
                existing.confidence = (max_conf + 0.15).min(1.0);
                if !existing.layer.contains(&item.layer) {
                    existing.layer = format!("{},{}", existing.layer, item.layer);
                }
                if let (Some(existing_meta), Some(new_meta)) = (&mut existing.metadata, &item.metadata) {
                    if let Some(obj_exist) = existing_meta.as_object_mut() {
                        if let Some(obj_new) = new_meta.as_object() {
                            for (k, v) in obj_new {
                                obj_exist.insert(k.clone(), v.clone());
                            }
                        }
                    }
                } else if existing.metadata.is_none() {
                    existing.metadata = item.metadata.clone();
                }
            } else {
                merged_items.push(item);
            }
        }

        // 5. Cross-layer entity name reference boosting
        let mut entity_names = Vec::new();
        for item in &merged_items {
            if item.layer.contains("graph") {
                if let Some(meta) = &item.metadata {
                    if let Some(name) = meta.get("name").and_then(|n| n.as_str()) {
                        entity_names.push(name.to_lowercase());
                    }
                }
            }
        }

        let mut boosts = vec![0.0; merged_items.len()];
        for (i, item) in merged_items.iter().enumerate() {
            let lower_content = item.content.to_lowercase();
            for ent_name in &entity_names {
                if item.layer.contains("graph") {
                    if let Some(meta) = &item.metadata {
                        if let Some(name) = meta.get("name").and_then(|n| n.as_str()) {
                            if name.to_lowercase() == *ent_name {
                                continue;
                            }
                        }
                    }
                }
                
                if lower_content.contains(ent_name) {
                    boosts[i] += 0.1;
                    for (j, other) in merged_items.iter().enumerate() {
                        if other.layer.contains("graph") {
                            if let Some(meta) = &other.metadata {
                                if let Some(name) = meta.get("name").and_then(|n| n.as_str()) {
                                    if name.to_lowercase() == *ent_name {
                                        boosts[j] += 0.1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        for (i, item) in merged_items.iter_mut().enumerate() {
            if boosts[i] > 0.0 {
                item.confidence = (item.confidence + boosts[i]).min(1.0);
            }
        }

        // 6. Sort and truncate
        merged_items.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal)
        });
        merged_items.truncate(max_results);

        Ok(merged_items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::MemoryScope;
    use crate::layers::graph::{Entity, Relation};
    use crate::layers::episodic::ReflectionItem;
    use std::fs;

    #[test]
    fn test_proactive_recall_fused() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_recall_{}.db", uuid::Uuid::new_v4()));
        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        // Initialize layers
        let semantic = SemanticMemory::new(&db_path)?;
        let graph = GraphMemory::new(&db_path)?;
        let episodic = EpisodicMemory::new(&db_path)?;
        let scope = MemoryScope::default();

        // 1. Populate Semantic Memory
        semantic.add_fact("node-1", "Rust compiler is very fast and safe.", 0.9, &scope)?;
        semantic.add_fact("node-2", "Python is an interpreted scripting language.", 0.6, &scope)?;

        // 2. Populate Graph Memory
        graph.create_entities(
            vec![
                Entity {
                    name: "Rust".to_string(),
                    entity_type: "Language".to_string(),
                    observations: vec!["Supports memory safety without GC".to_string()],
                },
                Entity {
                    name: "Compiler".to_string(),
                    entity_type: "Tool".to_string(),
                    observations: vec!["Translates Rust code to machine instructions".to_string()],
                },
            ],
            &scope,
        )?;
        graph.create_relations(
            vec![Relation {
                from: "Rust".to_string(),
                to: "Compiler".to_string(),
                relation_type: "compiled_by".to_string(),
            }],
            &scope,
        )?;

        // 3. Populate Episodic Memory
        episodic.log_reflection(
            ReflectionItem {
                id: "ref-1".to_string(),
                task_description: "Compile a rust application".to_string(),
                status: "Success".to_string(),
                attempt_number: 1,
                steps_taken: "Ran cargo build --release".to_string(),
                error_encountered: None,
                root_cause: None,
                solution_applied: None,
                reflection: "Using the compiler was extremely fast.".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
            &scope,
        )?;

        // Run proactive recall
        let query = "rust compiler";
        let results = RecallEngine::recall(&semantic, &graph, &episodic, query, 10, &scope)?;

        // Validate results
        assert!(!results.is_empty(), "Should return recalled items");

        // Verify that we got items from all layers (semantic, graph, episodic)
        let has_semantic = results.iter().any(|item| item.layer.contains("semantic"));
        let has_graph = results.iter().any(|item| item.layer.contains("graph"));
        let has_episodic = results.iter().any(|item| item.layer.contains("episodic"));

        assert!(has_semantic, "Recall should contain semantic items");
        assert!(has_graph, "Recall should contain graph items");
        assert!(has_episodic, "Recall should contain episodic items");

        // Verify sorting by confidence descending
        for i in 0..results.len() - 1 {
            assert!(
                results[i].confidence >= results[i + 1].confidence,
                "Results must be sorted by confidence descending: {} < {}",
                results[i].confidence,
                results[i + 1].confidence
            );
        }

        // Verify score normalization (all should be between 0.0 and 1.0)
        for item in &results {
            assert!(
                item.confidence >= 0.0 && item.confidence <= 1.0,
                "Confidence score {} must be normalized in [0.0, 1.0]",
                item.confidence
            );
        }

        // Cleanup
        let _ = fs::remove_file(&db_path);
        Ok(())
    }
}
