use std::collections::HashMap;

use super::{Memory, MemoryUpdate};

/// Fast, transient memory implementation using HashMap.
///
/// `InMemoryMemory` provides high-performance memory storage suitable for
/// development, testing, and scenarios where persistence across process
/// restarts is not required. All data is lost when the process terminates.
///
/// # Example
///
/// ```rust
/// use skreaver::memory::{Memory, MemoryUpdate, InMemoryMemory};
///
/// let mut memory = InMemoryMemory::new();
/// memory.store(MemoryUpdate {
///     key: "session_id".to_string(),
///     value: "abc123".to_string(),
/// });
///
/// assert_eq!(memory.load("session_id"), Some("abc123".to_string()));
/// ```
pub struct InMemoryMemory {
    store: HashMap<String, String>,
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
    fn load(&mut self, key: &str) -> Option<String> {
        self.store.get(key).cloned()
    }

    fn store(&mut self, update: MemoryUpdate) {
        self.store.insert(update.key, update.value);
    }
}
