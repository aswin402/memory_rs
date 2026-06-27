use crate::layers::semantic::SemanticMemory;
use crate::layers::MemoryScope;
use crate::error::Result;

pub struct SemanticDedup;

impl SemanticDedup {
    /// Finds facts in scope that are duplicates/closely related to the given text.
    /// Returns a list of (node_id, raw_text, similarity) for matches >= threshold.
    pub fn find_duplicates(
        semantic: &SemanticMemory,
        text: &str,
        threshold: f64,
        scope: &MemoryScope,
    ) -> Result<Vec<(String, String, f64)>> {
        let similar = semantic.query_similar_facts_vector(text, 5, scope)?;
        let mut duplicates = Vec::new();
        for fact in similar {
            if fact.similarity >= threshold {
                duplicates.push((fact.node_id, fact.raw_text, fact.similarity));
            }
        }
        Ok(duplicates)
    }

    /// Merges two facts by selecting the longer/more informative text.
    pub fn merge_facts(text_a: &str, text_b: &str) -> String {
        if text_a.len() >= text_b.len() {
            text_a.to_string()
        } else {
            text_b.to_string()
        }
    }
}
