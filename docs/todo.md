# openmemory_rs — TODO

> **Auto-generated from Implementation Plan v2** | Last updated: 2026-06-26
> **Rules:**
> - No web UI / dashboard features. This is a pure MCP server for AI agents.
> - **100% pure Rust.** Zero Python, Node.js, or external language runtimes.
> - **Zero LLM/API calls.** All intelligence is algorithmic (cosine similarity, BM25, TF-IDF, regex, heuristics).
> - All dependencies must be Rust crates only.

### New Rust Crate Dependencies (across all phases)

| Crate | Version | Phase | Purpose |
|:---|:---|:---:|:---|
| `unicode-segmentation` | 1.11 | P2, P6 | Sentence / word boundary splitting |
| `rust-stemmers` | 1.2 | P4, P6 | Word stemming for dedup + TF-IDF |
| `stop-words` | 0.8 | P6 | Stop word removal for compression |
| `regex` | 1.10 | P6 | Pattern matching for fact extraction |
| `aes-gcm` | 0.10 | P7 | AES-256-GCM encryption at rest |
| `argon2` | 0.5 | P7 | Key derivation from passphrase |
| `rand` | 0.8 | P7 | Nonce generation for encryption |
| `thiserror` | 2.0 | P7 | Typed error enums |
| `criterion` | 0.5 | P7 | Benchmark harness |

---

## 🔴 Phase 1 — Code Quality & Foundation Fixes `v0.2.0`

> **Branch:** `phase1/hardening` | **Est:** 1 week

### Tech Debt Cleanup
- [x] Fix 55 camelCase compiler warnings (`#[serde(rename_all = "camelCase")]` or `#[allow(non_snake_case)]`)
- [x] Target: zero warnings on `cargo build`

### Integrate Existing Unused Code
- [x] Wire `Ranker::score()` into `SemanticMemory::query_similar_facts()`
- [x] Add `importance` and `success_rate` fields to semantic metadata
- [x] Return results sorted by composite score, not raw cosine similarity
- [x] Integrate `Config` struct — read `MEMORY_DB_PATH`, `EMBEDDING_MODEL`, `RUST_LOG` from env
- [x] Pass `Config` to `MemoryCoordinator::new()`

### Performance & Reliability
- [x] Enable SQLite WAL mode (`PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;`)
- [x] Add graceful shutdown via `tokio::signal` (flush WAL, dump HNSW, close connections)
- [x] Fix `search_nodes` O(n) edge scan → proper SQL JOIN query
- [x] Remove or populate unused `codebase_signatures` table

---

## 🟠 Phase 2 — Hybrid Search & Memory Scoping `v0.3.0`

