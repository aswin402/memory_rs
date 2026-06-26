pub mod codebase;
pub mod episodic;
pub mod graph;
pub mod semantic;
pub mod shared;
pub mod working;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct MemoryScope {
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
}

