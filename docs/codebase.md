# Codebase Structure

The `memory_rs` server is structured as a modular Rust binary crate. This guide provides an overview of the modules, file layout, dependencies, and build steps.

---

## 1. Directory Tree

```
memory_rs/
├── assets/
│   └── logo.png              # Generated high-tech logo
├── docs/
│   ├── architecture.html     # Interactive dark-themed architecture diagram
│   ├── architecture.md       # Architectural layout details
│   ├── features.md           # Deep-dive of features & ranking equations
│   └── codebase.md           # Codebase modules & structure guide [This File]
├── src/
│   ├── layers/               # Memory Layer implementations
│   │   ├── codebase.rs       # Layer 5: Codebase signature storage
│   │   ├── episodic.rs       # Layer 4: Execution trace logs
│   │   ├── graph.rs          # Layer 2: Main knowledge graph core
│   │   ├── mod.rs            # Re-exports for layers module
│   │   ├── semantic.rs       # Layer 3: Vector metadata & semantic facts
│   │   └── working.rs        # Layer 1: Concurrency session variables
│   ├── search/               # Search and ranking algorithms
│   │   ├── mod.rs            # Re-exports for search module
│   │   └── ranker.rs         # Score equations and decay formulas
│   ├── config.rs             # Configuration reading from env variables
│   ├── coordinator.rs        # Memory Coordinator orchestrator
│   ├── main.rs               # Server Entrypoint
│   └── mcp.rs                # Model Context Protocol server tools
├── Cargo.lock                # Cargo lock dependencies
├── Cargo.toml                # Cargo manifest
└── memory.db                 # Compiled SQLite memory file (gitignored)
```

---

## 2. Code Modules Detailed Description

### Server Entrypoint
* **File**: [src/main.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/main.rs)
* **Function**: Initializes the environment logger, parses database configuration settings, constructs a thread-safe `MemoryCoordinator`, and hands off execution to the `mcp::run_server` stdio handler.

### Stdio MCP Handler
* **File**: [src/mcp.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/mcp.rs)
* **Function**: Defines the Stdio JSON-RPC transport wrapper mapping input structs. Implements `rmcp` handler routers to dispatch requests to coordinator layer actions, returning standardized MCP outputs.

### Orchestration layer
* **File**: [src/coordinator.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/coordinator.rs)
* **Function**: Bundles all 5 layers into a single coordinating struct wrapped inside atomic arc references (`Arc<MemoryCoordinator>`) for safe concurrent scheduling across threads.

### Search and Decay Models
* **File**: [src/search/ranker.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/search/ranker.rs)
* **Function**: Declares the `Ranker` utility implementing exponential time-based decay and weighting calculations to score context relevance.

### Persistent Layer Models
* **File**: [src/layers/graph.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/graph.rs)
  * Implements sqlite-backed graph node CRUD, relationship links, search filtering, and node updates.
* **File**: [src/layers/semantic.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/semantic.rs)
  * Implements sqlite-backed semantic metadata tracking and blob stores.
* **File**: [src/layers/episodic.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/episodic.rs)
  * Implements sqlite-backed execution steps, reflection logs, and successes.
* **File**: [src/layers/codebase.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/codebase.rs)
  * Implements sqlite-backed codebase signature index tracking.

---

## 3. Cargo Dependencies (Cargo.toml)

The crate manages its dependencies inside `Cargo.toml`. Key dependencies include:

* **`tokio`**: Runtime engine for driving asynchronous features.
* **`rmcp`**: Official Rust Model Context Protocol toolkit providing Stdio bindings and macros.
* **`rusqlite`**: Type-safe interface to the local SQLite database.
* **`parking_lot`**: High-performance, lightweight mutex locks.
* **`serde` & `serde_json`**: Serializing and deserializing JSON payloads.
* **`schemars`**: Generates JSON Schema definitions from Rust data types, required for MCP tools discovery.
* **`fastembed`**: Offline embedded text vectorization with ONNX Runtime. Specifically locked to avoid system SSL linking dependencies.

---

## 4. Build and Run Guide

### Prerequisite Dependencies
Ensure that `cargo` and `rustc` are installed on your path.

### 1. Build release binaries
To compile the server in optimized release mode, execute:
```bash
cargo build --release
```
The compiled executable will be written to:
`target/release/memory_rs`

### 2. Set environment parameters
You can configure path variables before launch:
```bash
# SQLite DB save location (defaults to local memory.db)
export MEMORY_DB_PATH="memory.db"

# Embedding Model (defaults to all-MiniLM-L6-v2)
export EMBEDDING_MODEL="all-MiniLM-L6-v2"
```

### 3. Launching
You can run the executable directly or through cargo:
```bash
cargo run --release
```
> [!NOTE]
> Since Stdio transport uses the standard input/output channels, running it directly in the terminal will wait for JSON-RPC requests. Use log outputs (`RUST_LOG=info`) redirection or debug clients to interface with it manually.