> **Branch:** `phase2/hybrid-search` | **Est:** 2 weeks
> **Ref:** [memory-mcp-rs](https://github.com/ssoj13/memory-mcp-rs), [Mem0](https://github.com/mem0ai/mem0), [AgentMemory](https://github.com/rohitg00/agentmemory)

### FTS5 Full-Text Search
- [x] Create `semantic_fts` FTS5 virtual table in `SemanticMemory::new()`
- [x] Add SQLite triggers to keep FTS index in sync with `semantic_metadata`
- [x] Add `search_text()` method on SemanticMemory
- [x] New MCP tool: `search_text`

### Hybrid Search (Reciprocal Rank Fusion)
- [x] Create `src/search/hybrid.rs` module
- [x] Implement RRF: `score = Σ 1/(k + rank_i)` across vector, FTS5, graph results
- [x] Modify `query_similar_facts` to use hybrid search by default
- [x] New MCP tool: `hybrid_search`

### Memory Scoping
- [x] Add `user_id`, `session_id`, `agent_id` columns to ALL memory tables
  - [x] `semantic_metadata`
  - [x] `graph_nodes`
  - [x] `graph_edges`
  - [x] `episodic_logs`
  - [x] `reflection_memory`
  - [x] `tool_performance`
  - [x] `code_elements`
  - [x] `shared_agent_memory`
- [x] Add optional scope filters to ALL query MCP tools
- [x] Default scope `'*'` = accessible by all

### Memory Access Tracking
- [x] Create `memory_access_log` table
- [x] Log every read/search hit with `memory_id`, `layer`, `accessed_at`, `accessed_by`
- [x] New MCP tool: `memory_stats` (total counts, db size, most accessed)

---

## 🟡 Phase 3 — Temporal Intelligence & Conflict Resolution `v0.4.0`

> **Branch:** `phase3/temporal` | **Est:** 2 weeks
> **Ref:** [Zep/Graphiti](https://github.com/getzep/graphiti)

### Bi-temporal Fact Model
- [x] Add `valid_from`, `valid_until`, `superseded_by`, `confidence` columns to `graph_edges`
- [x] Add `valid_from`, `valid_until`, `superseded_by` columns to `semantic_metadata`
- [x] Default `valid_until = NULL` means currently valid
- [x] New MCP tool: `invalidate_fact`
- [x] New MCP tool: `query_fact_history`
- [x] New MCP tool: `query_as_of`

### Conflict Resolution
- [x] Create `src/search/conflict.rs` module
- [x] `detect_conflicts()` — find same entity+relation with different values
- [x] `resolve_by_recency()` — newest fact wins, old gets `valid_until = NOW`
- [x] `resolve_by_confidence()` — higher confidence score wins
- [x] New MCP tool: `detect_and_resolve_conflicts`

### Working Memory Enhancement
- [x] Add `WorkingEntry` struct with `created_at`, `ttl`, `access_count`
- [x] Implement TTL-based expiration on `get()`
- [x] Add `evict_expired()` method
- [x] Add `promote_to_semantic()` — move important entries to long-term before eviction
- [x] Make `default_ttl` configurable via `Config`

---

## 🔵 Phase 4 — Memory Consolidation Engine `v0.5.0`

> **Branch:** `phase4/consolidation` | **Est:** 3 weeks
> **Ref:** [Mem0](https://github.com/mem0ai/mem0) (decision engine), [Letta/MemGPT](https://github.com/letta-ai/letta) (autonomous management)

### Semantic Deduplication
- [ ] Create `src/search/dedup.rs` module
- [ ] `find_duplicates()` — cosine similarity > 0.92 threshold
- [ ] `merge_facts()` — combine two related facts into one
- [ ] Run dedup check before every `add_fact()` call

### Memory Decision Engine (ADD/UPDATE/DELETE/NO-OP)
- [ ] Create `src/consolidation/engine.rs` module
- [ ] `decide()` — analyze new info against existing memories
- [ ] Logic:
  - [ ] Cosine > 0.98 → NO-OP (exact duplicate)
  - [ ] Cosine > 0.92 → UPDATE (enrich existing)
  - [ ] Entity+relation match with different value → DELETE old + ADD new
  - [ ] No match → ADD
- [ ] New MCP tool: `smart_store`

### Auto-compaction
- [ ] Create `src/consolidation/compactor.rs` module
- [ ] `compact_by_decay()` — remove memories below importance threshold
- [ ] `consolidate_clusters()` — merge similar memory clusters
- [ ] New MCP tool: `compact_memories`

### Auto-importance Scoring
- [ ] Create `src/search/importance.rs` module
- [ ] Calculate importance from: access_count, edge_count, age, reinforcement
- [ ] Update scores lazily on read or periodically
- [ ] Feed scores into ranker's `importance` field

---

## 🟣 Phase 5 — Advanced Graph Intelligence `v0.6.0`

> **Branch:** `phase5/graph-intelligence` | **Est:** 2 weeks
> **Ref:** [neo4j-graphrag](https://github.com/neo4j/neo4j-graphrag-python), [FalkorDB](https://github.com/FalkorDB/FalkorDB), [Graphiti](https://github.com/getzep/graphiti)

### Multi-hop Graph Traversal
- [ ] Create `src/layers/graph_traversal.rs` module
- [ ] `bfs_traverse()` — BFS from entity within N hops using petgraph
- [ ] `shortest_path()` — find shortest path between two entities
- [ ] `relation_chain()` — follow specific relation type sequences
- [ ] New MCP tool: `traverse_graph`
- [ ] New MCP tool: `find_path`

### Community Detection
- [ ] Create `src/search/community.rs` module
- [ ] `detect_communities()` — connected components + modularity using petgraph
- [ ] `summarize_community()` — generate text summary for each cluster
- [ ] New MCP tool: `analyze_graph_communities`

### Recursive Code Impact Analysis
- [ ] Add `impact_analysis()` to `CodebaseMemory`
- [ ] Traverse call graph transitively to find all affected symbols
- [ ] Calculate change risk levels based on dependency depth
- [ ] New MCP tool: `analyze_code_impact`

---

## 🟢 Phase 6 — Context Intelligence & Agent Optimization `v0.7.0`

> **Branch:** `phase6/context-intelligence` | **Est:** 2 weeks
> **Ref:** [Mem0](https://github.com/mem0ai/mem0) (context compression), [AgentMemory](https://github.com/rohitg00/agentmemory) (auto-capture)

### Fact Extraction (Pure Rust — `regex` + `unicode-segmentation`)
- [ ] Create `src/extraction/fact_extractor.rs`
- [ ] Build rule-based fact extractor using only Rust crates:
  - [ ] Sentence splitting via `unicode-segmentation` crate
  - [ ] Entity mention detection (capitalized words, code identifiers via regex)
  - [ ] Precompiled regex patterns for relation detection:
    - [ ] "X uses Y" → `uses` relation
    - [ ] "X depends on Y" → `depends_on` relation
    - [ ] "X prefers Y" → `prefers` relation
    - [ ] "X is a Y" → `is_a` relation
    - [ ] "X works with Y" → `works_with` relation
    - [ ] "X created Y" → `created` relation
  - [ ] Run through consolidation engine (cosine similarity dedup + decide)
- [ ] New MCP tool: `extract_and_store_facts`
- [ ] ⚠️ Verify: ZERO LLM/API calls in extraction pipeline

### Proactive Memory Recall
- [ ] Cross-layer hybrid search: semantic + graph + episodic
- [ ] Score fusion across all layers via RRF (pure Rust)
- [ ] New MCP tool: `proactive_recall`

### Context Compression (Pure Rust — TF-IDF + `rust-stemmers` + `stop-words`)
- [ ] Create `src/extraction/compressor.rs`
- [ ] Implement TF-IDF sentence scoring (pure Rust, no LLM):
  - [ ] Sentence splitting via `unicode-segmentation`
  - [ ] Word stemming via `rust-stemmers` crate
  - [ ] Stop word removal via `stop-words` crate
  - [ ] Score each sentence by TF-IDF relevance
  - [ ] Keep top N sentences (N = count × target_ratio)
  - [ ] Dedup against already-stored memories (cosine similarity via fastembed)
- [ ] New MCP tool: `compress_context`
- [ ] ⚠️ Verify: ZERO LLM/API calls in compression pipeline

---

## ⚫ Phase 7 — Production Hardening `v1.0.0`

> **Branch:** `phase7/production` | **Est:** 3 weeks

### Error Handling
- [ ] Create `src/error.rs` with `MemoryError` enum via thiserror
- [ ] Replace all `anyhow::bail!` with typed errors
- [ ] Map errors to proper MCP error codes

### Database Migrations
- [ ] Create `src/db/migrations.rs` with versioned SQL migrations
- [ ] Auto-run pending migrations on startup
- [ ] Store migration state in `_migrations` table

### Test Suite (~83 tests)
- [ ] Graph CRUD tests (12)
- [ ] Semantic add/query/dedup tests (8)
- [ ] Hybrid search tests (6)
- [ ] Episodic log/reflect/tool-perf tests (8)
- [ ] Codebase index/query/impact tests (6)
- [ ] Shared store/retrieve/scope tests (4)
- [ ] Working memory TTL/eviction tests (4)
- [ ] Consolidation engine decision tests (8)
- [ ] Conflict resolution tests (6)
- [ ] Temporal query tests (6)
- [ ] Database branching lifecycle tests (4)
- [ ] Ranker integration tests (3)
- [ ] Graph traversal tests (4)
- [ ] Memory compaction tests (4)

### CI/CD
- [ ] Create `.github/workflows/ci.yml`
  - [ ] `cargo fmt --check`
  - [ ] `cargo clippy -- -D warnings`
  - [ ] `cargo build --release`
  - [ ] `cargo test`
  - [ ] `cargo bench`

### Benchmarks (criterion)
- [ ] Semantic add fact: target < 50ms
- [ ] Semantic query top 10: target < 10ms
- [ ] FTS5 keyword search: target < 1ms
- [ ] Hybrid search: target < 15ms
- [ ] Graph entity create: target < 1ms
- [ ] Graph BFS 5 hops: target < 5ms
- [ ] Working memory get: target < 0.01ms
- [ ] Consolidation decision: target < 20ms

### Security
- [ ] Input validation on all MCP tool inputs (max string lengths, allowed characters)
- [ ] SQL injection audit (verify all parameterized statements)
- [ ] Optional memory encryption at rest (`MEMORY_ENCRYPT_KEY`)
- [ ] Per-agent access control (scoped read/write permissions)

### Publishing
- [ ] Complete `README.md` with quick start guide
- [ ] MCP client config examples (Claude Desktop, Cursor, Antigravity, Gemini CLI)
- [ ] `CHANGELOG.md`
- [ ] `cargo doc` API documentation
- [ ] Publish to [crates.io](https://crates.io)
- [ ] Docker image

---

## 📊 Feature Summary

| Metric | Current (v0.1.1) | After Plan (v1.0.0) |
|:---|:---:|:---:|
| MCP tools | 22 | **38** |
| Search modes | Vector only | **Vector + FTS5 + Graph + Hybrid** |
| Memory scoping | None | **User + Session + Agent** |
| Temporal awareness | Created timestamps only | **Bi-temporal (valid_from/valid_until)** |
| Conflict resolution | None | **Auto-detect + resolve** |
| Memory consolidation | None | **ADD/UPDATE/DELETE/NO-OP engine** |
| Working memory | Basic HashMap | **TTL + eviction + promotion** |
| Graph queries | Flat SQL | **Multi-hop BFS/DFS + community detection** |
| Code analysis | Index only | **+ Recursive impact analysis** |
| Test count | 2 | **~83** |
| Compiler warnings | 55 | **0** |

---

## 🔗 Reference Repos

| Repo | What to Learn |
|:---|:---|
| [mem0ai/mem0](https://github.com/mem0ai/mem0) | Memory decision engine, scoping, compression |
| [letta-ai/letta](https://github.com/letta-ai/letta) | Autonomous memory management, tiered architecture |
| [getzep/graphiti](https://github.com/getzep/graphiti) | Bi-temporal graphs, conflict resolution, communities |
| [rohitg00/agentmemory](https://github.com/rohitg00/agentmemory) | BM25 hybrid search, auto-capture, efficiency benchmarks |
| [ssoj13/memory-mcp-rs](https://github.com/ssoj13/memory-mcp-rs) | Rust + SQLite FTS5 reference |
| [edg-l/engram-mcp](https://github.com/edg-l/engram-mcp) | Branch-aware Rust MCP sessions |
| [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) | Official rmcp patterns |
| [neo4j/neo4j-graphrag-python](https://github.com/neo4j/neo4j-graphrag-python) | Multi-hop graph queries |
| [FalkorDB/FalkorDB](https://github.com/FalkorDB/FalkorDB) | Ultra-low-latency graph engine patterns |
| [Dicklesworthstone/fastmcp_rust](https://github.com/Dicklesworthstone/fastmcp_rust) | Zero-copy MCP patterns |
