use std::collections::HashMap;

use super::{Memory, MemoryUpdate};

pub struct InMemoryMemory {
    store: HashMap<String, String>,
}

impl Default for InMemoryMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryMemory {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
        }
    }
}

impl Memory for InMemoryMemory {
    fn load(&self, key: &str) -> Option<String> {
        self.store.get(key).cloned()
    }

    fn store(&mut self, update: MemoryUpdate) {
        self.store.insert(update.key, update.value);
    }
}
