# System Architecture

`openmemory_rs` implements a unified, multi-tiered cognitive memory engine designed specifically for AI agent networks like **OpenZ**. Unlike traditional single-layer database setups, `openmemory_rs` maps data across **6 cognitive layers**, combining session variables, explicit entity relationships, vector embeddings, episodic reflection tracks, AST dependencies, and shared multi-agent state boards.

An interactive dark-themed visualization is available in: [docs/architecture.html](architecture.html).

---

## The 6 Cognitive Memory Layers

```
                       ┌────────────────────────────────────────────────────────┐
                       │               Memory Coordinator                       │
                       └───────────────────────────┬────────────────────────────┘
                                                   │
         ┌──────────────────┬──────────────────────┼──────────────────────┬──────────────────┐
         ▼                  ▼                      ▼                      ▼                  ▼
┌────────────────┐ ┌────────────────┐     ┌────────────────┐     ┌────────────────┐ ┌────────────────┐
│ Working Memory │ │  Graph Memory  │     │Semantic Memory │     │Episodic Memory │ │Codebase Memory │
│ (Session RAM)  │ │(Entities/Edges)│     │(Vector Blobs)  │     │(Reflection Logs)││  (AST Graphs)  │
└────────────────┘ └────────────────┘     └────────────────┘     └────────────────┘ └────────────────┘
                                                   ▲                      ▲                  ▲
                                                   │                      │                  │
                                          ┌────────┴───────┐     ┌────────┴───────┐ ┌────────┴───────┐
                                          │Shared Workspace│     │Embedding Engine│ │Evolution Graph │
                                          │ (Team Board)   │     │(all-MiniLM-L6) │ │ (Version Logs) │
                                          └────────────────┘     └────────────────┘ └────────────────┘
```

### 1. Working Memory
* **File**: [src/layers/working.rs](../src/layers/working.rs)
* **Role**: A session-bound, volatile cache backed by concurrent `RwLock<HashMap<String, String>>`. It stores transaction IDs, active loop states, and runtime contexts, clearing automatically when the process restarts.

### 2. Graph Memory
* **File**: [src/layers/graph.rs](../src/layers/graph.rs)
* **Role**: Structured knowledge storage mapping nodes, relations, and observations. Out-of-the-box compatibility with standard MCP client calls (`read_graph`, `search_nodes`, `open_nodes`).

### 3. Semantic Memory
* **File**: [src/layers/semantic.rs](../src/layers/semantic.rs)
* **Role**: Local vector memory storing text snippets and high-dimension vector floats. Natural language queries are vectorized using the `fastembed` model (`all-MiniLM-L6-v2`) and matched using cosine similarity.

### 4. Episodic & Reflection Memory
* **File**: [src/layers/episodic.rs](../src/layers/episodic.rs)
* **Role**: Stores detailed logs of agent tasks, steps taken, errors, reflections (what worked/failed and why), and logs tool latencies and success counts. Allows agents to query their historical experience to avoid repeating past bugs or failing strategies.

### 5. Code & AST Memory
* **File**: [src/layers/codebase.rs](../src/layers/codebase.rs)
* **Role**: Parses workspace files into AST structural elements (functions, structs, classes, impls) and registers call chains and dependency maps. Also logs the repository's file version evolution and bug records.

### 6. Shared Team Memory Workspace
* **File**: [src/layers/shared.rs](../src/layers/shared.rs)
* **Role**: A synchronized key-value database allowing different agents (e.g. Research Agent and Coding Agent) to securely register and pull shared facts or context.

---

## Consolidated SQLite Schema

`openmemory_rs` persists all structured data inside a local, transaction-safe SQLite database file (`memory.db`), defining 9 normalized tables.

### Table Structure Mappings

#### 1. Graph Tables
* **`graph_nodes`**: Tracks entity profiles.
  ```sql
  CREATE TABLE IF NOT EXISTS graph_nodes (
      name TEXT PRIMARY KEY,
      entity_type TEXT NOT NULL,
      observations TEXT NOT NULL -- JSON list of observations
  );
  ```
