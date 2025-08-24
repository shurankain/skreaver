use super::{Memory, MemoryKey, MemoryUpdate};

/// Wraps a memory backend and prefixes all keys with a namespace
pub struct NamespacedMemory<M: Memory> {
    prefix: String,
    inner: M,
}

impl<M: Memory> NamespacedMemory<M> {
    /// Create a new namespaced memory wrapper.
    ///
    /// All keys will be prefixed with the provided namespace to enable
    /// key isolation in multi-tenant scenarios.
    ///
    /// # Parameters
    ///
    /// * `prefix` - The namespace prefix for all keys
    /// * `inner` - The underlying memory implementation to wrap
    pub fn new(prefix: impl Into<String>, inner: M) -> Self {
        Self {
            prefix: prefix.into(),
            inner,
        }
    }

    /// Wrap a key with the namespace prefix.
    fn wrap_key(&self, key: &MemoryKey) -> Result<MemoryKey, crate::error::MemoryError> {
        let wrapped_key_str = format!("{}:{}", self.prefix, key.as_str());
        MemoryKey::new(&wrapped_key_str).map_err(|e| crate::error::MemoryError::StoreFailed {
            key: wrapped_key_str,
            reason: format!("Invalid namespaced key: {}", e),
        })
    }

    /// Get a mutable reference to the underlying memory implementation.
    ///
    /// This allows direct access to the wrapped memory for operations
    /// that need to bypass the namespace prefix.
    pub fn inner(&mut self) -> &mut M {
        &mut self.inner
    }
}

impl<M: Memory> Memory for NamespacedMemory<M> {
    fn load(&mut self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
        let wrapped_key = self.wrap_key(key)?;
        self.inner.load(&wrapped_key)
    }

    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
        let wrapped_key = self.wrap_key(&update.key)?;
        self.inner.store(MemoryUpdate {
            key: wrapped_key,
            value: update.value,
        })
    }
}
