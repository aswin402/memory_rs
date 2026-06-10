use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

use crate::layers::working::WorkingMemory;
use crate::layers::episodic::EpisodicMemory;
use crate::layers::semantic::SemanticMemory;
use crate::layers::graph::GraphMemory;
use crate::layers::codebase::CodebaseMemory;
use crate::layers::shared::SharedMemory;

use parking_lot::Mutex;
use anyhow::Context;

pub struct MemoryCoordinator {
    pub working: Arc<WorkingMemory>,
    pub episodic: Arc<EpisodicMemory>,
    pub semantic: Arc<SemanticMemory>,
    pub graph: Arc<GraphMemory>,
    pub codebase: Arc<CodebaseMemory>,
    pub shared: Arc<SharedMemory>,
    pub base_db_path: String,
    pub active_branch: Mutex<Option<String>>,
}

impl MemoryCoordinator {
    pub fn new(db_path: &str) -> Result<Self> {
        let path = Path::new(db_path);

        let working = Arc::new(WorkingMemory::new());
        let episodic = Arc::new(EpisodicMemory::new(path)?);
        let semantic = Arc::new(SemanticMemory::new(path)?);
        let graph = Arc::new(GraphMemory::new(path)?);
        let codebase = Arc::new(CodebaseMemory::new(path)?);
        let shared = Arc::new(SharedMemory::new(path)?);

        Ok(Self {
            working,
            episodic,
            semantic,
            graph,
            codebase,
            shared,
            base_db_path: db_path.to_string(),
            active_branch: Mutex::new(None),
        })
    }

    pub fn create_branch(&self, branch_id: &str) -> Result<()> {
        let mut active = self.active_branch.lock();
        if active.is_some() {
            anyhow::bail!("A database branch is already active. Commit or rollback first.");
        }

        let branch_path = format!("{}.branch_{}", self.base_db_path, branch_id);
        std::fs::copy(&self.base_db_path, &branch_path)?;

        let path = Path::new(&branch_path);
        self.episodic.switch_connection(path)?;
        self.semantic.switch_connection(path)?;
        self.graph.switch_connection(path)?;
        self.codebase.switch_connection(path)?;
        self.shared.switch_connection(path)?;

        *active = Some(branch_id.to_string());
        log::info!("Switched memory coordinator to database branch: {}", branch_id);
        Ok(())
    }

    pub fn commit_branch(&self) -> Result<()> {
        let mut active = self.active_branch.lock();
        let branch_id = active.as_ref().context("No active branch to commit.")?.clone();
        let branch_path = format!("{}.branch_{}", self.base_db_path, branch_id);

        // Switch connections to an in-memory database to release file locks on both the base and branch files
        let in_memory_path = Path::new(":memory:");
        self.episodic.switch_connection(in_memory_path)?;
        self.semantic.switch_connection(in_memory_path)?;
        self.graph.switch_connection(in_memory_path)?;
        self.codebase.switch_connection(in_memory_path)?;
        self.shared.switch_connection(in_memory_path)?;

        // Safe copy: replace base database with the branch file
        std::fs::copy(&branch_path, &self.base_db_path)?;
        std::fs::remove_file(&branch_path)?;

        // Restore connections to the updated base file
        let main_path = Path::new(&self.base_db_path);
        self.episodic.switch_connection(main_path)?;
        self.semantic.switch_connection(main_path)?;
        self.graph.switch_connection(main_path)?;
        self.codebase.switch_connection(main_path)?;
        self.shared.switch_connection(main_path)?;

        *active = None;
        log::info!("Committed database branch: {}", branch_id);
        Ok(())
    }

    pub fn rollback_branch(&self) -> Result<()> {
        let mut active = self.active_branch.lock();
        let branch_id = active.as_ref().context("No active branch to rollback.")?.clone();
        let branch_path = format!("{}.branch_{}", self.base_db_path, branch_id);

        // Switch connections back to the base database
        let main_path = Path::new(&self.base_db_path);
        self.episodic.switch_connection(main_path)?;
        self.semantic.switch_connection(main_path)?;
        self.graph.switch_connection(main_path)?;
        self.codebase.switch_connection(main_path)?;
        self.shared.switch_connection(main_path)?;

        // Remove the branch file
        if Path::new(&branch_path).exists() {
            std::fs::remove_file(&branch_path)?;
        }

        *active = None;
        log::info!("Rolled back database branch: {}", branch_id);
        Ok(())
    }
}
