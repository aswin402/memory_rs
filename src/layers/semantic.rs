use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use small_world_rs::distance_metric::{CosineDistance, DistanceMetric};
use small_world_rs::primitives::vector::Vector;
use small_world_rs::world::world::World;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SemanticFact {
    pub node_id: String,
    pub raw_text: String,
    pub similarity: f64,
    pub timestamp: String,
    pub importance: f64,
}

pub struct SemanticMemory {
    conn: Mutex<Connection>,
    model: Mutex<TextEmbedding>,
    hnsw_index: Mutex<World>,
}

impl SemanticMemory {
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS semantic_metadata (
                node_id TEXT,
                raw_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                valid_until TEXT,
                superseded_by TEXT,
                PRIMARY KEY (node_id, valid_from)
            );
            CREATE INDEX IF NOT EXISTS idx_semantic_metadata_scope ON semantic_metadata (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS semantic_vector_mapping (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id TEXT UNIQUE NOT NULL
            );
            CREATE TABLE IF NOT EXISTS semantic_hnsw_index (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                index_data BLOB NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS semantic_fts USING fts5(
                node_id UNINDEXED,
                raw_text
            );
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ai AFTER INSERT ON semantic_metadata BEGIN
                INSERT INTO semantic_fts(node_id, raw_text) VALUES (new.node_id, new.raw_text);
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ad AFTER DELETE ON semantic_metadata BEGIN
                DELETE FROM semantic_fts WHERE node_id = old.node_id;
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_au AFTER UPDATE OF raw_text ON semantic_metadata BEGIN
                UPDATE semantic_fts SET raw_text = new.raw_text WHERE node_id = new.node_id;
            END;
            INSERT INTO semantic_fts(node_id, raw_text)
            SELECT node_id, raw_text FROM semantic_metadata
            WHERE NOT EXISTS (SELECT 1 FROM semantic_fts WHERE semantic_fts.node_id = semantic_metadata.node_id);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN valid_until TEXT", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN superseded_by TEXT", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_semantic_metadata_scope ON semantic_metadata (user_id, session_id, agent_id)", []);

        // Initialize local ONNX fastembed model
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(false),
        )?;

        let dimensions = 384; // AllMiniLML6V2 uses 384 dimensions

        // Load or rebuild HNSW index from SQLite
        let hnsw_index = {
            let mut stmt =
                conn.prepare("SELECT index_data FROM semantic_hnsw_index WHERE id = 1")?;
            let mut rows = stmt.query([])?;
            if let Some(row) = rows.next()? {
                let blob: Vec<u8> = row.get(0)?;
                match World::new_from_dump(&blob) {
                    Ok(world) => world,
                    Err(e) => {
                        log::warn!("Failed to load HNSW index from db: {}. Rebuilding...", e);
                        rebuild_hnsw_index(&conn, dimensions)?
                    }
                }
            } else {
                rebuild_hnsw_index(&conn, dimensions)?
            }
        };

        Ok(Self {
            conn: Mutex::new(conn),
            model: Mutex::new(model),
            hnsw_index: Mutex::new(hnsw_index),
        })
    }

    pub fn add_fact(
        &self,
        node_id: &str,
        text: &str,
        importance: f64,
        scope: &crate::layers::MemoryScope,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        // Generate embedding vector
        let embeddings = {
            let model = self.model.lock();
            model.embed(vec![text], None)?
        };

        if embeddings.is_empty() {
            anyhow::bail!("Failed to generate embedding");
        }
        let vector_values = &embeddings[0]; // Vec<f32>

        // Serialize vector into a byte blob (Vec<u8>)
        let blob: Vec<u8> = vector_values
            .iter()
            .flat_map(|val| val.to_ne_bytes().to_vec())
            .collect();

        conn.execute(
            "INSERT INTO semantic_metadata (node_id, raw_text, embedding, timestamp, importance, user_id, session_id, agent_id, valid_from) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
            params![node_id, text, blob, timestamp, importance, user_id, session_id, agent_id],
        )?;

        // Map node_id to u32 ID for HNSW
        let mapping_id = get_or_create_mapping_id(&conn, node_id)?;

        // Update HNSW index
        {
            let mut index = self.hnsw_index.lock();
            let vector = Vector::new_f32(vector_values);
            index.insert_vector(mapping_id, vector)?;

            let dumped = index.dump()?;
            conn.execute(
                "INSERT OR REPLACE INTO semantic_hnsw_index (id, index_data) VALUES (1, ?1)",
                params![dumped],
            )?;
        }

        Ok(())
    }

