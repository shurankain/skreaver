/// Validated memory key that prevents typos and ensures consistent naming.
///
/// `MemoryKey` is a newtype wrapper around `String` that provides compile-time
/// validation and prevents common errors like typos in memory keys. It enforces
/// naming conventions and length limits to ensure memory keys are valid.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryKey(String);

/// Errors that can occur when creating a `MemoryKey`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidMemoryKey {
    /// Memory key is empty or contains only whitespace.
    Empty,
    /// Memory key exceeds the maximum allowed length.
    TooLong(usize),
    /// Memory key contains invalid characters.
    InvalidChars(String),
}

impl std::fmt::Display for InvalidMemoryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvalidMemoryKey::Empty => write!(f, "Memory key cannot be empty"),
            InvalidMemoryKey::TooLong(len) => {
                write!(f, "Memory key too long: {} characters (max 128)", len)
            }
            InvalidMemoryKey::InvalidChars(key) => {
                write!(f, "Memory key contains invalid characters: '{}'", key)
            }
        }
    }
}

impl std::error::Error for InvalidMemoryKey {}

impl MemoryKey {
    /// Maximum allowed length for memory keys.
    pub const MAX_LENGTH: usize = 128;

    /// Create a new validated memory key.
    ///
    /// # Parameters
    ///
    /// * `key` - The memory key string to validate
    ///
    /// # Returns
    ///
    /// `Ok(MemoryKey)` if valid, `Err(InvalidMemoryKey)` if validation fails
    ///
    /// # Validation Rules
    ///
    /// - Must not be empty or only whitespace
    /// - Must not exceed 128 characters
    /// - Must contain only alphanumeric characters, underscores, hyphens, dots, and colons
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::memory::MemoryKey;
    ///
    /// let key = MemoryKey::new("user_context").unwrap();
    /// assert_eq!(key.as_str(), "user_context");
    /// ```
    pub fn new(key: &str) -> Result<Self, InvalidMemoryKey> {
        use crate::validation::IdentifierRules;

        let validated = IdentifierRules::MEMORY_KEY
            .validate(key)
            .map_err(|e| match e {
                crate::validation::ValidationError::Empty => InvalidMemoryKey::Empty,
                crate::validation::ValidationError::TooLong { length, .. } => {
                    InvalidMemoryKey::TooLong(length)
                }
                crate::validation::ValidationError::InvalidChar { input, .. } => {
                    InvalidMemoryKey::InvalidChars(input)
                }
            })?;

        Ok(MemoryKey(validated))
    }

    /// Get the memory key as a string slice.
    ///
    /// # Returns
    ///
    /// The validated memory key as a `&str`
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the length of the memory key in bytes.
    ///
    /// # Returns
    ///
    /// The length of the memory key
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the memory key is empty.
    ///
    /// # Returns
    ///
    /// `true` if the memory key is empty (this should never happen for validated keys)
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Convert into the underlying string.
    ///
    /// # Returns
    ///
    /// The validated memory key as an owned `String`
    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for MemoryKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for MemoryKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for MemoryKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl TryFrom<&str> for MemoryKey {
    type Error = InvalidMemoryKey;

    fn try_from(key: &str) -> Result<Self, Self::Error> {
        MemoryKey::new(key)
    }
}

impl TryFrom<String> for MemoryKey {
    type Error = InvalidMemoryKey;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        MemoryKey::new(&key)
    }
}

/// A key-value update for storing data in agent memory.
///
/// `MemoryUpdate` represents a single piece of information to be
/// persisted in the agent's memory system. The key is validated for consistency
/// and the value supports flexible string-based storage across different backends.
#[derive(Debug, Clone)]
pub struct MemoryUpdate {
    /// The validated key identifier for the data.
    ///
    /// Keys are validated to ensure consistency and prevent typos.
    /// They should be unique within an agent's memory space.
    pub key: MemoryKey,

    /// The value data to store.
    ///
    /// Values can be any string data - plain text, JSON, serialized objects, etc.
    /// The format depends on the agent's requirements and implementation.
    pub value: String,
}

