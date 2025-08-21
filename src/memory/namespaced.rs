use super::{Memory, MemoryUpdate};

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
    fn wrap_key(&self, key: &str) -> String {
        format!("{}:{}", self.prefix, key)
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
    fn load(&mut self, key: &str) -> Option<String> {
        self.inner.load(&self.wrap_key(key))
    }

    fn store(&mut self, update: MemoryUpdate) {
        self.inner.store(MemoryUpdate {
            key: self.wrap_key(&update.key),
            value: update.value,
        });
    }
}
