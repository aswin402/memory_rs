# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
