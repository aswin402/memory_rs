# System Architecture

The `memory_rs` server implements a multi-tiered, cognitive-inspired memory architecture specifically designed to empower AI agents like **OpenZ**. Rather than relying on a flat, simple knowledge graph, `memory_rs` structures memory into **5 cognitive layers**, providing a comprehensive memory retention and context retrieval pipeline.

A visual breakdown of the architecture is available in the interactive diagram: [architecture.html](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/docs/architecture.html).

---

## The 5 Cognitive Memory Layers

```
        ┌────────────────────────────────────────────────────────┐
        │                 Memory Coordinator                     │
        └───────────────────────────┬────────────────────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         ▼                          ▼                          ▼
┌──────────────────┐      ┌──────────────────┐      ┌──────────────────┐
│  Working Memory  │      │   Graph Memory   │      │ Semantic Memory  │
│  (Session RAM)   │      │ (Entities/Edges) │      │ (Vector Vectors) │
└──────────────────┘      └──────────────────┘      └──────────────────┘
                                    ▲                          ▲
                                    │                          │
                          ┌─────────┴────────┘      ┌──────────┴────────┐
                          ▼                         ▼                   ▼
                 ┌──────────────────┐      ┌──────────────────┐┌──────────────────┐
                 │ Episodic Memory  │      │ Codebase Memory  ││ Embedding Engine │
                 │ (Execution Logs) │      │  (Signatures)    ││(all-MiniLM-L6-v2)│
                 └──────────────────┘      └──────────────────┘└──────────────────┘
```

### 1. Working Memory
* **File**: [src/layers/working.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/working.rs)
* **Description**: A session-bound, in-memory store utilizing safe concurrency guards (`RwLock<HashMap<String, String>>`).
* **Role**: Temporarily caches subsecond transaction-level variables, execution states, and ephemeral environment contexts. This layer clears when the server shuts down or restarts.

### 2. Graph Memory
* **File**: [src/layers/graph.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/graph.rs)
* **Description**: A persistent SQLite store representing explicit entities, connections, and JSON-encoded observations.
* **Role**: Preserves structural relational knowledge graphs. Fully compatible with the reference MCP Memory Spec, mapping tools like `read_graph`, `search_nodes`, `open_nodes`, `create_entities`, and `create_relations`.

### 3. Semantic Memory
* **File**: [src/layers/semantic.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/semantic.rs)
* **Description**: SQLite metadata database storing text observations linked with floating-point embedding blobs.
* **Role**: Powers semantic search. Incoming queries are vectorized using the local `fastembed` engine and compared using cosine similarity against historical semantic facts, bypasses exact word matches.

### 4. Episodic Memory
* **File**: [src/layers/episodic.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/episodic.rs)
* **Description**: Log execution database storing step-by-step agent task histories, reflections, and success rates.
* **Role**: Tracks agent performance. By logging task parameters, reflections, and whether a task failed, it assists in computing context priority and avoids replicating failed execution strategies in future runs.

### 5. Codebase Memory
* **File**: [src/layers/codebase.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/layers/codebase.rs)
* **Description**: Code-index database mapping module, class, struct, function signatures, file paths, and dependency graphs.
* **Role**: Helps developers and agents map code workspaces. By storing source signatures, it allows agents to instantly retrieve references, definitions, and file hierarchy mappings during programming tasks.

---

## Unified SQLite Storage Schema

All persistent memory layers are consolidated into a single local SQLite database (defaulting to `memory.db`). This design avoids file-locking issues, increases speed, and allows multi-table joins.

### Table Definitions

#### 1. Graph Memory Tables
* **`graph_nodes`**: Defines knowledge graph entities.
  ```sql
  CREATE TABLE IF NOT EXISTS graph_nodes (
      name TEXT PRIMARY KEY,
      entity_type TEXT NOT NULL,
      observations TEXT NOT NULL -- Serialized JSON array of text observations
  );
  ```
* **`graph_edges`**: Defines links between nodes.
  ```sql
  CREATE TABLE IF NOT EXISTS graph_edges (
      from_name TEXT NOT NULL,
      to_name TEXT NOT NULL,
      relation_type TEXT NOT NULL,
      PRIMARY KEY (from_name, to_name, relation_type)
  );
  ```

#### 2. Semantic Memory Tables
* **`semantic_metadata`**: Holds vector metadata mappings.
  ```sql
  CREATE TABLE IF NOT EXISTS semantic_metadata (
      node_id TEXT PRIMARY KEY,
      raw_text TEXT NOT NULL,
      embedding BLOB NOT NULL, -- Binary float array representing vectors
      timestamp TEXT NOT NULL,
      importance REAL NOT NULL DEFAULT 1.0
  );
  ```

#### 3. Episodic Memory Tables
* **`episodic_logs`**: Tracks agent episodes and reflections.
  ```sql
  CREATE TABLE IF NOT EXISTS episodic_logs (
      id TEXT PRIMARY KEY,
      task_description TEXT NOT NULL,
      execution_status TEXT NOT NULL,
      steps_taken TEXT NOT NULL,
      error_message TEXT,
      reflection TEXT,
      created_at TEXT NOT NULL
  );
  ```

#### 4. Codebase Memory Tables
* **`codebase_signatures`**: Indexes code symbols.
  ```sql
  CREATE TABLE IF NOT EXISTS codebase_signatures (
      id TEXT PRIMARY KEY,
      file_path TEXT NOT NULL,
      item_name TEXT NOT NULL,
      item_type TEXT NOT NULL,
      signature TEXT NOT NULL,
      dependencies TEXT -- Serialized JSON list of dependencies/imports
  );
  ```

---

## Control Flow Lifecycle

When an MCP client initiates a query or execution step:

1. **Request Reception**: The JSON-RPC request is parsed via the `rmcp` stdin/stdout stdio transport handler in [src/mcp.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/mcp.rs).
2. **Coordinated Dispatch**: The [MemoryCoordinator](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/coordinator.rs) routes the parameters to the requested layers (e.g. searching entities, querying embeddings, logging steps).
3. **Local Embedding Generation**: For semantic/vector search operations, the text is fed to `fastembed` locally to produce a 384-dimensional vector (`all-MiniLM-L6-v2`).
4. **Ranked Matching**: Matches are evaluated using the `Ranker` model in [src/search/ranker.rs](file:///home/aswin/programming/vscode/myProjects/ai_agent_tools/memory_rs/src/search/ranker.rs), which integrates cosine similarity, temporal decay, and importance factors.
5. **JSON-RPC Response**: Output is formatted as a standardized MCP tool execution result and sent back to the stdout stream.
