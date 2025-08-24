use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::{Memory, MemoryKey, MemoryUpdate};

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
    fn load(&mut self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
        Ok(self.cache.get(key.as_str()).cloned())
    }

    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
        self.cache
            .insert(update.key.as_str().to_string(), update.value);
        self.persist();
        Ok(())
    }
}
