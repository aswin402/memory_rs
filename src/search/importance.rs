//! Auto-Importance Scoring calculation for memories.

/// Scorer for calculating memory importance based on usage, relationships, age, and reinforcement.
pub struct ImportanceScorer;

impl ImportanceScorer {
    /// Calculates an importance score between 0.0 and 1.0.
    ///
    /// The score is determined by:
    /// - Logarithmically scaled access count.
    /// - Logarithmically scaled edge/relationship count.
    /// - Exponentially decaying age freshness.
    /// - Constant reinforcement boost if flagged.
    pub fn calculate_importance(
        access_count: u32,
        edge_count: u32,
        age_hours: f64,
        was_reinforced: bool,
    ) -> f64 {
        let access_score = (access_count as f64 + 1.0).ln() / 10.0;
        let connection_score = (edge_count as f64 + 1.0).ln() / 5.0;
        let freshness = (-0.05 * age_hours).exp() / 3.0;
        let reinforcement = if was_reinforced { 0.3 } else { 0.0 };
        (access_score + connection_score + freshness + reinforcement).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_importance_calculation() {
        // Fresh memory with no accesses or edges
        let score_fresh = ImportanceScorer::calculate_importance(0, 0, 0.0, false);
        assert!((score_fresh - 0.333).abs() < 0.05, "Fresh score should be ~0.33");

        // Highly accessed and connected memory
        let score_high = ImportanceScorer::calculate_importance(100, 10, 24.0, true);
        assert!(score_high > 0.8, "High parameters should produce a high score");

        // Decayed memory
        let score_decayed = ImportanceScorer::calculate_importance(0, 0, 1000.0, false);
        assert!(score_decayed < 0.1, "Old and unused memory should decay close to 0");
    }
}
