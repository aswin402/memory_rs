# Phase 6 Implementation Plan: Context Intelligence & Agent Optimization

This document outlines the step-by-step implementation tasks for Phase 6 in `openmemory_rs`.

## Development Tasks

### Task 1: Fact Extraction Engine
- **Files:**
  - Create: `src/extraction/fact_extractor.rs`
  - Create: `src/extraction/mod.rs`
  - Modify: `src/lib.rs` (register `extraction` module)
- **Implementation:**
  - Define `ExtractedFact` and `FactExtractor` structs.
  - Implement `FactExtractor::new()` containing precompiled regexes for standard relations (`uses`, `depends_on`, `prefers`, `is_a`, `works_with`, `created`).
  - Implement `extract` method to segment text and matches patterns.
- **Verification:**
  - Add unit tests verifying extraction from example sentences.

---

### Task 2: Proactive Recall Engine
- **Files:**
  - Create: `src/extraction/recall.rs`
  - Modify: `src/extraction/mod.rs`
- **Implementation:**
  - Define `RecallItem` and query routing logic.
  - Implement retrieval across Semantic Memory (hybrid/semantic), Graph Memory (nodes/relations FTS query), and Episodic Memory (reflection query).
  - Normalize scores and merge/sort findings.
- **Verification:**
  - Add unit tests validating fused recall list results.

---

### Task 3: Context Compressor Engine
- **Files:**
  - Modify: `Cargo.toml` (add `rust-stemmers` dependency)
  - Create: `src/extraction/compressor.rs`
  - Modify: `src/extraction/mod.rs`
- **Implementation:**
  - Implement English stop-words array (~150 common words).
  - Implement TF-IDF sentence scoring using `rust-stemmers` Porter Stemmer.
  - Filter and reconstruct text based on the top-ranking sentences.
- **Verification:**
  - Add unit tests demonstrating context compression ratio and similarity preservation.

---

### Task 4: Expose MCP Tools
- **Files:**
  - Modify: `src/mcp.rs`
- **Implementation:**
  - Define input structs: `ExtractAndStoreFactsInput`, `ProactiveRecallInput`, `CompressContextInput`.
  - Expose `extract_and_store_facts`, `proactive_recall`, and `compress_context` MCP tools.
- **Verification:**
  - Add integration tests verifying all three endpoints in `src/mcp.rs` tests module.

---

### Task 5: gRPC Integration Testing
- **Files:**
  - Modify: `tests/test_grpc.rs`
- **Implementation:**
  - Add Step 12 to verify `proactive_recall` tool call over the Tonic gRPC client bridge.
- **Verification:**
  - Run `cargo test` to verify all unit and integration tests pass successfully.

---

## SDD Progress Checklist

To track execution progress, we will reset the ledger at `.superpowers/sdd/progress.md`.
- [ ] Task 1: Fact Extraction Engine
- [ ] Task 2: Proactive Recall Engine
- [ ] Task 3: Context Compressor Engine
- [ ] Task 4: Expose MCP Tools
- [ ] Task 5: gRPC Integration Testing
