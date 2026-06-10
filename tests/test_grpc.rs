use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

pub mod mcp_grpc {
    tonic::include_proto!("mcp");
}

use mcp_grpc::mcp_service_client::McpServiceClient;
use mcp_grpc::{McpRequest, McpResponse};

#[tokio::test]
async fn test_grpc_mcp_flow() -> Result<(), Box<dyn std::error::Error>> {
    let bin_path = std::env::var("CARGO_BIN_EXE_openmemory_rs")
        .unwrap_or_else(|_| "../target/release/openmemory_rs".to_string());
        
    println!("Spawning server from {}...", bin_path);
    
    // Clean up any old test database
    let db_path = "test_grpc_memory.db";
    if std::path::Path::new(db_path).exists() {
        let _ = std::fs::remove_file(db_path);
    }
    if std::path::Path::new(&format!("{}.branch_test_grpc_branch", db_path)).exists() {
        let _ = std::fs::remove_file(&format!("{}.branch_test_grpc_branch", db_path));
    }
    
    let mut child = Command::new(&bin_path)
        .arg("--grpc")
        .arg("50059")
        .env("MEMORY_DB_PATH", db_path)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
        
    // Wait for server to bind with a retry loop
    let mut client = None;
    for i in 0..20 {
        sleep(Duration::from_millis(500)).await;
        match tonic::transport::Channel::from_static("http://127.0.0.1:50059")
            .connect()
            .await
        {
            Ok(channel) => {
                client = Some(McpServiceClient::new(channel));
                println!("Successfully connected to gRPC server after {}ms", (i + 1) * 500);
                break;
            }
            Err(e) => {
                if i == 19 {
                    return Err(format!("Failed to connect to gRPC server after 10s: {}", e).into());
                }
            }
        }
    }
    let mut client = client.unwrap();
    
    // 1. Initialize
    println!("Sending initialize...");
    let init_params = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {},
        "clientInfo": {
            "name": "test-client",
            "version": "1.0"
        }
    });
    
    let init_req = McpRequest {
        method: "initialize".to_string(),
        params_json: init_params.to_string(),
        id: 1,
        has_id: true,
    };
    let init_resp = client.call(init_req).await?.into_inner();
    assert!(!init_resp.result_json.is_empty(), "Initialize response should not be empty");
    println!("Initialize Response: {}", init_resp.result_json);
    
    // 2. notifications/initialized
    println!("Sending notifications/initialized...");
    let notif_req = McpRequest {
        method: "notifications/initialized".to_string(),
        params_json: "{}".to_string(),
        id: 0,
        has_id: false,
    };
    client.call(notif_req).await?;
    
    // 3. tools/list
    println!("Sending tools/list...");
    let list_req = McpRequest {
        method: "tools/list".to_string(),
        params_json: "{}".to_string(),
        id: 2,
        has_id: true,
    };
    let list_resp = client.call(list_req).await?.into_inner();
    assert!(!list_resp.result_json.is_empty(), "Tools list should not be empty");
    let list_val: serde_json::Value = serde_json::from_str(&list_resp.result_json)?;
    let tools = list_val.get("tools").and_then(|t| t.as_array()).expect("Expected tools array");
    println!("Loaded {} tools successfully.", tools.len());
    assert!(tools.len() > 0, "Should have loaded at least one tool");
    
    // 4. tools/call: create_database_branch
    println!("Creating branch via gRPC...");
    let branch_params = serde_json::json!({
        "name": "create_database_branch",
        "arguments": {
            "branchId": "test_grpc_branch"
        }
    });
    let branch_req = McpRequest {
        method: "tools/call".to_string(),
        params_json: branch_params.to_string(),
        id: 3,
        has_id: true,
    };
    let branch_resp = client.call(branch_req).await?.into_inner();
    assert!(branch_resp.error_json.is_empty(), "Should not return error");
    println!("Branch Response: {}", branch_resp.result_json);
    
    // Verify branch file exists
    let branch_db_file = format!("{}.branch_test_grpc_branch", db_path);
    assert!(std::path::Path::new(&branch_db_file).exists(), "Branch file must exist");
    println!("✓ Branch file exists on disk.");
    
    // 5. tools/call: rollback_database_branch
    println!("Rolling back branch via gRPC...");
    let rollback_params = serde_json::json!({
        "name": "rollback_database_branch",
        "arguments": {}
    });
    let rollback_req = McpRequest {
        method: "tools/call".to_string(),
        params_json: rollback_params.to_string(),
        id: 4,
        has_id: true,
    };
    let rollback_resp = client.call(rollback_req).await?.into_inner();
    assert!(rollback_resp.error_json.is_empty(), "Should not return error");
    println!("Rollback Response: {}", rollback_resp.result_json);
    
    // Verify branch file was deleted
    assert!(!std::path::Path::new(&branch_db_file).exists(), "Branch file must be deleted");
    println!("✓ Branch file was deleted from disk.");
    
    // Kill the server process
    child.kill()?;
    
    // Cleanup DB files
    let _ = std::fs::remove_file(db_path);
    
    println!("All gRPC integration tests passed successfully!");
    Ok(())
}
