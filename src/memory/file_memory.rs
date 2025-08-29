use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use super::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter};
use skreaver_core::error::MemoryError;

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

impl MemoryReader for FileMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        Ok(self.cache.get(key.as_str()).cloned())
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        Ok(keys
            .iter()
            .map(|key| self.cache.get(key.as_str()).cloned())
            .collect())
    }
}

impl MemoryWriter for FileMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        self.cache
            .insert(update.key.as_str().to_string(), update.value);
        self.persist();
        Ok(())
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        for update in updates {
            self.cache
                .insert(update.key.as_str().to_string(), update.value);
        }
        self.persist();
        Ok(())
    }
}

impl Default for FileMemory {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("skreaver_temp_memory.json"))
    }
}