impl MemoryUpdate {
    /// Create a new MemoryUpdate from string references with validation.
    ///
    /// This validates the key and creates a new MemoryUpdate instance.
    ///
    /// # Parameters
    ///
    /// * `key` - The key identifier (will be validated)
    /// * `value` - The value to store
    ///
    /// # Returns
    ///
    /// `Ok(MemoryUpdate)` if the key is valid, `Err(InvalidMemoryKey)` otherwise
    pub fn new(key: &str, value: &str) -> Result<Self, InvalidMemoryKey> {
        Ok(Self {
            key: MemoryKey::new(key)?,
            value: value.to_string(),
        })
    }

    /// Create a new MemoryUpdate from a validated MemoryKey and value string.
    ///
    /// Use this when you already have a validated MemoryKey to avoid re-validation.
    ///
    /// # Parameters
    ///
    /// * `key` - The validated memory key
    /// * `value` - The value to store
    ///
    /// # Returns
    ///
    /// A new `MemoryUpdate` instance
    pub fn from_validated(key: MemoryKey, value: String) -> Self {
        Self { key, value }
    }

    /// Create a new MemoryUpdate from owned strings with validation.
    ///
    /// # Parameters
    ///
    /// * `key` - The key string (will be validated)
    /// * `value` - The owned value string
    ///
    /// # Returns
    ///
    /// `Ok(MemoryUpdate)` if the key is valid, `Err(InvalidMemoryKey)` otherwise
    pub fn from_owned(key: String, value: String) -> Result<Self, InvalidMemoryKey> {
        Ok(Self {
            key: MemoryKey::new(&key)?,
            value,
        })
    }
}

/// Read-only memory operations trait.
///
/// This trait provides immutable access to memory for read operations.
/// It allows multiple concurrent readers and enables performance optimizations
/// like caching and connection pooling.
///
/// # Benefits
///
/// - **Performance**: No exclusive access required for reads
/// - **Concurrency**: Multiple readers can access memory simultaneously
/// - **Safety**: Prevents accidental modifications during read operations
/// - **Optimization**: Enables read-specific optimizations like caching
///
/// # Example
///
/// ```rust
/// use skreaver_core::memory::{MemoryReader, MemoryKey};
/// use skreaver_core::InMemoryMemory;
///
/// let memory = InMemoryMemory::new();
/// let key = MemoryKey::new("user_preference").unwrap();
///
/// // Read operations don't require mutable access
/// let value = memory.load(&key).unwrap();
/// ```
pub trait MemoryReader: Send + Sync {
    /// Load a value from memory by its key (immutable access).
    ///
    /// Returns the stored value if the key exists, or `None` if the key
    /// is not found in the memory system. This operation does not require
    /// mutable access and can be called concurrently.
    ///
    /// # Parameters
    ///
    /// * `key` - The validated key identifier to look up
    ///
    /// # Returns
    ///
    /// `Ok(Some(value))` if the key exists, `Ok(None)` if not found, `Err(MemoryError)` on failure
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError>;

    /// Load multiple values from memory by their keys (batch operation).
    ///
    /// Returns a vector of optional values in the same order as the input keys.
    /// This is more efficient than multiple individual `load` calls for backends
    /// that support batch operations.
    ///
    /// # Parameters
    ///
    /// * `keys` - The validated key identifiers to look up
    ///
    /// # Returns
    ///
    /// `Ok(Vec<Option<value>>)` with results in key order, `Err(MemoryError)` on failure
    fn load_many(
        &self,
        keys: &[MemoryKey],
    ) -> Result<Vec<Option<String>>, crate::error::MemoryError> {
        // Default implementation using individual loads
        keys.iter().map(|key| self.load(key)).collect()
    }
}