    pub fn query_similar_facts_vector(&self, query: &str, limit: usize, scope: &crate::layers::MemoryScope) -> Result<Vec<SemanticFact>> {
        let conn = self.conn.lock();

        // Short-circuit if there are no active facts in scope to avoid embedding / HNSW panic on empty index
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM semantic_metadata 
             WHERE valid_until IS NULL
               AND (?1 IS NULL OR user_id = ?1 OR user_id = '*')
               AND (?2 IS NULL OR session_id = ?2 OR session_id = '*')
               AND (?3 IS NULL OR agent_id = ?3 OR agent_id = '*')",
            params![scope.user_id, scope.session_id, scope.agent_id],
            |r| r.get(0)
        )?;
        if count == 0 {
            return Ok(Vec::new());
        }

        // Generate query embedding
        let embeddings = {
            let model = self.model.lock();
            model.embed(vec![query], None)?
        };

        if embeddings.is_empty() {
            anyhow::bail!("Failed to generate query embedding");
        }
        let query_vector_values = &embeddings[0]; // Vec<f32>
        let query_vector = Vector::new_f32(query_vector_values);

        // Perform HNSW search (beam_width = 100)
        let candidate_ids = {
            let index = self.hnsw_index.lock();
            index.search(&query_vector, limit, 100)?
        };

        if candidate_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Map u32 IDs back to node_id strings
        let mut node_ids = Vec::new();
        for id in candidate_ids {
            let node_id: Option<String> = conn
                .query_row(
                    "SELECT node_id FROM semantic_vector_mapping WHERE id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .ok();
            if let Some(nid) = node_id {
                node_ids.push(nid);
            }
        }

        // Retrieve metadata and compute similarity for candidates
        let mut facts_with_scores = Vec::new();
        for node_id in node_ids {
            let mut stmt = conn.prepare(
                "SELECT raw_text, embedding, timestamp, importance 
                 FROM semantic_metadata 
                 WHERE node_id = ?1
                   AND valid_until IS NULL
                   AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
                   AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
                   AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
            )?;
            let mut rows = stmt.query(params![
                node_id,
                scope.user_id,
                scope.session_id,
                scope.agent_id
            ])?;
            if let Some(row) = rows.next()? {
                let raw_text: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                let timestamp: String = row.get(2)?;
                let importance: f64 = row.get(3)?;

                let mut vector = Vec::new();
                for chunk in blob.chunks_exact(4) {
                    let array: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                    vector.push(f32::from_ne_bytes(array));
                }

                let similarity = calculate_cosine_similarity(query_vector_values, &vector);

                let parsed_time = chrono::DateTime::parse_from_rfc3339(&timestamp)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|_| chrono::Utc::now());
                let elapsed = chrono::Utc::now().signed_duration_since(parsed_time);
                let elapsed_hours = elapsed.num_seconds() as f64 / 3600.0;

                let item = crate::search::ranker::MemoryItem {
                    content: raw_text.clone(),
                    similarity,
                    elapsed_hours,
                    importance,
                    success_rate: 1.0,
                };
                let score = crate::search::ranker::Ranker::score(&item, 0.5, 0.3, 0.2, 0.0);

                facts_with_scores.push((
                    SemanticFact {
                        node_id,
                        raw_text,
                        similarity,
                        timestamp,
                        importance,
                    },
                    score,
                ));
            }
        }

