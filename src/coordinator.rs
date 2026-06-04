use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::layers::working::WorkingMemory;
use crate::layers::episodic::EpisodicMemory;
use crate::layers::semantic::SemanticMemory;
use crate::layers::graph::GraphMemory;
use crate::layers::codebase::CodebaseMemory;

pub struct MemoryCoordinator {
    pub working: Arc<WorkingMemory>,
    pub episodic: Arc<EpisodicMemory>,
    pub semantic: Arc<SemanticMemory>,
    pub graph: Arc<GraphMemory>,
    pub codebase: Arc<CodebaseMemory>,
}

impl MemoryCoordinator {
    pub fn new(db_path: &str) -> Result<Self> {
        let path = Path::new(db_path);

        let working = Arc::new(WorkingMemory::new());
        let episodic = Arc::new(EpisodicMemory::new(path)?);
        let semantic = Arc::new(SemanticMemory::new(path)?);
        let graph = Arc::new(GraphMemory::new(path)?);
        let codebase = Arc::new(CodebaseMemory::new(path)?);

        Ok(Self {
            working,
            episodic,
            semantic,
            graph,
            codebase,
        })
    }
}
