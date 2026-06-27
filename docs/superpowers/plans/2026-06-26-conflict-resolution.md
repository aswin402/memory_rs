# Conflict Resolution Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the algorithmic conflict resolution engine for Graph and Semantic layers, and expose it via the `detect_and_resolve_conflicts` MCP tool.

**Architecture:** Create a new stateless module `src/search/conflict.rs` to query active graph edges and semantic facts, detect contradictions, and apply recency or confidence/importance resolution updates to the database. Expose it via an MCP tool and verify with gRPC integration tests.

**Tech Stack:** Rust (edition 2024), SQLite (rusqlite), serde/schemars, chrono.

## Global Constraints
- Run all cargo build/check/test commands with `-j 1`.
- Keep tool properties strictly camelCase via `#[serde(rename_all = "camelCase")]`.
- Preserve existing codebase structure and functionality completely.

---

### Task 1: Scaffolding and Data Structures

**Files:**
- Create: `src/search/conflict.rs`
- Modify: `src/search/mod.rs` (if it exists, or register in `src/main.rs`)

**Interfaces:**
- Consumes: None
- Produces: `ConflictResolver` structures and initial registration.

- [ ] **Step 1: Create conflict.rs with structures**
  Create the file `src/search/conflict.rs` with the following contents:
  ```rust
  use serde::{Serialize, Deserialize};
  use schemars::JsonSchema;
  use crate::layers::{GraphMemory, SemanticMemory, MemoryScope};
  use anyhow::Result;

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
          _graph: &GraphMemory,
          _semantic: &SemanticMemory,
          _exclusive_relations: &[String],
          _semantic_threshold: f64,
          _strategy: &str,
          _dry_run: bool,
          _scope: &MemoryScope,
      ) -> Result<ConflictResolutionReport> {
          Ok(ConflictResolutionReport {
              conflicts_found: 0,
              resolutions_applied: vec![],
              dry_run: true,
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
  }
  ```

- [ ] **Step 2: Register search submodules**
  Check if `src/search/mod.rs` exists. If not, create it. Register the `conflict` module:
  ```rust
  pub mod conflict;
  pub mod ranker;
  ```
  Ensure `pub mod search;` is registered in `src/main.rs`. Let's check `src/main.rs` to verify search is already registered. If not, add `pub mod search;` to `src/main.rs`.

