# Importance & Compaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement automated importance scoring and database memory compaction (garbage collection & clustering) for the semantic memory layer of `openmemory_rs`.

**Architecture:** Create two new modules: `src/search/importance.rs` for calculations and `src/consolidation/compactor.rs` for decay/clustering logic. Expose them via a `compact_memories` MCP tool and add gRPC integration coverage.

**Tech Stack:** Rust (Edition 2024), SQLite (rusqlite), all-MiniLM-L6-v2 embeddings, small-world-rs (HNSW).

## Global Constraints
- The project must remain 100% pure Rust (Edition 2024).
- No external LLM APIs are allowed.
- Follow existing patterns for SQLite database schema and model loading.

---

### Task 1: Auto-Importance Scoring Implementation

**Files:**
- Create: `src/search/importance.rs`
- Modify: `src/search/mod.rs`

**Interfaces:**
- Consumes: none
- Produces: `ImportanceScorer::calculate_importance(access_count: u32, edge_count: u32, age_hours: f64, was_reinforced: bool) -> f64`

- [ ] **Step 1: Create importance.rs and write unit tests**
  Create the file `src/search/importance.rs` containing tests for scoring calculations:
  ```rust
  pub struct ImportanceScorer;

  impl ImportanceScorer {
      pub fn calculate_importance(
          access_count: u32,
          edge_count: u32,
          age_hours: f64,
          was_reinforced: bool,
      ) -> f64 {
          let access_score = (access_count as f64 + 1.0).ln() / 10.0;
          let connection_score = (edge_count as f64 + 1.0).ln() / 5.0;
          let freshness = (-0.005 * age_hours).exp();
          let reinforcement = if was_reinforced { 0.3 } else { 0.0 };
          (access_score + connection_score + freshness + reinforcement).min(1.0)
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_importance_calculation() {
          // Fresh memory with no accesses or edges
          let score_fresh = ImportanceScorer::calculate_importance(0, 0, 0.0, false);
          assert!((score_fresh - 0.333).abs() < 0.05, "Fresh score should be ~0.33");

          // Highly accessed and connected memory
          let score_high = ImportanceScorer::calculate_importance(100, 10, 24.0, true);
          assert!(score_high > 0.8, "High parameters should produce a high score");

          // Decayed memory
          let score_decayed = ImportanceScorer::calculate_importance(0, 0, 1000.0, false);
          assert!(score_decayed < 0.1, "Old and unused memory should decay close to 0");
      }
  }
  ```

- [ ] **Step 2: Register in search/mod.rs**
  Add the module to `src/search/mod.rs`:
  ```rust
  pub mod conflict;
  pub mod hybrid;
  pub mod ranker;
  pub mod dedup;
  pub mod importance;
  ```

