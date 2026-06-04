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
use std::path::Path;
use tokio::io::{stdin, stdout};

use crate::coordinator::MemoryCoordinator;
use crate::layers::graph::{Entity, Relation, AddObservationsInput, DeleteObservationsInput};
use crate::layers::episodic::{EpisodeLog, ReflectionItem, ToolPerformanceRecord};
use crate::layers::codebase::{CodeElement, RepositoryEvolution};
use crate::layers::shared::SharedMemoryItem;

// ==================== WRAPPER STRUCTS FOR INPUTS ====================
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

// Extended cognitive memory inputs
#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct IndexCodebaseInput {
    pub path: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct QueryCodeGraphInput {
    pub filePath: Option<String>,
    pub query: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct LogEpisodeInput {
    pub id: Option<String>,
    pub taskDescription: String,
    pub executionStatus: String,
    pub stepsTaken: String,
    pub errorMessage: Option<String>,
    pub reflection: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct LogReflectionInput {
    pub taskDescription: String,
    pub status: String,
    pub attemptNumber: i64,
    pub stepsTaken: String,
    pub errorEncountered: Option<String>,
    pub rootCause: Option<String>,
    pub solutionApplied: Option<String>,
    pub reflection: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct RetrieveReflectionsInput {
    pub query: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct RecordToolPerfInput {
    pub toolName: String,
    pub modelName: String,
    pub taskType: String,
    pub successCount: i64,
    pub failureCount: i64,
    pub averageLatency: f64,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct QueryToolPerfInput {
    pub taskType: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct StoreSharedMemoryInput {
    pub key: String,
    pub value: String,
    pub sourceAgent: String,
    pub targetAgents: Vec<String>,
    pub importance: Option<f64>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct RetrieveSharedMemoryInput {
    pub agentId: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct LogRepoEvolutionInput {
    pub filePath: String,
    pub version: String,
    pub commitHash: Option<String>,
    pub author: Option<String>,
    pub changeType: String, // "Added", "Modified", "Deleted"
    pub summaryOfChanges: String,
    pub bugIntroduced: bool,
    pub bugFixed: bool,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct QueryRepoEvolutionInput {
    pub filePath: Option<String>,
}

// ==================== MCP SERVER DEFINITION ====================
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

    // -------------------- KNOWLEDGE GRAPH TOOLS --------------------
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

    // -------------------- CODE INTELLIGENCE TOOLS --------------------
    #[tool(description = "Index functions, structs, enums and types in the codebase to build the codebase graph. Path defaults to '.'")]
    async fn index_codebase(&self, Parameters(input): Parameters<IndexCodebaseInput>) -> Result<CallToolResult, McpError> {
        let scan_path = input.path.unwrap_or_else(|| ".".to_string());
        let path = Path::new(&scan_path);
        match scan_directory(&self.coordinator, path) {
            Ok(count) => {
                let text = format!("Successfully indexed {} source files under {:?}", count, path);
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Query structural elements (structs, functions, impls) and calling patterns indexed in the codebase")]
    async fn query_code_graph(&self, Parameters(input): Parameters<QueryCodeGraphInput>) -> Result<CallToolResult, McpError> {
        let file_path = input.filePath.unwrap_or_default();
        let query = input.query.unwrap_or_default();
        
        match self.coordinator.codebase.query_elements(&file_path, &query) {
            Ok(elements) => {
                let text = serde_json::to_string_pretty(&elements).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- EPISODIC LEARNING & REFLECTIONS --------------------
    #[tool(description = "Log an execution episode: details tasks attempted, execution logs, status and reflections")]
    async fn log_execution_episode(&self, Parameters(input): Parameters<LogEpisodeInput>) -> Result<CallToolResult, McpError> {
        let id = input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let created_at = chrono::Utc::now().to_rfc3339();
        
        let ep = EpisodeLog {
            id,
            task_description: input.taskDescription,
            execution_status: input.executionStatus,
            steps_taken: input.stepsTaken,
            error_message: input.errorMessage,
            reflection: input.reflection,
            created_at,
        };

        match self.coordinator.episodic.log_episode(ep) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Episode logged successfully")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Store a reflection memory summarizing what worked, what failed, why, and error analysis")]
    async fn log_reflection(&self, Parameters(input): Parameters<LogReflectionInput>) -> Result<CallToolResult, McpError> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        let item = ReflectionItem {
            id,
            task_description: input.taskDescription,
            status: input.status,
            attempt_number: input.attemptNumber,
            steps_taken: input.stepsTaken,
            error_encountered: input.errorEncountered,
            root_cause: input.rootCause,
            solution_applied: input.solutionApplied,
            reflection: input.reflection,
            created_at,
        };

        match self.coordinator.episodic.log_reflection(item) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Reflection logged successfully")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Retrieve reflections to guide current attempts based on query parameters")]
    async fn retrieve_episodic_reflections(&self, Parameters(input): Parameters<RetrieveReflectionsInput>) -> Result<CallToolResult, McpError> {
        let query = input.query.unwrap_or_default();
        match self.coordinator.episodic.get_reflections(&query) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- TOOL PERFORMANCE METRICS --------------------
    #[tool(description = "Record the success rates and latencies of an LLM or specific tool usage")]
    async fn record_tool_performance(&self, Parameters(input): Parameters<RecordToolPerfInput>) -> Result<CallToolResult, McpError> {
        let last_used = chrono::Utc::now().to_rfc3339();
        let rec = ToolPerformanceRecord {
            tool_name: input.toolName,
            model_name: input.modelName,
            task_type: input.taskType,
            success_count: input.successCount,
            failure_count: input.failureCount,
            average_latency: input.averageLatency,
            last_used,
        };

        match self.coordinator.episodic.record_tool_performance(rec) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Tool performance metrics recorded")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Query tool performance logs to recommend optimal tools/models for specific task types")]
    async fn query_tool_performance(&self, Parameters(input): Parameters<QueryToolPerfInput>) -> Result<CallToolResult, McpError> {
        match self.coordinator.episodic.query_tool_performance(&input.taskType) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- MULTI-AGENT SHARED MEMORY --------------------
    #[tool(description = "Store a key-value memory shared across target agent IDs")]
    async fn store_shared_team_memory(&self, Parameters(input): Parameters<StoreSharedMemoryInput>) -> Result<CallToolResult, McpError> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let item = SharedMemoryItem {
            key: input.key,
            value: input.value,
            source_agent: input.sourceAgent,
            target_agents: input.targetAgents,
            importance: input.importance.unwrap_or(1.0),
            timestamp,
        };

        match self.coordinator.shared.store_shared_memory(item) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Shared team memory stored successfully")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Retrieve shared team memories targetted at a specific agent ID (or wildcard '*')")]
    async fn retrieve_shared_team_memory(&self, Parameters(input): Parameters<RetrieveSharedMemoryInput>) -> Result<CallToolResult, McpError> {
        let agent_id = input.agentId.unwrap_or_default();
        match self.coordinator.shared.retrieve_shared_memory(&agent_id) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- REPOSITORY EVOLUTION --------------------
    #[tool(description = "Log file changes, refactoring records, commits, versions, and bug status metrics")]
    async fn log_repository_evolution(&self, Parameters(input): Parameters<LogRepoEvolutionInput>) -> Result<CallToolResult, McpError> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let evo = RepositoryEvolution {
            file_path: input.filePath,
            version: input.version,
            commit_hash: input.commitHash,
            author: input.author,
            change_type: input.changeType,
            summary_of_changes: input.summaryOfChanges,
            bug_introduced: input.bugIntroduced,
            bug_fixed: input.bugFixed,
            timestamp,
        };

        match self.coordinator.codebase.log_evolution(evo) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text("Repository evolution stage logged")])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Query repository file history and change statistics")]
    async fn query_repository_evolution(&self, Parameters(input): Parameters<QueryRepoEvolutionInput>) -> Result<CallToolResult, McpError> {
        let file_path = input.filePath.unwrap_or_default();
        match self.coordinator.codebase.query_evolution(&file_path) {
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

// -------------------- STATIC PARSER HELPERS --------------------
fn parse_and_index_file(coordinator: &MemoryCoordinator, file_path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(file_path)?;
    let relative_path = file_path.to_string_lossy().to_string();
    
    let lines: Vec<&str> = content.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as i64;
        let trimmed = line.trim();
        
        let mut element_type = None;
        let mut name = None;
        let mut signature = String::new();
        
        // Simple regex-like symbol definitions for Rust, Python, and JS/TS
        if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") {
            element_type = Some("Function".to_string());
            let parts: Vec<&str> = trimmed.split('(').collect();
            if !parts.is_empty() {
                let name_part = parts[0];
                let fn_name = name_part.split_whitespace().last().unwrap_or("unknown");
                name = Some(fn_name.to_string());
                signature = parts[0].to_string() + "(...)";
            }
        } else if trimmed.starts_with("def ") {
            element_type = Some("Function".to_string());
            let parts: Vec<&str> = trimmed.split('(').collect();
            if !parts.is_empty() {
                let name_part = parts[0];
                let fn_name = name_part.split_whitespace().last().unwrap_or("unknown");
                name = Some(fn_name.to_string());
                signature = parts[0].to_string() + "(...)";
            }
        } else if trimmed.starts_with("pub struct ") || trimmed.starts_with("struct ") {
            element_type = Some("Struct".to_string());
            let parts: Vec<&str> = trimmed.split('{').collect();
            let struct_name = parts[0].split_whitespace().last().unwrap_or("unknown");
            name = Some(struct_name.to_string());
            signature = parts[0].trim().to_string();
        } else if trimmed.starts_with("class ") {
            element_type = Some("Class".to_string());
            let parts: Vec<&str> = trimmed.split(':').collect();
            let class_name = parts[0].split_whitespace().last().unwrap_or("unknown");
            name = Some(class_name.to_string());
            signature = parts[0].trim().to_string();
        } else if trimmed.starts_with("pub impl") || trimmed.starts_with("impl") {
            element_type = Some("ImplBlock".to_string());
            let parts: Vec<&str> = trimmed.split('{').collect();
            let impl_name = parts[0].split_whitespace().last().unwrap_or("unknown");
            name = Some(format!("impl_{}", impl_name));
            signature = parts[0].trim().to_string();
        } else if trimmed.starts_with("pub enum ") || trimmed.starts_with("enum ") {
            element_type = Some("Enum".to_string());
            let parts: Vec<&str> = trimmed.split('{').collect();
            let enum_name = parts[0].split_whitespace().last().unwrap_or("unknown");
            name = Some(enum_name.to_string());
            signature = parts[0].trim().to_string();
        }
        
        if let (Some(el_type), Some(el_name)) = (element_type, name) {
            let el_id = format!("{}:{}:{}", relative_path, el_name, line_num);
            let el = CodeElement {
                id: el_id,
                file_path: relative_path.clone(),
                element_type: el_type,
                name: el_name,
                signature,
                ast_json: None,
                parent_id: None,
                start_line: line_num,
                end_line: line_num + 5,
            };
            coordinator.codebase.index_element(el)?;
        }
    }
    Ok(())
}

fn scan_directory(coordinator: &MemoryCoordinator, dir: &Path) -> Result<i64> {
    let mut count = 0;
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != "target" && name != ".git" && name != "external" && name != "node_modules" {
                    count += scan_directory(coordinator, &path)?;
                }
            } else {
                let ext = path.extension().unwrap_or_default().to_string_lossy();
                if ext == "rs" || ext == "py" || ext == "js" || ext == "ts" || ext == "go" {
                    if let Err(e) = parse_and_index_file(coordinator, &path) {
                        log::error!("Failed to index file {:?}: {}", path, e);
                    } else {
                        count += 1;
                    }
                }
            }
        }
    }
    Ok(count)
}

pub async fn run_server(coordinator: Arc<MemoryCoordinator>) -> Result<()> {
    let service = MemoryServer::new(coordinator);
    let transport = (stdin(), stdout());
    
    log::info!("Serving openmemory_rs Server over stdio...");
    service.serve(transport).await?.waiting().await?;
    
    Ok(())
}
