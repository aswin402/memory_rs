# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.7] - 2026-06-27

### Added
- **Benchmark Suite**: Added criterion micro-benchmarks targeting semantic insertions, semantic similarity queries, context compression, and graph traversal.
- **Library Target**: Refactored the package to compile as both a library crate (`src/lib.rs`) and binary crate (`src/main.rs`) to expose cognitive layers to benchmark suites and external tools.

### Changed
- **Robust BFS Traversal**: Refactored graph BFS to use a custom `VecDeque` queue instead of petgraph's `Bfs` to ensure safety and prevent key-lookup panics under `max_depth` constraints.

### Fixed
- **Database Schema Constraints**: Fixed `graph_edges` re-creation in migrations to restore `valid_from` and `confidence` default constraints.
- **Unit Test Table Migration Errors**: Fixed table migrations to automatically run on connection opening inside individual layers, ensuring all unit tests succeed.

## [0.1.6] - 2026-06-27

### Added
- **Fact Extraction Engine**: Implemented algorithmic entity-relation triple parsing from raw texts using sentence tokenization, precompiled regex patterns (`uses`, `depends_on`, `prefers`, `is_a`, `works_with`, `created`), and common noun/stop-word filters.
- **Proactive Recall Engine**: Implemented cross-layer context-based retrieval querying Semantic, Graph, and Episodic memory, normalizing scores, deduplicating elements, and applying cross-layer entity name reference boosting.
- **Context Compressor**: Implemented local TF-IDF sentence scoring, Porter Stemmer word normalization, English stop-words filtering, and original document order reconstruction.
- **New MCP Tools**: Exposed `extract_and_store_facts`, `proactive_recall`, and `compress_context` MCP tools.
- **gRPC Integration Testing**: Extended Tonic gRPC integration test suite with step 12 verifying context intelligence tools.

## [0.1.5] - 2026-06-27

### Added
- **Multi-hop Graph Traversal**: Implemented depth-limited BFS traversal, A* shortest path, and relation chain following algorithms inside `src/layers/graph_traversal.rs`.
- **Connected Component Community Detection**: Segmented graph memory into communities using Tarjan's SCC algorithm, generating textual summaries from member observations inside `src/search/community.rs`.
- **Recursive Codebase Impact Analysis**: Created transitive downstream caller analysis tracing callee-to-caller dependencies using reversed call-graph BFS and calculating risk metrics.
- **New MCP Tools**: Exposed `traverse_graph`, `find_path`, `analyze_graph_communities`, and `analyze_code_impact` tools for advanced graph intelligence.
- **gRPC Integration Testing**: Extended Tonic gRPC integration test suite with verification coverage for the new graph traversal endpoint.

## [0.1.4] - 2026-06-26


### Added
- **Bi-temporal Fact Model**: Introduced bi-temporal metadata columns (`valid_from`, `valid_until`, `superseded_by`, and `confidence`) in `graph_edges` and `semantic_metadata` tables.
- **Temporal Query Functions**: Implemented `invalidate_edge`, `query_fact_history`, and `query_as_of` layers logic to inspect memory engine states at arbitrary historical timestamps.
- **New MCP Tools**: Exposed `invalidate_fact`, `query_fact_history`, and `query_as_of` JSON-RPC tools to let AI agents query/manipulate temporal facts.
- **gRPC Temporal Testing**: Added full gRPC integration tests verifying temporal operations over Tonic protocol.

### Fixed
- **SQLite ISO 8601 string sorting precision trap**: Fixed test queries by formatting UTC timestamps to a strict 20-character format `%Y-%m-%dT%H:%M:%SZ` matching SQLite's `strftime` format.
- **Unused variable warning**: Cleaned up the unused variable warning in `memory_stats` input.

## [0.1.3] - 2026-06-26

### Added
- **Hybrid Search**: Implemented Hybrid Search combining FTS5 keyword matching and fastembed semantic embeddings via Reciprocal Rank Fusion (RRF) with a constant parameter $k=60$.
- **Memory Scoping**: Scoped all memory layers (Graph, Semantic, Episodic, Codebase, Shared) using `user_id`, `session_id`, and `agent_id` columns, with wildcards (`*`) indicating cross-scope accessibility.
- **Memory Access Logging**: Added a persistent `memory_access_log` tracking all read operations on memory elements.
- **Memory Stats Tool**: Exposed a new MCP tool `memory_stats` to query memory usage metrics, database sizes, table row counts, and memory access records.

### Fixed
- **Codebase Indexing Helpers**: Refactored static parser helpers to accept and propagate the `MemoryScope` parameter, resolving compiler type mismatch errors.

## [0.1.2] - 2026-06-26

### Added
- **Graceful Shutdown & Signal Handlers**: Added tokio signal listeners for `SIGINT` (Ctrl+C) and `SIGTERM`. The stdio and gRPC servers now listen to a shutdown channel.
- **WAL Checkpoints on Shutdown**: Added `checkpoint()` to all sqlite layers (`GraphMemory`, `SemanticMemory`, `EpisodicMemory`, `CodebaseMemory`, `SharedMemory`) to execute `PRAGMA wal_checkpoint(TRUNCATE)` on shutdown to flush WAL logs to primary database files.
- **SQLite WAL Mode Integration**: Configured `PRAGMA journal_mode=WAL` and `PRAGMA synchronous=NORMAL` by default on all SQLite connections (both initialization and branch switches) to improve concurrent read performance.
- **Relevance Ranker Integration**: Wired the `Ranker::score` decay calculation into `SemanticMemory::query_similar_facts`. Results are now ordered by composite cognitive ranking (similarity, recency, and importance) rather than raw similarity.

### Fixed
- **camelCase Struct Warning Fixes**: Added `#[serde(rename_all = "camelCase")]` and refactored MCP input wrapper struct fields to snake_case in Rust. This fixes 55 compiler warnings while maintaining full camelCase compatibility in JSON-RPC.
- **Optimized Edge Filtering**: Replaced $O(N)$ in-memory edge scan in `search_nodes` and `open_nodes` with optimized SQLite subqueries matching from/to endpoints, drastically reducing memory footprint and lookup times.
- **Cleaned Up Unused Tables**: Removed the unused `codebase_signatures` table definition from `src/layers/codebase.rs`.
