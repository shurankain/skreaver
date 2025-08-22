/// A key-value update for storing data in agent memory.
///
/// `MemoryUpdate` represents a single piece of information to be
/// persisted in the agent's memory system. Both key and value are
/// string-based for maximum flexibility across different storage backends.
#[derive(Debug, Clone)]
pub struct MemoryUpdate {
    /// The key identifier for the data.
    ///
    /// Keys should be unique within an agent's memory space and follow
    /// a consistent naming convention (e.g., "context", "last_action").
    pub key: String,

    /// The value data to store.
    ///
    /// Values can be any string data - plain text, JSON, serialized objects, etc.
    /// The format depends on the agent's requirements and implementation.
    pub value: String,
}

impl MemoryUpdate {
    /// Create a new MemoryUpdate from string references.
    ///
    /// This is more efficient than using struct literals when you have &str values,
    /// as it avoids intermediate String allocations at the call site.
    ///
    /// # Parameters
    ///
    /// * `key` - The key identifier
    /// * `value` - The value to store
    ///
    /// # Returns
    ///
    /// A new `MemoryUpdate` instance
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    /// Create a new MemoryUpdate from owned strings.
    ///
    /// Use this when you already have owned String values to avoid unnecessary cloning.
    ///
    /// # Parameters
    ///
    /// * `key` - The owned key string
    /// * `value` - The owned value string
    ///
    /// # Returns
    ///
    /// A new `MemoryUpdate` instance
    pub fn from_owned(key: String, value: String) -> Self {
        Self { key, value }
    }
}

/// Basic trait for agent memory systems.
///
/// Memory provides persistent storage for agent state, context, and learned
/// information across interactions. Different implementations can provide
/// varying levels of persistence, performance, and distribution capabilities.
///
/// # Example
///
/// ```rust
/// use skreaver::memory::{Memory, MemoryUpdate};
/// use skreaver::memory::InMemoryMemory;
///
/// let mut memory = InMemoryMemory::new();
///
/// // Store some context
/// memory.store(MemoryUpdate {
///     key: "user_preference".to_string(),
///     value: "concise responses".to_string(),
/// });
///
/// // Retrieve it later
/// let preference = memory.load("user_preference");
/// assert_eq!(preference, Some("concise responses".to_string()));
/// ```
pub trait Memory {
    /// Load a value from memory by its key.
    ///
    /// Returns the stored value if the key exists, or `None` if the key
    /// is not found in the memory system.
    ///
    /// # Parameters
    ///
    /// * `key` - The key identifier to look up
    ///
    /// # Returns
    ///
    /// `Some(value)` if the key exists, `None` otherwise
    fn load(&mut self, key: &str) -> Option<String>;

    /// Store a key-value pair in memory.
    ///
    /// If the key already exists, its value will be updated with the new data.
    /// The specific persistence behavior (immediate vs. batched writes) depends
    /// on the memory implementation.
    ///
    /// # Parameters
    ///
    /// * `update` - The memory update containing key and value data
    fn store(&mut self, update: MemoryUpdate);
}

/// Optional extension for memory types that support snapshot/restore operations.
///
/// This trait provides backup and restore capabilities for memory systems
/// that can serialize their entire state. Useful for checkpointing,
/// debugging, and migration scenarios.
///
/// # Example
///
/// ```rust
/// use skreaver::memory::{Memory, MemoryUpdate, SnapshotableMemory};
/// use std::collections::HashMap;
///
/// // Simple error type for this example
/// #[derive(Debug)]
/// struct SimpleError(String);
/// impl std::fmt::Display for SimpleError {
///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
///         write!(f, "{}", self.0)
///     }
/// }
/// impl std::error::Error for SimpleError {}
///
/// // Example implementation that supports snapshots
/// struct ExampleMemory {
///     store: HashMap<String, String>,
/// }
///
/// impl Memory for ExampleMemory {
///     fn load(&mut self, key: &str) -> Option<String> {
///         self.store.get(key).cloned()
///     }
///     fn store(&mut self, update: MemoryUpdate) {
///         self.store.insert(update.key, update.value);
///     }
/// }
///
/// impl SnapshotableMemory for ExampleMemory {
///     fn snapshot(&mut self) -> Option<String> {
///         serde_json::to_string(&self.store).ok()
///     }
///     fn restore(&mut self, snapshot: &str) -> Result<(), skreaver::error::MemoryError> {
///         match serde_json::from_str(snapshot) {
///             Ok(data) => { self.store = data; Ok(()) }
///             Err(e) => Err(skreaver::error::MemoryError::RestoreFailed {
///                 reason: e.to_string()
///             })
///         }
///     }
/// }
///
/// let mut memory = ExampleMemory { store: HashMap::new() };
/// memory.store(MemoryUpdate {
///     key: "data".to_string(),
///     value: "important".to_string(),
/// });
///
/// // Create a snapshot
/// let snapshot = memory.snapshot().unwrap();
///
/// // Restore to a new memory instance  
/// let mut new_memory = ExampleMemory { store: HashMap::new() };
/// new_memory.restore(&snapshot).unwrap();
/// assert_eq!(new_memory.load("data"), Some("important".to_string()));
/// ```
pub trait SnapshotableMemory: Memory {
    /// Create a snapshot of the current memory state.
    ///
    /// Returns a serialized representation of all stored data that can
    /// be used to restore the memory to its current state later.
    ///
    /// # Returns
    ///
    /// `Some(snapshot)` if successful, `None` if serialization fails
    fn snapshot(&mut self) -> Option<String>;

    /// Restore memory state from a previously created snapshot.
    ///
    /// Replaces the current memory contents with the data from the snapshot.
    /// This operation should be atomic - either it succeeds completely or
    /// leaves the memory in its original state.
    ///
    /// # Parameters
    ///
    /// * `snapshot` - The snapshot data to restore from
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, `Err(MemoryError)` if restoration fails
    fn restore(&mut self, snapshot: &str) -> Result<(), crate::error::MemoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct DummyMemory {
        store: HashMap<String, String>,
    }

    impl Memory for DummyMemory {
        fn load(&mut self, key: &str) -> Option<String> {
            self.store.get(key).cloned()
        }

        fn store(&mut self, update: MemoryUpdate) {
            self.store.insert(update.key, update.value);
        }
    }

    impl SnapshotableMemory for DummyMemory {
        fn snapshot(&mut self) -> Option<String> {
            serde_json::to_string_pretty(&self.store).ok()
        }

        fn restore(&mut self, snapshot: &str) -> Result<(), crate::error::MemoryError> {
            match serde_json::from_str::<HashMap<String, String>>(snapshot) {
                Ok(data) => {
                    self.store = data;
                    Ok(())
                }
                Err(err) => Err(crate::error::MemoryError::RestoreFailed {
                    reason: format!("JSON parsing failed: {}", err),
                }),
            }
        }
    }

    #[test]
    fn memory_can_store_and_load() {
        let mut mem = DummyMemory {
            store: Default::default(),
        };
        mem.store(MemoryUpdate {
            key: "foo".into(),
            value: "bar".into(),
        });
        assert_eq!(mem.load("foo"), Some("bar".into()));
    }

    #[test]
    fn memory_can_snapshot_and_restore() {
        let mut mem = DummyMemory {
            store: HashMap::from([("a".into(), "1".into())]),
        };

        let snap = mem.snapshot().unwrap();
        let mut new_mem = DummyMemory {
            store: Default::default(),
        };
        new_mem.restore(&snap).unwrap();

        assert_eq!(new_mem.load("a"), Some("1".into()));
    }
}
