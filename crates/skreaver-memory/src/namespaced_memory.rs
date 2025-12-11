use std::marker::PhantomData;

use skreaver_core::error::MemoryError;
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

/// A memory wrapper that adds namespacing to keys.
///
/// This allows multiple agents or contexts to share the same underlying
/// memory backend while maintaining isolation between their data.
pub struct NamespacedMemory<M> {
    prefix: String,
    inner: M,
    _phantom: PhantomData<M>,
}

impl<M> NamespacedMemory<M> {
    /// Create a new namespaced memory wrapper.
    ///
    /// # Parameters
    ///
    /// * `prefix` - The namespace prefix to prepend to all keys
    /// * `inner` - The underlying memory implementation
    ///
    /// # Returns
    ///
    /// A new `NamespacedMemory` instance
    pub fn new(prefix: impl Into<String>, inner: M) -> Self {
        Self {
            prefix: prefix.into(),
            inner,
            _phantom: PhantomData,
        }
    }

    /// Wrap a key with the namespace prefix.
    fn wrap_key(&self, key: &MemoryKey) -> Result<MemoryKey, MemoryError> {
        let wrapped_key_str = format!("{}:{}", self.prefix, key.as_str());
        MemoryKey::new(&wrapped_key_str).map_err(|e| MemoryError::StoreFailed {
            key: skreaver_core::memory::MemoryKeys::fallback_namespaced(),
            backend: skreaver_core::error::MemoryBackend::InMemory,
            kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                validation_error: format!("Invalid namespaced key: {}", e),
            },
        })
    }

    /// Get a mutable reference to the underlying memory implementation.
    ///
    /// This allows direct access to the wrapped memory for operations
    /// that don't need namespacing.
    pub fn inner_mut(&mut self) -> &mut M {
        &mut self.inner
    }

    /// Get an immutable reference to the underlying memory implementation.
    pub fn inner(&self) -> &M {
        &self.inner
    }
}

impl<M: MemoryReader> MemoryReader for NamespacedMemory<M> {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        let wrapped_key = self.wrap_key(key)?;
        self.inner.load(&wrapped_key)
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        let wrapped_keys: Result<Vec<_>, _> = keys.iter().map(|k| self.wrap_key(k)).collect();
        let wrapped_keys = wrapped_keys?;
        self.inner.load_many(&wrapped_keys)
    }
}

impl<M: MemoryWriter> MemoryWriter for NamespacedMemory<M> {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let wrapped_key = self.wrap_key(&update.key)?;
        let wrapped_update = MemoryUpdate {
            key: wrapped_key,
            value: update.value,
        };
        self.inner.store(wrapped_update)
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        let wrapped_updates: Result<Vec<_>, _> = updates
            .into_iter()
            .map(|update| {
                let wrapped_key = self.wrap_key(&update.key)?;
                Ok(MemoryUpdate {
                    key: wrapped_key,
                    value: update.value,
                })
            })
            .collect();
        self.inner.store_many(wrapped_updates?)
    }
}

impl<M: TransactionalMemory> TransactionalMemory for NamespacedMemory<M> {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, skreaver_core::error::TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, skreaver_core::error::TransactionError>,
    {
        self.inner.transaction(f)
    }
}

impl<M: SnapshotableMemory> SnapshotableMemory for NamespacedMemory<M> {
    fn snapshot(&mut self) -> Option<String> {
        self.inner.snapshot()
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        self.inner.restore(snapshot)
    }
}
