# Engine Features

The `memory_rs` server is packed with modern architectural and performance upgrades over standard memory servers. Below is an in-depth breakdown of these features.

---

## 1. High Performance & Low Overhead (Rust Native)
* **Compile-Time Safety**: Written in pure Rust with strict ownership constraints, guaranteeing memory safety without a garbage collector.
* **Fast Startup**: Sub-millisecond startup times compared to Node.js virtual machines.
* **Minimal Resource Footprint**: Extremely low RAM usage (less than 10MB idle, excluding loaded model vectors), making it highly suitable for running locally in background environments.
* **Connection Pooling**: Utilizes SQLite's local performance and simple locking mechanisms, ensuring that concurrency is managed cleanly by `parking_lot::Mutex` thread locks.

---

## 2. Dynamic Memory Ranking and Decay
The memory engine uses a multi-factor ranking model in [src/search/ranker.rs](../src/search/ranker.rs) to rank search results. Rather than just returning items matching keywords, it implements cognitive models that evaluate:

1. **Semantic Relevance**: How similar is the query to the memory's content using cosine similarity?
2. **Temporal Recency**: How long ago was the memory captured? Old memories decay unless they are highly important.
3. **Importance Multiplier**: A manual multiplier assigned to critical facts (e.g. user name, API endpoints).
4. **Historical Success Rate**: Prioritizes reflections from successful agent loops while deprioritizing strategies that failed.

### The Scoring Equation
$$Score = (\alpha \cdot Similarity) + (\beta \cdot Recency) + (\gamma \cdot Importance) + (\delta \cdot SuccessRate)$$

Where:
* **Similarity**: Cosine similarity value $[0.0, 1.0]$.
* **Recency**: Calculated using exponential decay: $Recency = e^{-\lambda \cdot t}$, where $t$ is the elapsed hours, and $\lambda = 0.01$ (decay factor).
* **Importance**: User-defined fact importance score $[1.0, 5.0]$.
* **SuccessRate**: Ratio of successful tasks associated with this memory $[0.0, 1.0]$.

---

## 3. Pure-Rust Vector Search (Local Embeddings)
* **Zero Cloud Dependencies**: The server does not send texts or queries to external services like OpenAI. All embedding generation is done locally on your CPU using ONNX Runtime.
* **FastEmbed Crate Integration**: Uses `fastembed` for fast model inference.
* **Model Configuration**:
  * Default Model: `all-MiniLM-L6-v2` (384-dimensional dense vectors).
  * Backend: ONNX Runtime (using pure Rust-tls bindings `hf-hub-rustls-tls` to avoid system OpenSSL dependencies).
* **SQLite Embedding Storage**: Embedded vectors are saved directly inside SQLite as binary blobs, avoiding the complexity of a separate vector database.

---

## 4. Normalized SQLite Architecture
The TypeScript reference memory server stores data in a flat JSON file on disk, which requires reading, writing, and parsing the entire graph for every single tool call. `memory_rs` moves away from this slow design by using a normalized SQLite structure:
* **Incremental Writes**: Only changed rows, nodes, or edges are written to disk.
* **Transaction Safety**: All modifications use SQLite's transactional guarantees, preventing data corruption during sudden shutdowns.
* **Native Indexes**: Fast lookups on entity names, edge endpoints, and signature IDs.

---

## 5. Complete Model Context Protocol (MCP) Compliance
The server fully implements the standard Model Context Protocol Stdio transport. It registers 9 compatible tools that allow any MCP client (like Claude Desktop, Cursor, or OpenZ) to interact with it out-of-the-box:

| Tool Name | Parameters | Description |
| :--- | :--- | :--- |
| `create_entities` | `entities: Vec<Entity>` | Create multiple new entities in the graph. |
| `create_relations` | `relations: Vec<Relation>` | Create multiple directed relations between entities. |
| `add_observations` | `observations: Vec<Observation>` | Append observation texts to existing entities. |
| `delete_entities` | `entityNames: Vec<String>` | Delete multiple entities and all associated relations. |
| `delete_observations` | `deletions: Vec<ObservationDeletion>` | Delete specific observation lines from an entity. |
| `delete_relations` | `relations: Vec<Relation>` | Remove specific directed relations from the graph. |
| `read_graph` | None (Empty) | Retrieve the entire active entity-relationship knowledge graph. |
| `search_nodes` | `query: String` | Find entities and relations matching a query pattern. |
| `open_nodes` | `names: Vec<String>` | Retreive the full observation records of specific nodes by name. |