- [ ] **Step 3: Run cargo test to verify**
  Run: `cargo test -j 1 search::conflict::tests`
  Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/search/conflict.rs src/search/mod.rs
  git commit -m "feat: scaffold ConflictResolver data structures and initial registration"
  ```

---

### Task 2: Graph Layer Conflict Detection & Resolution

**Files:**
- Modify: `src/search/conflict.rs`

**Interfaces:**
- Consumes: `GraphMemory` SQL records.
- Produces: `ConflictResolver::detect_graph_conflicts` and `ConflictResolver::resolve_graph_conflicts`.

- [ ] **Step 1: Write failing unit test for Graph conflict detection**
  Add the following test inside `mod tests` in `src/search/conflict.rs`:
  ```rust
  #[tokio::test]
  async fn test_graph_conflict_detection_and_resolution() -> Result<()> {
      let db_path = std::env::temp_dir().join(format!("test_graph_conflict_{}.db", uuid::Uuid::new_v4()));
      let graph = GraphMemory::new(db_path.to_str().unwrap())?;
      let semantic = SemanticMemory::new(db_path.to_str().unwrap())?;
      let scope = MemoryScope::default();

      // Seed A lives_in NY (valid_from = earlier)
      let entity_a = crate::layers::Entity {
          name: "A".to_string(),
          entity_type: "Person".to_string(),
          observations: vec![],
      };
      let entity_ny = crate::layers::Entity {
          name: "NY".to_string(),
          entity_type: "City".to_string(),
          observations: vec![],
      };
      let entity_la = crate::layers::Entity {
          name: "LA".to_string(),
          entity_type: "City".to_string(),
          observations: vec![],
      };
      graph.create_entities(vec![entity_a, entity_ny, entity_la], &scope)?;

      // Add NY edge
      graph.create_relations(vec![crate::layers::Relation {
          from: "A".to_string(),
          to: "NY".to_string(),
          relation_type: "lives_in".to_string(),
      }], &scope)?;

      std::thread::sleep(std::time::Duration::from_secs(1));

      // Add LA edge
      graph.create_relations(vec![crate::layers::Relation {
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
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -j 1 search::conflict::tests::test_graph_conflict_detection_and_resolution`
  Expected: FAIL (assertion `report_dry.conflicts_found == 1` fails since it returns 0)

- [ ] **Step 3: Implement Graph conflict detection & resolution in conflict.rs**
  Replace the `impl ConflictResolver` block with:
  ```rust
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
  ```
  Ensure `use rusqlite::params;` is added to imports in `src/search/conflict.rs`.

- [ ] **Step 4: Run test to verify it passes**
  Run: `cargo test -j 1 search::conflict::tests::test_graph_conflict_detection_and_resolution`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add src/search/conflict.rs
  git commit -m "feat: implement Graph Layer conflict detection and resolution algorithms"
  ```

---

### Task 3: Semantic Layer Conflict Detection & Resolution

**Files:**
- Modify: `src/search/conflict.rs`

**Interfaces:**
- Consumes: `SemanticMemory` database facts.
- Produces: `ConflictResolver` semantic conflict logic.

- [ ] **Step 1: Write failing unit test for Semantic conflict detection**
  Add the following test inside `mod tests` in `src/search/conflict.rs`:
  ```rust
  #[tokio::test]
  async fn test_semantic_conflict_detection_and_resolution() -> Result<()> {
      let db_path = std::env::temp_dir().join(format!("test_semantic_conflict_{}.db", uuid::Uuid::new_v4()));
      let graph = GraphMemory::new(db_path.to_str().unwrap())?;
      let semantic = SemanticMemory::new(db_path.to_str().unwrap())?;
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

      // Resolve live
      let report_live = ConflictResolver::run(&graph, &semantic, &[], 0.85, "recency", false, &scope)?;
      assert_eq!(report_live.conflicts_found, 1);
      assert!(report_live.resolutions_applied[0].resolved);

      // Verify fact-1 is invalidated and only fact-2 is active
      let active = semantic.query_as_of(&chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(), &scope)?;
      assert_eq!(active.len(), 1);
      assert_eq!(active[0].node_id, "fact-2");

      let _ = std::fs::remove_file(db_path);
      Ok(())
  }
  ```

- [ ] **Step 2: Run test to verify it fails**
  Run: `cargo test -j 1 search::conflict::tests::test_semantic_conflict_detection_and_resolution`
  Expected: FAIL

- [ ] **Step 3: Implement Semantic conflict detection and resolution in conflict.rs**
  Modify `ConflictResolver::run` to include semantic logic:
  ```rust
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
      }

      let mut stmt = conn_g.prepare(
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
                                      if r1.valid_from > r2.valid_from { (r1, r2) } else { (r2, r1) }
                                  }
                              }
                              _ => {
                                  if r1.valid_from > r2.valid_from { (r1, r2) } else { (r2, r1) }
                              }
                          };

                          let action_taken = format!(
                              "Resolved graph conflict for {}({}): {} (valid: {}) wins over {} (valid: {})",
                              from, rel, winner.to_name, winner.valid_from, loser.to_name, loser.valid_from
                          );

                          if !dry_run {
                              conn_g.execute(
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
      }

      let mut stmt_s = conn_s.prepare(
          "SELECT node_id, raw_text, embedding, valid_from, importance 
           FROM semantic_metadata 
           WHERE valid_until IS NULL 
             AND (user_id = ?1 OR user_id = '*')
             AND (session_id = ?2 OR session_id = '*')
             AND (agent_id = ?3 OR agent_id = '*')"
      )?;
      let mut rows_s = stmt_s.query(params![user_id, session_id, agent_id])?;
      let mut facts = Vec::new();
      while let Some(row) = rows_s.next()? {
          let node_id: String = row.get(0)?;
          let raw_text: String = row.get(1)?;
          let blob: Vec<u8> = row.get(2)?;
          let valid_from: String = row.get(3)?;
          let importance: f64 = row.get(4)?;

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
          });
      }

      fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f64 {
          let dot_product: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
          let norm_v1: f32 = v1.iter().map(|a| a * a).sum::<f32>().sqrt();
          let norm_v2: f32 = v2.iter().map(|a| a * a).sum::<f32>().sqrt();
          if norm_v1 == 0.0 || norm_v2 == 0.0 {
              0.0
          } else {
              (dot_product / (norm_v1 * norm_v2)) as f64
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

              let sim = cosine_similarity(&f1.embedding, &f2.embedding);
              if sim >= semantic_threshold {
                  conflicts_found += 1;
                  let conflict_id = uuid::Uuid::new_v4().to_string();

                  let (winner, loser) = match strategy {
                      "confidence" => {
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

                  let action_taken = format!(
                      "Resolved semantic conflict (sim={:.3}): \"{}\" (valid: {}) wins over \"{}\" (valid: {})",
                      sim, winner.raw_text, winner.valid_from, loser.raw_text, loser.valid_from
                  );

                  if !dry_run {
                      conn_s.execute(
                          "UPDATE semantic_metadata 
                           SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), 
                               superseded_by = ?1 
                           WHERE node_id = ?2 AND valid_from = ?3",
                          params![winner.node_id, loser.node_id, loser.valid_from],
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
          resolutions_applied,
          dry_run,
      })
  }
  ```
  Ensure `use crate::layers::semantic::rebuild_hnsw_index;` is imported in `src/search/conflict.rs`. Since `rebuild_hnsw_index` is module-private in `src/layers/semantic.rs`, we need to change it to `pub(crate)` or `pub` in `src/layers/semantic.rs`.

- [ ] **Step 4: Make rebuild_hnsw_index pub(crate) in semantic.rs**
  In `src/layers/semantic.rs`, change `fn rebuild_hnsw_index` (around line 496) to `pub(crate) fn rebuild_hnsw_index`.

- [ ] **Step 5: Run cargo test to verify**
  Run: `cargo test -j 1 search::conflict::tests::test_semantic_conflict_detection_and_resolution`
  Expected: PASS

- [ ] **Step 6: Commit**
  ```bash
  git add src/search/conflict.rs src/layers/semantic.rs
  git commit -m "feat: implement Semantic Layer conflict detection and resolution"
  ```

---

### Task 4: Expose MCP Tool

**Files:**
- Modify: `src/mcp.rs`

**Interfaces:**
- Consumes: `ConflictResolver`
- Produces: `detect_and_resolve_conflicts` MCP tool.

- [ ] **Step 1: Define Input Struct and Endpoint in mcp.rs**
  Add the input deserializer and tool implementation to `src/mcp.rs`:
  ```rust
  #[derive(serde::Deserialize, schemars::JsonSchema)]
  #[serde(rename_all = "camelCase")]
  pub struct DetectAndResolveConflictsInput {
      pub strategy: Option<String>,
      pub dry_run: Option<bool>,
      pub semantic_threshold: Option<f64>,
      pub user_id: Option<String>,
      pub session_id: Option<String>,
      pub agent_id: Option<String>,
  }
  ```
  Add the async method:
  ```rust
      #[tool(
          description = "Detect and resolve contradictions or conflicts in graph relations and semantic memories"
      )]
      async fn detect_and_resolve_conflicts(
          &self,
          Parameters(input): Parameters<DetectAndResolveConflictsInput>,
      ) -> Result<CallToolResult, McpError> {
          let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
          let dry_run = input.dry_run.unwrap_or(true);
          let strategy = input.strategy.unwrap_or_else(|| "recency".to_string());
          let semantic_threshold = input.semantic_threshold.unwrap_or(0.85);

          let exclusive_relations = vec![
              "lives_in".to_string(),
              "current_job".to_string(),
              "spouse".to_string(),
              "has_status".to_string(),
              "is_born_in".to_string(),
              "located_in".to_string(),
          ];

          match crate::search::conflict::ConflictResolver::run(
              &self.coordinator.graph,
              &self.coordinator.semantic,
              &exclusive_relations,
              semantic_threshold,
              &strategy,
              dry_run,
              &scope,
          ) {
              Ok(report) => {
                  let text = serde_json::to_string_pretty(&report).unwrap_or_default();
                  Ok(CallToolResult::success(vec![Content::text(text)]))
              }
              Err(e) => Err(McpError::internal_error(e.to_string(), None)),
          }
      }
  ```

- [ ] **Step 2: Add integration unit test to mcp.rs**
  Add the following test case inside the `tests` module in `src/mcp.rs`:
  ```rust
      #[tokio::test]
      async fn test_mcp_conflict_tool() -> Result<()> {
          let db_path = std::env::temp_dir().join(format!("test_mcp_conflict_{}.db", uuid::Uuid::new_v4()));
          let coordinator = Arc::new(MemoryCoordinator::new(db_path.to_str().unwrap())?);
          let server = MemoryServer::new(coordinator.clone());
          let scope = MemoryScope::default();

          // Seed conflicting status
          coordinator.graph.create_entities(vec![
              Entity {
                  name: "Alice".to_string(),
                  entity_type: "Person".to_string(),
                  observations: vec![],
              },
              Entity {
                  name: "Single".to_string(),
                  entity_type: "Status".to_string(),
                  observations: vec![],
              },
              Entity {
                  name: "Married".to_string(),
                  entity_type: "Status".to_string(),
                  observations: vec![],
              },
          ], &scope)?;

          coordinator.graph.create_relations(vec![Relation {
              from: "Alice".to_string(),
              to: "Single".to_string(),
              relation_type: "has_status".to_string(),
          }], &scope)?;

          std::thread::sleep(std::time::Duration::from_secs(1));

          coordinator.graph.create_relations(vec![Relation {
              from: "Alice".to_string(),
              to: "Married".to_string(),
              relation_type: "has_status".to_string(),
          }], &scope)?;

          // Call conflict tool
          let input = DetectAndResolveConflictsInput {
              strategy: Some("recency".to_string()),
              dry_run: Some(false),
              semantic_threshold: None,
              user_id: None,
              session_id: None,
              agent_id: None,
          };
          let res = server.detect_and_resolve_conflicts(Parameters(input)).await?;
          let val = serde_json::to_value(&res)?;
          let content = val["content"][0]["text"].as_str().unwrap();
          assert!(content.contains("\"conflictsFound\": 1"));
          assert!(content.contains("\"resolved\": true"));

          let _ = std::fs::remove_file(db_path);
          Ok(())
      }
  ```

- [ ] **Step 3: Run cargo test to verify**
  Run: `cargo test -j 1 mcp::tests::test_mcp_conflict_tool`
  Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/mcp.rs
  git commit -m "feat: expose detect_and_resolve_conflicts MCP tool"
  ```

---

### Task 5: gRPC Integration Testing

**Files:**
- Modify: `tests/test_grpc.rs`

**Interfaces:**
- Consumes: `detect_and_resolve_conflicts` MCP tool.
- Produces: Test verification.

- [ ] **Step 1: Add gRPC test step**
  Add the following gRPC tool call assertion to `tests/test_grpc.rs` after Step 8:
  ```rust
      // 9. Test detect_and_resolve_conflicts via gRPC
      println!("Calling detect_and_resolve_conflicts via gRPC...");
      let conflict_params = serde_json::json!({
          "name": "detect_and_resolve_conflicts",
          "arguments": {
              "strategy": "recency",
              "dryRun": true
          }
      });
      let conflict_req = McpRequest {
          method: "tools/call".to_string(),
          params_json: conflict_params.to_string(),
          id: 8,
          has_id: true,
      };
      let conflict_resp = client.call(conflict_req).await?.into_inner();
      assert!(conflict_resp.error_json.is_empty(), "Should not return error");
      println!("Conflict Tool Response: {}", conflict_resp.result_json);
  ```

- [ ] **Step 2: Run final integration test suite**
  Run: `cargo test -j 1`
  Expected: All tests pass.

- [ ] **Step 3: Commit**
  ```bash
  git add tests/test_grpc.rs
  git commit -m "test: add gRPC integration test coverage for detect_and_resolve_conflicts"
  ```
