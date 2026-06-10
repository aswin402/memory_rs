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

    let args: Vec<String> = std::env::args().collect();
    let grpc_port = if let Some(pos) = args.iter().position(|a| a == "--grpc") {
        args.get(pos + 1).and_then(|p| p.parse::<u16>().ok())
    } else {
        None
    };

    if let Some(port) = grpc_port {
        log::info!("Starting gRPC transport for Memory MCP server on port {}...", port);
        mcp::run_grpc_server(coordinator, port).await?;
    } else {
        log::info!("Starting Stdio transport for Memory MCP server...");
        mcp::run_server(coordinator).await?;
    }

    Ok(())
}