* **`graph_edges`**: Tracks relational connections.
  ```sql
  CREATE TABLE IF NOT EXISTS graph_edges (
      from_name TEXT NOT NULL,
      to_name TEXT NOT NULL,
      relation_type TEXT NOT NULL,
      PRIMARY KEY (from_name, to_name, relation_type)
  );
  ```

#### 2. Vector Semantic Tables
* **`semantic_metadata`**: Matches text fragments to model vectors.
  ```sql
  CREATE TABLE IF NOT EXISTS semantic_metadata (
      node_id TEXT PRIMARY KEY,
      raw_text TEXT NOT NULL,
      embedding BLOB NOT NULL, -- Binary vector representation
      timestamp TEXT NOT NULL,
      importance REAL NOT NULL DEFAULT 1.0
  );
  ```

#### 3. Agent Experience & Tool Performance Tables
* **`episodic_logs`**: Tracks runtime steps and reflections.
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
* **`reflection_memory`**: Stores root-cause analysis and lessons learned.
  ```sql
  CREATE TABLE IF NOT EXISTS reflection_memory (
      id TEXT PRIMARY KEY,
      task_description TEXT NOT NULL,
      status TEXT NOT NULL, -- "Success" or "Failed"
      attempt_number INTEGER NOT NULL,
      steps_taken TEXT NOT NULL,
      error_encountered TEXT,
      root_cause TEXT,
      solution_applied TEXT,
      reflection TEXT NOT NULL,
      created_at TEXT NOT NULL
  );
  ```
* **`tool_performance`**: Aggregates latency and usage metrics for tools/models.
  ```sql
  CREATE TABLE IF NOT EXISTS tool_performance (
      tool_name TEXT NOT NULL,
      model_name TEXT NOT NULL,
      task_type TEXT NOT NULL,
      success_count INTEGER NOT NULL DEFAULT 0,
      failure_count INTEGER NOT NULL DEFAULT 0,
      average_latency REAL NOT NULL DEFAULT 0.0,
      last_used TEXT NOT NULL,
      PRIMARY KEY (tool_name, model_name, task_type)
  );
  ```

#### 4. AST Code & Version Tables
* **`code_elements`**: Maps AST code items.
  ```sql
  CREATE TABLE IF NOT EXISTS code_elements (
      element_id TEXT PRIMARY KEY,
      file_path TEXT NOT NULL,
      element_type TEXT NOT NULL, -- "Function", "Struct", "Class", etc.
      name TEXT NOT NULL,
      signature TEXT NOT NULL,
      ast_json TEXT, -- Optional JSON AST node data
      parent_id TEXT,
      start_line INTEGER NOT NULL,
      end_line INTEGER NOT NULL
  );
  ```
* **`code_calls`**: Maps calling stacks.
  ```sql
  CREATE TABLE IF NOT EXISTS code_calls (
      caller_id TEXT NOT NULL,
      callee_id TEXT NOT NULL,
      call_site TEXT,
      PRIMARY KEY (caller_id, callee_id)
  );
  ```
* **`repository_evolution`**: Monitors file modifications and bug history.
  ```sql
  CREATE TABLE IF NOT EXISTS repository_evolution (
      file_path TEXT NOT NULL,
      version TEXT NOT NULL,
      commit_hash TEXT,
      author TEXT,
      change_type TEXT NOT NULL,
      summary_of_changes TEXT NOT NULL,
      bug_introduced INTEGER NOT NULL DEFAULT 0,
      bug_fixed INTEGER NOT NULL DEFAULT 0,
      timestamp TEXT NOT NULL,
      PRIMARY KEY (file_path, version)
  );
  ```

#### 5. Shared Agent Workspace Tables
* **`shared_agent_memory`**: Allows state exchange between agent instances.
  ```sql
  CREATE TABLE IF NOT EXISTS shared_agent_memory (
      memory_key TEXT PRIMARY KEY,
      memory_value TEXT NOT NULL,
      source_agent TEXT NOT NULL,
      target_agents TEXT NOT NULL, -- JSON list of recipient IDs
      importance REAL NOT NULL DEFAULT 1.0,
      timestamp TEXT NOT NULL
  );
  ```
