use anyhow::Result;
use std::sync::Arc;

mod config;
mod coordinator;
mod layers;
mod mcp;
mod search;

use coordinator::MemoryCoordinator;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Initializing OpenZ-Inspired Memory Engine (memory_rs)...");

    let db_path = std::env::var("MEMORY_DB_PATH")
        .unwrap_or_else(|_| "memory.db".to_string());

    let coordinator = Arc::new(MemoryCoordinator::new(&db_path)?);

    log::info!("Starting Stdio transport for Memory MCP server...");
    mcp::run_server(coordinator).await?;

    Ok(())
}
