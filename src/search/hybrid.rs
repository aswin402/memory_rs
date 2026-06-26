use crate::layers::semantic::SemanticFact;

pub struct HybridSearch;

impl HybridSearch {
    /// Merges vector search results and keyword FTS5 search results using Reciprocal Rank Fusion (RRF).
    /// RRF score = sum(1.0 / (k + rank_i))
    pub fn rrf(
        vector_results: &[SemanticFact],
        fts_results: &[SemanticFact],
        k: usize,
    ) -> Vec<(SemanticFact, f64)> {
        use std::collections::HashMap;

        let mut doc_scores: HashMap<String, f64> = HashMap::new();
        let mut doc_map: HashMap<String, SemanticFact> = HashMap::new();

        // 1. Process vector results (already sorted/scored by relevance ranker)
        for (i, fact) in vector_results.iter().enumerate() {
            let rank = (i + 1) as f64;
            let score = 1.0 / (k as f64 + rank);
            *doc_scores.entry(fact.node_id.clone()).or_insert(0.0) += score;
            doc_map.insert(fact.node_id.clone(), fact.clone());
        }

        // 2. Process FTS results (sorted by BM25 keyword score)
        for (i, fact) in fts_results.iter().enumerate() {
            let rank = (i + 1) as f64;
            let score = 1.0 / (k as f64 + rank);
            *doc_scores.entry(fact.node_id.clone()).or_insert(0.0) += score;
            // Prefer keeping the vector search version if it exists, since it contains the cosine similarity
            doc_map.entry(fact.node_id.clone()).or_insert_with(|| fact.clone());
        }

        // 3. Collect and sort by RRF score descending
        let mut ranked: Vec<(SemanticFact, f64)> = doc_scores
            .into_iter()
            .filter_map(|(node_id, rrf_score)| {
                doc_map.remove(&node_id).map(|fact| (fact, rrf_score))
            })
            .collect();

        ranked.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ranked
    }
}
