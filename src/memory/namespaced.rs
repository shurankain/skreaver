use super::{Memory, MemoryUpdate};

/// Wraps a memory backend and prefixes all keys with a namespace
pub struct NamespacedMemory<M: Memory> {
    prefix: String,
    inner: M,
}

impl<M: Memory> NamespacedMemory<M> {
    pub fn new(prefix: impl Into<String>, inner: M) -> Self {
        Self {
            prefix: prefix.into(),
            inner,
        }
    }

    fn wrap_key(&self, key: &str) -> String {
        format!("{}:{}", self.prefix, key)
    }

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