        // Sort by ranker score descending
        facts_with_scores.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let sorted_facts = facts_with_scores.into_iter().map(|(fact, _)| fact).collect();
        Ok(sorted_facts)
    }

    pub fn query_similar_facts(&self, query: &str, limit: usize, scope: &crate::layers::MemoryScope) -> Result<Vec<SemanticFact>> {
        // Fetch candidate lists from vector search (ranked by relevance) and FTS5 search
        let vector_results = self.query_similar_facts_vector(query, limit * 2, scope)?;
        let fts_results = self.search_text(query, limit * 2, scope)?;

        // Perform Reciprocal Rank Fusion (RRF) with default k = 60
        let rrf_results = crate::search::hybrid::HybridSearch::rrf(&vector_results, &fts_results, 60);

        // Take top `limit` merged results
        let final_results = rrf_results
            .into_iter()
            .take(limit)
            .map(|(fact, _score)| fact)
            .collect();

        Ok(final_results)
    }

    pub fn search_text(&self, query: &str, limit: usize, scope: &crate::layers::MemoryScope) -> Result<Vec<SemanticFact>> {
        let conn = self.conn.lock();

        let clean_query = query
            .chars()
            .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
            .collect::<String>();
        let words: Vec<&str> = clean_query.split_whitespace().collect();
        if words.is_empty() {
            return Ok(Vec::new());
        }

        // Join words with matching syntax, e.g. "term1* term2*"
        let match_query = words
            .iter()
            .map(|w| format!("{}*", w))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = conn.prepare(
            "SELECT m.node_id, m.raw_text, m.timestamp, m.importance 
             FROM semantic_fts f
             JOIN semantic_metadata m ON f.node_id = m.node_id
             WHERE semantic_fts MATCH ?1
               AND m.valid_until IS NULL
               AND (?3 IS NULL OR m.user_id = ?3 OR m.user_id = '*')
               AND (?4 IS NULL OR m.session_id = ?4 OR m.session_id = '*')
               AND (?5 IS NULL OR m.agent_id = ?5 OR m.agent_id = '*')
             ORDER BY rank
             LIMIT ?2"
        )?;

        let mut rows = stmt.query(params![
            match_query,
            limit,
            scope.user_id,
            scope.session_id,
            scope.agent_id
        ])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(SemanticFact {
                node_id: row.get(0)?,
                raw_text: row.get(1)?,
                similarity: 1.0, // Placeholder similarity for FTS match
                timestamp: row.get(2)?,
                importance: row.get(3)?,
            });
        }

        Ok(results)
    }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            CREATE TABLE IF NOT EXISTS semantic_metadata (
                node_id TEXT,
                raw_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0,
                user_id TEXT NOT NULL DEFAULT '*',
                session_id TEXT NOT NULL DEFAULT '*',
                agent_id TEXT NOT NULL DEFAULT '*',
                valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                valid_until TEXT,
                superseded_by TEXT,
                PRIMARY KEY (node_id, valid_from)
            );
            CREATE INDEX IF NOT EXISTS idx_semantic_metadata_scope ON semantic_metadata (user_id, session_id, agent_id);
            CREATE TABLE IF NOT EXISTS semantic_vector_mapping (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id TEXT UNIQUE NOT NULL
            );
            CREATE TABLE IF NOT EXISTS semantic_hnsw_index (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                index_data BLOB NOT NULL
            );
            CREATE VIRTUAL TABLE IF NOT EXISTS semantic_fts USING fts5(
                node_id UNINDEXED,
                raw_text
            );
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ai AFTER INSERT ON semantic_metadata BEGIN
                INSERT INTO semantic_fts(node_id, raw_text) VALUES (new.node_id, new.raw_text);
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_ad AFTER DELETE ON semantic_metadata BEGIN
                DELETE FROM semantic_fts WHERE node_id = old.node_id;
            END;
            CREATE TRIGGER IF NOT EXISTS semantic_metadata_au AFTER UPDATE OF raw_text ON semantic_metadata BEGIN
                UPDATE semantic_fts SET raw_text = new.raw_text WHERE node_id = new.node_id;
            END;
            INSERT INTO semantic_fts(node_id, raw_text)
            SELECT node_id, raw_text FROM semantic_metadata
            WHERE NOT EXISTS (SELECT 1 FROM semantic_fts WHERE semantic_fts.node_id = semantic_metadata.node_id);",
        )?;

        // Ensure scope columns exist in older database schemas
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN user_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN session_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN agent_id TEXT NOT NULL DEFAULT '*'", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN valid_from TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN valid_until TEXT", []);
        let _ = conn.execute("ALTER TABLE semantic_metadata ADD COLUMN superseded_by TEXT", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_semantic_metadata_scope ON semantic_metadata (user_id, session_id, agent_id)", []);

        let dimensions = 384;
        let hnsw_index = {
            let mut stmt =
                conn.prepare("SELECT index_data FROM semantic_hnsw_index WHERE id = 1")?;
            let mut rows = stmt.query([])?;
            if let Some(row) = rows.next()? {
                let blob: Vec<u8> = row.get(0)?;
                match World::new_from_dump(&blob) {
                    Ok(world) => world,
                    Err(e) => {
                        log::warn!("Failed to load HNSW index from db: {}. Rebuilding...", e);
                        rebuild_hnsw_index(&conn, dimensions)?
                    }
                }
            } else {
                rebuild_hnsw_index(&conn, dimensions)?
            }
        };

        *self.conn.lock() = conn;
        *self.hnsw_index.lock() = hnsw_index;
        Ok(())
    }

    pub fn invalidate_fact(&self, node_id: &str, scope: &crate::layers::MemoryScope) -> Result<()> {
        let conn = self.conn.lock();
        let user_id = scope.user_id.as_deref().unwrap_or("*");
        let session_id = scope.session_id.as_deref().unwrap_or("*");
        let agent_id = scope.agent_id.as_deref().unwrap_or("*");

        conn.execute(
            "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') 
             WHERE node_id = ?1 AND user_id = ?2 AND session_id = ?3 AND agent_id = ?4 AND valid_until IS NULL",
            params![node_id, user_id, session_id, agent_id],
        )?;
        Ok(())
    }

    pub fn query_as_of(&self, as_of: &str, scope: &crate::layers::MemoryScope) -> Result<Vec<SemanticFact>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT node_id, raw_text, timestamp, importance 
             FROM semantic_metadata 
             WHERE valid_from <= ?1 
               AND (valid_until IS NULL OR valid_until > ?1)
               AND (?2 IS NULL OR user_id = ?2 OR user_id = '*')
               AND (?3 IS NULL OR session_id = ?3 OR session_id = '*')
               AND (?4 IS NULL OR agent_id = ?4 OR agent_id = '*')"
        )?;
        let mut rows = stmt.query(params![
            as_of,
            scope.user_id,
            scope.session_id,
            scope.agent_id
        ])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(SemanticFact {
                node_id: row.get(0)?,
                raw_text: row.get(1)?,
                similarity: 1.0, // Placeholder similarity
                timestamp: row.get(2)?,
                importance: row.get(3)?,
            });
        }
        Ok(results)
    }

    pub fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        Ok(())
    }
}