/// Write-only memory operations trait.
///
/// This trait provides mutable access to memory for write operations.
/// It clearly separates write operations from read operations and enables
/// optimizations like batching and transactions.
///
/// # Example
///
/// ```rust
/// use skreaver_core::memory::{MemoryWriter, MemoryUpdate};
/// use skreaver_core::InMemoryMemory;
///
/// let mut memory = InMemoryMemory::new();
///
/// // Write operations require mutable access
/// let update = MemoryUpdate::new("user_id", "123").unwrap();
/// memory.store(update).unwrap();
/// ```
pub trait MemoryWriter: Send + Sync {
    /// Store a key-value pair in memory.
    ///
    /// If the key already exists, its value will be updated with the new data.
    /// The specific persistence behavior (immediate vs. batched writes) depends
    /// on the memory implementation.
    ///
    /// # Parameters
    ///
    /// * `update` - The memory update containing validated key and value data
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, `Err(MemoryError)` if the operation fails
    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError>;

    /// Store multiple key-value pairs in memory (batch operation).
    ///
    /// This is more efficient than multiple individual `store` calls for backends
    /// that support batch operations. The operation should be atomic where possible.
    ///
    /// # Parameters
    ///
    /// * `updates` - The memory updates to store
    ///
    /// # Returns
    ///
    /// `Ok(())` if all updates succeed, `Err(MemoryError)` if any operation fails
    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), crate::error::MemoryError> {
        // Default implementation using individual stores
        for update in updates {
            self.store(update)?;
        }
        Ok(())
    }
}

/// Transactional memory operations trait.
///
/// This trait extends read and write capabilities with atomic transaction support.
/// Transactions ensure that either all operations succeed or none are applied.
///
/// # Example
///
/// ```rust
/// use skreaver_core::memory::{TransactionalMemory, MemoryUpdate};
/// use skreaver_core::InMemoryMemory;
///
/// let mut memory = InMemoryMemory::new();
///
/// // All operations in the transaction succeed or none are applied
/// memory.transaction(|tx| {
///     tx.store(MemoryUpdate::new("key1", "value1")?)?;
///     tx.store(MemoryUpdate::new("key2", "value2")?)?;
///     Ok(())
/// }).unwrap();
/// ```
pub trait TransactionalMemory: MemoryReader + MemoryWriter {
    /// Execute operations within a transaction.
    ///
    /// The transaction ensures that either all operations succeed or none
    /// are applied to the memory. This is useful for maintaining consistency
    /// when multiple related updates must be applied together.
    ///
    /// # Parameters
    ///
    /// * `f` - The transaction function that performs memory operations
    ///
    /// # Returns
    ///
    /// `Ok(result)` if the transaction succeeds, `Err(TransactionError)` if it fails
    fn transaction<F, R>(&mut self, f: F) -> Result<R, crate::error::TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, crate::error::TransactionError>;
}

/// Adapter to provide backwards compatibility with the legacy Memory trait.
///
/// This allows any type that implements MemoryReader + MemoryWriter to also
/// implement the legacy Memory trait, providing a smooth migration path.
pub struct MemoryCompat<T> {
    inner: T,
}

impl<T> MemoryCompat<T>
where
    T: MemoryReader + MemoryWriter,
{
    /// Create a new compatibility adapter
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner memory implementation
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the inner memory implementation
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> MemoryReader for MemoryCompat<T>
where
    T: MemoryReader,
{
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
        self.inner.load(key)
    }

    fn load_many(
        &self,
        keys: &[MemoryKey],
    ) -> Result<Vec<Option<String>>, crate::error::MemoryError> {
        self.inner.load_many(keys)
    }
}

