# Design Spec: Memory Compaction & Auto-Importance Scoring

**Date:** 2026-06-27  
**Status:** Approved  
**Topic:** Memory Consolidation Engine (Phase 4 Continuation)  

## 1. Overview
As an AI agent interacts with users over weeks and months, the semantic memory layer accumulates an unbounded number of facts. Without automated cleanup, this leads to:
1. Slower vector similarity search.
2. Inefficient context injection (due to redundant or low-value facts).
3. Increased database size.

To resolve this, we introduce **Auto-Importance Scoring** and **Memory Compaction** (Garbage Collection and Cluster Consolidation).

## 2. Component Design

### 2.1 Auto-Importance Scoring (`src/search/importance.rs`)
The importance scorer calculates a value between `0.0` and `1.0` representing a memory's overall utility.

#### Formula:
$$\text{Score} = \min\left(1.0, \text{access\_score} + \text{connection\_score} + \text{freshness} + \text{reinforcement}\right)$$

Where:
- $\text{access\_score} = \frac{\ln(\text{access\_count} + 1)}{10.0}$
- $\text{connection\_score} = \frac{\ln(\text{edge\_count} + 1)}{5.0}$
- $\text{freshness} = e^{-0.005 \times \text{age\_hours}}$
- $\text{reinforcement} = 0.3$ if explicitly reinforced, else $0.0$

#### API Signature:
```rust
pub struct ImportanceScorer;

impl ImportanceScorer {
    /// Computes the numerical importance score of a fact.
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
```

### 2.2 Memory Compactor & Cluster Consolidation (`src/consolidation/compactor.rs`)
The `MemoryCompactor` exposes two strategies to compress and prune semantic facts:

#### A. Decay Compaction (`compact_by_decay`)
- Scans all active semantic facts in scope.
- Evaluates access counts (from `memory_access_log`), graph connection counts, and age.
- Computes new importance scores.
- Any fact with an importance score $< \text{min\_importance}$ (default `0.15`) and age $> \text{max\_age\_hours}$ (default `24.0`) is **archived**.
- **Archiving** updates the fact's `valid_until` to the current timestamp.

#### B. Cluster Consolidation (`consolidate_clusters`)
- Groups active facts in the current scope.
- Computes pairwise cosine similarity of their embedding vectors.
- Clusters facts sharing a similarity $\ge \text{cluster\_threshold}$ (default `0.90`).
- For each cluster of size $\ge 2$:
  - Determines the **winner** fact (highest importance score / most recently updated).
  - Merges the texts of other cluster members (losers) into the winner's text.
  - Marks the losers as superseded: updates `valid_until = NOW` and `superseded_by = winner.node_id`.
  - Re-saves the winner's merged text and regenerates its embedding vector.
- Rebuilds the HNSW index if any facts were archived or updated.

```rust
pub struct CompactionReport {
    pub removed_count: u32,
    pub merged_count: u32,
    pub details: String,
}

pub struct MemoryCompactor;

impl MemoryCompactor {
    pub fn run_compaction(
        coordinator: &crate::coordinator::MemoryCoordinator,
        strategy: &str, // "decay", "cluster", "both"
        dry_run: bool,
        min_importance: f64,
        max_age_hours: f64,
        cluster_threshold: f64,
        scope: &crate::layers::MemoryScope,
    ) -> anyhow::Result<CompactionReport> {
        // Implementation details...
    }
}
```

## 3. MCP Tool Interface

### Tool Schema: `compact_memories`
- **Input Parameters (`CompactMemoriesInput`)**:
  ```json
  {
    "strategy": "decay" | "cluster" | "both",
    "dryRun": boolean (optional, default: false),
    "minImportance": number (optional, default: 0.15),
    "maxAgeHours": number (optional, default: 24.0),
    "clusterThreshold": number (optional, default: 0.90),
    "userId": string (optional),
    "sessionId": string (optional),
    "agentId": string (optional)
  }
  ```
- **Response Format**:
  ```json
  {
    "removedCount": 2,
    "mergedCount": 1,
    "dryRun": false,
    "details": "Archived 2 decayed facts. Consolidated 1 cluster containing 3 similar facts."
  }
  ```

## 4. Test Strategy
We will implement integration tests in `src/mcp.rs` and `tests/` verifying:
1. **Decay Compaction**: Adding facts, simulating time elapsed/low access, running decay compaction, verifying they are archived (no longer returned in standard queries but visible in `query_as_of`).
2. **Cluster Consolidation**: Adding three highly similar facts (similarity $> 0.90$), running cluster compaction, verifying only one remains active, and its text contains the merged information.
3. **gRPC Interface**: Testing the new tool over the gRPC endpoint.
