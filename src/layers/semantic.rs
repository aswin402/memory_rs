use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};
use small_world_rs::world::world::World;
use small_world_rs::distance_metric::{DistanceMetric, CosineDistance};
use small_world_rs::primitives::vector::Vector;

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
            "CREATE TABLE IF NOT EXISTS semantic_metadata (
                node_id TEXT PRIMARY KEY,
                raw_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0
            );
            CREATE TABLE IF NOT EXISTS semantic_vector_mapping (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id TEXT UNIQUE NOT NULL
            );
            CREATE TABLE IF NOT EXISTS semantic_hnsw_index (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                index_data BLOB NOT NULL
            );"
        )?;

        // Initialize local ONNX fastembed model
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_show_download_progress(false)
        )?;

        let dimensions = 384; // AllMiniLML6V2 uses 384 dimensions

        // Load or rebuild HNSW index from SQLite
        let hnsw_index = {
            let mut stmt = conn.prepare("SELECT index_data FROM semantic_hnsw_index WHERE id = 1")?;
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

    pub fn add_fact(&self, node_id: &str, text: &str, importance: f64) -> Result<()> {
        let conn = self.conn.lock();
        let timestamp = chrono::Utc::now().to_rfc3339();
        
        // Generate embedding vector
        let embeddings = {
            let mut model = self.model.lock();
            model.embed(vec![text], None)?
        };
        
        if embeddings.is_empty() {
            anyhow::bail!("Failed to generate embedding");
        }
        let vector_values = &embeddings[0]; // Vec<f32>
        
        // Serialize vector into a byte blob (Vec<u8>)
        let blob: Vec<u8> = vector_values.iter()
            .flat_map(|val| val.to_ne_bytes().to_vec())
            .collect();

        conn.execute(
            "INSERT OR REPLACE INTO semantic_metadata (node_id, raw_text, embedding, timestamp, importance) 
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![node_id, text, blob, timestamp, importance],
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

    pub fn query_similar_facts(&self, query: &str, limit: usize) -> Result<Vec<SemanticFact>> {
        let conn = self.conn.lock();
        
        // Generate query embedding
        let embeddings = {
            let mut model = self.model.lock();
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
            let node_id: Option<String> = conn.query_row(
                "SELECT node_id FROM semantic_vector_mapping WHERE id = ?1",
                params![id],
                |row| row.get(0),
            ).ok();
            if let Some(nid) = node_id {
                node_ids.push(nid);
            }
        }

        // Retrieve metadata and compute similarity for candidates
        let mut facts = Vec::new();
        for node_id in node_ids {
            let mut stmt = conn.prepare(
                "SELECT raw_text, embedding, timestamp, importance FROM semantic_metadata WHERE node_id = ?1"
            )?;
            let mut rows = stmt.query(params![node_id])?;
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
                
                facts.push(SemanticFact {
                    node_id,
                    raw_text,
                    similarity,
                    timestamp,
                    importance,
                });
            }
        }

        // Sort by similarity descending
        facts.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

        Ok(facts)
    }

    pub fn switch_connection(&self, db_path: &Path) -> Result<()> {
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS semantic_metadata (
                node_id TEXT PRIMARY KEY,
                raw_text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                timestamp TEXT NOT NULL,
                importance REAL NOT NULL DEFAULT 1.0
            );
            CREATE TABLE IF NOT EXISTS semantic_vector_mapping (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                node_id TEXT UNIQUE NOT NULL
            );
            CREATE TABLE IF NOT EXISTS semantic_hnsw_index (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                index_data BLOB NOT NULL
            );"
        )?;

        let dimensions = 384;
        let hnsw_index = {
            let mut stmt = conn.prepare("SELECT index_data FROM semantic_hnsw_index WHERE id = 1")?;
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
}

fn rebuild_hnsw_index(conn: &Connection, dimensions: usize) -> Result<World> {
    log::info!("Rebuilding local HNSW index from database embeddings...");
    let mut world = World::new(32, 200, 100, DistanceMetric::Cosine(CosineDistance))?;
    
    let mut stmt = conn.prepare("SELECT node_id, embedding FROM semantic_metadata")?;
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
    let mapping_id: Option<u32> = conn.query_row(
        "SELECT id FROM semantic_vector_mapping WHERE node_id = ?1",
        params![node_id],
        |row| row.get(0),
    ).ok();

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
