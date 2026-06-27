# Graph Intelligence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Graph Traversal, Community Detection, and Code Impact Analysis in `openmemory_rs` using `petgraph` in pure Rust without external APIs.

**Architecture:** Load active scope-specific graph/code relationship records into an in-memory `petgraph` structure, execute traversal/clustering algorithms, and return structural summaries. Expose the functionality via MCP tools and verify with unit/gRPC tests.

**Tech Stack:** Rust 2024, `petgraph 0.6`, `rusqlite`, `serde/serde_json`.

## Global Constraints
- **100% Offline-First**: No external API or LLM calls. All summarization is structural.
- **Pure Rust**: Code must compile with zero warnings using standard cargo tools.
- **Bi-temporal compatibility**: Filter queries to only use active edges/nodes where `valid_until IS NULL`.

---

### Task 1: Graph Traversal Engine

**Files:**
- Create: `src/layers/graph_traversal.rs`
- Modify: `src/layers/mod.rs` (register the module)
- Test: unit tests within `src/layers/graph_traversal.rs`

**Interfaces:**
- Consumes: `GraphMemory` connection and active edges
- Produces: 
  ```rust
  pub fn bfs_traverse(graph: &GraphMemory, start_entity: &str, max_depth: u32, scope: &crate::layers::MemoryScope) -> Result<Vec<TraversalStep>>;
  pub fn shortest_path(graph: &GraphMemory, start: &str, target: &str, scope: &crate::layers::MemoryScope) -> Result<PathResult>;
  pub fn relation_chain(graph: &GraphMemory, start: &str, chain: &[String], scope: &crate::layers::MemoryScope) -> Result<Vec<String>>;
  ```

