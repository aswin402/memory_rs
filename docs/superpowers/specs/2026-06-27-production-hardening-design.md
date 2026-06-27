# Production Hardening Design Spec

This document specifies the design, architectures, migrations, and structures for **Phase 7: Production Hardening** in `openmemory_rs`.

## 1. Overview & Goals

Phase 7 prepares `openmemory_rs` for production use and potential crates.io publishing. Its main objectives are:
1. **Typed Error Handling**: Shift from generic `anyhow` errors to structured, typed errors using the `thiserror` crate, which maps cleanly to JSON-RPC `McpError` error codes.
2. **Database Migration System**: Implement a schema migration system in SQLite utilizing an index/table `schema_migrations` to cleanly run versioned scripts instead of raw initialization strings.
3. **Security & Input Validation**: Add defensive bounds checks to MCP parameter inputs (e.g. maximum length constraints on facts, queries, identifiers) to protect against buffer overflows and memory/disk exhaustion.

---

## 2. Typed Error Handling Design (`src/error.rs`)

We will define `MemoryError` in `src/error.rs` and map its variants to the `rmcp` server errors:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("HNSW Index error: {0}")]
    Index(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("Conflict occurred: {0}")]
    Conflict(String),

    #[error("Database branching error: {0}")]
    Branch(String),

    #[error("Invalid input validation: {0}")]
    Validation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;
```

### 2.1 MCP Error Mapping
Within `src/mcp.rs`, we will implement `From<MemoryError> for McpError`:
- `MemoryError::NotFound` -> `McpError::invalid_params` or specific application code.
- `MemoryError::Validation` -> `McpError::invalid_params`.
- `MemoryError::Conflict` -> `McpError::internal_error`.
- All other variants -> `McpError::internal_error`.

---

## 3. Database Migration System (`src/db/migrations.rs`)

Instead of each layer doing raw table creation queries on startup, `openmemory_rs` will use a unified migration runner.

### 3.1 Migration Schema
We will create a table `schema_migrations` tracking active version names:
```sql
CREATE TABLE IF NOT EXISTS schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TEXT NOT NULL
);
```

### 3.2 Defined Migrations
We will define migrations in order:
1. `001_initial`: Sets up core SQLite tables (nodes, edges, semantic mapping, index mapping, episodic, code, shared).
2. `002_fts5`: Configures FTS5 virtual tables and search triggers.
3. `003_temporal`: Configures temporal/bi-temporal metadata columns.
4. `004_scoping`: Configures scopes and access log triggers.

On start, the `MemoryCoordinator` will invoke `run_migrations(conn)` inside a transaction:
1. Get already applied versions from `schema_migrations`.
2. For each defined migration not in the list:
   - Execute the SQL.
   - Insert version into `schema_migrations`.
   - Commit the transaction.

---

## 4. Input & Security Hardening

To prevent resource-exhaustion attacks or garbage insertion:
- Limit maximum string size for text inputs to `65,536` characters.
- Validate that identifiers (user ID, session ID, agent ID, node name) contain only alphanumeric characters, dashes, underscores, and dots, with a max length of `128` characters.
- Reject requests exceeding these bounds with a `MemoryError::Validation`.