fn rebuild_hnsw_index(conn: &Connection, _dimensions: usize) -> Result<World> {
    log::info!("Rebuilding local HNSW index from database embeddings...");
    let mut world = World::new(32, 200, 100, DistanceMetric::Cosine(CosineDistance))?;

    let mut stmt = conn.prepare("SELECT node_id, embedding FROM semantic_metadata WHERE valid_until IS NULL")?;
    let mut rows = stmt.query([])?;

    while let Some(row) = rows.next()? {
        let node_id: String = row.get(0)?;
        let blob: Vec<u8> = row.get(1)?;

        let mut vector_values = Vec::new();
        for chunk in blob.chunks_exact(4) {
            let array: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
            vector_values.push(f32::from_ne_bytes(array));
        }

        let mapping_id = get_or_create_mapping_id(conn, &node_id)?;
        let vector = Vector::new_f32(&vector_values);
        world.insert_vector(mapping_id, vector)?;
    }

    let dumped = world.dump()?;
    conn.execute(
        "INSERT OR REPLACE INTO semantic_hnsw_index (id, index_data) VALUES (1, ?1)",
        params![dumped],
    )?;

    Ok(world)
}

fn get_or_create_mapping_id(conn: &Connection, node_id: &str) -> Result<u32> {
    let mapping_id: Option<u32> = conn
        .query_row(
            "SELECT id FROM semantic_vector_mapping WHERE node_id = ?1",
            params![node_id],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = mapping_id {
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO semantic_vector_mapping (node_id) VALUES (?1)",
            params![node_id],
        )?;
        let last_id = conn.last_insert_rowid();
        Ok(last_id as u32)
    }
}

