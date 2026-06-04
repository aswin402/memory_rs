# Engine Features

`openmemory_rs` is a complete, unified cognitive engine for AI agents. It implements advanced context storage, AST code structure mapping, mathematical recency decay, and episodic learning loops.

---

## 1. Modular Memory Layers (Multi-Tier Storage)
* **Working Layer**: RwLock RAM cache for transient variables, context loops, and subsecond execution steps.
* **Graph Layer**: Directed entity-relationship graph with observative JSON arrays, conforming with the standard Model Context Protocol.
* **Semantic Layer**: Purely local high-dimensional vector search utilizing cosine similarity matches.
* **Episodic Layer**: Persists step-by-step task logs and reflections (what failed/worked and why), allowing the agent to check its history before trying a workflow again.
* **Code & AST Layer**: Indexes files, declarations, enums, functions, and structs. Tracks caller-callee relations and codebase refactor statistics.
* **Shared Layer**: Dynamically syncs key-value entries across multiple subagents running in parallel.

---

## 2. Multi-Factor Context Ranking & Decay
Standard retrieval engines only scan keyword similarities. `openmemory_rs` calculates a composite relevance score for semantic context lookups, dynamically prioritizing recent or highly important facts while letting stale, minor information decay over time.

### The Scoring Equation
$$Score = (\alpha \cdot Similarity) + (\beta \cdot Recency) + (\gamma \cdot Importance) + (\delta \cdot SuccessRate)$$

Where:
* **Similarity**: Cosine vector distance score between query and fact embeddings $[0.0, 1.0]$.
* **Recency**: Calculated using exponential time-based decay: $Recency = e^{-\lambda \cdot t}$, where $t$ is the elapsed hours since creation, and $\lambda = 0.01$ (decay coefficient).
* **Importance**: Manual weighting factor assigned to critical configs/rules $[1.0, 5.0]$.
* **SuccessRate**: Historical success rate of the agent tasks associated with this memory $[0.0, 1.0]$, prioritizing strategies that succeeded and deprioritizing those that failed.

---

## 3. Pure-Rust Vector Search
* **100% Local**: No external API dependencies. All embedding generation runs locally on your CPU via ONNX Runtime.
* **FastEmbed**: Driven by the `fastembed` crate utilizing `all-MiniLM-L6-v2` (producing 384-dimensional dense vectors).
* **No OpenSSL Dependency**: Locked to rustls/TLS packages (`hf-hub-rustls-tls`) to ensure trouble-free static builds on Linux, macOS, and Windows.

---

## 4. AST Code Parsing & Call Graphs
`openmemory_rs` replaces generic codebase grepping with a local structural code analyzer.
* **Symbol Extraction**: Extracts definitions, Structs, Functions, Classes, Impls, and Enums from `.rs`, `.py`, `.js`, `.ts`, and `.go` source files.
* **Call Mapping**: Correlates element declarations to construct call hierarchy trees.
* **Impact Analysis**: Allows agents to evaluate what files or methods are affected if a specific struct or function signature is changed.
* **Repository Evolution**: Logs commit summaries, authors, versions, and tags files containing bugs to pinpoint error-prone components in the repo.

---

## 5. Tool & Model Performance Tracker
AI agents often use the wrong LLMs or tools for specific tasks. `openmemory_rs` tracks tool performances globally to optimize routing decisions.
* **Success/Failure Stats**: Counts runs for each tool/model combination.
* **Latency Logs**: Tracks average execution latencies.
* **Smart Routing**: Allows AI agents to query `query_tool_performance` to automatically select the most successful or fastest tool for their active task.
