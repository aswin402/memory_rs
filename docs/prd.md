# openmemory_rs — Product Requirements Document (PRD)

> **Version:** 1.0.0-draft  
> **Date:** 2026-06-26  
> **Author:** openmemory_rs Core Team  
> **Status:** Living Document — Active Development  
> **Crate Version:** 0.1.1  

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Product Vision](#2-product-vision)
3. [Target Audience](#3-target-audience)
4. [Problem Statement](#4-problem-statement)
5. [Goals & Success Metrics](#5-goals--success-metrics)
6. [Functional Requirements](#6-functional-requirements)
7. [Non-Functional Requirements](#7-non-functional-requirements)
8. [System Requirements](#8-system-requirements)
9. [Transport Protocols](#9-transport-protocols)
10. [Data Model](#10-data-model)
11. [User Stories](#11-user-stories)
12. [Acceptance Criteria](#12-acceptance-criteria)
13. [Risks & Mitigations](#13-risks--mitigations)
14. [Release Plan](#14-release-plan)

---

## 1. Executive Summary

**openmemory_rs** is a high-performance, unified cognitive memory engine for AI agent frameworks, written entirely in pure Rust (edition 2024). It serves as a local-first, zero-cloud-dependency memory backbone that gives AI agents persistent structured knowledge, semantic recall, episodic learning, codebase intelligence, and multi-agent collaboration — all within a single native binary.

The engine exposes **22 MCP tools** across **6 cognitive memory layers** via Stdio JSON-RPC (MCP standard) and Tonic gRPC transport. All vector embeddings are generated locally using fastembed with the `all-MiniLM-L6-v2` model (384-dimension) running on ONNX Runtime, ensuring complete data privacy and offline operation. Persistent storage is handled by a single SQLite database file (`memory.db`) with 13 relational tables, and vector similarity search is powered by an HNSW index via `small-world-rs`.

### Key Differentiators

| Attribute | openmemory_rs | Typical Alternatives |
|:---|:---|:---|
| **Language** | Pure Rust | TypeScript / Python / Go |
| **Startup Time** | Sub-millisecond | 200ms – 2s |
| **RAM Footprint** | < 10 MB baseline | 50 – 200 MB |
| **Cloud Dependency** | None (100% offline) | Cloud embeddings / vector DBs |
| **Embedding Model** | Local ONNX (all-MiniLM-L6-v2) | OpenAI / Cohere API calls |
| **AST Parsing** | tree-sitter (Rust, Python, JS, TS, TSX) | None or limited |
| **Episodic Reflection** | Built-in with root-cause analysis | Not available |
| **Multi-Agent Sync** | Built-in shared memory board | Not available |
| **Database Branching** | Copy-on-write SQLite branches | Not available |

---

## 2. Product Vision

### 2.1 Mission Statement

> To provide every AI agent — regardless of framework, model provider, or deployment environment — with a unified, high-performance, privacy-first cognitive memory system that enables true learning, reasoning, and collaboration across sessions.

### 2.2 One-Year Vision (2026–2027)

| Goal | Description | Success Signal |
|:---|:---|:---|
| **Production-Ready Core** | All 22 MCP tools battle-tested, with comprehensive test coverage (>85%) and zero critical bugs in the last 30 days | Adopted by ≥ 3 major agent frameworks |
| **Performance Leadership** | Sub-5ms p99 latency for all graph and episodic operations; sub-50ms for semantic search across 100K vectors | Published benchmark suite |
| **Ecosystem Integration** | First-class integration guides for Claude Desktop, OpenZ, LangChain, CrewAI, and AutoGen | ≥ 500 monthly downloads on crates.io |
| **Developer Experience** | Complete documentation, CLI diagnostic tool, and interactive architecture explorer | NPS score ≥ 40 among early adopters |
| **Plugin Architecture** | Extensible layer system allowing custom memory layers via dynamic dispatch traits | ≥ 2 community-contributed layers |

### 2.3 Three-Year Vision (2026–2029)

| Goal | Description | Success Signal |
|:---|:---|:---|
| **Industry Standard** | Become the de-facto local memory backend for MCP-compatible agent tools | ≥ 10,000 monthly active installations |
| **Distributed Memory** | Optional peer-to-peer sync for team memories across networked agents without centralized servers | Working multi-node demo |
| **Adaptive Intelligence** | Self-optimizing memory that automatically prunes, consolidates, and prioritizes memories based on usage patterns | Measurable improvement in agent task success rates |
| **Multi-Modal Memory** | Support for image embeddings, audio transcription memories, and structured diagram storage | ≥ 3 modalities supported |
| **Enterprise Features** | Encrypted-at-rest databases, RBAC for shared memories, audit logging, and compliance exports | SOC 2 readiness checklist complete |

---

## 3. Target Audience

### 3.1 Primary Audience — AI Agent Developers

| Persona | Profile |
|:---|:---|
| **Name** | **Alex — The Agent Builder** |
| **Role** | AI/ML Engineer building autonomous agent systems |
| **Experience** | 3–8 years in software engineering, 1–3 years in AI/agent development |
| **Pain Points** | Agents lose context between sessions; no structured way to store tool performance data; embedding APIs are expensive and slow; existing memory solutions are fragmented (separate tools for graph, vectors, episodes) |
| **Goal** | A single, fast, local memory backend that "just works" with their MCP client |
| **Tools** | Claude Desktop, VS Code + Copilot, custom agent orchestrators, Rust/Python toolchains |

### 3.2 Secondary Audience — DevTool & Framework Authors

| Persona | Profile |
|:---|:---|
| **Name** | **Sam — The Framework Maintainer** |
| **Role** | Open-source maintainer of an agent framework (e.g., LangChain plugin, CrewAI extension) |
| **Experience** | 5–12 years, deep systems knowledge |
| **Pain Points** | Needs a memory backend with zero external dependencies that can be bundled; must support both stdio and gRPC transports for different deployment models; needs a stable, well-documented API surface |
| **Goal** | Embed openmemory_rs as the default memory layer in their framework |

### 3.3 Tertiary Audience — Researchers & Power Users

| Persona | Profile |
|:---|:---|
| **Name** | **Dr. Priya — The AI Researcher** |
| **Role** | PhD researcher studying agent memory architectures, meta-learning, and reflection |
| **Experience** | Academic background in cognitive science / AI |
| **Pain Points** | Needs access to raw memory data for analysis; wants to experiment with different decay functions and scoring algorithms; needs reproducible environments with database branching |
| **Goal** | Use openmemory_rs as an instrumented testbed for memory architecture research |

---

## 4. Problem Statement

### 4.1 Problem Inventory

| # | Problem | Severity | Impact Area |
|:---|:---|:---:|:---|
| **P1** | **Context Amnesia** — AI agents lose all accumulated knowledge between sessions, forcing repeated discovery of the same facts, preferences, and patterns | **Critical** | Productivity, UX, cost |
| **P2** | **Fragmented Memory Tooling** — Developers must integrate 3–5 separate tools (graph DB, vector store, key-value cache, log store, code indexer) to achieve comprehensive agent memory, each with different APIs, data formats, and failure modes | **Critical** | Development velocity, reliability |
| **P3** | **Cloud Dependency & Privacy Leakage** — Most embedding and vector search solutions require sending all memory content to external APIs (OpenAI, Pinecone, Weaviate), creating privacy violations for sensitive codebases, proprietary business logic, and PII | **Critical** | Privacy, compliance, security |
| **P4** | **No Episodic Learning** — Agents cannot learn from their own execution history; they repeat the same mistakes, use suboptimal tools, and cannot reason about what worked vs. what failed in previous attempts | **High** | Agent intelligence, efficiency |
| **P5** | **No Codebase Awareness** — Agents operating on codebases lack structural understanding of the code; they cannot query function signatures, call hierarchies, or track how files have evolved over time | **High** | Code quality, developer trust |
| **P6** | **Multi-Agent Coordination Gaps** — When multiple agents or subagents work in parallel (e.g., research + implementation + review), there is no structured mechanism for sharing discovered facts, variables, or decisions between them | **High** | Multi-agent workflows |
| **P7** | **Performance Bottlenecks** — TypeScript/Python memory servers impose 200ms+ startup times, 50–100MB RAM overhead, and GC pauses that disrupt real-time agent interactions | **Medium** | Latency, resource usage |
| **P8** | **Unsafe Concurrent Access** — Most file-based memory solutions (flat JSON files, unprotected SQLite) corrupt data under concurrent multi-agent writes without proper locking | **Medium** | Data integrity |
| **P9** | **No Experimental Isolation** — Agents performing speculative tasks (e.g., "try approach A, then try approach B") cannot create isolated memory sandboxes to prevent cross-contamination of experimental data | **Medium** | Reliability, experimentation |

---

## 5. Goals & Success Metrics

### 5.1 Performance KPIs

| Metric | Target | Measurement Method |
|:---|:---|:---|
| Cold startup time (first run, model download cached) | < 2 seconds | `time` command on release binary |
| Warm startup time (subsequent runs) | < 500 ms | `time` command on release binary |
| `create_entities` latency (batch of 10) | < 5 ms p99 | Instrumented benchmark |
| `search_nodes` latency (1,000 nodes) | < 3 ms p99 | Instrumented benchmark |
| Semantic search latency (10K vectors) | < 30 ms p99 | Instrumented benchmark |
| Semantic search latency (100K vectors) | < 100 ms p99 | Instrumented benchmark |
| Embedding generation (single sentence) | < 15 ms p50 | fastembed benchmark |
| `index_codebase` throughput | > 500 files/sec | Directory scan benchmark |
| Baseline RAM footprint | < 10 MB | `htop` / `smem` measurement |
| Peak RAM under load (10K nodes, 10K vectors) | < 150 MB | Load test measurement |

### 5.2 Reliability KPIs

| Metric | Target | Measurement Method |
|:---|:---|:---|
| Zero data loss on normal shutdown | 100% | SQLite WAL + integration tests |
| Crash recovery (power loss simulation) | Zero corruption | SQLite journal mode tests |
| Concurrent write safety | No panics under 50 parallel tool calls | Tokio-based stress test |
| Branch create/commit/rollback cycle | Zero data leakage between branches | Isolation integration tests |
| Database file integrity after 10K operations | Zero corruption | `PRAGMA integrity_check` |

### 5.3 Adoption KPIs

| Metric | 6-Month Target | 12-Month Target |
|:---|:---|:---|
| crates.io monthly downloads | 100 | 500 |
| GitHub stars | 200 | 1,000 |
| Framework integrations | 2 | 5 |
| Active contributors | 5 | 15 |
| Documentation completeness | 80% | 95% |

### 5.4 Developer Experience KPIs

| Metric | Target | Measurement Method |
|:---|:---|:---|
| Time to first working integration | < 10 minutes | User testing |
| Lines of config required | ≤ 5 (MCP client JSON) | Config file audit |
| Error message actionability | 90% of errors include fix suggestion | Error catalog review |
| Build time (release, clean) | < 3 minutes on M1/Ryzen 7 | CI timing |
| Binary size (release, stripped) | < 30 MB | `ls -la` check |

---

## 6. Functional Requirements

All 22 MCP tools are organized by cognitive layer. Each tool specification includes its complete input schema, output format, error handling behavior, and priority classification.

**Priority Legend:**
- **P0** — Must-have for v0.1 launch; blocking for core functionality
- **P1** — Must-have for v0.5; required for production readiness
- **P2** — Nice-to-have; planned for v1.0

---

### 6.1 Graph Memory Layer (9 Tools)

The Graph Memory layer stores entities (nodes) and directed relations (edges) in a knowledge graph backed by `graph_nodes` and `graph_edges` SQLite tables. It supports CRUD operations, keyword search, and targeted node retrieval.

---

#### 6.1.1 `create_entities`

| Field | Detail |
|:---|:---|
| **Description** | Create multiple new entities (nodes) in the knowledge graph. Duplicate entity names are silently skipped (idempotent). |
| **Priority** | P0 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `entities` | `Array<Entity>` | ✅ | Non-empty array | List of entities to create |
| `entities[].name` | `String` | ✅ | Non-empty, unique per entity | Human-readable entity name (acts as primary key) |
| `entities[].entityType` | `String` | ✅ | Non-empty | Category label (e.g., "Person", "Project", "Concept") |
| `entities[].observations` | `Array<String>` | ✅ | Can be empty array | Initial list of observed facts about this entity |

**Output Format:**
```json
[
  { "name": "Rust", "entityType": "Language", "observations": ["Systems programming language"] }
]
```
Returns only the entities that were **newly created** (excludes already-existing names).

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Empty `entities` array | Returns empty array `[]` (no-op) |
| SQLite write failure | `McpError::internal_error` with rusqlite error message |
| Serialization failure | `McpError::internal_error` with serde_json error |

---

#### 6.1.2 `create_relations`

| Field | Detail |
|:---|:---|
| **Description** | Create multiple directed relations (edges) between existing entities. Relations should use active-voice labels (e.g., "uses", "implements", "depends_on"). Duplicates (same from/to/type) are silently skipped. |
| **Priority** | P0 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `relations` | `Array<Relation>` | ✅ | Non-empty array | List of relations to create |
| `relations[].from` | `String` | ✅ | Must match existing entity name | Source entity name |
| `relations[].to` | `String` | ✅ | Must match existing entity name | Target entity name |
| `relations[].relationType` | `String` | ✅ | Active-voice verb phrase | Edge label describing the relationship |

**Output Format:**
```json
[
  { "from": "openmemory_rs", "to": "Rust", "relationType": "written_in" }
]
```
Returns only newly created relations.

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Non-existent `from`/`to` entity | Relation created regardless (no foreign key enforcement at present) |
| Duplicate relation | Silently skipped |
| SQLite failure | `McpError::internal_error` |

---

#### 6.1.3 `add_observations`

| Field | Detail |
|:---|:---|
| **Description** | Append new observation strings to existing entities. If an entity does not exist, an error is returned for that entry. Observations are stored as a JSON array in the `observations` column. |
| **Priority** | P0 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `observations` | `Array<AddObservationsInput>` | ✅ | Non-empty | List of observation additions |
| `observations[].entityName` | `String` | ✅ | Must match existing entity | Target entity name |
| `observations[].contents` | `Array<String>` | ✅ | Non-empty | New observations to append |

**Output Format:**
```json
[
  { "entityName": "Rust", "addedObservations": ["Memory safe", "Zero-cost abstractions"] }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Entity not found | `McpError::internal_error("Entity not found: <name>")` |
| JSON parse failure on stored observations | `McpError::internal_error` |

---

#### 6.1.4 `delete_entities`

| Field | Detail |
|:---|:---|
| **Description** | Delete multiple entities and all their associated relations (edges where the entity is either `from` or `to`) from the knowledge graph. |
| **Priority** | P0 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `entityNames` | `Array<String>` | ✅ | Non-empty | Names of entities to delete |

**Output Format:**
```
"Entities deleted successfully"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Non-existent entity name | No-op for that name (idempotent) |
| SQLite failure | `McpError::internal_error` |

---

#### 6.1.5 `delete_observations`

| Field | Detail |
|:---|:---|
| **Description** | Remove specific observation strings from existing entities. Only exact string matches are removed. |
| **Priority** | P1 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `deletions` | `Array<DeleteObservationsInput>` | ✅ | Non-empty | List of observation deletions |
| `deletions[].entityName` | `String` | ✅ | Must match existing entity | Target entity name |
| `deletions[].observations` | `Array<String>` | ✅ | Non-empty | Exact observation strings to remove |

**Output Format:**
```
"Observations deleted successfully"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Entity not found | `McpError::internal_error` |
| Observation string not found in entity | Silently skipped |

---

#### 6.1.6 `delete_relations`

| Field | Detail |
|:---|:---|
| **Description** | Delete multiple specific relations from the knowledge graph. Matching is by exact `(from, to, relationType)` tuple. |
| **Priority** | P1 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `relations` | `Array<Relation>` | ✅ | Non-empty | List of relations to delete |
| `relations[].from` | `String` | ✅ | — | Source entity name |
| `relations[].to` | `String` | ✅ | — | Target entity name |
| `relations[].relationType` | `String` | ✅ | — | Edge label |

**Output Format:**
```
"Relations deleted successfully"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Relation not found | No-op (idempotent) |
| SQLite failure | `McpError::internal_error` |

---

#### 6.1.7 `read_graph`

| Field | Detail |
|:---|:---|
| **Description** | Retrieve the entire knowledge graph including all entities (with their observations) and all relations. Intended for full-graph inspection and visualization. |
| **Priority** | P0 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `dummy` | `Option<bool>` | ❌ | — | Unused placeholder for schema compliance |

**Output Format:**
```json
{
  "entities": [
    { "name": "Rust", "entityType": "Language", "observations": ["Systems programming"] }
  ],
  "relations": [
    { "from": "openmemory_rs", "to": "Rust", "relationType": "written_in" }
  ]
}
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Empty graph | Returns `{ "entities": [], "relations": [] }` |
| SQLite read failure | `McpError::internal_error` |

---

#### 6.1.8 `search_nodes`

| Field | Detail |
|:---|:---|
| **Description** | Search for entities in the knowledge graph by keyword matching against entity names, types, and observation text. Returns matching entities with their full observation lists. |
| **Priority** | P0 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `query` | `String` | ✅ | Non-empty | Search keyword or phrase |

**Output Format:**
```json
[
  { "name": "Rust", "entityType": "Language", "observations": ["Systems programming", "Memory safe"] }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No matches | Returns empty array `[]` |
| Empty query string | Returns all entities |
| SQLite failure | `McpError::internal_error` |

---

#### 6.1.9 `open_nodes`

| Field | Detail |
|:---|:---|
| **Description** | Retrieve full entity records for specific named nodes. Used when the caller knows exactly which entities to inspect. |
| **Priority** | P0 |
| **Layer** | Graph Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `names` | `Array<String>` | ✅ | Non-empty | Exact entity names to retrieve |

**Output Format:**
```json
[
  { "name": "Rust", "entityType": "Language", "observations": ["Systems programming"] }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Name not found | Excluded from results (no error) |
| SQLite failure | `McpError::internal_error` |

---

### 6.2 Codebase Memory Layer (4 Tools)

The Codebase Memory layer provides AST-based static analysis, code element indexing, call graph construction, and repository evolution tracking. It parses source files using tree-sitter grammars for Rust, Python, JavaScript, TypeScript, and TSX.

---

#### 6.2.1 `index_codebase`

| Field | Detail |
|:---|:---|
| **Description** | Recursively scan a directory, parse source files using tree-sitter, and index all structural code elements (functions, structs, enums, traits, impl blocks, classes, methods) along with their call relationships into the `code_elements` and `code_calls` tables. Defaults to current directory (`"."`) if no path is provided. |
| **Priority** | P0 |
| **Layer** | Codebase Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `path` | `Option<String>` | ❌ | Valid filesystem path | Root directory to scan. Defaults to `"."` |

**Supported Languages & Grammar Elements:**

| Language | Extensions | Parsed Elements |
|:---|:---|:---|
| Rust | `.rs` | `function_item`, `struct_item`, `enum_item`, `trait_item`, `impl_item` |
| Python | `.py` | `function_definition`, `class_definition` |
| JavaScript | `.js` | `function_declaration`, `class_declaration`, `arrow_function`, `method_definition` |
| TypeScript | `.ts`, `.tsx` | `function_declaration`, `class_declaration`, `arrow_function`, `method_definition` |

**Output Format:**
```
"Successfully indexed 142 source files under \"/home/user/project\""
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Invalid path | `McpError::internal_error` with IO error |
| Parse failure for a file | File skipped, other files continue |
| No parseable files found | Returns count of 0 |

---

#### 6.2.2 `query_code_graph`

| Field | Detail |
|:---|:---|
| **Description** | Query indexed code elements (functions, structs, impl blocks) and their call patterns. Filter by file path and/or keyword query against element names and signatures. |
| **Priority** | P0 |
| **Layer** | Codebase Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `filePath` | `Option<String>` | ❌ | — | Filter results to elements in this file path |
| `query` | `Option<String>` | ❌ | — | Keyword filter against element names and signatures |

**Output Format:**
```json
[
  {
    "id": "uuid-here",
    "file_path": "src/layers/graph.rs",
    "element_type": "Function",
    "name": "create_entities",
    "signature": "pub fn create_entities(&self, entities: Vec<Entity>) -> Result<Vec<Entity>>",
    "parent_id": null,
    "start_line": 53,
    "end_line": 73
  }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No matching elements | Returns empty array `[]` |
| SQLite failure | `McpError::internal_error` |

---

#### 6.2.3 `log_repository_evolution`

| Field | Detail |
|:---|:---|
| **Description** | Record a file-level change event in the repository evolution history. Tracks versions, commit hashes, authors, change types, and bug introduction/fix signals. |
| **Priority** | P1 |
| **Layer** | Codebase Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `filePath` | `String` | ✅ | Non-empty | Path of the changed file |
| `version` | `String` | ✅ | Non-empty | Version tag or identifier |
| `commitHash` | `Option<String>` | ❌ | — | Git commit hash |
| `author` | `Option<String>` | ❌ | — | Author of the change |
| `changeType` | `String` | ✅ | `"Added"`, `"Modified"`, `"Deleted"` | Nature of the file change |
| `summaryOfChanges` | `String` | ✅ | Non-empty | Human-readable description of what changed |
| `bugIntroduced` | `bool` | ✅ | — | Whether this change introduced a known bug |
| `bugFixed` | `bool` | ✅ | — | Whether this change fixed a known bug |

**Output Format:**
```
"Repository evolution stage logged"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Duplicate `(filePath, version)` key | `INSERT OR REPLACE` behavior — updates existing record |
| SQLite failure | `McpError::internal_error` |

---

#### 6.2.4 `query_repository_evolution`

| Field | Detail |
|:---|:---|
| **Description** | Retrieve the evolution history of files in the repository. Optionally filter by file path. Returns chronological change records with bug metrics. |
| **Priority** | P1 |
| **Layer** | Codebase Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `filePath` | `Option<String>` | ❌ | — | Filter by specific file path. Empty string returns all records |

**Output Format:**
```json
[
  {
    "file_path": "src/mcp.rs",
    "version": "0.1.1",
    "commit_hash": "abc123",
    "author": "aswin",
    "change_type": "Modified",
    "summary_of_changes": "Added database branching tools",
    "bug_introduced": false,
    "bug_fixed": false,
    "timestamp": "2026-06-26T09:00:00Z"
  }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No evolution records | Returns empty array `[]` |
| SQLite failure | `McpError::internal_error` |

---

### 6.3 Episodic Memory Layer (5 Tools)

The Episodic Memory layer records execution episodes, structured reflections, and tool/model performance metrics. It enables agents to learn from past successes and failures.

---

#### 6.3.1 `log_execution_episode`

| Field | Detail |
|:---|:---|
| **Description** | Record a complete execution episode including task description, execution status, step-by-step log, optional error message, and optional reflection. Auto-generates UUID and timestamp if not provided. |
| **Priority** | P0 |
| **Layer** | Episodic Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `id` | `Option<String>` | ❌ | UUID format recommended | Episode identifier. Auto-generated UUID v4 if omitted |
| `taskDescription` | `String` | ✅ | Non-empty | What the agent was trying to accomplish |
| `executionStatus` | `String` | ✅ | `"Success"`, `"Failed"`, `"Partial"` | Outcome of the episode |
| `stepsTaken` | `String` | ✅ | Non-empty | Detailed log of steps performed |
| `errorMessage` | `Option<String>` | ❌ | — | Error details if the episode failed |
| `reflection` | `Option<String>` | ❌ | — | Post-hoc analysis of the episode |

**Output Format:**
```
"Episode logged successfully"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Duplicate ID | `INSERT OR REPLACE` — overwrites existing episode |
| SQLite failure | `McpError::internal_error` |

---

#### 6.3.2 `log_reflection`

| Field | Detail |
|:---|:---|
| **Description** | Store a structured reflection memory with root-cause analysis, solution tracking, and attempt numbering. Designed for iterative problem-solving where agents build on previous attempts. |
| **Priority** | P0 |
| **Layer** | Episodic Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `taskDescription` | `String` | ✅ | Non-empty | Task being reflected upon |
| `status` | `String` | ✅ | `"Success"` or `"Failed"` | Outcome of this attempt |
| `attemptNumber` | `i64` | ✅ | ≥ 1 | Which attempt this represents |
| `stepsTaken` | `String` | ✅ | Non-empty | Steps taken in this attempt |
| `errorEncountered` | `Option<String>` | ❌ | — | Error description if failed |
| `rootCause` | `Option<String>` | ❌ | — | Diagnosed root cause of the error |
| `solutionApplied` | `Option<String>` | ❌ | — | Fix or workaround applied |
| `reflection` | `String` | ✅ | Non-empty | Synthesized learning from this attempt |

**Output Format:**
```
"Reflection logged successfully"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| SQLite failure | `McpError::internal_error` |

---

#### 6.3.3 `retrieve_episodic_reflections`

| Field | Detail |
|:---|:---|
| **Description** | Query stored reflections to guide current task execution. Returns reflections matching a keyword query, or all reflections if no query is provided. |
| **Priority** | P0 |
| **Layer** | Episodic Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `query` | `Option<String>` | ❌ | — | Keyword to search in task descriptions. Empty/null returns all reflections |

**Output Format:**
```json
[
  {
    "id": "uuid",
    "task_description": "Deploy to production",
    "status": "Failed",
    "attempt_number": 1,
    "steps_taken": "...",
    "error_encountered": "Connection timeout",
    "root_cause": "Firewall rule blocking port 443",
    "solution_applied": "Added egress rule for HTTPS",
    "reflection": "Always verify network policies before deployment",
    "created_at": "2026-06-26T09:00:00Z"
  }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No matching reflections | Returns empty array `[]` |
| SQLite failure | `McpError::internal_error` |

---

#### 6.3.4 `record_tool_performance`

| Field | Detail |
|:---|:---|
| **Description** | Record performance metrics (success/failure counts, average latency) for a specific tool + model + task type combination. Uses `INSERT OR REPLACE` keyed on `(toolName, modelName, taskType)` — newer records overwrite older ones. |
| **Priority** | P1 |
| **Layer** | Episodic Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `toolName` | `String` | ✅ | Non-empty | Name of the tool (e.g., "grep_search", "write_to_file") |
| `modelName` | `String` | ✅ | Non-empty | Model used (e.g., "gemini-2.5-pro", "claude-sonnet-4") |
| `taskType` | `String` | ✅ | Non-empty | Category of task (e.g., "code_generation", "debugging") |
| `successCount` | `i64` | ✅ | ≥ 0 | Number of successful invocations |
| `failureCount` | `i64` | ✅ | ≥ 0 | Number of failed invocations |
| `averageLatency` | `f64` | ✅ | ≥ 0.0 | Average latency in milliseconds |

**Output Format:**
```
"Tool performance metrics recorded"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Duplicate key | `INSERT OR REPLACE` — updates existing record |
| SQLite failure | `McpError::internal_error` |

---

#### 6.3.5 `query_tool_performance`

| Field | Detail |
|:---|:---|
| **Description** | Query tool performance logs filtered by task type. Returns all tool/model combinations that have been recorded for the given task type, enabling agents to select optimal tools and models based on historical performance. |
| **Priority** | P1 |
| **Layer** | Episodic Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `taskType` | `String` | ✅ | Non-empty | Task type to query performance records for |

**Output Format:**
```json
[
  {
    "tool_name": "grep_search",
    "model_name": "gemini-2.5-pro",
    "task_type": "code_search",
    "success_count": 47,
    "failure_count": 3,
    "average_latency": 120.5,
    "last_used": "2026-06-26T09:00:00Z"
  }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No records for task type | Returns empty array `[]` |
| SQLite failure | `McpError::internal_error` |

---

### 6.4 Shared Memory Layer (2 Tools)

The Shared Memory layer enables multi-agent collaboration by providing a key-value store with source attribution, target routing, and importance scoring.

---

#### 6.4.1 `store_shared_team_memory`

| Field | Detail |
|:---|:---|
| **Description** | Store a key-value memory item accessible to specified target agents. The memory includes source attribution, target routing (specific agent IDs or wildcard `"*"`), and an importance score for prioritization. |
| **Priority** | P1 |
| **Layer** | Shared Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `key` | `String` | ✅ | Non-empty, unique | Memory key (acts as primary key; overwrites on duplicate) |
| `value` | `String` | ✅ | Non-empty | Memory content (arbitrary text/JSON) |
| `sourceAgent` | `String` | ✅ | Non-empty | ID of the agent storing this memory |
| `targetAgents` | `Array<String>` | ✅ | Non-empty | Agent IDs that should receive this memory. Use `["*"]` for broadcast |
| `importance` | `Option<f64>` | ❌ | 0.0 – 10.0 | Priority score. Defaults to `1.0` |

**Output Format:**
```
"Shared team memory stored successfully"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Duplicate key | `INSERT OR REPLACE` — overwrites existing |
| SQLite failure | `McpError::internal_error` |

---

#### 6.4.2 `retrieve_shared_team_memory`

| Field | Detail |
|:---|:---|
| **Description** | Retrieve shared memories targeted at a specific agent ID. Returns memories where: (a) the agent is in `targetAgents`, (b) `targetAgents` contains `"*"`, or (c) the agent is the `sourceAgent`. If no agent ID is provided, returns all shared memories. |
| **Priority** | P1 |
| **Layer** | Shared Memory |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `agentId` | `Option<String>` | ❌ | — | Agent ID to filter memories for. Empty/null returns all |

**Output Format:**
```json
[
  {
    "key": "project_status",
    "value": "Phase 2 complete, starting phase 3",
    "source_agent": "planner-agent",
    "target_agents": ["implementer-agent", "reviewer-agent"],
    "importance": 5.0,
    "timestamp": "2026-06-26T09:00:00Z"
  }
]
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No matching memories | Returns empty array `[]` |
| Malformed `target_agents` JSON in DB | Defaults to empty array |
| SQLite failure | `McpError::internal_error` |

---

### 6.5 Database Branch Management (3 Tools)

The Branch Management tools provide copy-on-write database isolation for experimental or subagent workflows. Only one branch can be active at a time.

---

#### 6.5.1 `create_database_branch`

| Field | Detail |
|:---|:---|
| **Description** | Create an isolated database branch by copying the main `memory.db` file and switching all layer connections to the branch file. Only one branch can be active at a time. The branch file is stored as `memory.db.branch_<branchId>`. |
| **Priority** | P2 |
| **Layer** | Branch Management |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `branchId` | `String` | ✅ | Non-empty, unique, filesystem-safe | Identifier for the branch |

**Output Format:**
```
"Successfully created database branch: experiment-1"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| Branch already active | `McpError::internal_error("A database branch is already active. Commit or rollback first.")` |
| File copy failure | `McpError::internal_error` with IO error |
| Connection switch failure | `McpError::internal_error` |

---

#### 6.5.2 `commit_database_branch`

| Field | Detail |
|:---|:---|
| **Description** | Commit the active branch by replacing the main database file with the branch file, then switching all connections back to the main database path. The branch file is deleted after commit. |
| **Priority** | P2 |
| **Layer** | Branch Management |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `dummy` | `Option<bool>` | ❌ | — | Unused placeholder |

**Output Format:**
```
"Successfully committed database branch"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No active branch | `McpError::internal_error("No active branch to commit.")` |
| File copy/delete failure | `McpError::internal_error` with IO error |
| Connection switch failure | `McpError::internal_error` |

---

#### 6.5.3 `rollback_database_branch`

| Field | Detail |
|:---|:---|
| **Description** | Roll back the active branch by discarding the branch file and switching all connections back to the original main database. No changes from the branch are preserved. |
| **Priority** | P2 |
| **Layer** | Branch Management |

**Input Schema:**

| Parameter | Type | Required | Constraints | Description |
|:---|:---|:---:|:---|:---|
| `dummy` | `Option<bool>` | ❌ | — | Unused placeholder |

**Output Format:**
```
"Successfully rolled back database branch"
```

**Error Handling:**

| Error Condition | Response |
|:---|:---|
| No active branch | `McpError::internal_error("No active branch to rollback.")` |
| Branch file not found | Silent no-op (already cleaned up) |
| Connection switch failure | `McpError::internal_error` |

---

### 6.6 Working Memory Layer (Implicit)

The Working Memory layer is an in-memory RAM cache (`WorkingMemory` struct) that provides fast ephemeral storage for the current session. It is not directly exposed as MCP tools but is used internally by the coordinator for session-scoped data. Future versions may expose explicit `set_working_memory` and `get_working_memory` tools.

---

### 6.7 Semantic Memory Layer (Implicit)

The Semantic Memory layer provides vector embedding storage and HNSW-based similarity search. It is used internally by the coordinator to enrich graph node observations with semantic embeddings and support cosine similarity retrieval. It manages:

- **Embedding generation** via fastembed (`all-MiniLM-L6-v2`, 384 dimensions)
- **HNSW index** via `small-world-rs` with serialization to `semantic_hnsw_index` table
- **Vector-to-node mapping** via `semantic_vector_mapping` table
- **Fact metadata** via `semantic_metadata` table

The semantic layer is invoked transparently when graph operations involve text that benefits from vector search (e.g., `search_nodes` semantic fallback). Explicit semantic search tools are planned for future releases.

---

## 7. Non-Functional Requirements

### 7.1 Performance

| Requirement | Specification |
|:---|:---|
| **Startup latency** | < 500ms warm start (ONNX model cached to `.fastembed_cache/`) |
| **Tool call latency** | < 10ms p99 for all graph/episodic CRUD operations |
| **Embedding throughput** | ≥ 50 sentences/sec on modern CPU (single-threaded) |
| **HNSW search** | < 50ms for k=10 nearest neighbors across 100K vectors |
| **SQLite write throughput** | ≥ 1,000 inserts/sec with WAL mode |
| **Memory efficiency** | < 10MB baseline; < 200MB with 100K vectors loaded in HNSW |
| **Binary size** | < 30MB stripped release binary (excluding ONNX model cache) |

### 7.2 Security

| Requirement | Specification |
|:---|:---|
| **Data locality** | All data MUST remain on the local filesystem. Zero network calls for memory operations |
| **No telemetry** | No usage data, crash reports, or analytics transmitted |
| **File permissions** | `memory.db` created with user-only read/write permissions (0600) |
| **Input sanitization** | All SQLite queries use parameterized statements (`params![]` macro) — zero SQL injection surface |
| **No eval/exec** | No dynamic code execution from stored memory content |
| **Branch isolation** | Branch files MUST be completely isolated; no cross-branch data leakage |

### 7.3 Privacy

| Requirement | Specification |
|:---|:---|
| **Offline embeddings** | Embedding model runs 100% locally via ONNX Runtime; no API calls |
| **No cloud storage** | Memory data never leaves the local machine |
| **User-controlled data** | All data stored in a single `memory.db` file that users can delete, backup, or inspect with standard SQLite tools |
| **No PII detection** | v1.0 does not include automatic PII detection/redaction (planned for v2.0) |

### 7.4 Scalability

| Dimension | Current Limit | Design Target (v1.0) |
|:---|:---|:---|
| Graph nodes | 100K | 1M |
| Graph edges | 500K | 5M |
| Semantic vectors | 50K | 500K |
| Episodic logs | 100K | 1M |
| Code elements | 200K | 2M |
| Database file size | 500MB | 5GB |
| Concurrent tool calls | 50 | 200 |

### 7.5 Concurrency

| Requirement | Specification |
|:---|:---|
| **Locking strategy** | `parking_lot::Mutex` wrapping each SQLite `Connection` per layer |
| **Async runtime** | Tokio multi-threaded runtime (`features = ["full"]`) |
| **Deadlock prevention** | Each layer holds its own independent lock; no cross-layer locking |
| **Connection pooling** | Single connection per layer (sufficient for local SQLite) |
| **Branch atomicity** | Branch operations acquire all layer locks sequentially to prevent partial state |

---

## 8. System Requirements

### 8.1 Hardware Requirements

| Component | Minimum | Recommended |
|:---|:---|:---|
| **CPU** | x86_64 or aarch64, 2 cores | 4+ cores |
| **RAM** | 512 MB available | 2 GB available |
| **Disk** | 500 MB free (for binary + model cache + DB) | 2 GB free |
| **GPU** | Not required (CPU-only ONNX inference) | — |

### 8.2 Platform Support

| Platform | Status | Notes |
|:---|:---|:---|
| Linux (x86_64) | ✅ Fully supported | Primary development platform |
| Linux (aarch64) | ✅ Supported | Tested on ARM servers |
| macOS (Apple Silicon) | ✅ Supported | M1/M2/M3 tested |
| macOS (Intel) | ✅ Supported | — |
| Windows (x86_64) | 🟡 Experimental | ONNX Runtime compatible; path handling needs testing |
| WSL2 | ✅ Supported | Recommended over native Windows |

### 8.3 Rust Toolchain

| Component | Version |
|:---|:---|
| Rust Edition | 2024 |
| Minimum Rust Version (MSRV) | 1.85.0+ (edition 2024 support) |
| Cargo | Matching Rust version |
| Target | `stable` channel |

### 8.4 Environment Variables

| Variable | Required | Default | Description |
|:---|:---|:---:|:---|
| `MEMORY_DB_PATH` | ❌ | `./memory.db` | Absolute or relative path to the SQLite database file |
| `EMBEDDING_MODEL` | ❌ | `AllMiniLML6V2` | fastembed model identifier (only `AllMiniLML6V2` supported in v0.1) |
| `RUST_LOG` | ❌ | `info` | Logging level filter (`trace`, `debug`, `info`, `warn`, `error`) |

### 8.5 Key Dependencies

| Crate | Version | Purpose |
|:---|:---|:---|
| `rmcp` | 0.16.0 | MCP SDK for Stdio JSON-RPC server |
| `tokio` | 1.35 | Async runtime |
| `rusqlite` | 0.32 (bundled) | SQLite database driver with built-in SQLite |
| `fastembed` | 4.9.1 | Local text embedding generation |
| `ort` | 2.0.0-rc.9 | ONNX Runtime for model inference |
| `small-world-rs` | 1.1 | HNSW vector index |
| `petgraph` | 0.6 | Graph data structures |
| `parking_lot` | 0.12 | High-performance synchronization primitives |
| `serde` / `serde_json` | 1.0 | Serialization / deserialization |
| `schemars` | 1.0 | JSON Schema generation for MCP tool inputs |
| `chrono` | 0.4 | Timestamp handling |
| `uuid` | 1.6 | UUID v4 generation |
| `tree-sitter` | 0.24 | Incremental parsing framework |
| `tree-sitter-rust` | 0.23.0 | Rust grammar |
| `tree-sitter-python` | 0.23.0 | Python grammar |
| `tree-sitter-javascript` | 0.23.0 | JavaScript grammar |
| `tree-sitter-typescript` | 0.23.0 | TypeScript / TSX grammar |
| `tonic` | 0.11 | gRPC server framework |
| `prost` | 0.12 | Protocol Buffers code generation |
| `tonic-build` | 0.11 | Build-time proto compilation |
| `anyhow` | 1.0 | Error handling |
| `log` / `env_logger` | 0.4 / 0.11 | Structured logging |
| `ndarray` | 0.15 | N-dimensional array operations |

---

## 9. Transport Protocols

### 9.1 Stdio JSON-RPC (Primary — MCP Standard)

| Attribute | Detail |
|:---|:---|
| **Protocol** | JSON-RPC 2.0 over stdin/stdout |
| **Standard** | MCP (Model Context Protocol) |
| **SDK** | `rmcp` 0.16.0 with `server` feature |
| **Lifecycle** | Server reads JSON-RPC requests from stdin, writes responses to stdout |
| **Framing** | Newline-delimited JSON messages |
| **Initialization** | Client sends `initialize` → server responds with `ServerCapabilities` (tools enabled) |
| **Tool Discovery** | `tools/list` returns all 22 registered tool schemas with JSON Schema inputs |
| **Tool Invocation** | `tools/call` with `{ name, arguments }` → `CallToolResult` with `Content::text` |
| **Error Protocol** | JSON-RPC error objects with `McpError::internal_error(message, None)` |

**Configuration Example (Claude Desktop):**
```json
{
  "mcpServers": {
    "openmemory": {
      "command": "/path/to/openmemory_rs",
      "env": {
        "MEMORY_DB_PATH": "/path/to/memory.db"
      }
    }
  }
}
```

### 9.2 gRPC Transport (Secondary)

| Attribute | Detail |
|:---|:---|
| **Protocol** | HTTP/2 with Protocol Buffers |
| **Framework** | Tonic 0.11 |
| **Proto Compilation** | `tonic-build` at build time via `build.rs` |
| **Activation** | CLI flag `--grpc <port>` (e.g., `--grpc 50051`) |
| **Advantages** | Immune to stdout pollution from library logs; supports multiple concurrent clients; standard load balancer compatibility |
| **Use Cases** | Multi-agent deployments, networked agent clusters, production environments where stdio is impractical |

**Launch Example:**
```bash
./openmemory_rs --grpc 50051
```

### 9.3 Transport Selection Matrix

| Scenario | Recommended Transport | Reason |
|:---|:---|:---|
| Single-agent MCP client (Claude Desktop, Cursor) | Stdio JSON-RPC | Standard MCP protocol, zero configuration |
| Multi-agent orchestrator | gRPC | Concurrent client support, no stdout conflicts |
| Development & debugging | Stdio JSON-RPC | Simpler setup, piping-friendly |
| Production deployment | gRPC | Network resilience, load balancing |
| CI/CD integration testing | Either | Depends on test harness |

---

## 10. Data Model

### 10.1 Complete SQLite Schema

The following 13 tables are created across the memory layers. All tables use `CREATE TABLE IF NOT EXISTS` for idempotent initialization.

---

#### 10.1.1 Graph Memory Tables

```sql
-- Knowledge graph nodes (entities)
CREATE TABLE IF NOT EXISTS graph_nodes (
    name            TEXT PRIMARY KEY,       -- Unique entity name
    entity_type     TEXT NOT NULL,          -- Category (e.g., "Person", "Project")
    observations    TEXT NOT NULL           -- JSON array of observation strings
);

-- Knowledge graph edges (relations)
CREATE TABLE IF NOT EXISTS graph_edges (
    from_name       TEXT NOT NULL,          -- Source entity name
    to_name         TEXT NOT NULL,          -- Target entity name
    relation_type   TEXT NOT NULL,          -- Active-voice relationship label
    PRIMARY KEY (from_name, to_name, relation_type)
);
```

---

#### 10.1.2 Semantic Memory Tables

```sql
-- Semantic fact metadata with embedded vectors
CREATE TABLE IF NOT EXISTS semantic_metadata (
    node_id         TEXT PRIMARY KEY,       -- Unique fact identifier
    raw_text        TEXT NOT NULL,          -- Original text content
    embedding       BLOB NOT NULL,          -- 384-dim float32 vector (1536 bytes)
    timestamp       TEXT NOT NULL,          -- ISO 8601 creation timestamp
    importance      REAL NOT NULL DEFAULT 1.0  -- Priority score (0.0 – 10.0)
);

-- Maps HNSW vector index IDs to semantic node IDs
CREATE TABLE IF NOT EXISTS semantic_vector_mapping (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,  -- HNSW internal vector ID
    node_id         TEXT UNIQUE NOT NULL               -- Corresponding semantic node ID
);

-- Serialized HNSW index (single-row table)
CREATE TABLE IF NOT EXISTS semantic_hnsw_index (
    id              INTEGER PRIMARY KEY CHECK (id = 1),  -- Always row 1
    index_data      BLOB NOT NULL                       -- Serialized World state
);
```

---

#### 10.1.3 Episodic Memory Tables

```sql
-- Execution episode logs
CREATE TABLE IF NOT EXISTS episodic_logs (
    id                  TEXT PRIMARY KEY,       -- UUID v4
    task_description    TEXT NOT NULL,          -- What was attempted
    execution_status    TEXT NOT NULL,          -- "Success", "Failed", "Partial"
    steps_taken         TEXT NOT NULL,          -- Detailed step log
    error_message       TEXT,                  -- Error details (nullable)
    reflection          TEXT,                  -- Post-hoc analysis (nullable)
    created_at          TEXT NOT NULL           -- ISO 8601 timestamp
);

-- Structured reflection memories
CREATE TABLE IF NOT EXISTS reflection_memory (
    id                  TEXT PRIMARY KEY,       -- UUID v4
    task_description    TEXT NOT NULL,          -- Task being reflected on
    status              TEXT NOT NULL,          -- "Success" or "Failed"
    attempt_number      INTEGER NOT NULL,       -- Attempt sequence number (≥ 1)
    steps_taken         TEXT NOT NULL,          -- Steps in this attempt
    error_encountered   TEXT,                  -- Error description (nullable)
    root_cause          TEXT,                  -- Diagnosed root cause (nullable)
    solution_applied    TEXT,                  -- Applied fix (nullable)
    reflection          TEXT NOT NULL,          -- Synthesized learning
    created_at          TEXT NOT NULL           -- ISO 8601 timestamp
);

-- Tool and model performance metrics
CREATE TABLE IF NOT EXISTS tool_performance (
    tool_name           TEXT NOT NULL,          -- Tool identifier
    model_name          TEXT NOT NULL,          -- Model identifier
    task_type           TEXT NOT NULL,          -- Task category
    success_count       INTEGER NOT NULL DEFAULT 0,   -- Successful invocations
    failure_count       INTEGER NOT NULL DEFAULT 0,   -- Failed invocations
    average_latency     REAL NOT NULL DEFAULT 0.0,    -- Avg latency in ms
    last_used           TEXT NOT NULL,                -- ISO 8601 timestamp
    PRIMARY KEY (tool_name, model_name, task_type)
);
```

---

#### 10.1.4 Codebase Memory Tables

```sql
-- Legacy codebase signatures (for backward compatibility)
CREATE TABLE IF NOT EXISTS codebase_signatures (
    id              TEXT PRIMARY KEY,       -- UUID
    file_path       TEXT NOT NULL,          -- Source file path
    item_name       TEXT NOT NULL,          -- Symbol name
    item_type       TEXT NOT NULL,          -- "Function", "Struct", etc.
    signature       TEXT NOT NULL,          -- Full signature text
    dependencies    TEXT                   -- JSON array of dependency references
);

-- AST-parsed code elements
CREATE TABLE IF NOT EXISTS code_elements (
    element_id      TEXT PRIMARY KEY,       -- UUID
    file_path       TEXT NOT NULL,          -- Source file path
    element_type    TEXT NOT NULL,          -- "Function", "Struct", "Method", "Class", etc.
    name            TEXT NOT NULL,          -- Element name
    signature       TEXT NOT NULL,          -- Signature/header text
    ast_json        TEXT,                  -- Optional AST subtree JSON
    parent_id       TEXT,                  -- Parent element ID (for nested elements)
    start_line      INTEGER NOT NULL,       -- Start line in source file
    end_line        INTEGER NOT NULL        -- End line in source file
);

-- Call graph edges between code elements
CREATE TABLE IF NOT EXISTS code_calls (
    caller_id       TEXT NOT NULL,          -- Caller element ID
    callee_id       TEXT NOT NULL,          -- Callee element ID
    call_site       TEXT,                  -- Call location description
    PRIMARY KEY (caller_id, callee_id)
);

-- Repository file evolution history
CREATE TABLE IF NOT EXISTS repository_evolution (
    file_path           TEXT NOT NULL,          -- Changed file path
    version             TEXT NOT NULL,          -- Version tag
    commit_hash         TEXT,                  -- Git commit hash
    author              TEXT,                  -- Change author
    change_type         TEXT NOT NULL,          -- "Added", "Modified", "Deleted"
    summary_of_changes  TEXT NOT NULL,          -- Change description
    bug_introduced      INTEGER NOT NULL DEFAULT 0,  -- Boolean (0/1)
    bug_fixed           INTEGER NOT NULL DEFAULT 0,  -- Boolean (0/1)
    timestamp           TEXT NOT NULL,               -- ISO 8601 timestamp
    PRIMARY KEY (file_path, version)
);
```

---

#### 10.1.5 Shared Memory Table

```sql
-- Multi-agent shared memory board
CREATE TABLE IF NOT EXISTS shared_agent_memory (
    memory_key      TEXT PRIMARY KEY,       -- Unique memory key
    memory_value    TEXT NOT NULL,          -- Memory content (text/JSON)
    source_agent    TEXT NOT NULL,          -- ID of the agent that stored this
    target_agents   TEXT NOT NULL,          -- JSON array of target agent IDs
    importance      REAL NOT NULL DEFAULT 1.0,  -- Priority score (0.0 – 10.0)
    timestamp       TEXT NOT NULL               -- ISO 8601 timestamp
);
```

---

### 10.2 Schema Summary

| Table | Layer | Primary Key | Columns | Approximate Row Size |
|:---|:---|:---|:---:|:---|
| `graph_nodes` | Graph | `name` | 3 | ~200B |
| `graph_edges` | Graph | `(from_name, to_name, relation_type)` | 3 | ~100B |
| `semantic_metadata` | Semantic | `node_id` | 5 | ~1.7KB (1536B vector) |
| `semantic_vector_mapping` | Semantic | `id` (auto) | 2 | ~50B |
| `semantic_hnsw_index` | Semantic | `id` (fixed = 1) | 2 | Variable (grows with index) |
| `episodic_logs` | Episodic | `id` | 7 | ~500B |
| `reflection_memory` | Episodic | `id` | 10 | ~800B |
| `tool_performance` | Episodic | `(tool_name, model_name, task_type)` | 7 | ~150B |
| `codebase_signatures` | Codebase | `id` | 6 | ~300B |
| `code_elements` | Codebase | `element_id` | 9 | ~400B |
| `code_calls` | Codebase | `(caller_id, callee_id)` | 3 | ~100B |
| `repository_evolution` | Codebase | `(file_path, version)` | 9 | ~400B |
| `shared_agent_memory` | Shared | `memory_key` | 6 | ~300B |

---

## 11. User Stories

### 11.1 Context Persistence

| ID | Story | Priority |
|:---|:---|:---:|
| **US-01** | As an AI agent, I want to store facts about the user's project as entities with observations, so that I can recall them in future sessions without re-asking. | P0 |
| **US-02** | As an AI agent, I want to create relationships between entities (e.g., "ProjectX uses Rust"), so that I can reason about connections in the user's domain. | P0 |
| **US-03** | As an AI agent, I want to search my knowledge graph by keyword, so that I can find relevant entities when the user mentions a topic. | P0 |

### 11.2 Episodic Learning

| ID | Story | Priority |
|:---|:---|:---:|
| **US-04** | As an AI agent, I want to log each task execution with its status and steps, so that I can review my execution history when facing similar tasks. | P0 |
| **US-05** | As an AI agent, I want to store structured reflections including root-cause analysis and solutions applied, so that I can avoid repeating the same mistakes. | P0 |
| **US-06** | As an AI agent, I want to query past reflections relevant to my current task, so that I can apply previously learned solutions. | P0 |

### 11.3 Tool & Model Selection

| ID | Story | Priority |
|:---|:---|:---:|
| **US-07** | As an AI agent, I want to record which tools and models performed well for specific task types, so that I can make data-driven tool selections. | P1 |
| **US-08** | As an AI agent, I want to query tool performance metrics by task type, so that I can recommend the fastest and most reliable tool/model combination. | P1 |

### 11.4 Codebase Understanding

| ID | Story | Priority |
|:---|:---|:---:|
| **US-09** | As a developer, I want my AI agent to index my codebase and understand function signatures, struct definitions, and call hierarchies, so that it can provide contextually-aware code suggestions. | P0 |
| **US-10** | As a developer, I want to track how files in my repository have evolved over time, including which changes introduced or fixed bugs, so that my agent can identify high-risk files. | P1 |
| **US-11** | As a developer, I want to query code elements by file path or keyword, so that my agent can quickly find relevant functions and types. | P0 |

### 11.5 Multi-Agent Collaboration

| ID | Story | Priority |
|:---|:---|:---:|
| **US-12** | As a multi-agent orchestrator, I want to store shared variables and decisions accessible to all subagents, so that parallel agents can coordinate without explicit message passing. | P1 |
| **US-13** | As a subagent, I want to retrieve memories specifically targeted at me by other agents, so that I can act on shared context from collaborators. | P1 |

### 11.6 Experimental Isolation

| ID | Story | Priority |
|:---|:---|:---:|
| **US-14** | As an AI agent performing a speculative task, I want to create an isolated database branch before experimenting, so that failed experiments don't pollute my main memory. | P2 |
| **US-15** | As an AI agent, I want to commit a successful branch back to the main database, or roll back a failed branch cleanly, so that only validated knowledge is persisted. | P2 |

### 11.7 Privacy & Performance

| ID | Story | Priority |
|:---|:---|:---:|
| **US-16** | As a developer working on proprietary code, I want all embeddings and memory operations to run 100% locally with zero network calls, so that my code and data never leave my machine. | P0 |
| **US-17** | As a developer, I want the memory server to start in under 500ms and consume less than 10MB of RAM at baseline, so that it doesn't interfere with my development workflow. | P0 |
| **US-18** | As a developer, I want to inspect and export my memory database using standard SQLite tools, so that I maintain full control over my data. | P0 |

---

## 12. Acceptance Criteria

### 12.1 Graph Memory

| Criterion | Test Method |
|:---|:---|
| Creating 100 entities completes in < 50ms | Benchmark test |
| Creating duplicate entities returns empty array (not error) | Unit test |
| Deleting an entity removes all associated edges | Integration test |
| `search_nodes("rust")` returns entities with "rust" in name, type, or observations | Unit test |
| `read_graph` returns consistent entity + relation counts | Property-based test |
| Observations are correctly appended (not replaced) | Unit test |

### 12.2 Codebase Memory

| Criterion | Test Method |
|:---|:---|
| Indexing a Rust project extracts functions, structs, enums, traits, impl blocks | Integration test with sample project |
| Indexing a Python project extracts classes and functions | Integration test |
| Indexing a TypeScript project extracts functions, classes, arrow functions, methods | Integration test |
| `query_code_graph` filters correctly by file path | Unit test |
| `query_code_graph` filters correctly by keyword | Unit test |
| Repository evolution records maintain `(file_path, version)` uniqueness | Constraint test |

### 12.3 Episodic Memory

| Criterion | Test Method |
|:---|:---|
| Logging an episode with auto-generated UUID produces valid UUID v4 | Unit test |
| Reflections store all 10 fields correctly | Round-trip test |
| `retrieve_episodic_reflections` with query filters by `task_description` keyword | Unit test |
| Tool performance `INSERT OR REPLACE` correctly updates existing records | Unit test |
| `query_tool_performance` returns only records matching `taskType` | Unit test |

### 12.4 Shared Memory

| Criterion | Test Method |
|:---|:---|
| Storing shared memory with `targetAgents: ["*"]` is retrievable by any agent ID | Unit test |
| Retrieving with specific agent ID returns only targeted + wildcard memories | Unit test |
| Source agent can always retrieve their own stored memories | Unit test |
| Importance defaults to 1.0 when not specified | Unit test |

### 12.5 Branch Management

| Criterion | Test Method |
|:---|:---|
| Creating a branch produces a separate `.branch_<id>` file | File existence check |
| Writes to branch do not affect main database | Cross-read integration test |
| Committing branch replaces main database content | Content comparison test |
| Rolling back branch restores original main database | Content comparison test |
| Attempting to create a second branch fails with descriptive error | Error handling test |
| Branch file is deleted after commit or rollback | File cleanup check |

### 12.6 Transport

| Criterion | Test Method |
|:---|:---|
| Stdio server responds to `initialize` with valid `ServerCapabilities` | Protocol test |
| `tools/list` returns exactly 22 tools with valid JSON Schemas | Schema validation test |
| gRPC server binds to specified port and accepts connections | Network test |
| Invalid JSON-RPC requests return proper error responses | Fuzzing test |

---

## 13. Risks & Mitigations

| # | Risk | Likelihood | Impact | Severity | Mitigation |
|:---|:---|:---:|:---:|:---:|:---|
| **R1** | **ONNX model download fails** on first run due to network restrictions | Medium | High | **High** | Pre-bundle model in `.fastembed_cache/`; document offline installation; add clear error messages with download URLs |
| **R2** | **SQLite file corruption** under unexpected process termination | Low | Critical | **High** | SQLite WAL journal mode provides crash recovery; add `PRAGMA integrity_check` on startup; document backup procedures |
| **R3** | **HNSW index deserialization failure** after version upgrade of `small-world-rs` | Medium | Medium | **Medium** | Auto-rebuild HNSW index from `semantic_metadata` table on deserialization error (already implemented as fallback) |
| **R4** | **tree-sitter grammar version mismatch** causes parse failures | Medium | Low | **Low** | Pin exact grammar crate versions (`=0.23.0`); add fallback line-based parser for unsupported files |
| **R5** | **Single-branch limitation** blocks parallel multi-agent workflows | Low | Medium | **Medium** | Document limitation clearly; plan multi-branch support for v0.5 using branch stacks |
| **R6** | **Memory growth** with large HNSW index exceeds available RAM | Low | High | **Medium** | Monitor index size; implement lazy loading for cold vectors; add configurable max vector count |
| **R7** | **Concurrent writes** cause `parking_lot::Mutex` contention under heavy load | Low | Medium | **Medium** | Profile under load; consider per-table connection pools or read/write lock separation if contention is measured |
| **R8** | **Embedding model quality** insufficient for domain-specific text | Medium | Medium | **Medium** | Design model configuration as extensible (env var `EMBEDDING_MODEL`); plan support for alternative models in v0.5 |
| **R9** | **Breaking changes in `rmcp`** SDK between minor versions | Medium | High | **High** | Pin `rmcp` version; monitor upstream releases; maintain compatibility test suite |
| **R10** | **Binary size bloat** from ONNX Runtime and tree-sitter grammars | Low | Low | **Low** | Use `default-features = false` where possible; strip release binaries; consider feature flags for optional components |
| **R11** | **Windows path handling** issues with SQLite and file-based branching | Medium | Medium | **Medium** | Use `std::path::Path` consistently; add Windows-specific integration tests; document WSL2 as recommended alternative |

---

## 14. Release Plan

### 14.1 Release Timeline

```
v0.1.0 (Alpha)          v0.2.0 (Beta)           v0.5.0 (RC)              v1.0.0 (Stable)
    │                       │                        │                        │
    ├── Core 22 tools       ├── Semantic search      ├── Multi-branch         ├── Plugin API
    ├── Stdio transport     │   MCP tools             │   support              ├── Encrypted DBs
    ├── SQLite storage      ├── Decay ranking         ├── Configurable         ├── PII detection
    ├── 6 layers            ├── gRPC production       │   embedding models     ├── Distributed sync
    └── Basic docs          │   hardening             ├── Performance          └── Enterprise features
                            ├── CI/CD pipeline        │   benchmarks
                            └── Error improvement     └── 85% test coverage
```

### 14.2 Release Details

#### v0.1.0 — Alpha (Current)

| Area | Deliverables | Status |
|:---|:---|:---:|
| **Core Engine** | MemoryCoordinator with 6 layers wired | ✅ Complete |
| **MCP Tools** | All 22 tools registered and functional | ✅ Complete |
| **Transport** | Stdio JSON-RPC via rmcp | ✅ Complete |
| **Storage** | SQLite with 13 tables | ✅ Complete |
| **Embeddings** | Local fastembed (all-MiniLM-L6-v2) | ✅ Complete |
| **AST Parsing** | tree-sitter for Rust, Python, JS, TS | ✅ Complete |
| **gRPC** | Basic Tonic server with `--grpc` flag | ✅ Complete |
| **Documentation** | README, architecture.md, features.md, codebase.md | ✅ Complete |

#### v0.2.0 — Beta

| Area | Deliverables | Target Date |
|:---|:---|:---:|
| **Semantic Tools** | Explicit `semantic_search` and `store_semantic_fact` MCP tools | Q3 2026 |
| **Decay Ranking** | Temporal recency decay (e^{-λt}) applied to search results | Q3 2026 |
| **Error Messages** | All errors include actionable fix suggestions | Q3 2026 |
| **CI Pipeline** | GitHub Actions with build, test, lint, and release automation | Q3 2026 |
| **gRPC Hardening** | TLS support, connection limits, request timeouts | Q3 2026 |
| **Working Memory Tools** | Explicit `set_working_memory` / `get_working_memory` MCP tools | Q3 2026 |

#### v0.5.0 — Release Candidate

| Area | Deliverables | Target Date |
|:---|:---|:---:|
| **Multi-Branch** | Support for multiple concurrent database branches (branch stacks) | Q4 2026 |
| **Model Config** | Runtime-configurable embedding models via env var | Q4 2026 |
| **Benchmarks** | Published performance benchmark suite with CI regression tracking | Q4 2026 |
| **Test Coverage** | ≥ 85% code coverage across all layers | Q4 2026 |
| **Integration Guides** | First-class guides for Claude Desktop, Cursor, OpenZ, LangChain | Q4 2026 |
| **CLI Diagnostics** | `openmemory_rs --doctor` command for environment validation | Q4 2026 |

#### v1.0.0 — Stable Release

| Area | Deliverables | Target Date |
|:---|:---|:---:|
| **Plugin API** | Trait-based layer plugin system for community-contributed memory layers | Q1 2027 |
| **Encryption** | SQLCipher integration for encrypted-at-rest databases | Q1 2027 |
| **PII Detection** | Optional regex + NER-based PII detection and redaction before storage | Q1 2027 |
| **Distributed Sync** | Optional peer-to-peer sync for shared team memories across networked nodes | Q2 2027 |
| **Enterprise** | RBAC for shared memories, audit logging, compliance exports | Q2 2027 |
| **Stability** | Zero critical bugs for 90 consecutive days | Q2 2027 |
| **Community** | ≥ 3 community-contributed layers, ≥ 1,000 GitHub stars | Q2 2027 |

---

## Appendix A: Glossary

| Term | Definition |
|:---|:---|
| **MCP** | Model Context Protocol — a standard for connecting AI models to external tools and data sources via JSON-RPC |
| **HNSW** | Hierarchical Navigable Small World — an algorithm for approximate nearest neighbor search in high-dimensional spaces |
| **AST** | Abstract Syntax Tree — a tree representation of the structure of source code |
| **ONNX** | Open Neural Network Exchange — a cross-platform format for machine learning models |
| **WAL** | Write-Ahead Logging — SQLite journaling mode that enables concurrent reads during writes |
| **fastembed** | A Rust crate for generating text embeddings locally using ONNX Runtime |
| **tree-sitter** | An incremental parsing system for programming languages |
| **Cognitive Layer** | One of the 6 memory subsystems (Working, Graph, Semantic, Episodic, Codebase, Shared) |
| **Branch** | An isolated copy of the database for experimental or subagent workflows |
| **Observation** | A fact string associated with a knowledge graph entity |
| **Reflection** | A structured analysis record of a completed task attempt including root-cause analysis |

---

## Appendix B: Document Revision History

| Version | Date | Author | Changes |
|:---|:---|:---|:---|
| 1.0.0-draft | 2026-06-26 | openmemory_rs Core Team | Initial comprehensive PRD covering all 22 MCP tools, 13 tables, 6 layers |

---

> **Document Owner:** openmemory_rs Core Team  
> **Review Cycle:** Bi-weekly during active development; monthly post-v1.0  
> **Feedback Channel:** GitHub Issues with label `prd-feedback`