fn calculate_cosine_similarity(v1: &[f32], v2: &[f32]) -> f64 {
    if v1.len() != v2.len() || v1.is_empty() {
        return 0.0;
    }
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for i in 0..v1.len() {
        dot_product += (v1[i] * v2[i]) as f64;
        norm_a += (v1[i] * v1[i]) as f64;
        norm_b += (v2[i] * v2[i]) as f64;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a.sqrt() * norm_b.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::MemoryScope;
    use std::fs;

    #[test]
    fn test_semantic_bitemporal_behavior() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_semantic_bitemporal_{}.db", uuid::Uuid::new_v4()));
        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        let semantic = SemanticMemory::new(&db_path)?;
        let scope = MemoryScope::default();

        // 1. Add a fact
        semantic.add_fact("fact-1", "Rust is safe and fast", 0.9, &scope)?;

        // 2. Query it to verify it is returned
        let res_before = semantic.query_similar_facts("Rust", 10, &scope)?;
        assert_eq!(res_before.len(), 1);
        assert_eq!(res_before[0].node_id, "fact-1");

        // 3. Mark it inactive (valid_until = now)
        {
            let conn = semantic.conn.lock();
            conn.execute(
                "UPDATE semantic_metadata SET valid_until = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE node_id = 'fact-1'",
                [],
            )?;
        }

        // 4. Query again - should NOT return the inactive fact
        let res_after = semantic.query_similar_facts("Rust", 10, &scope)?;
        assert!(res_after.is_empty(), "Inactive facts should not be queried");

        // 5. Test FTS directly via search_text - should NOT return the inactive fact
        let res_fts = semantic.search_text("Rust", 10, &scope)?;
        assert!(res_fts.is_empty(), "Inactive facts should not be returned by FTS");

        // 6. Test HNSW rebuilding
        // Since HNSW is rebuilt on reload / connection switch if the dump is missing,
        // let's delete the index dump and trigger switch_connection to rebuild the index.
        {
            let conn = semantic.conn.lock();
            conn.execute("DELETE FROM semantic_hnsw_index WHERE id = 1", [])?;
        }
        semantic.switch_connection(&db_path)?;

        // HNSW index is now rebuilt. Let's do a vector search directly (which queries the HNSW index first)
        let res_vector = semantic.query_similar_facts_vector("Rust", 10, &scope)?;
        assert!(res_vector.is_empty(), "Rebuilt HNSW index should exclude inactive facts");

        // Cleanup
        let _ = fs::remove_file(&db_path);
        Ok(())
    }

    #[test]
    fn test_semantic_history_and_temporal_queries() -> Result<()> {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_semantic_history_temporal_{}.db", uuid::Uuid::new_v4()));
        if db_path.exists() {
            let _ = fs::remove_file(&db_path);
        }

        let semantic = SemanticMemory::new(&db_path)?;
        let scope = MemoryScope::default();

        let before_creation = chrono::Utc::now().to_rfc3339();
        std::thread::sleep(std::time::Duration::from_secs(1));

        // 1. Add a fact
        semantic.add_fact("fact-1", "Rust is safe and fast", 0.9, &scope)?;

        std::thread::sleep(std::time::Duration::from_secs(1));
        let after_creation = chrono::Utc::now().to_rfc3339();
        std::thread::sleep(std::time::Duration::from_secs(1));

        // 2. Invalidate the fact using the new invalidate_fact method
        semantic.invalidate_fact("fact-1", &scope)?;

        std::thread::sleep(std::time::Duration::from_secs(1));
        let after_invalidation = chrono::Utc::now().to_rfc3339();

        // 3. Test query_as_of before creation (should return 0 facts)
        let facts_before = semantic.query_as_of(&before_creation, &scope)?;
        assert_eq!(facts_before.len(), 0);

        // 4. Test query_as_of after creation (should return 1 fact)
        let facts_after = semantic.query_as_of(&after_creation, &scope)?;
        assert_eq!(facts_after.len(), 1);
        assert_eq!(facts_after[0].node_id, "fact-1");

        // 5. Test query_as_of after invalidation (should return 0 facts)
        let facts_final = semantic.query_as_of(&after_invalidation, &scope)?;
        assert_eq!(facts_final.len(), 0);

        // Cleanup
        let _ = fs::remove_file(&db_path);
        Ok(())
    }
}

