use crate::error::Result;
use petgraph::graph::UnGraph;
use petgraph::algo::tarjan_scc;
use crate::layers::graph::GraphMemory;
use crate::layers::MemoryScope;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Community {
    pub id: u32,
    pub members: Vec<String>,
    pub summary: String,
}

pub fn detect_communities(graph: &GraphMemory, scope: &MemoryScope) -> Result<Vec<Community>> {
    let kg = graph.read_graph(scope)?;

    let mut pet_graph = UnGraph::new_undirected();
    let mut node_map = std::collections::HashMap::new();
    let mut obs_map = std::collections::HashMap::new();

    for entity in &kg.entities {
        let idx = pet_graph.add_node(entity.name.clone());
        node_map.insert(entity.name.clone(), idx);
        obs_map.insert(entity.name.clone(), entity.observations.clone());
    }

    for relation in &kg.relations {
        if let (Some(&f), Some(&t)) = (node_map.get(&relation.from), node_map.get(&relation.to)) {
            pet_graph.add_edge(f, t, ());
        }
    }

    // Find components
    let components = tarjan_scc(&pet_graph);
    let mut communities = Vec::new();

    for (id, comp) in components.into_iter().enumerate() {
        let mut members: Vec<String> = comp.iter().map(|&idx| pet_graph.node_weight(idx).unwrap().clone()).collect();
        // Sort members to make order deterministic
        members.sort();

        let mut observations = Vec::new();
        for m in &members {
            if let Some(obs_list) = obs_map.get(m) {
                observations.extend(obs_list.clone());
            }
        }

        let summary = if observations.is_empty() {
            format!("Community of {} entities ({}) with no observations.", members.len(), members.join(", "))
        } else {
            format!("Community of {} entities ({}). Observations: {}", members.len(), members.join(", "), observations.join("; "))
        };

        communities.push(Community {
            id: id as u32,
            members,
            summary,
        });
    }

    Ok(communities)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::graph::{GraphMemory, Entity, Relation};
    use crate::layers::MemoryScope;

    #[test]
    fn test_community_detection() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_comm_{}.db", uuid::Uuid::new_v4()));
        let graph = GraphMemory::new(&db_path)?;
        let scope = MemoryScope::default();

        graph.create_entities(vec![
            Entity { name: "A".to_string(), entity_type: "Person".to_string(), observations: vec!["A lives in NY".to_string()] },
            Entity { name: "B".to_string(), entity_type: "Person".to_string(), observations: vec!["B lives in NY".to_string()] },
            Entity { name: "C".to_string(), entity_type: "Person".to_string(), observations: vec!["C lives in SF".to_string()] },
        ], &scope)?;

        graph.create_relations(vec![
            Relation { from: "A".to_string(), to: "B".to_string(), relation_type: "friend".to_string() },
        ], &scope)?;

        let comms = detect_communities(&graph, &scope)?;
        assert_eq!(comms.len(), 2); // Component {A, B} and Component {C}
        let has_ab = comms.iter().any(|c| c.members.contains(&"A".to_string()) && c.members.contains(&"B".to_string()));
        assert!(has_ab);

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
