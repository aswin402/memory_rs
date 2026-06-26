# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
