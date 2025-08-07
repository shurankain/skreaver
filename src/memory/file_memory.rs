use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::{Memory, MemoryUpdate};

/// A simple persistent key-value memory that syncs to a JSON file.
pub struct FileMemory {
    path: PathBuf,
    cache: HashMap<String, String>,
}

impl FileMemory {
    /// Initializes a new FileMemory and loads existing data if available.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let cache = Self::load_cache(&path).unwrap_or_default();
        Self { path, cache }
    }

    fn load_cache(path: &PathBuf) -> Option<HashMap<String, String>> {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    }

    fn persist(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.cache) {
            // Attempt to write atomically: write to temp then rename
            let tmp_path = self.path.with_extension("tmp");
            if fs::write(&tmp_path, json).is_ok() {
                let _ = fs::rename(&tmp_path, &self.path);
            }
        }
    }
}

impl Memory for FileMemory {
    fn load(&mut self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    fn store(&mut self, update: MemoryUpdate) {
        self.cache.insert(update.key, update.value);
        self.persist();
    }
}
