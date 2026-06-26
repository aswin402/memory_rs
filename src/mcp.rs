use anyhow::Result;
use rmcp::{
    ErrorData as McpError, ServiceExt, handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters, model::*, tool, tool_handler, tool_router,
};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{stdin, stdout};

use crate::coordinator::MemoryCoordinator;
use crate::layers::codebase::{CodeElement, RepositoryEvolution};
use crate::layers::episodic::{EpisodeLog, ReflectionItem, ToolPerformanceRecord};
use crate::layers::graph::{AddObservationsInput, DeleteObservationsInput, Entity, Relation};
use crate::layers::shared::SharedMemoryItem;
use tree_sitter::{Node, Parser};

// ==================== WRAPPER STRUCTS FOR INPUTS ====================
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateEntitiesInput {
    pub entities: Vec<Entity>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateRelationsInput {
    pub relations: Vec<Relation>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddObservationsWrapper {
    pub observations: Vec<AddObservationsInput>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteEntitiesInput {
    pub entity_names: Vec<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteObservationsWrapper {
    pub deletions: Vec<DeleteObservationsInput>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRelationsInput {
    pub relations: Vec<Relation>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmptyInput {
    pub dummy: Option<bool>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchNodesInput {
    pub query: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OpenNodesInput {
    pub names: Vec<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

// Extended cognitive memory inputs
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IndexCodebaseInput {
    pub path: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueryCodeGraphInput {
    pub file_path: Option<String>,
    pub query: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogEpisodeInput {
    pub id: Option<String>,
    pub task_description: String,
    pub execution_status: String,
    pub steps_taken: String,
    pub error_message: Option<String>,
    pub reflection: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogReflectionInput {
    pub task_description: String,
    pub status: String,
    pub attempt_number: i64,
    pub steps_taken: String,
    pub error_encountered: Option<String>,
    pub root_cause: Option<String>,
    pub solution_applied: Option<String>,
    pub reflection: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RetrieveReflectionsInput {
    pub query: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchTextInput {
    pub query: String,
    pub limit: Option<usize>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordToolPerfInput {
    pub tool_name: String,
    pub model_name: String,
    pub task_type: String,
    pub success_count: i64,
    pub failure_count: i64,
    pub average_latency: f64,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueryToolPerfInput {
    pub task_type: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StoreSharedMemoryInput {
    pub key: String,
    pub value: String,
    pub source_agent: String,
    pub target_agents: Vec<String>,
    pub importance: Option<f64>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RetrieveSharedMemoryInput {
    pub agent_id: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id_scope: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogRepoEvolutionInput {
    pub file_path: String,
    pub version: String,
    pub commit_hash: Option<String>,
    pub author: Option<String>,
    pub change_type: String,
    pub summary_of_changes: String,
    pub bug_introduced: bool,
    pub bug_fixed: bool,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueryRepoEvolutionInput {
    pub file_path: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BranchIdInput {
    pub branch_id: String,
}

fn get_scope(
    user_id: &Option<String>,
    session_id: &Option<String>,
    agent_id: &Option<String>,
) -> crate::layers::MemoryScope {
    crate::layers::MemoryScope {
        user_id: user_id.clone(),
        session_id: session_id.clone(),
        agent_id: agent_id.clone(),
    }
}

fn get_accessed_by(user_id: &Option<String>, agent_id: &Option<String>) -> String {
    agent_id
        .as_deref()
        .or(user_id.as_deref())
        .unwrap_or("unknown")
        .to_string()
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
    async fn create_entities(
        &self,
        Parameters(input): Parameters<CreateEntitiesInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        match self.coordinator.graph.create_entities(input.entities, &scope) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Create multiple new relations between entities in the knowledge graph. Relations should be in active voice"
    )]
    async fn create_relations(
        &self,
        Parameters(input): Parameters<CreateRelationsInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        match self.coordinator.graph.create_relations(input.relations, &scope) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Add new observations to existing entities in the knowledge graph")]
    async fn add_observations(
        &self,
        Parameters(input): Parameters<AddObservationsWrapper>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        match self.coordinator.graph.add_observations(input.observations, &scope) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Delete multiple entities and their associated relations from the knowledge graph"
    )]
    async fn delete_entities(
        &self,
        Parameters(input): Parameters<DeleteEntitiesInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        match self.coordinator.graph.delete_entities(input.entity_names, &scope) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Entities deleted successfully",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Delete specific observations from entities in the knowledge graph")]
    async fn delete_observations(
        &self,
        Parameters(input): Parameters<DeleteObservationsWrapper>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        match self.coordinator.graph.delete_observations(input.deletions, &scope) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Observations deleted successfully",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Delete multiple relations from the knowledge graph")]
    async fn delete_relations(
        &self,
        Parameters(input): Parameters<DeleteRelationsInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        match self.coordinator.graph.delete_relations(input.relations, &scope) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Relations deleted successfully",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Read the entire knowledge graph")]
    async fn read_graph(&self, Parameters(input): Parameters<EmptyInput>) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let res = self.coordinator.graph.read_graph(&scope);
        if let Ok(ref graph) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for entity in &graph.entities {
                let _ = self.coordinator.episodic.log_access(&entity.name, "graph", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Search for nodes in the knowledge graph based on a query")]
    async fn search_nodes(
        &self,
        Parameters(input): Parameters<SearchNodesInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let res = self.coordinator.graph.search_nodes(&input.query, &scope);
        if let Ok(ref graph) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for entity in &graph.entities {
                let _ = self.coordinator.episodic.log_access(&entity.name, "graph", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Open specific nodes in the knowledge graph by their names")]
    async fn open_nodes(
        &self,
        Parameters(input): Parameters<OpenNodesInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let res = self.coordinator.graph.open_nodes(input.names, &scope);
        if let Ok(ref graph) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for entity in &graph.entities {
                let _ = self.coordinator.episodic.log_access(&entity.name, "graph", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- CODE INTELLIGENCE TOOLS --------------------
    #[tool(
        description = "Index functions, structs, enums and types in the codebase to build the codebase graph. Path defaults to '.'"
    )]
    async fn index_codebase(
        &self,
        Parameters(input): Parameters<IndexCodebaseInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let scan_path = input.path.unwrap_or_else(|| ".".to_string());
        let path = Path::new(&scan_path);
        match scan_directory(&self.coordinator, path, &scope) {
            Ok(count) => {
                let text = format!(
                    "Successfully indexed {} source files under {:?}",
                    count, path
                );
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Query structural elements (structs, functions, impls) and calling patterns indexed in the codebase"
    )]
    async fn query_code_graph(
        &self,
        Parameters(input): Parameters<QueryCodeGraphInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let file_path = input.file_path.unwrap_or_default();
        let query = input.query.unwrap_or_default();

        let res = self.coordinator.codebase.query_elements(&file_path, &query, &scope);
        if let Ok(ref elements) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for el in elements {
                let _ = self.coordinator.episodic.log_access(&el.id, "codebase", &accessed_by);
            }
        }
        match res {
            Ok(elements) => {
                let text = serde_json::to_string_pretty(&elements).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- EPISODIC LEARNING & REFLECTIONS --------------------
    #[tool(
        description = "Log an execution episode: details tasks attempted, execution logs, status and reflections"
    )]
    async fn log_execution_episode(
        &self,
        Parameters(input): Parameters<LogEpisodeInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let id = input.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let created_at = chrono::Utc::now().to_rfc3339();

        let ep = EpisodeLog {
            id,
            task_description: input.task_description,
            execution_status: input.execution_status,
            steps_taken: input.steps_taken,
            error_message: input.error_message,
            reflection: input.reflection,
            created_at,
        };

        match self.coordinator.episodic.log_episode(ep, &scope) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Episode logged successfully",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Store a reflection memory summarizing what worked, what failed, why, and error analysis"
    )]
    async fn log_reflection(
        &self,
        Parameters(input): Parameters<LogReflectionInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        let item = ReflectionItem {
            id,
            task_description: input.task_description,
            status: input.status,
            attempt_number: input.attempt_number,
            steps_taken: input.steps_taken,
            error_encountered: input.error_encountered,
            root_cause: input.root_cause,
            solution_applied: input.solution_applied,
            reflection: input.reflection,
            created_at,
        };

        match self.coordinator.episodic.log_reflection(item, &scope) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Reflection logged successfully",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Retrieve reflections to guide current attempts based on query parameters"
    )]
    async fn retrieve_episodic_reflections(
        &self,
        Parameters(input): Parameters<RetrieveReflectionsInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let query = input.query.unwrap_or_default();
        let res = self.coordinator.episodic.get_reflections(&query, &scope);
        if let Ok(ref reflections) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for r in reflections {
                let _ = self.coordinator.episodic.log_access(&r.id, "episodic", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Search for semantic facts using keyword-based SQLite FTS5 index. Matches prefix terms (e.g. 'rust*' or 'memory*')."
    )]
    async fn search_text(
        &self,
        Parameters(input): Parameters<SearchTextInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let limit = input.limit.unwrap_or(10);
        let res = self.coordinator.semantic.search_text(&input.query, limit, &scope);
        if let Ok(ref facts) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for f in facts {
                let _ = self.coordinator.episodic.log_access(&f.node_id, "semantic", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Search for semantic facts using hybrid search (vector search + FTS5 full-text search) merged via Reciprocal Rank Fusion (RRF)."
    )]
    async fn hybrid_search(
        &self,
        Parameters(input): Parameters<SearchTextInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let limit = input.limit.unwrap_or(10);
        let res = self.coordinator.semantic.query_similar_facts(&input.query, limit, &scope);
        if let Ok(ref facts) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for f in facts {
                let _ = self.coordinator.episodic.log_access(&f.node_id, "semantic", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- TOOL PERFORMANCE METRICS --------------------
    #[tool(description = "Record the success rates and latencies of an LLM or specific tool usage")]
    async fn record_tool_performance(
        &self,
        Parameters(input): Parameters<RecordToolPerfInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let last_used = chrono::Utc::now().to_rfc3339();
        let rec = ToolPerformanceRecord {
            tool_name: input.tool_name,
            model_name: input.model_name,
            task_type: input.task_type,
            success_count: input.success_count,
            failure_count: input.failure_count,
            average_latency: input.average_latency,
            last_used,
        };

        match self.coordinator.episodic.record_tool_performance(rec, &scope) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Tool performance metrics recorded",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Query tool performance logs to recommend optimal tools/models for specific task types"
    )]
    async fn query_tool_performance(
        &self,
        Parameters(input): Parameters<QueryToolPerfInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let res = self.coordinator.episodic.query_tool_performance(&input.task_type, &scope);
        if let Ok(ref records) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id);
            for r in records {
                let _ = self.coordinator.episodic.log_access(&r.tool_name, "episodic", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- MULTI-AGENT SHARED MEMORY --------------------
    #[tool(description = "Store a key-value memory shared across target agent IDs")]
    async fn store_shared_team_memory(
        &self,
        Parameters(input): Parameters<StoreSharedMemoryInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id);
        let timestamp = chrono::Utc::now().to_rfc3339();
        let item = SharedMemoryItem {
            key: input.key,
            value: input.value,
            source_agent: input.source_agent,
            target_agents: input.target_agents,
            importance: input.importance.unwrap_or(1.0),
            timestamp,
        };

        match self.coordinator.shared.store_shared_memory(item, &scope) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Shared team memory stored successfully",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Retrieve shared team memories targetted at a specific agent ID (or wildcard '*')"
    )]
    async fn retrieve_shared_team_memory(
        &self,
        Parameters(input): Parameters<RetrieveSharedMemoryInput>,
    ) -> Result<CallToolResult, McpError> {
        let scope = get_scope(&input.user_id, &input.session_id, &input.agent_id_scope);
        let agent_id = input.agent_id.unwrap_or_default();
        let res = self.coordinator.shared.retrieve_shared_memory(&agent_id, &scope);
        if let Ok(ref items) = res {
            let accessed_by = get_accessed_by(&input.user_id, &input.agent_id_scope);
            for item in items {
                let _ = self.coordinator.episodic.log_access(&item.key, "shared", &accessed_by);
            }
        }
        match res {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Get memory access statistics and record counts for all layers")]
    async fn memory_stats(
        &self,
        Parameters(input): Parameters<EmptyInput>,
    ) -> Result<CallToolResult, McpError> {
        match self.coordinator.episodic.get_memory_stats() {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    // -------------------- REPOSITORY EVOLUTION --------------------
    #[tool(
        description = "Log file changes, refactoring records, commits, versions, and bug status metrics"
    )]
    async fn log_repository_evolution(
        &self,
        Parameters(input): Parameters<LogRepoEvolutionInput>,
    ) -> Result<CallToolResult, McpError> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let evo = RepositoryEvolution {
            file_path: input.file_path,
            version: input.version,
            commit_hash: input.commit_hash,
            author: input.author,
            change_type: input.change_type,
            summary_of_changes: input.summary_of_changes,
            bug_introduced: input.bug_introduced,
            bug_fixed: input.bug_fixed,
            timestamp,
        };

        match self.coordinator.codebase.log_evolution(evo) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Repository evolution stage logged",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(description = "Query repository file history and change statistics")]
    async fn query_repository_evolution(
        &self,
        Parameters(input): Parameters<QueryRepoEvolutionInput>,
    ) -> Result<CallToolResult, McpError> {
        let file_path = input.file_path.unwrap_or_default();
        match self.coordinator.codebase.query_evolution(&file_path) {
            Ok(res) => {
                let text = serde_json::to_string_pretty(&res).unwrap_or_default();
                Ok(CallToolResult::success(vec![Content::text(text)]))
            }
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Create an isolated database branch for subagent/task execution. Branch ID must be unique"
    )]
    async fn create_database_branch(
        &self,
        Parameters(input): Parameters<BranchIdInput>,
    ) -> Result<CallToolResult, McpError> {
        match self.coordinator.create_branch(&input.branch_id) {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Successfully created database branch: {}",
                input.branch_id
            ))])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Commit changes from the active database branch to the main database and delete the branch"
    )]
    async fn commit_database_branch(
        &self,
        _input: Parameters<EmptyInput>,
    ) -> Result<CallToolResult, McpError> {
        match self.coordinator.commit_branch() {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Successfully committed database branch",
            )])),
            Err(e) => Err(McpError::internal_error(e.to_string(), None)),
        }
    }

    #[tool(
        description = "Roll back changes from the active database branch, restoring the main database state and deleting the branch"
    )]
    async fn rollback_database_branch(
        &self,
        _input: Parameters<EmptyInput>,
    ) -> Result<CallToolResult, McpError> {
        match self.coordinator.rollback_branch() {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Successfully rolled back database branch",
            )])),
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
// -------------------- STATIC PARSER HELPERS --------------------
fn get_node_signature(node: &Node, source_bytes: &[u8], body_delim: &str) -> String {
    if let Ok(text) = node.utf8_text(source_bytes) {
        if let Some(idx) = text.find(body_delim) {
            let sig = text[..idx].trim().to_string();
            let sig = sig.replace('\n', " ");
            let sig: Vec<&str> = sig.split_whitespace().collect();
            sig.join(" ")
        } else {
            text.lines().next().unwrap_or("").trim().to_string()
        }
    } else {
        String::new()
    }
}

fn node_name(node: &Node, source_bytes: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        if child.kind() == "identifier"
            || child.kind() == "type_identifier"
            || child.kind() == "property_identifier"
        {
            if let Ok(name) = child.utf8_text(source_bytes) {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn traverse_and_index(
    coordinator: &MemoryCoordinator,
    relative_path: &str,
    node: Node,
    source_bytes: &[u8],
    parent_id: Option<&str>,
    scope: &crate::layers::MemoryScope,
) -> Result<()> {
    let kind = node.kind();
    let mut current_id = None;
    let start_pos = node.start_position();
    let end_pos = node.end_position();
    let start_line = (start_pos.row + 1) as i64;
    let end_line = (end_pos.row + 1) as i64;

    let mut element_type = None;
    let mut name = None;
    let mut signature = String::new();

    match kind {
        // Rust grammars
        "function_item" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some(if parent_id.is_some() {
                    "Method".to_string()
                } else {
                    "Function".to_string()
                });
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "struct_item" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Struct".to_string());
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "enum_item" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Enum".to_string());
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "trait_item" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Trait".to_string());
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "impl_item" => {
            let impl_sig = get_node_signature(&node, source_bytes, "{");
            name = Some(impl_sig.clone());
            element_type = Some("ImplBlock".to_string());
            signature = impl_sig;
        }

        // Python grammars
        "class_definition" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Class".to_string());
                signature = get_node_signature(&node, source_bytes, ":");
            }
        }
        "function_definition" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some(if parent_id.is_some() {
                    "Method".to_string()
                } else {
                    "Function".to_string()
                });
                signature = get_node_signature(&node, source_bytes, ":");
            }
        }

        // JavaScript / TypeScript grammars
        "class_declaration" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Class".to_string());
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "function_declaration" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some(if parent_id.is_some() {
                    "Method".to_string()
                } else {
                    "Function".to_string()
                });
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "method_definition" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Method".to_string());
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "interface_declaration" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Interface".to_string());
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        "type_alias_declaration" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("TypeAlias".to_string());
                signature = get_node_signature(&node, source_bytes, "=");
            }
        }
        "enum_declaration" => {
            if let Some(n) = node_name(&node, source_bytes) {
                name = Some(n);
                element_type = Some("Enum".to_string());
                signature = get_node_signature(&node, source_bytes, "{");
            }
        }
        _ => {}
    }

    if let (Some(el_type), Some(el_name)) = (element_type, name) {
        let el_id = format!("{}:{}:{}", relative_path, el_name, start_line);
        current_id = Some(el_id.clone());

        let el = CodeElement {
            id: el_id,
            file_path: relative_path.to_string(),
            element_type: el_type,
            name: el_name,
            signature,
            ast_json: Some(node.to_sexp()),
            parent_id: parent_id.map(String::from),
            start_line,
            end_line,
        };
        coordinator.codebase.index_element(el, scope)?;
    }

    let next_parent = current_id.as_deref().or(parent_id);
    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        traverse_and_index(coordinator, relative_path, child, source_bytes, next_parent, scope)?;
    }

    Ok(())
}

fn parse_and_index_file_fallback(
    coordinator: &MemoryCoordinator,
    file_path: &Path,
    scope: &crate::layers::MemoryScope,
) -> Result<()> {
    let content = std::fs::read_to_string(file_path)?;
    let relative_path = file_path.to_string_lossy().to_string();

    let lines: Vec<&str> = content.lines().collect();
    for (idx, line) in lines.iter().enumerate() {
        let line_num = (idx + 1) as i64;
        let trimmed = line.trim();

        let mut element_type = None;
        let mut name = None;
        let mut signature = String::new();

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
            coordinator.codebase.index_element(el, scope)?;
        }
    }
    Ok(())
}

fn parse_and_index_file(
    coordinator: &MemoryCoordinator,
    file_path: &Path,
    scope: &crate::layers::MemoryScope,
) -> Result<()> {
    let ext = file_path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if ext != "rs" && ext != "py" && ext != "js" && ext != "jsx" && ext != "ts" && ext != "tsx" {
        return parse_and_index_file_fallback(coordinator, file_path, scope);
    }

    let content = std::fs::read_to_string(file_path)?;
    let relative_path = file_path.to_string_lossy().to_string();
    let source_bytes = content.as_bytes();

    let mut parser = Parser::new();
    match ext.as_str() {
        "rs" => {
            parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        }
        "py" => {
            parser.set_language(&tree_sitter_python::LANGUAGE.into())?;
        }
        "js" | "jsx" => {
            parser.set_language(&tree_sitter_javascript::LANGUAGE.into())?;
        }
        "ts" => {
            parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())?;
        }
        "tsx" => {
            parser.set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())?;
        }
        _ => {
            return parse_and_index_file_fallback(coordinator, file_path, scope);
        }
    }

    if let Some(tree) = parser.parse(&content, None) {
        traverse_and_index(
            coordinator,
            &relative_path,
            tree.root_node(),
            source_bytes,
            None,
            scope,
        )?;
    }

    Ok(())
}

fn scan_directory(
    coordinator: &MemoryCoordinator,
    dir: &Path,
    scope: &crate::layers::MemoryScope,
) -> Result<i64> {
    let mut count = 0;
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name != "target"
                    && name != ".git"
                    && name != "external"
                    && name != "node_modules"
                {
                    count += scan_directory(coordinator, &path, scope)?;
                }
            } else {
                let ext = path.extension().unwrap_or_default().to_string_lossy();
                if ext == "rs"
                    || ext == "py"
                    || ext == "js"
                    || ext == "jsx"
                    || ext == "ts"
                    || ext == "tsx"
                    || ext == "go"
                {
                    if let Err(e) = parse_and_index_file(coordinator, &path, scope) {
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

pub async fn run_server(
    coordinator: Arc<MemoryCoordinator>,
    mut shutdown_rx: tokio::sync::mpsc::Receiver<()>,
) -> Result<()> {
    let service = MemoryServer::new(coordinator);
    let transport = (stdin(), stdout());

    log::info!("Serving openmemory_rs Server over stdio...");
    
    tokio::select! {
        res = async {
            let running = service.serve(transport).await?;
            running.waiting().await?;
            Ok::<(), anyhow::Error>(())
        } => {
            if let Err(e) = res {
                log::error!("rmcp server error: {:?}", e);
            }
        }
        _ = shutdown_rx.recv() => {
            log::info!("Shutting down Stdio transport...");
        }
    }

    Ok(())
}

pub mod mcp_grpc {
    tonic::include_proto!("mcp");
}

use mcp_grpc::mcp_service_server::{McpService, McpServiceServer};
use mcp_grpc::{McpRequest, McpResponse};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

pub struct McpServiceHandler {
    writer: Arc<tokio::sync::Mutex<tokio::io::WriteHalf<tokio::io::DuplexStream>>>,
    reader:
        Arc<tokio::sync::Mutex<tokio::io::BufReader<tokio::io::ReadHalf<tokio::io::DuplexStream>>>>,
}

#[tonic::async_trait]
impl McpService for McpServiceHandler {
    async fn call(
        &self,
        request: tonic::Request<McpRequest>,
    ) -> Result<tonic::Response<McpResponse>, tonic::Status> {
        let req = request.into_inner();

        let rpc_req = if !req.has_id {
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": req.method,
                "params": serde_json::from_str::<serde_json::Value>(&req.params_json).unwrap_or(serde_json::Value::Null)
            })
        } else {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": req.id,
                "method": req.method,
                "params": serde_json::from_str::<serde_json::Value>(&req.params_json).unwrap_or(serde_json::Value::Null)
            })
        };

        let req_str = format!(
            "{}\n",
            serde_json::to_string(&rpc_req)
                .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?
        );

        let mut writer_lock = self.writer.lock().await;
        let mut reader_lock = self.reader.lock().await;

        writer_lock
            .write_all(req_str.as_bytes())
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;
        writer_lock
            .flush()
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        if !req.has_id {
            return Ok(tonic::Response::new(McpResponse {
                result_json: String::new(),
                error_json: String::new(),
                id: 0,
            }));
        }

        let mut line = String::new();
        reader_lock
            .read_line(&mut line)
            .await
            .map_err(|e| tonic::Status::internal(e.to_string()))?;

        let resp_val: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
            tonic::Status::internal(format!("Failed to parse JSON-RPC response: {}", e))
        })?;

        let id = resp_val.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let result_json = resp_val
            .get("result")
            .map(|v| v.to_string())
            .unwrap_or_default();
        let error_json = resp_val
            .get("error")
            .map(|v| v.to_string())
            .unwrap_or_default();

        Ok(tonic::Response::new(McpResponse {
            result_json,
            error_json,
            id,
        }))
    }
}

pub async fn run_grpc_server(
    coordinator: Arc<MemoryCoordinator>,
    port: u16,
    mut shutdown_rx: tokio::sync::mpsc::Receiver<()>,
) -> Result<()> {
    let (client_half, server_half) = tokio::io::duplex(1024 * 1024);

    let service = MemoryServer::new(coordinator);
    tokio::spawn(async move {
        let (r, w) = tokio::io::split(server_half);
        if let Err(e) = service.serve((r, w)).await.unwrap().waiting().await {
            log::error!("In-memory rmcp server crashed: {:?}", e);
        }
    });

    let (client_reader, client_writer) = tokio::io::split(client_half);
    let handler = McpServiceHandler {
        writer: Arc::new(tokio::sync::Mutex::new(client_writer)),
        reader: Arc::new(tokio::sync::Mutex::new(tokio::io::BufReader::new(
            client_reader,
        ))),
    };

    let addr = format!("127.0.0.1:{}", port).parse()?;
    log::info!("gRPC MCP server listening on {}", addr);

    tonic::transport::Server::builder()
        .add_service(McpServiceServer::new(handler))
        .serve_with_shutdown(addr, async move {
            let _ = shutdown_rx.recv().await;
            log::info!("Shutting down gRPC transport...");
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use crate::layers::MemoryScope;

    #[test]
    fn test_js_ts_indexing() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_mem_{}.db", uuid::Uuid::new_v4()));
        let coordinator = MemoryCoordinator::new(db_path.to_str().unwrap())?;
        let scope = MemoryScope::default();

        // 1. JS file setup
        let js_path = std::env::temp_dir().join(format!("test_{}.js", uuid::Uuid::new_v4()));
        let js_content = r#"
class MyClass {
  myMethod() {
    return 42;
  }
}

function myFunction() {
  return "hello";
}
"#;
        fs::write(&js_path, js_content)?;

        // 2. TS file setup
        let ts_path = std::env::temp_dir().join(format!("test_{}.ts", uuid::Uuid::new_v4()));
        let ts_content = r#"
interface MyInterface {
  foo: string;
}

type MyType = string | number;

enum MyEnum {
  ValA,
  ValB
}

class MyTSClass {
  tsMethod() {
    return 10;
  }
}
"#;
        fs::write(&ts_path, ts_content)?;

        // Run parser
        parse_and_index_file(&coordinator, &js_path, &scope)?;
        parse_and_index_file(&coordinator, &ts_path, &scope)?;

        // Query indexed elements
        let js_elements = coordinator
            .codebase
            .query_elements(js_path.to_str().unwrap(), "", &scope)?;
        let ts_elements = coordinator
            .codebase
            .query_elements(ts_path.to_str().unwrap(), "", &scope)?;

        // Cleanup temp files
        let _ = fs::remove_file(&js_path);
        let _ = fs::remove_file(&ts_path);
        let _ = fs::remove_file(&db_path);

        // Assertions for JS
        println!("JS Elements found: {:?}", js_elements);
        let mut found_class = false;
        let mut found_method = false;
        let mut found_func = false;

        for el in &js_elements {
            if el.element_type == "Class" && el.name == "MyClass" {
                found_class = true;
            }
            if el.element_type == "Method" && el.name == "myMethod" {
                found_method = true;
            }
            if el.element_type == "Function" && el.name == "myFunction" {
                found_func = true;
            }
        }
        assert!(found_class, "Should index JS Class");
        assert!(found_method, "Should index JS Method");
        assert!(found_func, "Should index JS Function");

        // Assertions for TS
        println!("TS Elements found: {:?}", ts_elements);
        let mut found_interface = false;
        let mut found_type_alias = false;
        let mut found_enum = false;
        let mut found_ts_class = false;
        let mut found_ts_method = false;

        for el in &ts_elements {
            if el.element_type == "Interface" && el.name == "MyInterface" {
                found_interface = true;
            }
            if el.element_type == "TypeAlias" && el.name == "MyType" {
                found_type_alias = true;
            }
            if el.element_type == "Enum" && el.name == "MyEnum" {
                found_enum = true;
            }
            if el.element_type == "Class" && el.name == "MyTSClass" {
                found_ts_class = true;
            }
            if el.element_type == "Method" && el.name == "tsMethod" {
                found_ts_method = true;
            }
        }

        assert!(found_interface, "Should index TS Interface");
        assert!(found_type_alias, "Should index TS TypeAlias");
        assert!(found_enum, "Should index TS Enum");
        assert!(found_ts_class, "Should index TS Class");
        assert!(found_ts_method, "Should index TS Method");

        Ok(())
    }

    #[test]
    fn test_fts_search() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_fts_{}.db", uuid::Uuid::new_v4()));
        let coordinator = MemoryCoordinator::new(db_path.to_str().unwrap())?;
        let scope = MemoryScope::default();

        // 1. Add facts to semantic memory
        coordinator.semantic.add_fact("fact-1", "Rust is a systems programming language focused on safety and speed.", 0.8, &scope)?;
        coordinator.semantic.add_fact("fact-2", "Model Context Protocol (MCP) defines a standard transport for context-aware AI tools.", 0.9, &scope)?;
        coordinator.semantic.add_fact("fact-3", "SQLite is an in-process library that implements a self-contained, serverless SQL database engine.", 0.7, &scope)?;

        // 2. Perform FTS query for "systems programming"
        let res_1 = coordinator.semantic.search_text("systems programming", 10, &scope)?;
        assert_eq!(res_1.len(), 1);
        assert_eq!(res_1[0].node_id, "fact-1");

        // 3. Perform FTS query with prefix wildcard
        let res_2 = coordinator.semantic.search_text("mcp*", 10, &scope)?;
        assert_eq!(res_2.len(), 1);
        assert_eq!(res_2[0].node_id, "fact-2");

        // 4. Perform FTS query for SQL
        let res_3 = coordinator.semantic.search_text("SQL database", 10, &scope)?;
        assert_eq!(res_3.len(), 1);
        assert_eq!(res_3[0].node_id, "fact-3");

        // 5. Cleanup DB
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[test]
    fn test_hybrid_search() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!("test_hybrid_{}.db", uuid::Uuid::new_v4()));
        let coordinator = MemoryCoordinator::new(db_path.to_str().unwrap())?;
        let scope = MemoryScope::default();

        // 1. Add facts to semantic memory
        coordinator.semantic.add_fact("fact-1", "Rust is a systems programming language focused on safety and speed.", 0.8, &scope)?;
        coordinator.semantic.add_fact("fact-2", "Model Context Protocol (MCP) defines a standard transport for context-aware AI tools.", 0.9, &scope)?;
        coordinator.semantic.add_fact("fact-3", "SQLite is an in-process library that implements a self-contained, serverless SQL database engine.", 0.7, &scope)?;

        // 2. Perform hybrid query for "mcp"
        let res_1 = coordinator.semantic.query_similar_facts("mcp", 10, &scope)?;
        assert!(!res_1.is_empty(), "Should return results");
        assert_eq!(res_1[0].node_id, "fact-2");

        // 3. Perform hybrid query for "SQL database"
        let res_2 = coordinator.semantic.query_similar_facts("SQL database", 10, &scope)?;
        assert!(!res_2.is_empty(), "Should return results");
        assert_eq!(res_2[0].node_id, "fact-3");

        // 4. Cleanup DB
        let _ = std::fs::remove_file(db_path);
        Ok(())
    }
}