- [ ] **Step 3: Run the tests to verify compilation and success**
  Run: `cargo test -- search::importance::tests`
  Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/search/importance.rs src/search/mod.rs
  git commit -m "feat: implement ImportanceScorer and register module"
  ```

---

### Task 2: Memory Compactor Logic

**Files:**
- Create: `src/consolidation/compactor.rs`
- Modify: `src/consolidation/mod.rs`

**Interfaces:**
- Consumes: `ImportanceScorer::calculate_importance` from Task 1
- Produces: `MemoryCompactor::run_compaction(coordinator: &MemoryCoordinator, strategy: &str, dry_run: bool, min_importance: f64, max_age_hours: f64, cluster_threshold: f64, scope: &MemoryScope) -> Result<CompactionReport>`

- [ ] **Step 1: Create compactor.rs and write unit test**
  Create `src/consolidation/compactor.rs` with the `MemoryCompactor` implementation:
  ```rust
  use crate::coordinator::MemoryCoordinator;
  use crate::layers::MemoryScope;
  use crate::search::importance::ImportanceScorer;
  use crate::search::dedup::SemanticDedup;
  use crate::layers::semantic::calculate_cosine_similarity;
  use anyhow::Result;
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

          let mut facts = Vec::new();
          for r in rows {
              facts.push(r?);
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
                          "SELECT name FROM graph_nodes WHERE valid_until IS NULL"
                      ).unwrap();
                      let nodes = stmt_nodes.query_map([], |r| r.get::<_, String>(0)).unwrap();
                      for node_name in nodes {
                          if let Ok(name) = node_name {
                              if fact.raw_text.to_lowercase().contains(&name.to_lowercase()) {
                                  let count: u32 = conn_g.query_row(
                                      "SELECT COUNT(*) FROM graph_edges WHERE (from_name = ?1 OR to_name = ?1) AND valid_until IS NULL",
                                      params![name],
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
                          return Self::run_compaction(coordinator, strategy, dry_run, min_importance, max_age_hours, cluster_threshold, scope);
                      }
                  }
                  visited.insert(active_facts[i].node_id.clone());
              }
          }

          // Rebuild HNSW if not dry_run and changes occurred
          if !dry_run && (removed_count > 0 || merged_count > 0) {
              // Rebuild index
              let _ = coordinator.semantic.query_similar_facts_vector("rebuild HNSW index trigger", 1, scope);
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
              0.80,
              &scope,
          )?;

          assert!(report.removed_count >= 1, "Should decay and remove fact-1");
          assert!(report.merged_count >= 1, "Should merge similar facts fact-3 and fact-4");

          let _ = std::fs::remove_file(db_path);
          Ok(())
      }
  }
  ```

- [ ] **Step 2: Register in consolidation/mod.rs**
  Add the module:
  ```rust
  pub mod engine;
  pub mod compactor;
  ```

- [ ] **Step 3: Run the compactor test to verify compilation and success**
  Run: `cargo test -- consolidation::compactor::tests`
  Expected: PASS

- [ ] **Step 4: Commit**
  ```bash
  git add src/consolidation/compactor.rs src/consolidation/mod.rs
  git commit -m "feat: implement MemoryCompactor and decay/cluster strategies"
  ```

---

### Task 3: Expose compact_memories MCP Tool

**Files:**
- Modify: `src/mcp.rs`

**Interfaces:**
- Consumes: `MemoryCompactor::run_compaction` from Task 2
- Produces: `compact_memories` tool registered on `MemoryServer`

- [ ] **Step 1: Add input structs and annotate the tool**
  Add `CompactMemoriesInput` under other Input structs in `src/mcp.rs`:
  ```rust
  #[derive(serde::Deserialize, schemars::JsonSchema)]
  #[serde(rename_all = "camelCase")]
  pub struct CompactMemoriesInput {
      pub strategy: String, // "decay", "cluster", "both"
      pub dry_run: Option<bool>,
      pub min_importance: Option<f64>,
      pub max_age_hours: Option<f64>,
      pub cluster_threshold: Option<f64>,
      pub user_id: Option<String>,
      pub session_id: Option<String>,
      pub agent_id: Option<String>,
  }
  ```

- [ ] **Step 2: Implement compact_memories handler inside `impl MemoryServer`**
  ```rust
      #[tool(name = "compact_memories", description = "Compacts semantic memories in scope by removing decayed memories and merging highly similar ones.")]
      pub async fn compact_memories(&self, input: CompactMemoriesInput) -> Result<CallToolResult> {
          let scope = MemoryScope {
              user_id: input.user_id.clone(),
              session_id: input.session_id.clone(),
              agent_id: input.agent_id.clone(),
          };

          let dry_run = input.dry_run.unwrap_or(false);
          let min_importance = input.min_importance.unwrap_or(0.15);
          let max_age_hours = input.max_age_hours.unwrap_or(24.0);
          let cluster_threshold = input.cluster_threshold.unwrap_or(0.90);

          let report = crate::consolidation::compactor::MemoryCompactor::run_compaction(
              &self.coordinator,
              &input.strategy,
              dry_run,
              min_importance,
              max_age_hours,
              cluster_threshold,
              &scope,
          )?;

          let accessed_by = input.agent_id.as_deref().unwrap_or("agent");
          let _ = self.coordinator.episodic.log_access("compactor", "semantic", accessed_by);

          Ok(CallToolResult::new_text(serde_json::to_string_pretty(&report)?))
      }
  ```

- [ ] **Step 3: Add integration unit test `test_mcp_compaction_tool`**
  Add the unit test under the `tests` module in `src/mcp.rs`:
  ```rust
      #[tokio::test]
      async fn test_mcp_compaction_tool() -> Result<()> {
          let db_path = std::env::temp_dir().join(format!("test_mcp_compaction_{}.db", uuid::Uuid::new_v4()));
          let coordinator = Arc::new(MemoryCoordinator::new(db_path.to_str().unwrap(), 300)?);
          let server = MemoryServer::new(coordinator.clone());
          let scope = MemoryScope::default();

          // Add similar facts
          coordinator.semantic.add_fact("fact-1", "I love clean Rust coding", 0.8, &scope)?;
          coordinator.semantic.add_fact("fact-2", "I love clean Rust programming", 0.8, &scope)?;

          let input = CompactMemoriesInput {
              strategy: "cluster".to_string(),
              dry_run: Some(false),
              min_importance: None,
              max_age_hours: None,
              cluster_threshold: Some(0.80),
              user_id: None,
              session_id: None,
              agent_id: None,
          };

          let res = server.compact_memories(Parameters(input)).await?;
          let val = serde_json::to_value(&res)?;
          let text = val["content"][0]["text"].as_str().unwrap();
          assert!(text.contains("\"mergedCount\": 1"));

          let _ = std::fs::remove_file(db_path);
          Ok(())
      }
  ```

- [ ] **Step 4: Run the test to verify it passes**
  Run: `cargo test -- mcp::tests::test_mcp_compaction_tool -- --nocapture`
  Expected: PASS

- [ ] **Step 5: Commit**
  ```bash
  git add src/mcp.rs
  git commit -m "feat: expose compact_memories MCP tool and verify with integration test"
  ```

---

### Task 4: gRPC Integration Test

**Files:**
- Modify: `tests/test_grpc.rs`

**Interfaces:**
- Consumes: gRPC bridge of `compact_memories`
- Produces: Added verification case in `test_grpc_mcp_flow`

- [ ] **Step 1: Add a test step calling `compact_memories` via gRPC**
  Locate `test_grpc_mcp_flow` inside `tests/test_grpc.rs` and add a step:
  ```rust
      // Step: compact_memories
      println!("Sending compact_memories call...");
      let compact_params = serde_json::json!({
          "name": "compact_memories",
          "arguments": {
              "strategy": "cluster",
              "dryRun": true
          }
      });
      let compact_req = McpRequest {
          method: "tools/call".to_string(),
          params_json: compact_params.to_string(),
          id: 5,
          has_id: true,
      };
      let compact_resp = client.call(compact_req).await?.into_inner();
      assert!(
          !compact_resp.result_json.is_empty(),
          "compact_memories response should not be empty"
      );
      println!("compact_memories Response: {}", compact_resp.result_json);
  ```

- [ ] **Step 2: Run all tests to make sure everything builds and passes**
  Run: `cargo test`
  Expected: PASS for all unit and integration tests (including gRPC flow).

- [ ] **Step 3: Commit**
  ```bash
  git add tests/test_grpc.rs
  git commit -m "test: verify compact_memories tool over gRPC bridge"
  ```
