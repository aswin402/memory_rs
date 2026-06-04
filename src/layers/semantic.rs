use anyhow::Result;
use std::path::Path;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

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
            );"
        )?;

        // Initialize local ONNX fastembed model
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_show_download_progress(false)
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            model: Mutex::new(model),
        })
    }

    pub fn add_fact(&self, node_id: &str, text: &str, importance: f64) -> Result<()> {
        let conn = self.conn.lock();
        let timestamp = chrono::Utc::now().to_rfc3339();
        
        // Generate embedding vector using locked model
        let embeddings = {
            let mut model = self.model.lock();
            model.embed(vec![text], None)?
        };
        
        if embeddings.is_empty() {
            anyhow::bail!("Failed to generate embedding");
        }
        let vector = &embeddings[0]; // Vec<f32>
        
        // Serialize vector into a byte blob (Vec<u8>)
        let blob: Vec<u8> = vector.iter()
            .flat_map(|val| val.to_ne_bytes().to_vec())
            .collect();

        conn.execute(
            "INSERT OR REPLACE INTO semantic_metadata (node_id, raw_text, embedding, timestamp, importance) 
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![node_id, text, blob, timestamp, importance],
        )?;
        Ok(())
    }

    pub fn query_similar_facts(&self, query: &str, limit: usize) -> Result<Vec<SemanticFact>> {
        let conn = self.conn.lock();
        
        // Generate query embedding using locked model
        let embeddings = {
            let mut model = self.model.lock();
            model.embed(vec![query], None)?
        };
        
        if embeddings.is_empty() {
            anyhow::bail!("Failed to generate query embedding");
        }
        let query_vector = &embeddings[0]; // Vec<f32>

        // Retrieve all facts from database
        let mut stmt = conn.prepare("SELECT node_id, raw_text, embedding, timestamp, importance FROM semantic_metadata")?;
        let mut rows = stmt.query([])?;
        let mut facts = Vec::new();

        while let Some(row) = rows.next()? {
            let node_id: String = row.get(0)?;
            let raw_text: String = row.get(1)?;
            let blob: Vec<u8> = row.get(2)?;
            let timestamp: String = row.get(3)?;
            let importance: f64 = row.get(4)?;

            // Deserialize vector from byte blob
            let mut vector = Vec::new();
            for chunk in blob.chunks_exact(4) {
                let array: [u8; 4] = chunk.try_into().unwrap_or([0; 4]);
                vector.push(f32::from_ne_bytes(array));
            }

            // Calculate cosine similarity
            let similarity = calculate_cosine_similarity(query_vector, &vector);
            
            facts.push(SemanticFact {
                node_id,
                raw_text,
                similarity,
                timestamp,
                importance,
            });
        }

        // Sort by similarity descending
        facts.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
        facts.truncate(limit);

        Ok(facts)
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
