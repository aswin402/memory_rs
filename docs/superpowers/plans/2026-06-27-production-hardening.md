# Phase 7 Implementation Plan: Production Hardening

This document outlines the step-by-step implementation tasks for Phase 7 in `openmemory_rs`.

## Development Tasks

### Task 1: Typed Error Handling
- **Files:**
  - Create: `src/error.rs`
  - Modify: `Cargo.toml` (add `thiserror` dependency)
  - Modify: `src/main.rs` (register `error` module)
  - Modify: `src/mcp.rs` (implement `From<MemoryError> for McpError`)
  - Modify: Layer files (`src/coordinator.rs`, `src/layers/*`, `src/search/*`, `src/consolidation/*`, `src/extraction/*`)
- **Implementation:**
  - Define `MemoryError` and a custom `Result<T>` type alias.
  - Replace all occurrences of `anyhow::Result` and `anyhow::bail!` in the library layers with `crate::error::Result` and appropriate `MemoryError` variants.
- **Verification:**
  - Run `cargo check` and `cargo test` to verify compiler errors are resolved and tests continue to pass.

---

### Task 2: Centralized Database Migrations
- **Files:**
  - Create: `src/db/mod.rs`
  - Create: `src/db/migrations.rs`
  - Modify: `src/main.rs` (register `db` module)
  - Modify: Layer files (`src/layers/*.rs` to remove ad-hoc `CREATE TABLE` queries and invoke migrations on startup)
- **Implementation:**
  - Create versioned migrations:
    - `001_initial.sql` (core tables setup)
    - `002_fts5.sql` (FTS5 search setup)
    - `003_temporal.sql` (temporal schemas setup)
    - `004_scoping.sql` (scoping access logs and tables setup)
  - Write `run_migrations(conn: &Connection)` utilizing a `schema_migrations` tracking table.
  - Centralize connection setup in `MemoryCoordinator::new`.
- **Verification:**
  - Verify initialization works on fresh database files.

---

### Task 3: Security & Input Hardening
- **Files:**
  - Modify: `src/mcp.rs`
- **Implementation:**
  - Implement validation functions:
    - `validate_identifier(id: &Option<String>) -> Result<()>` (alphanumeric/dashes/dots check, max 128 chars).
    - `validate_text_length(text: &str, max_len: usize) -> Result<()>` (max 65,536 chars).
  - Inject these validations at the beginning of all MCP tool handler entrypoints in `src/mcp.rs`.
- **Verification:**
  - Write unit tests in `src/mcp.rs` verifying that invalid identifiers or overly large text inputs are correctly blocked and return invalid parameter errors.

---

## SDD Progress Checklist

To track execution progress, we will reset the ledger at `.superpowers/sdd/progress.md`.
- [ ] Task 1: Typed Error Handling
- [ ] Task 2: Centralized Database Migrations
- [ ] Task 3: Security & Input Hardening
