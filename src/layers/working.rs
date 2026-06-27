use std::collections::HashMap;
use std::sync::RwLock;
use std::time::Instant;
use crate::layers::semantic::SemanticMemory;
use crate::layers::MemoryScope;
use crate::error::{Result, MemoryError};

#[derive(Debug, Clone)]
pub struct WorkingEntry {
    pub value: String,
    pub created_at: Instant,
    pub ttl_seconds: u64,
    pub access_count: usize,
}

pub struct WorkingMemory {
    session_data: RwLock<HashMap<String, WorkingEntry>>,
    default_ttl: u64,
}

impl WorkingMemory {
    pub fn new(default_ttl: u64) -> Self {
        Self {
            session_data: RwLock::new(HashMap::new()),
            default_ttl,
        }
    }

    pub fn set(&self, key: &str, value: &str, ttl: Option<u64>, scope: &MemoryScope) {
        let scoped_key = get_scoped_key(key, scope);
        let ttl_seconds = ttl.unwrap_or(self.default_ttl);
        let entry = WorkingEntry {
            value: value.to_string(),
            created_at: Instant::now(),
            ttl_seconds,
            access_count: 0,
        };
        if let Ok(mut map) = self.session_data.write() {
            map.insert(scoped_key, entry);
        }
    }

    pub fn get(&self, key: &str, scope: &MemoryScope) -> Option<String> {
        let scoped_key = get_scoped_key(key, scope);
        let mut expired = false;

        if let Ok(map) = self.session_data.read() {
            if let Some(entry) = map.get(&scoped_key) {
                if entry.created_at.elapsed().as_secs() >= entry.ttl_seconds {
                    expired = true;
                }
            } else {
                return None;
            }
        }

        if expired {
            if let Ok(mut map) = self.session_data.write() {
                map.remove(&scoped_key);
            }
            return None;
        }

        if let Ok(mut map) = self.session_data.write() {
            if let Some(entry) = map.get_mut(&scoped_key) {
                entry.access_count += 1;
                return Some(entry.value.clone());
            }
        }

        None
    }

    pub fn evict_expired(&self, semantic: &SemanticMemory) -> Result<usize> {
        let mut map = self.session_data.write().map_err(|_| MemoryError::LockPoisoned("WorkingMemory session_data".to_string()))?;
        let mut evicted_count = 0;
        let mut to_remove = Vec::new();

        for (scoped_key, entry) in map.iter() {
            if entry.created_at.elapsed().as_secs() >= entry.ttl_seconds {
                to_remove.push(scoped_key.clone());
            }
        }

        for scoped_key in to_remove {
            if let Some(entry) = map.remove(&scoped_key) {
                evicted_count += 1;
                if entry.access_count >= 3 {
                    let parts: Vec<&str> = scoped_key.splitn(4, ':').collect();
                    if parts.len() == 4 {
                        let user_id = if parts[0] == "*" { None } else { Some(parts[0].to_string()) };
                        let session_id = if parts[1] == "*" { None } else { Some(parts[1].to_string()) };
                        let agent_id = if parts[2] == "*" { None } else { Some(parts[2].to_string()) };
                        let real_key = parts[3];
                        let scope = MemoryScope { user_id, session_id, agent_id };
                        
                        let fact_id = format!("working-promoted-{}", uuid::Uuid::new_v4());
                        let raw_text = format!("Ephemeral working memory under key '{}' was promoted: {}", real_key, entry.value);
                        let importance = 0.8;
                        
                        let _ = semantic.add_fact(&fact_id, &raw_text, importance, &scope);
                    }
                }
            }
        }

        Ok(evicted_count)
    }

    pub fn promote_to_semantic(&self, key: &str, semantic: &SemanticMemory, scope: &MemoryScope) -> Result<bool> {
        let scoped_key = get_scoped_key(key, scope);
        let entry_opt = {
            let mut map = self.session_data.write().map_err(|_| MemoryError::LockPoisoned("WorkingMemory session_data".to_string()))?;
            map.remove(&scoped_key)
        };

        if let Some(entry) = entry_opt {
            let fact_id = format!("working-promoted-{}", uuid::Uuid::new_v4());
            let raw_text = format!("Ephemeral working memory under key '{}' was promoted: {}", key, entry.value);
            let importance = 0.8;
            semantic.add_fact(&fact_id, &raw_text, importance, scope)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn get_scoped_key(key: &str, scope: &MemoryScope) -> String {
    let u = scope.user_id.as_deref().unwrap_or("*");
    let s = scope.session_id.as_deref().unwrap_or("*");
    let a = scope.agent_id.as_deref().unwrap_or("*");
    format!("{}:{}:{}:{}", u, s, a, key)
}
