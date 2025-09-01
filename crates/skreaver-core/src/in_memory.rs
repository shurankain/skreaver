use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

/// Fast, transient memory implementation using HashMap with concurrent access.
///
/// `InMemoryMemory` provides high-performance memory storage suitable for
/// development, testing, and scenarios where persistence across process
/// restarts is not required. All data is lost when the process terminates.
///
/// This implementation supports concurrent read access while maintaining
/// exclusive write access through internal RwLock synchronization.
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
    store: Arc<RwLock<HashMap<MemoryKey, String>>>,
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
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

// Implement new trait hierarchy
impl MemoryReader for InMemoryMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
        let store = self
            .store
            .read()
            .map_err(|e| crate::error::MemoryError::LoadFailed {
                key: key.as_str().to_string(),
                reason: format!("Lock poisoned: {}", e),
            })?;
        Ok(store.get(key).cloned())
    }

    fn load_many(
        &self,
        keys: &[MemoryKey],
    ) -> Result<Vec<Option<String>>, crate::error::MemoryError> {
        let store = self
            .store
            .read()
            .map_err(|e| crate::error::MemoryError::LoadFailed {
                key: "batch".to_string(),
                reason: format!("Lock poisoned: {}", e),
            })?;
        Ok(keys.iter().map(|key| store.get(key).cloned()).collect())
    }
}

impl MemoryWriter for InMemoryMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| crate::error::MemoryError::StoreFailed {
                key: update.key.as_str().to_string(),
                reason: format!("Lock poisoned: {}", e),
            })?;
        store.insert(update.key, update.value);
        Ok(())
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), crate::error::MemoryError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| crate::error::MemoryError::StoreFailed {
                key: "batch".to_string(),
                reason: format!("Lock poisoned: {}", e),
            })?;
        for update in updates {
            store.insert(update.key, update.value);
        }
        Ok(())
    }
}

impl TransactionalMemory for InMemoryMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, crate::error::TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, crate::error::TransactionError>,
    {
        // For InMemoryMemory, we implement a simple transaction using a clone-and-swap approach
        let original_state = {
            let store = self.store.read().map_err(|e| {
                crate::error::TransactionError::TransactionFailed {
                    reason: format!("Failed to acquire read lock: {}", e),
                }
            })?;
            store.clone()
        };

        // Create a temporary transaction memory
        let mut tx_memory = InMemoryMemory {
            store: Arc::new(RwLock::new(original_state.clone())),
        };

        // Execute the transaction
        match f(&mut tx_memory) {
            Ok(result) => {
                // Commit: replace the original store with the transaction state
                let tx_state = tx_memory.store.read().map_err(|e| {
                    crate::error::TransactionError::TransactionFailed {
                        reason: format!("Failed to read transaction state: {}", e),
                    }
                })?;

                let mut store = self.store.write().map_err(|e| {
                    crate::error::TransactionError::TransactionFailed {
                        reason: format!("Failed to acquire write lock for commit: {}", e),
                    }
                })?;

                *store = tx_state.clone();
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
        let store = self.store.read().ok()?;

        // Convert HashMap<MemoryKey, String> to HashMap<String, String> for JSON serialization
        let serializable_store: HashMap<String, String> = store
            .iter()
            .map(|(key, value)| (key.as_str().to_string(), value.clone()))
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

        // Convert back to HashMap<MemoryKey, String>
        let mut new_store = HashMap::new();
        for (key_str, value) in serializable_store {
            let memory_key =
                MemoryKey::new(&key_str).map_err(|e| crate::error::MemoryError::RestoreFailed {
                    reason: format!("Invalid key '{}': {}", key_str, e),
                })?;
            new_store.insert(memory_key, value);
        }

        // Replace the store
        let mut store =
            self.store
                .write()
                .map_err(|e| crate::error::MemoryError::RestoreFailed {
                    reason: format!("Lock poisoned during restore: {}", e),
                })?;
        *store = new_store;

        Ok(())
    }
}
