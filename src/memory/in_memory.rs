use std::collections::HashMap;

use super::{Memory, MemoryKey, MemoryUpdate};

/// Fast, transient memory implementation using HashMap.
///
/// `InMemoryMemory` provides high-performance memory storage suitable for
/// development, testing, and scenarios where persistence across process
/// restarts is not required. All data is lost when the process terminates.
///
/// # Example
///
/// ```rust
/// use skreaver::memory::{Memory, MemoryUpdate, MemoryKey, InMemoryMemory};
///
/// let mut memory = InMemoryMemory::new();
/// let key = MemoryKey::new("session_id").unwrap();
/// memory.store(MemoryUpdate::from_validated(key.clone(), "abc123".to_string())).unwrap();
///
/// assert_eq!(memory.load(&key).unwrap(), Some("abc123".to_string()));
/// ```
pub struct InMemoryMemory {
    store: HashMap<MemoryKey, String>,
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
            store: HashMap::new(),
        }
    }
}

impl Memory for InMemoryMemory {
    fn load(&mut self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
        Ok(self.store.get(key).cloned())
    }

    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
        self.store.insert(update.key, update.value);
        Ok(())
    }
}