- [ ] **Step 1: Write the failing test**
  Add unit tests at the bottom of `src/layers/graph_traversal.rs` using `#[test]`.
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::layers::graph::{GraphMemory, Entity, Relation};
      use crate::layers::MemoryScope;
      use std::sync::Arc;

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
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -- layers::graph_traversal::tests`
  Expected: FAIL (compilation error, functions undefined)

- [ ] **Step 3: Write minimal implementation**
  Create `src/layers/graph_traversal.rs` with the structural petgraph BFS and Dijkstra implementation.
  ```rust
  use anyhow::Result;
  use petgraph::graph::DiGraph;
  use petgraph::visit::Bfs;
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
      let mut bfs = Bfs::new(&pet_graph, start_idx);
      let mut depths = std::collections::HashMap::new();
      depths.insert(start_idx, 0);

      while let Some(node) = bfs.next(&pet_graph) {
          let depth = depths[&node];
          if depth >= max_depth {
              continue;
          }

          let mut neighbors = pet_graph.neighbors(node).detach();
          while let Some((edge, neighbor)) = neighbors.next(&pet_graph) {
              if !depths.contains_key(&neighbor) {
                  let next_depth = depth + 1;
                  depths.insert(neighbor, next_depth);
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
          None => anyhow::bail!("Start entity not found"),
      };
      let target_idx = match node_map.get(target) {
          Some(&idx) => idx,
          None => anyhow::bail!("Target entity not found"),
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
          anyhow::bail!("No path found between {} and {}", start, target)
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
                      next.push(neighbor);
                  }
              }
          }
          current = next;
      }

      let res = current.iter().map(|&idx| pet_graph.node_weight(idx).unwrap().clone()).collect();
      Ok(res)
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -- layers::graph_traversal::tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add src/layers/graph_traversal.rs src/layers/mod.rs
  git commit -m "feat: implement BFS, shortest path, and relation chain graph traversals"
  ```

---

### Task 2: Community Detection & Summarization

**Files:**
- Create: `src/search/community.rs`
- Modify: `src/search/mod.rs` (register the module)
- Test: unit tests within `src/search/community.rs`

**Interfaces:**
- Consumes: `GraphMemory` connection, active nodes, and edges
- Produces:
  ```rust
  pub fn detect_communities(graph: &GraphMemory, scope: &crate::layers::MemoryScope) -> Result<Vec<Community>>;
  ```

- [ ] **Step 1: Write the failing test**
  Add unit tests at the bottom of `src/search/community.rs`.
  ```rust
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
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -- search::community::tests`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Create `src/search/community.rs` with the structural connected components and summary logic.
  ```rust
  use anyhow::Result;
  use petgraph::graph::UnGraph;
  use petgraph::algo::tarjan_scc;
  use crate::layers::graph::GraphMemory;
  use crate::layers::MemoryScope;
  use rusqlite::params;

  #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct Community {
      pub id: u32,
      pub members: Vec<String>,
      pub summary: String,
  }

  pub fn detect_communities(graph: &GraphMemory, scope: &MemoryScope) -> Result<Vec<Community>> {
      let conn = graph.conn.lock();
      let user_id = scope.user_id.as_deref().unwrap_or("*");
      let session_id = scope.session_id.as_deref().unwrap_or("*");
      let agent_id = scope.agent_id.as_deref().unwrap_or("*");

      // 1. Fetch active nodes in scope
      let mut stmt_n = conn.prepare(
          "SELECT name, observations 
           FROM graph_nodes 
           WHERE (user_id = ?1 OR user_id = '*')
             AND (session_id = ?2 OR session_id = '*')
             AND (agent_id = ?3 OR agent_id = '*')"
      )?;

      let mut obs_map = std::collections::HashMap::new();
      let mut node_list = Vec::new();
      let rows_n = stmt_n.query_map(params![user_id, session_id, agent_id], |r| {
          Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
      })?;

      for r in rows_n {
          let (name, obs) = r?;
          node_list.push(name.clone());
          obs_map.insert(name, obs);
      }

      // 2. Fetch active edges in scope
      let mut stmt_e = conn.prepare(
          "SELECT from_name, to_name 
           FROM graph_edges 
           WHERE valid_until IS NULL
             AND (user_id = ?1 OR user_id = '*')
             AND (session_id = ?2 OR session_id = '*')
             AND (agent_id = ?3 OR agent_id = '*')"
      )?;

      let rows_e = stmt_e.query_map(params![user_id, session_id, agent_id], |r| {
          Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
      })?;

      let mut pet_graph = UnGraph::new_undirected();
      let mut node_map = std::collections::HashMap::new();

      for name in &node_list {
          let idx = pet_graph.add_node(name.clone());
          node_map.insert(name.clone(), idx);
      }

      for r in rows_e {
          let (from, to) = r?;
          if let (Some(&f), Some(&t)) = (node_map.get(&from), node_map.get(&to)) {
              pet_graph.add_edge(f, t, ());
          }
      }

      // Find components
      let components = tarjan_scc(&pet_graph);
      let mut communities = Vec::new();

      for (id, comp) in components.into_iter().enumerate() {
          let members: Vec<String> = comp.iter().map(|&idx| pet_graph.node_weight(idx).unwrap().clone()).collect();
          
          // Generate summary by joining observations of all members
          let mut observations = Vec::new();
          for m in &members {
              if let Some(obs_str) = obs_map.get(m) {
                  if let Ok(vec) = serde_json::from_str::<Vec<String>>(obs_str) {
                      observations.extend(vec);
                  }
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
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -- search::community::tests`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add src/search/community.rs src/search/mod.rs
  git commit -m "feat: implement community detection using connected components and observations summaries"
  ```

---

### Task 3: Recursive Code Impact Analysis

**Files:**
- Modify: `src/layers/codebase.rs`
- Test: unit tests within `src/layers/codebase.rs`

**Interfaces:**
- Consumes: `code_elements` and `code_calls` tables
- Produces:
  ```rust
  pub fn impact_analysis(&self, target_symbol: &str, scope: &crate::layers::MemoryScope) -> Result<ImpactReport>;
  ```

- [ ] **Step 1: Write the failing test**
  Add test case `test_code_impact_analysis` to the `tests` module in `src/layers/codebase.rs`.
  ```rust
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
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -- layers::codebase::tests::test_code_impact_analysis`
  Expected: FAIL (compilation error, `impact_analysis` not defined)

- [ ] **Step 3: Write minimal implementation**
  Add `ImpactReport` and implement `impact_analysis` method inside the `impl CodebaseMemory` block in `src/layers/codebase.rs`.
  ```rust
  #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct ImpactReport {
      pub affected_symbols: Vec<String>,
      pub max_depth: u32,
      pub risk_score: f64,
      pub details: String,
  }

  impl CodebaseMemory {
      // ... existing code ...
      
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
              None => anyhow::bail!("Symbol '{}' not found in indexed codebase", target_symbol),
          };

          // BFS on reversed callgraph
          let mut bfs = petgraph::visit::Bfs::new(&pet_graph, start_idx);
          let mut depths = std::collections::HashMap::new();
          depths.insert(start_idx, 0);

          let mut affected = Vec::new();
          let mut max_depth = 0;

          while let Some(node) = bfs.next(&pet_graph) {
              let d = depths[&node];
              if d > 0 {
                  affected.push(pet_graph.node_weight(node).unwrap().clone());
                  if d > max_depth {
                      max_depth = d;
                  }
              }

              let mut neighbors = pet_graph.neighbors(node).detach();
              while let Some((_, neighbor)) = neighbors.next(&pet_graph) {
                  if !depths.contains_key(&neighbor) {
                      depths.insert(neighbor, d + 1);
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
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -- layers::codebase::tests::test_code_impact_analysis`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add src/layers/codebase.rs
  git commit -m "feat: implement call-graph based recursive codebase impact analysis"
  ```

---

### Task 4: Expose MCP Tools

**Files:**
- Modify: `src/mcp.rs`
- Test: unit tests within `src/mcp.rs`

**Interfaces:**
- Consumes: Graph Traversal, Community Detection, and Code Impact APIs
- Produces: MCP tools `traverse_graph`, `find_path`, `analyze_graph_communities`, and `analyze_code_impact`

- [ ] **Step 1: Write the failing test**
  Add a integration test `test_mcp_graph_intelligence_tools` to the `tests` module in `src/mcp.rs`.
  ```rust
  #[tokio::test]
  async fn test_mcp_graph_intelligence_tools() -> Result<()> {
      let db_path = std::env::temp_dir().join(format!("test_mcp_gi_{}.db", uuid::Uuid::new_v4()));
      let coordinator = Arc::new(MemoryCoordinator::new(db_path.to_str().unwrap(), 300)?);
      let server = MemoryServer::new(coordinator.clone());
      
      // Setup simple graph
      let scope = MemoryScope::default();
      coordinator.graph.create_entities(vec![
          Entity { name: "X".to_string(), entity_type: "Label".to_string(), observations: vec![] }
      ], &scope)?;

      // 1. Test traverse_graph
      let input_trav = TraverseGraphInput {
          start_entity: "X".to_string(),
          max_depth: Some(1),
          user_id: None,
          session_id: None,
          agent_id: None,
      };
      let res_trav = server.traverse_graph(Parameters(input_trav)).await?;
      assert!(!res_trav.content.is_empty());

      let _ = std::fs::remove_file(db_path);
      Ok(())
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -- mcp::tests::test_mcp_graph_intelligence_tools`
  Expected: FAIL

- [ ] **Step 3: Write minimal implementation**
  Add input structs and expose annotated `#[tool]` endpoints in `src/mcp.rs`.
  ```rust
  #[derive(serde::Deserialize, schemars::JsonSchema, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct TraverseGraphInput {
      pub start_entity: String,
      pub max_depth: Option<u32>,
      pub user_id: Option<String>,
      pub session_id: Option<String>,
      pub agent_id: Option<String>,
  }

  #[derive(serde::Deserialize, schemars::JsonSchema, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct FindPathInput {
      pub start_entity: String,
      pub target_entity: String,
      pub user_id: Option<String>,
      pub session_id: Option<String>,
      pub agent_id: Option<String>,
  }

  #[derive(serde::Deserialize, schemars::JsonSchema, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct AnalyzeGraphCommunitiesInput {
      pub user_id: Option<String>,
      pub session_id: Option<String>,
      pub agent_id: Option<String>,
  }

  #[derive(serde::Deserialize, schemars::JsonSchema, Clone)]
  #[serde(rename_all = "camelCase")]
  pub struct AnalyzeCodeImpactInput {
      pub target_symbol: String,
      pub user_id: Option<String>,
      pub session_id: Option<String>,
      pub agent_id: Option<String>,
  }

  impl MemoryServer {
      // Add inside impl block:

      #[tool(description = "Traverse nodes and edges from a start entity using BFS up to a maximum depth")]
      async fn traverse_graph(&self, Parameters(input): Parameters<TraverseGraphInput>) -> Result<CallToolResult, McpError> {
          let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
          let depth = input.max_depth.unwrap_or(2);
          match crate::layers::graph_traversal::bfs_traverse(&self.coordinator.graph, &input.start_entity, depth, &scope) {
              Ok(res) => Ok(CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&res).unwrap_or_default())])),
              Err(e) => Err(McpError::internal_error(e.to_string(), None)),
          }
      }

      #[tool(description = "Find the shortest path and relations between two entity nodes")]
      async fn find_path(&self, Parameters(input): Parameters<FindPathInput>) -> Result<CallToolResult, McpError> {
          let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
          match crate::layers::graph_traversal::shortest_path(&self.coordinator.graph, &input.start_entity, &input.target_entity, &scope) {
              Ok(res) => Ok(CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&res).unwrap_or_default())])),
              Err(e) => Err(McpError::internal_error(e.to_string(), None)),
          }
      }

      #[tool(description = "Cluster the entity-relation graph into weakly connected communities with summaries")]
      async fn analyze_graph_communities(&self, Parameters(input): Parameters<AnalyzeGraphCommunitiesInput>) -> Result<CallToolResult, McpError> {
          let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
          match crate::search::community::detect_communities(&self.coordinator.graph, &scope) {
              Ok(res) => Ok(CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&res).unwrap_or_default())])),
              Err(e) => Err(McpError::internal_error(e.to_string(), None)),
          }
      }

      #[tool(description = "Calculate downstream callers and change risk for a code symbol")]
      async fn analyze_code_impact(&self, Parameters(input): Parameters<AnalyzeCodeImpactInput>) -> Result<CallToolResult, McpError> {
          let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
          match self.coordinator.codebase.impact_analysis(&input.target_symbol, &scope) {
              Ok(res) => Ok(CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&res).unwrap_or_default())])),
              Err(e) => Err(McpError::internal_error(e.to_string(), None)),
          }
      }
  }
  ```

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -- mcp::tests::test_mcp_graph_intelligence_tools`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add src/mcp.rs
  git commit -m "feat: expose graph traversal, communities, and codebase impact MCP tools"
  ```

---

### Task 5: gRPC Integration Testing

**Files:**
- Modify: `tests/test_grpc.rs`

**Interfaces:**
- Consumes: gRPC MCP client
- Produces: Verified gRPC coverage for new Graph Intelligence tools

- [ ] **Step 1: Write the gRPC test integration**
  Open `tests/test_grpc.rs` and add Step 11 to execute a traversal call over the gRPC Tonic bridge:
  ```rust
  // 11. Test traverse_graph via gRPC
  println!("Calling traverse_graph via gRPC...");
  let trav_params = serde_json::json!({
      "name": "traverse_graph",
      "arguments": {
          "startEntity": "A",
          "maxDepth": 1
      }
  });
  let trav_req = McpRequest {
      method: "tools/call".to_string(),
      params_json: trav_params.to_string(),
      id: 10,
      has_id: true,
  };
  let trav_resp = client.call(trav_req).await?.into_inner();
  assert!(trav_resp.error_json.is_empty(), "Should not return error");
  println!("Traverse Graph Response: {}", trav_resp.result_json);
  ```

- [ ] **Step 2: Run all tests to make sure everything passes**
  Run: `cargo test`
  Expected: PASS for all unit and integration tests

- [ ] **Step 3: Commit**
  ```bash
  git add tests/test_grpc.rs
  git commit -m "test: verify traverse_graph over gRPC Tonic interface"
  ```
