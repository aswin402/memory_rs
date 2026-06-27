use crate::error::{Result, MemoryError};
use petgraph::graph::DiGraph;
use crate::layers::graph::GraphMemory;
use crate::layers::MemoryScope;
use rusqlite::params;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TraversalStep {
    pub entity_name: String,
    pub relation_type: String,
    pub depth: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PathResult {
    pub path: Vec<String>,
    pub relations: Vec<String>,
}

fn build_petgraph(graph: &GraphMemory, scope: &MemoryScope) -> Result<(DiGraph<String, String>, std::collections::HashMap<String, petgraph::graph::NodeIndex>)> {
    let conn = graph.conn.lock();
    let user_id = scope.user_id.as_deref().unwrap_or("*");
    let session_id = scope.session_id.as_deref().unwrap_or("*");
    let agent_id = scope.agent_id.as_deref().unwrap_or("*");

    let mut stmt = conn.prepare(
        "SELECT from_name, to_name, relation_type 
         FROM graph_edges 
         WHERE valid_until IS NULL
           AND (user_id = ?1 OR user_id = '*')
           AND (session_id = ?2 OR session_id = '*')
           AND (agent_id = ?3 OR agent_id = '*')"
    )?;

    let mut pet_graph = DiGraph::new();
    let mut node_map = std::collections::HashMap::new();

    let rows = stmt.query_map(params![user_id, session_id, agent_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
    })?;

    for row in rows {
        let (from, to, rel) = row?;
        let from_idx = *node_map.entry(from.clone()).or_insert_with(|| pet_graph.add_node(from));
        let to_idx = *node_map.entry(to.clone()).or_insert_with(|| pet_graph.add_node(to));
        pet_graph.add_edge(from_idx, to_idx, rel);
    }

    Ok((pet_graph, node_map))
}

pub fn bfs_traverse(graph: &GraphMemory, start_entity: &str, max_depth: u32, scope: &MemoryScope) -> Result<Vec<TraversalStep>> {
    let (pet_graph, node_map) = build_petgraph(graph, scope)?;
    let start_idx = match node_map.get(start_entity) {
        Some(&idx) => idx,
        None => return Ok(Vec::new()),
    };

    let mut steps = Vec::new();
    let mut queue = std::collections::VecDeque::new();
    let mut visited = std::collections::HashSet::new();

    queue.push_back((start_idx, 0));
    visited.insert(start_idx);

    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let mut neighbors = pet_graph.neighbors(node).detach();
        while let Some((edge, neighbor)) = neighbors.next(&pet_graph) {
            if visited.insert(neighbor) {
                let next_depth = depth + 1;
                queue.push_back((neighbor, next_depth));

                let rel = pet_graph.edge_weight(edge).unwrap().clone();
                let name = pet_graph.node_weight(neighbor).unwrap().clone();
                steps.push(TraversalStep {
                    entity_name: name,
                    relation_type: rel,
                    depth: next_depth,
                });
            }
        }
    }

    Ok(steps)
}

pub fn shortest_path(graph: &GraphMemory, start: &str, target: &str, scope: &MemoryScope) -> Result<PathResult> {
    let (pet_graph, node_map) = build_petgraph(graph, scope)?;
    let start_idx = match node_map.get(start) {
        Some(&idx) => idx,
        None => return Err(MemoryError::EntityNotFound(format!("Start entity '{}' not found", start))),
    };
    let target_idx = match node_map.get(target) {
        Some(&idx) => idx,
        None => return Err(MemoryError::EntityNotFound(format!("Target entity '{}' not found", target))),
    };

    let path_indices = petgraph::algo::astar(
        &pet_graph,
        start_idx,
        |n| n == target_idx,
        |_| 1,
        |_| 0,
    );

    if let Some((_, indices)) = path_indices {
        let path: Vec<String> = indices.iter().map(|&idx| pet_graph.node_weight(idx).unwrap().clone()).collect();
        let mut relations = Vec::new();
        for i in 0..(indices.len() - 1) {
            let edge = pet_graph.find_edge(indices[i], indices[i+1]).unwrap();
            relations.push(pet_graph.edge_weight(edge).unwrap().clone());
        }
        Ok(PathResult { path, relations })
    } else {
        return Err(MemoryError::PathNotFound(format!("No path found between {} and {}", start, target)));
    }
}

pub fn relation_chain(graph: &GraphMemory, start: &str, chain: &[String], scope: &MemoryScope) -> Result<Vec<String>> {
    let (pet_graph, node_map) = build_petgraph(graph, scope)?;
    let start_idx = match node_map.get(start) {
        Some(&idx) => idx,
        None => return Ok(Vec::new()),
    };

    let mut current = vec![start_idx];
    for rel_type in chain {
        let mut next = Vec::new();
        for &node in &current {
            let mut neighbors = pet_graph.neighbors(node).detach();
            while let Some((edge, neighbor)) = neighbors.next(&pet_graph) {
                if pet_graph.edge_weight(edge).unwrap() == rel_type {
                    if !next.contains(&neighbor) {
                        next.push(neighbor);
                    }
                }
            }
        }
        current = next;
    }

    let res = current.iter().map(|&idx| pet_graph.node_weight(idx).unwrap().clone()).collect();
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::graph::{GraphMemory, Entity, Relation};
    use crate::layers::MemoryScope;
    use anyhow::Result;

    #[test]
    fn test_graph_traversal_flows() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_trav_{}.db", uuid::Uuid::new_v4()));
        let graph = GraphMemory::new(&db_path)?;
        let scope = MemoryScope::default();

        graph.create_entities(vec![
            Entity { name: "Alice".to_string(), entity_type: "Person".to_string(), observations: vec![] },
            Entity { name: "Bob".to_string(), entity_type: "Person".to_string(), observations: vec![] },
            Entity { name: "Charlie".to_string(), entity_type: "Person".to_string(), observations: vec![] },
        ], &scope)?;

        graph.create_relations(vec![
            Relation { from: "Alice".to_string(), to: "Bob".to_string(), relation_type: "knows".to_string() },
            Relation { from: "Bob".to_string(), to: "Charlie".to_string(), relation_type: "knows".to_string() },
        ], &scope)?;

        let bfs_res = bfs_traverse(&graph, "Alice", 2, &scope)?;
        assert_eq!(bfs_res.len(), 2);
        assert_eq!(bfs_res[0].entity_name, "Bob");
        assert_eq!(bfs_res[1].entity_name, "Charlie");

        let path_res = shortest_path(&graph, "Alice", "Charlie", &scope)?;
        assert_eq!(path_res.path, vec!["Alice", "Bob", "Charlie"]);

        let chain_res = relation_chain(&graph, "Alice", &["knows".to_string(), "knows".to_string()], &scope)?;
        assert_eq!(chain_res, vec!["Charlie"]);

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
