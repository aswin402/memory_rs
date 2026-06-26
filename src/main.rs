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
    log::info!("Initializing OpenMemory Cognitive Engine (memory_rs)...");

    let config = config::Config::from_env();
    let coordinator = Arc::new(MemoryCoordinator::new(&config.db_path, config.default_ttl)?);

    let args: Vec<String> = std::env::args().collect();
    let grpc_port = if let Some(pos) = args.iter().position(|a| a == "--grpc") {
        args.get(pos + 1).and_then(|p| p.parse::<u16>().ok())
    } else {
        None
    };

    // Setup graceful shutdown channel
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let shutdown_tx_clone = shutdown_tx.clone();

    tokio::spawn(async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Received Ctrl+C signal, initiating graceful shutdown...");
            }
            _ = async {
                #[cfg(unix)]
                {
                    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
                    sigterm.recv().await;
                    log::info!("Received SIGTERM signal, initiating graceful shutdown...");
                }
                #[cfg(not(unix))]
                {
                    // Sleep indefinitely on non-unix platforms
                    tokio::time::sleep(tokio::time::Duration::from_secs(999999)).await;
                }
            } => {}
        }
        let _ = shutdown_tx_clone.send(()).await;
    });

    if let Some(port) = grpc_port {
        log::info!(
            "Starting gRPC transport for Memory MCP server on port {}...",
            port
        );
        mcp::run_grpc_server(coordinator.clone(), port, shutdown_rx).await?;
    } else {
        log::info!("Starting Stdio transport for Memory MCP server...");
        mcp::run_server(coordinator.clone(), shutdown_rx).await?;
    }

    log::info!("Performing final database checkpoints...");
    if let Err(e) = coordinator.checkpoint() {
        log::error!("Failed to run WAL checkpoints during graceful shutdown: {:?}", e);
    } else {
        log::info!("All database checkpoints completed successfully.");
    }

    Ok(())
}
