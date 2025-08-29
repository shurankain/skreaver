//! Namespaced memory wrapper implementation.
//!
//! This module provides a memory wrapper that prefixes keys with a namespace
//! for multi-tenant scenarios and key isolation.

use super::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter};
use skreaver_core::error::MemoryError;

/// Wraps a memory backend and prefixes all keys with a namespace
pub struct NamespacedMemory<M>
where
    M: MemoryReader + MemoryWriter,
{
    prefix: String,
    inner: M,
}

impl<M> NamespacedMemory<M>
where
    M: MemoryReader + MemoryWriter,
{
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
    fn wrap_key(&self, key: &MemoryKey) -> Result<MemoryKey, MemoryError> {
        let wrapped_key_str = format!("{}:{}", self.prefix, key.as_str());
        MemoryKey::new(&wrapped_key_str).map_err(|e| MemoryError::StoreFailed {
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

impl<M> MemoryReader for NamespacedMemory<M>
where
    M: MemoryReader + MemoryWriter,
{
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        let wrapped_key = self.wrap_key(key)?;
        self.inner.load(&wrapped_key)
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        let wrapped_keys: Result<Vec<_>, _> = keys.iter().map(|key| self.wrap_key(key)).collect();
        let wrapped_keys = wrapped_keys?;
        self.inner.load_many(&wrapped_keys)
    }
}

impl<M> MemoryWriter for NamespacedMemory<M>
where
    M: MemoryReader + MemoryWriter,
{
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let wrapped_key = self.wrap_key(&update.key)?;
        self.inner.store(MemoryUpdate {
            key: wrapped_key,
            value: update.value,
        })
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
        let wrapped_updates = wrapped_updates?;
        self.inner.store_many(wrapped_updates)
    }
}
