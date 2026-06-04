# Codebase Structure

`openmemory_rs` is organized as a modular, unified Rust codebase. This directory contains detailed descriptions of each source file, target modules, dependencies, and testing parameters.

---

## 1. Directory Tree

```
memory_rs/
├── assets/
│   └── logo.png              # Project logo asset
├── docs/
│   ├── architecture.html     # Standalone interactive SVG/CSS card
│   ├── architecture.md       # Architecture spec (Layers & SQLite Tables)
│   ├── features.md           # Deep-dive features (Scoring & AST Graphs)
│   └── codebase.md           # Crate structure & build guide [This File]
├── external/                 # Cloned references (gitignored)
│   ├── supermemory-mcp/      # Reference Supermemory MCP
│   └── codebase-memory-mcp/  # Reference Codebase-Memory MCP
├── src/
│   ├── layers/               # 6 Cognitive Memory Layers
│   │   ├── working.rs        # Layer 1: RAM session store
│   │   ├── graph.rs          # Layer 2: Entity-relation graph
│   │   ├── semantic.rs       # Layer 3: SQLite Vector Fact store
│   │   ├── episodic.rs       # Layer 4: Execution log, reflection & tool metrics
│   │   ├── codebase.rs       # Layer 5: AST code structure & evolutions
│   │   ├── shared.rs         # Layer 6: Synchronized cross-agent team board
│   │   └── mod.rs            # Crate re-exports for layers module
│   ├── search/               # Search and decay engines
│   │   ├── ranker.rs         # Mathematical composite decay ranker
│   │   └── mod.rs            # Crate re-exports for search module
│   ├── config.rs             # Configuration env readings
│   ├── coordinator.rs        # Memory Coordinator orchestrator
│   ├── main.rs               # Server Entrypoint
│   └── mcp.rs                # Stdio MCP Server tools registration
├── Cargo.lock                # Cargo lock dependencies
├── Cargo.toml                # Package definition
└── memory.db                 # Unified SQLite database store (gitignored)
```

---

## 2. Module Implementations

### Crate Entrypoint
* **File**: [src/main.rs](../src/main.rs)
* **Function**: Bootstraps the engine. Initializes environment logging, checks database paths, and boots up the stdio transport server.

### Stdio JSON-RPC Tools Router
* **File**: [src/mcp.rs](../src/mcp.rs)
* **Function**: Handles Stdio transport and defines 19 compatible MCP tools. Implements a static codebase directory crawler that extracts code symbols and imports using line-by-line syntax analysis.

### System Coordinator
* **File**: [src/coordinator.rs](../src/coordinator.rs)
* **Function**: Exposes the `MemoryCoordinator` wrapper containing Arc pointers to all 6 layers. Drives CRUD execution across multiple storage modules concurrently.

### Memory Layers
* **File**: [src/layers/working.rs](../src/layers/working.rs)
  * Implements atomic `RwLock<HashMap<String, String>>` session cache tables.
* **File**: [src/layers/graph.rs](../src/layers/graph.rs)
  * Implements SQLite-backed entities, directed relationships, and observations.
* **File**: [src/layers/semantic.rs](../src/layers/semantic.rs)
  * Handles vector blobs, metadata timestamps, and cosine similarity.
* **File**: [src/layers/episodic.rs](../src/layers/episodic.rs)
  * Manages logs, reflections (what worked, what failed, why), attempt sequences, and aggregates model latencies/success statistics.
* **File**: [src/layers/codebase.rs](../src/layers/codebase.rs)
  * Manages AST classes, functions, impls, caller hierarchies, and records file evolution logs.
* **File**: [src/layers/shared.rs](../src/layers/shared.rs)
  * Synchronizes context keys and values across multiple subagents using wildcard targets.

### Decay & Search Model
* **File**: [src/search/ranker.rs](../src/search/ranker.rs)
  * Applies exponential decay factors based on hours elapsed to prioritize current facts.

---

## 3. Cargo Dependencies (Cargo.toml)

* **`tokio`**: Driving asynchronous runtimes.
* **`rmcp`**: Provides server-side Model Context Protocol SDK bindings and macros.
* **`rusqlite`**: Thread-safe SQL persistence bundle.
* **`fastembed`**: Fast vector embeddings generator utilizing ORT. Locked to `default-features = false` with pure-Rust `hf-hub-rustls-tls` to avoid system OpenSSL dependencies.
* **`petgraph`**: Relational graph representation tables.
* **`serde` & `serde_json`**: Serializations and JSON mappings.
* **`schemars`**: Generates schemas from Rust types for client-side tool discovery.
* **`chrono`**: Time representations and calculations.
* **`uuid`**: Generates random identifier strings.

---

## 4. Compilation & Execution

### Compile the optimized binary:
```bash
cargo build --release
```
The compiled output is located at:
`target/release/openmemory_rs`

### Start the server locally:
```bash
cargo run --release
```
To run the server in background debug mode with detailed logs, set:
`RUST_LOG=info cargo run --release`