impl<T> MemoryWriter for MemoryCompat<T>
where
    T: MemoryWriter,
{
    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
        self.inner.store(update)
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), crate::error::MemoryError> {
        self.inner.store_many(updates)
    }
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
/// use skreaver_core::memory::{MemoryReader, MemoryWriter, MemoryUpdate, SnapshotableMemory, MemoryKey};
/// use std::collections::HashMap;
///
/// // Example implementation that supports snapshots
/// struct ExampleMemory {
///     store: HashMap<String, String>,
/// }
///
/// impl MemoryReader for ExampleMemory {
///     fn load(&self, key: &MemoryKey) -> Result<Option<String>, skreaver_core::error::MemoryError> {
///         Ok(self.store.get(key.as_str()).cloned())
///     }
///     fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, skreaver_core::error::MemoryError> {
///         Ok(keys.iter().map(|key| self.store.get(key.as_str()).cloned()).collect())
///     }
/// }
/// impl MemoryWriter for ExampleMemory {
///     fn store(&mut self, update: MemoryUpdate) -> Result<(), skreaver_core::error::MemoryError> {
///         self.store.insert(update.key.as_str().to_string(), update.value);
///         Ok(())
///     }
///     fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), skreaver_core::error::MemoryError> {
///         for update in updates {
///             self.store.insert(update.key.as_str().to_string(), update.value);
///         }
///         Ok(())
///     }
/// }
///
/// impl SnapshotableMemory for ExampleMemory {
///     fn snapshot(&mut self) -> Option<String> {
///         serde_json::to_string(&self.store).ok()
///     }
///     fn restore(&mut self, snapshot: &str) -> Result<(), skreaver_core::error::MemoryError> {
///         match serde_json::from_str(snapshot) {
///             Ok(data) => { self.store = data; Ok(()) }
///             Err(e) => Err(skreaver_core::error::MemoryError::RestoreFailed {
///                 backend: skreaver_core::error::MemoryBackend::InMemory,
///                 kind: skreaver_core::error::MemoryErrorKind::SerializationError {
///                     details: e.to_string(),
///                 },
///             })
///         }
///     }
/// }
///
/// let mut memory = ExampleMemory { store: HashMap::new() };
/// let data_key = MemoryKey::new("data").unwrap();
/// MemoryWriter::store(&mut memory, MemoryUpdate {
///     key: data_key.clone(),
///     value: "important".to_string(),
/// }).unwrap();
///
/// // Create a snapshot
/// let snapshot = memory.snapshot().unwrap();
///
/// // Restore to a new memory instance  
/// let mut new_memory = ExampleMemory { store: HashMap::new() };
/// new_memory.restore(&snapshot).unwrap();
/// assert_eq!(MemoryReader::load(&new_memory, &data_key).unwrap(), Some("important".to_string()));
/// ```
pub trait SnapshotableMemory: Send + Sync {
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

    impl MemoryReader for DummyMemory {
        fn load(&self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
            Ok(self.store.get(key.as_str()).cloned())
        }

        fn load_many(
            &self,
            keys: &[MemoryKey],
        ) -> Result<Vec<Option<String>>, crate::error::MemoryError> {
            Ok(keys
                .iter()
                .map(|key| self.store.get(key.as_str()).cloned())
                .collect())
        }
    }

    impl MemoryWriter for DummyMemory {
        fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
            self.store
                .insert(update.key.as_str().to_string(), update.value);
            Ok(())
        }

        fn store_many(
            &mut self,
            updates: Vec<MemoryUpdate>,
        ) -> Result<(), crate::error::MemoryError> {
            for update in updates {
                self.store
                    .insert(update.key.as_str().to_string(), update.value);
            }
            Ok(())
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
                    backend: crate::error::MemoryBackend::InMemory,
                    kind: crate::error::MemoryErrorKind::SerializationError {
                        details: format!("JSON parsing failed: {}", err),
                    },
                }),
            }
        }
    }

    #[test]
    fn memory_can_store_and_load() {
        let mut mem = DummyMemory {
            store: Default::default(),
        };
        let key = MemoryKey::new("foo").unwrap();
        MemoryWriter::store(
            &mut mem,
            MemoryUpdate::from_validated(key.clone(), "bar".to_string()),
        )
        .unwrap();
        assert_eq!(MemoryReader::load(&mem, &key).unwrap(), Some("bar".into()));
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

        let key = MemoryKey::new("a").unwrap();
        assert_eq!(
            MemoryReader::load(&new_mem, &key).unwrap(),
            Some("1".into())
        );
    }
}
