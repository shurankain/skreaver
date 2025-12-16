use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use skreaver_core::error::MemoryError;
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory,
};

/// A simple persistent key-value memory that syncs to a JSON file.
///
/// # Thread Safety
///
/// **This type is NOT thread-safe and should not be shared across threads without
/// external synchronization.**
///
/// `FileMemory` does not implement `Send` or `Sync` protections for its internal
/// state. If you need to share a `FileMemory` instance across threads, you must
/// wrap it in appropriate synchronization primitives:
///
/// ```rust,ignore
/// use std::sync::{Arc, Mutex};
/// use skreaver_memory::FileMemory;
///
/// // Safe: Each thread gets its own FileMemory instance
/// let memory1 = FileMemory::new("thread1.json");
/// let memory2 = FileMemory::new("thread2.json");
///
/// // Safe: Properly synchronized with Mutex
/// let shared_memory = Arc::new(Mutex::new(FileMemory::new("shared.json")));
/// let memory_clone = Arc::clone(&shared_memory);
/// std::thread::spawn(move || {
///     let mut mem = memory_clone.lock().unwrap();
///     // ... use mem ...
/// });
/// ```
///
/// # File Access Patterns
///
/// Each operation that modifies the cache (`store`, `store_many`, `restore`) will
/// write to the file system. This ensures durability but may impact performance
/// for high-frequency updates. Consider batching writes using `store_many` for
/// better throughput.
///
/// # Concurrent File Access
///
/// Multiple `FileMemory` instances pointing to the same file path will conflict
/// and may corrupt data. Ensure that each file path is used by only one
/// `FileMemory` instance at a time, or use proper file locking mechanisms.
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

    /// Clean up old corrupted backup files, keeping only the most recent N backups
    ///
    /// This method removes old `.corrupted.*` backup files for this FileMemory instance,
    /// keeping only the `keep_count` most recent backups based on file modification time.
    ///
    /// # Arguments
    ///
    /// * `keep_count` - Number of most recent backups to keep (e.g., 5)
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read or if file operations fail.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use skreaver_memory::FileMemory;
    ///
    /// let mut memory = FileMemory::new("data.json");
    /// // ... use memory ...
    ///
    /// // Clean up old backups, keeping only the 5 most recent
    /// memory.cleanup_backups(5)?;
    /// ```
    pub fn cleanup_backups(&self, keep_count: usize) -> std::io::Result<()> {
        let Some(parent) = self.path.parent() else {
            return Ok(());
        };

        let prefix = format!(
            "{}.corrupted.",
            self.path.file_name().unwrap_or_default().to_string_lossy()
        );

        // Collect all backup files for this memory instance
        let mut backups: Vec<_> = fs::read_dir(parent)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .collect();

        // Sort by modification time (oldest first)
        backups.sort_by_key(|entry| {
            entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
        });

        // Remove oldest backups beyond keep_count
        let remove_count = backups.len().saturating_sub(keep_count);
        for backup in backups.iter().take(remove_count) {
            match fs::remove_file(backup.path()) {
                Ok(()) => {
                    tracing::debug!(
                        path = ?backup.path(),
                        "Removed old corrupted backup"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        path = ?backup.path(),
                        error = %e,
                        "Failed to remove old corrupted backup"
                    );
                }
            }
        }

        if remove_count > 0 {
            tracing::info!(
                removed = remove_count,
                kept = keep_count.min(backups.len()),
                "Cleaned up old corrupted backups"
            );
        }

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

        // Create a backup of the current state for rollback
        let old_cache = std::mem::replace(&mut self.cache, new_cache);

        // Try to persist - if it fails, rollback to old state
        if let Err(e) = self.persist() {
            tracing::error!(error = %e, "Failed to persist restored state, rolling back");
            self.cache = old_cache;
            return Err(e);
        }

        Ok(())
    }
}
