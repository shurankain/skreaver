use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::{Memory, MemoryUpdate};

pub struct FileMemory {
    path: PathBuf,
    cache: HashMap<String, String>,
}

impl FileMemory {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let cache = fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, cache }
    }

    fn persist(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.cache) {
            let _ = fs::write(&self.path, json);
        }
    }
}

impl Memory for FileMemory {
    fn load(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    fn store(&mut self, update: MemoryUpdate) {
        self.cache.insert(update.key, update.value);
        self.persist();
    }
}
