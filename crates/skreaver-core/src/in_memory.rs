use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;

use crate::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

/// Fast, transient memory implementation using lock-free DashMap for concurrent access.
///
/// `InMemoryMemory` provides high-performance memory storage suitable for
/// development, testing, and scenarios where persistence across process
/// restarts is not required. All data is lost when the process terminates.
///
/// This implementation uses DashMap for lock-free concurrent access, providing
/// excellent performance under high concurrency with minimal contention.
///
/// # Example
///
/// ```rust
/// use skreaver_core::{InMemoryMemory, MemoryReader, MemoryWriter, MemoryUpdate};
/// use skreaver_core::memory::MemoryKey;
///
/// let mut memory = InMemoryMemory::new();
/// let key = MemoryKey::new("session_id").unwrap();
/// MemoryWriter::store(&mut memory, MemoryUpdate::from_validated(key.clone(), "abc123".to_string())).unwrap();
///
/// assert_eq!(MemoryReader::load(&memory, &key).unwrap(), Some("abc123".to_string()));
/// ```
#[derive(Clone)]
pub struct InMemoryMemory {
    store: Arc<DashMap<MemoryKey, String>>,
}

impl Default for InMemoryMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryMemory {
    /// Create a new empty in-memory storage instance.
    ///
    /// # Returns
    ///
    /// A new `InMemoryMemory` with no stored data
    pub fn new() -> Self {
        Self {
            store: Arc::new(DashMap::new()),
        }
    }
}

// Implement new trait hierarchy
impl MemoryReader for InMemoryMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
        Ok(self.store.get(key).map(|entry| entry.value().clone()))
    }

    fn load_many(
        &self,
        keys: &[MemoryKey],
    ) -> Result<Vec<Option<String>>, crate::error::MemoryError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        // Pre-allocate result vector with exact capacity
        let mut result = Vec::with_capacity(keys.len());
        for key in keys {
            result.push(self.store.get(key).map(|entry| entry.value().clone()));
        }
        Ok(result)
    }
}

impl MemoryWriter for InMemoryMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
        self.store.insert(update.key, update.value);
        Ok(())
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), crate::error::MemoryError> {
        if updates.is_empty() {
            return Ok(());
        }

        // DashMap handles concurrent access internally
        for update in updates {
            self.store.insert(update.key, update.value);
        }
        Ok(())
    }
}

impl TransactionalMemory for InMemoryMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, crate::error::TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, crate::error::TransactionError>,
    {
        // For DashMap-based InMemoryMemory, we snapshot current state
        let original_state: HashMap<MemoryKey, String> = self
            .store
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        // Create a temporary transaction memory with the same state
        let mut tx_memory = InMemoryMemory::new();
        for (key, value) in &original_state {
            tx_memory.store.insert(key.clone(), value.clone());
        }

        // Execute the transaction
        match f(&mut tx_memory) {
            Ok(result) => {
                // Commit: replace our store with the transaction store
                self.store.clear();
                for entry in tx_memory.store.iter() {
                    self.store
                        .insert(entry.key().clone(), entry.value().clone());
                }
                Ok(result)
            }
            Err(err) => {
                // Rollback: original state is preserved automatically
                Err(err)
            }
        }
    }
}

impl SnapshotableMemory for InMemoryMemory {
    fn snapshot(&mut self) -> Option<String> {
        // Convert DashMap<MemoryKey, String> to HashMap<String, String> for JSON serialization
        let serializable_store: HashMap<String, String> = self
            .store
            .iter()
            .map(|entry| (entry.key().as_str().to_string(), entry.value().clone()))
            .collect();

        serde_json::to_string(&serializable_store).ok()
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), crate::error::MemoryError> {
        // Parse the JSON snapshot
        let serializable_store: HashMap<String, String> =
            serde_json::from_str(snapshot).map_err(|e| {
                crate::error::MemoryError::RestoreFailed {
                    reason: format!("JSON parsing failed: {}", e),
                }
            })?;

        // Clear and populate the DashMap
        self.store.clear();
        for (key_str, value) in serializable_store {
            let memory_key =
                MemoryKey::new(&key_str).map_err(|e| crate::error::MemoryError::RestoreFailed {
                    reason: format!("Invalid key '{}': {}", key_str, e),
                })?;
            self.store.insert(memory_key, value);
        }

        Ok(())
    }
}
