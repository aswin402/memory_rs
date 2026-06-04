use std::collections::HashMap;
use std::sync::RwLock;

pub struct WorkingMemory {
    session_data: RwLock<HashMap<String, String>>,
}

impl WorkingMemory {
    pub fn new() -> Self {
        Self {
            session_data: RwLock::new(HashMap::new()),
        }
    }

    pub fn set(&self, key: &str, value: &str) {
        if let Ok(mut map) = self.session_data.write() {
            map.insert(key.to_string(), value.to_string());
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.session_data.read().ok().and_then(|map| map.get(key).cloned())
    }
}
