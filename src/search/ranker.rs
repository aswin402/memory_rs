pub struct MemoryItem {
    pub content: String,
    pub similarity: f64,
    pub elapsed_hours: f64,
    pub importance: f64,
    pub success_rate: f64,
}

pub struct Ranker;

impl Ranker {
    pub fn score(item: &MemoryItem, alpha: f64, beta: f64, gamma: f64, delta: f64) -> f64 {
        // Recency decay factor (e^-0.01 * elapsed_hours)
        let lambda = 0.01;
        let recency = (-lambda * item.elapsed_hours).exp();

        (alpha * item.similarity)
            + (beta * recency)
            + (gamma * item.importance)
            + (delta * item.success_rate)
    }
}
