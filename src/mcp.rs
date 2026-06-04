use anyhow::Result;
use rmcp::{
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::*,
    tool, tool_handler, tool_router,
    ErrorData as McpError,
    ServiceExt,
};
use std::sync::Arc;
use tokio::io::{stdin, stdout};

use crate::coordinator::MemoryCoordinator;
use crate::layers::graph::{Entity, Relation, AddObservationsInput, DeleteObservationsInput};

// Wrapper structs for tool inputs to comply with rmcp's deserialization
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct CreateEntitiesInput {
    pub entities: Vec<Entity>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct CreateRelationsInput {
    pub relations: Vec<Relation>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AddObservationsWrapper {
    pub observations: Vec<AddObservationsInput>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteEntitiesInput {
    pub entityNames: Vec<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteObservationsWrapper {
    pub deletions: Vec<DeleteObservationsInput>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteRelationsInput {
    pub relations: Vec<Relation>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct EmptyInput {
    pub dummy: Option<bool>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SearchNodesInput {
    pub query: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct OpenNodesInput {
    pub names: Vec<String>,
}

#[derive(Clone)]
pub struct MemoryServer {
    coordinator: Arc<MemoryCoordinator>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl MemoryServer {
    pub fn new(coordinator: Arc<MemoryCoordinator>) -> Self {
        Self {
            coordinator,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Create multiple new entities in the knowledge graph")]
    async fn create_entities(&self, Parameters(input): Parameters<CreateEntitiesInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.create_entities(input.entities) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Create multiple new relations between entities in the knowledge graph. Relations should be in active voice")]
    async fn create_relations(&self, Parameters(input): Parameters<CreateRelationsInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.create_relations(input.relations) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Add new observations to existing entities in the knowledge graph")]
    async fn add_observations(&self, Parameters(input): Parameters<AddObservationsWrapper>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.add_observations(input.observations) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Delete multiple entities and their associated relations from the knowledge graph")]
    async fn delete_entities(&self, Parameters(input): Parameters<DeleteEntitiesInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.delete_entities(input.entityNames) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Entities deleted successfully")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Delete specific observations from entities in the knowledge graph")]
    async fn delete_observations(&self, Parameters(input): Parameters<DeleteObservationsWrapper>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.delete_observations(input.deletions) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Observations deleted successfully")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Delete multiple relations from the knowledge graph")]
    async fn delete_relations(&self, Parameters(input): Parameters<DeleteRelationsInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.delete_relations(input.relations) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Relations deleted successfully")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Read the entire knowledge graph")]
    async fn read_graph(&self, _input: Parameters<EmptyInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.read_graph() {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Search for nodes in the knowledge graph based on a query")]
    async fn search_nodes(&self, Parameters(input): Parameters<SearchNodesInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.search_nodes(&input.query) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Open specific nodes in the knowledge graph by their names")]
    async fn open_nodes(&self, Parameters(input): Parameters<OpenNodesInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.graph.open_nodes(input.names) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }
}

#[tool_handler]
impl rmcp::ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {
        InitializeResult {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

pub async fn run_server(coordinator: Arc<MemoryCoordinator>) -> Result<()> {
    let service = MemoryServer::new(coordinator);
    let transport = (stdin(), stdout());
    
    log::info!("Serving Memory Server over stdio...");
    service.serve(transport).await?.waiting().await?;
    
    Ok(())
}
