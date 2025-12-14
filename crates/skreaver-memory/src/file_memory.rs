use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use skreaver_core::error::MemoryError;
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory,
};

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
        match fs::read_to_string(path) {
            Ok(contents) => match serde_json::from_str::<HashMap<String, String>>(&contents) {
                Ok(cache) => {
                    tracing::debug!(path = ?path, entries = cache.len(), "Loaded memory cache");
                    Some(cache)
                }
                Err(e) => {
                    tracing::error!(
                        path = ?path,
                        error = %e,
                        "Failed to parse memory cache JSON, starting fresh"
                    );
                    // Optionally backup corrupted file
                    if let Some(parent) = path.parent() {
                        let backup = parent.join(format!(
                            "{}.corrupted.{}",
                            path.file_name().unwrap_or_default().to_string_lossy(),
                            chrono::Utc::now().timestamp()
                        ));
                        let _ = fs::copy(path, backup);
                    }
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(path = ?path, "Memory cache file not found, starting fresh");
                None
            }
            Err(e) => {
                tracing::warn!(path = ?path, error = %e, "Failed to read memory cache");
                None
            }
        }
    }

    fn persist(&self) -> Result<(), MemoryError> {
        let json = serde_json::to_string_pretty(&self.cache).map_err(|e| {
            tracing::error!(error = %e, "Failed to serialize memory cache");
            MemoryError::StoreFailed {
                key: skreaver_core::memory::MemoryKeys::snapshot(),
                backend: skreaver_core::error::MemoryBackend::File,
                kind: skreaver_core::error::MemoryErrorKind::SerializationError {
                    details: format!("Failed to serialize cache: {}", e),
                },
            }
        })?;

        let tmp_path = self.path.with_extension("tmp");

        fs::write(&tmp_path, json).map_err(|e| {
            tracing::error!(
                path = ?tmp_path,
                error = %e,
                "Failed to write memory cache to temporary file"
            );
            MemoryError::StoreFailed {
                key: skreaver_core::memory::MemoryKeys::snapshot(),
                backend: skreaver_core::error::MemoryBackend::File,
                kind: skreaver_core::error::MemoryErrorKind::IoError {
                    details: format!("Failed to write to {}: {}", tmp_path.display(), e),
                },
            }
        })?;

        fs::rename(&tmp_path, &self.path).map_err(|e| {
            tracing::error!(
                from = ?tmp_path,
                to = ?self.path,
                error = %e,
                "Failed to atomically rename memory cache"
            );
            MemoryError::StoreFailed {
                key: skreaver_core::memory::MemoryKeys::snapshot(),
                backend: skreaver_core::error::MemoryBackend::File,
                kind: skreaver_core::error::MemoryErrorKind::IoError {
                    details: format!(
                        "Failed to rename {} to {}: {}",
                        tmp_path.display(),
                        self.path.display(),
                        e
                    ),
                },
            }
        })?;

        tracing::debug!(path = ?self.path, entries = self.cache.len(), "Persisted memory cache");
        Ok(())
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
        self.persist()
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        for update in updates {
            self.cache
                .insert(update.key.as_str().to_string(), update.value);
        }
        self.persist()
    }
}

impl Default for FileMemory {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("skreaver_temp_memory.json"))
    }
}

impl SnapshotableMemory for FileMemory {
    fn snapshot(&mut self) -> Option<String> {
        // For FileMemory, we can simply serialize the current cache
        // which represents the current state
        serde_json::to_string(&self.cache).ok()
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        // Parse the JSON snapshot
        let new_cache: HashMap<String, String> =
            serde_json::from_str(snapshot).map_err(|e| MemoryError::RestoreFailed {
                backend: skreaver_core::error::MemoryBackend::File,
                kind: skreaver_core::error::MemoryErrorKind::SerializationError {
                    details: format!("JSON parsing failed: {}", e),
                },
            })?;

        // Replace the current cache
        self.cache = new_cache;

        // Persist the restored state to file
        self.persist()
    }
}
