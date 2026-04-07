//! Memory backend Python bindings.
//!
//! This module provides Python bindings for Skreaver's memory backends:
//! - `FileMemory` - JSON file-based persistence
//! - `RedisMemory` - Redis-based distributed memory (requires `redis` feature)
//!
//! ## Example
//!
//! ```python
//! from skreaver.memory import FileMemory
//!
//! # Create file-based memory
//! memory = FileMemory("/tmp/agent_memory.json")
//!
//! # Store values
//! memory.store("user_preference", "dark_mode")
//! memory.store_many({"key1": "value1", "key2": "value2"})
//!
//! # Load values
//! value = memory.load("user_preference")  # Returns "dark_mode"
//! values = memory.load_many(["key1", "key2"])  # Returns ["value1", "value2"]
//!
//! # Snapshot and restore
//! snapshot = memory.snapshot()
//! memory.restore(snapshot)
//! ```

use pyo3::prelude::*;
use pyo3::types::PyDict;
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory,
};
use skreaver_memory::FileMemory;
use std::collections::HashMap;
use std::sync::Mutex;

/// Helper to create a MemoryKey from a Python string
fn make_memory_key(key: &str) -> PyResult<MemoryKey> {
    MemoryKey::new(key)
        .map_err(|e| crate::errors::MemoryError::new_err(format!("Invalid key: {}", e)))
}

/// File-based memory backend.
///
/// Persists key-value data to a JSON file with atomic writes.
/// Thread-safe through internal synchronization.
///
/// Args:
///     path: Path to the JSON file for persistence
///
/// Example:
///     >>> from skreaver.memory import FileMemory
///     >>> memory = FileMemory("/tmp/agent_memory.json")
///     >>> memory.store("key", "value")
///     >>> memory.load("key")
///     'value'
#[pyclass(name = "FileMemory")]
pub struct PyFileMemory {
    inner: Mutex<FileMemory>,
    path: String,
}

#[pymethods]
impl PyFileMemory {
    /// Create a new FileMemory instance.
    ///
    /// Args:
    ///     path: Path to the JSON file for persistence
    #[new]
    pub fn new(path: String) -> Self {
        let inner = FileMemory::new(&path);
        Self {
            inner: Mutex::new(inner),
            path,
        }
    }

    /// Create a FileMemory with default path (temp directory).
    #[staticmethod]
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        let inner = FileMemory::default();
        let path = std::env::temp_dir()
            .join("skreaver_temp_memory.json")
            .to_string_lossy()
            .to_string();
        Self {
            inner: Mutex::new(inner),
            path,
        }
    }

    /// Get the file path for this memory backend.
    #[getter]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Load a value by key.
    ///
    /// Args:
    ///     key: The key to load
    ///
    /// Returns:
    ///     The value if found, None otherwise
    pub fn load(&self, key: &str) -> PyResult<Option<String>> {
        let memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        let memory_key = make_memory_key(key)?;
        memory
            .load(&memory_key)
            .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
    }

    /// Load multiple values by keys.
    ///
    /// Args:
    ///     keys: List of keys to load
    ///
    /// Returns:
    ///     List of values (None for missing keys)
    pub fn load_many(&self, keys: Vec<String>) -> PyResult<Vec<Option<String>>> {
        let memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        let memory_keys: Vec<MemoryKey> = keys
            .iter()
            .map(|k| make_memory_key(k))
            .collect::<PyResult<Vec<_>>>()?;
        memory
            .load_many(&memory_keys)
            .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
    }

    /// Store a key-value pair.
    ///
    /// Args:
    ///     key: The key to store
    ///     value: The value to store
    pub fn store(&self, key: &str, value: &str) -> PyResult<()> {
        let mut memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        let update = MemoryUpdate {
            key: make_memory_key(key)?,
            value: value.to_string(),
        };
        memory
            .store(update)
            .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
    }

    /// Store multiple key-value pairs atomically.
    ///
    /// Args:
    ///     updates: Dictionary of key-value pairs to store
    pub fn store_many(&self, _py: Python<'_>, updates: &Bound<'_, PyDict>) -> PyResult<()> {
        let mut memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        let mut memory_updates = Vec::new();
        for (key, value) in updates.iter() {
            let key_str: String = key.extract()?;
            let value_str: String = value.extract()?;
            memory_updates.push(MemoryUpdate {
                key: make_memory_key(&key_str)?,
                value: value_str,
            });
        }

        memory
            .store_many(memory_updates)
            .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
    }

    /// Create a snapshot of the current memory state.
    ///
    /// Returns:
    ///     JSON string containing all key-value pairs
    pub fn snapshot(&self) -> PyResult<Option<String>> {
        let mut memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        Ok(memory.snapshot())
    }

    /// Restore memory state from a snapshot.
    ///
    /// Args:
    ///     snapshot: JSON string from a previous snapshot() call
    pub fn restore(&self, snapshot: &str) -> PyResult<()> {
        let mut memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        memory
            .restore(snapshot)
            .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
    }

    /// Clean up old corrupted backup files.
    ///
    /// Args:
    ///     keep_count: Number of recent backups to keep (minimum 1)
    ///
    /// Returns:
    ///     Tuple of (succeeded, failed) counts
    pub fn cleanup_backups(&self, keep_count: usize) -> PyResult<(usize, usize)> {
        let memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        let result = memory
            .cleanup_backups(keep_count)
            .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))?;

        Ok((result.succeeded, result.failed))
    }

    /// Get all keys currently stored in memory.
    ///
    /// Returns:
    ///     List of all keys
    pub fn keys(&self) -> PyResult<Vec<String>> {
        let mut memory = self
            .inner
            .lock()
            .map_err(|e| crate::errors::MemoryError::new_err(format!("Lock poisoned: {}", e)))?;

        // Use snapshot to get all keys
        if let Some(snapshot) = memory.snapshot() {
            let data: HashMap<String, String> = serde_json::from_str(&snapshot)
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))?;
            Ok(data.keys().cloned().collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Get the number of entries in memory.
    ///
    /// Returns:
    ///     Number of key-value pairs stored
    pub fn __len__(&self) -> PyResult<usize> {
        self.keys().map(|k| k.len())
    }

    /// Check if the memory is empty.
    pub fn is_empty(&self) -> PyResult<bool> {
        self.__len__().map(|n| n == 0)
    }

    /// Check if a key exists in memory.
    ///
    /// Args:
    ///     key: The key to check
    ///
    /// Returns:
    ///     True if the key exists
    pub fn __contains__(&self, key: &str) -> PyResult<bool> {
        self.load(key).map(|v| v.is_some())
    }

    /// Get a value by key (dict-like access).
    pub fn __getitem__(&self, key: &str) -> PyResult<String> {
        self.load(key)?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err(key.to_string()))
    }

    /// Set a value by key (dict-like access).
    pub fn __setitem__(&self, key: &str, value: &str) -> PyResult<()> {
        self.store(key, value)
    }

    fn __repr__(&self) -> String {
        format!("FileMemory(path='{}')", self.path)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}

/// Redis-based memory backend.
///
/// High-performance distributed memory with connection pooling,
/// clustering support, and async operations.
///
/// Requires the `redis` feature to be enabled.
///
/// Args:
///     url: Redis connection URL (e.g., "redis://localhost:6379")
///     pool_size: Connection pool size (default: 10)
///     key_prefix: Optional prefix for all keys
///
/// Example:
///     >>> from skreaver.memory import RedisMemory
///     >>> memory = await RedisMemory.connect("redis://localhost:6379")
///     >>> await memory.store("key", "value")
///     >>> await memory.load("key")
///     'value'
#[cfg(feature = "redis")]
#[pyclass(name = "RedisMemory")]
pub struct PyRedisMemory {
    inner: std::sync::Arc<tokio::sync::Mutex<skreaver_memory::RedisMemory>>,
    url: String,
}

#[cfg(feature = "redis")]
#[pymethods]
impl PyRedisMemory {
    /// Connect to a Redis server.
    ///
    /// Args:
    ///     url: Redis connection URL (e.g., "redis://localhost:6379")
    ///     pool_size: Connection pool size (default: 10)
    ///     key_prefix: Optional prefix for all keys
    ///
    /// Returns:
    ///     Connected RedisMemory instance
    #[staticmethod]
    #[pyo3(signature = (url, pool_size=10, key_prefix=None))]
    pub fn connect<'py>(
        py: Python<'py>,
        url: String,
        pool_size: usize,
        key_prefix: Option<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        use skreaver_memory::redis::RedisConfigBuilder;

        let url_clone = url.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let mut builder = RedisConfigBuilder::new()
                .standalone(&url_clone)
                .with_pool_size(pool_size);

            if let Some(prefix) = key_prefix {
                builder = builder.with_key_prefix(prefix);
            }

            let config = builder
                .build()
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))?;

            let memory = skreaver_memory::RedisMemory::new(config)
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))?;

            Ok(PyRedisMemory {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(memory)),
                url: url_clone,
            })
        })
    }

    /// Connect to localhost Redis with default settings.
    ///
    /// Returns:
    ///     Connected RedisMemory instance
    #[staticmethod]
    pub fn localhost(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = skreaver_memory::RedisMemory::localhost()
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))?;

            Ok(PyRedisMemory {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(memory)),
                url: "redis://localhost:6379".to_string(),
            })
        })
    }

    /// Connect to a Redis cluster.
    ///
    /// Args:
    ///     nodes: List of cluster node URLs
    ///
    /// Returns:
    ///     Connected RedisMemory instance
    #[staticmethod]
    pub fn cluster(py: Python<'_>, nodes: Vec<String>) -> PyResult<Bound<'_, PyAny>> {
        let first_node = nodes.first().cloned().unwrap_or_default();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = skreaver_memory::RedisMemory::cluster(nodes)
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))?;

            Ok(PyRedisMemory {
                inner: std::sync::Arc::new(tokio::sync::Mutex::new(memory)),
                url: first_node,
            })
        })
    }

    /// Get the Redis URL.
    #[getter]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Load a value by key (async).
    ///
    /// Args:
    ///     key: The key to load
    ///
    /// Returns:
    ///     The value if found, None otherwise
    pub fn load<'py>(&self, py: Python<'py>, key: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = inner.lock().await;
            let memory_key = MemoryKey::new(&key)
                .map_err(|e| crate::errors::MemoryError::new_err(format!("Invalid key: {}", e)))?;
            memory
                .load_async(&memory_key)
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
        })
    }

    /// Load multiple values by keys (async).
    ///
    /// Args:
    ///     keys: List of keys to load
    ///
    /// Returns:
    ///     List of values (None for missing keys)
    pub fn load_many<'py>(
        &self,
        py: Python<'py>,
        keys: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = inner.lock().await;
            let memory_keys: Vec<MemoryKey> = keys
                .iter()
                .map(|k| {
                    MemoryKey::new(k).map_err(|e| {
                        crate::errors::MemoryError::new_err(format!("Invalid key: {}", e))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            memory
                .load_many_async(&memory_keys)
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
        })
    }

    /// Store a key-value pair (async).
    ///
    /// Args:
    ///     key: The key to store
    ///     value: The value to store
    pub fn store<'py>(
        &self,
        py: Python<'py>,
        key: String,
        value: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = inner.lock().await;
            let memory_key = MemoryKey::new(&key)
                .map_err(|e| crate::errors::MemoryError::new_err(format!("Invalid key: {}", e)))?;
            let update = MemoryUpdate {
                key: memory_key,
                value,
            };
            memory
                .store_async(update)
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
        })
    }

    /// Store multiple key-value pairs atomically (async).
    ///
    /// Args:
    ///     updates: Dictionary of key-value pairs to store
    pub fn store_many<'py>(
        &self,
        py: Python<'py>,
        updates: HashMap<String, String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = inner.lock().await;
            let memory_updates: Vec<MemoryUpdate> = updates
                .into_iter()
                .map(|(k, v)| {
                    let memory_key = MemoryKey::new(&k).map_err(|e| {
                        crate::errors::MemoryError::new_err(format!("Invalid key: {}", e))
                    })?;
                    Ok(MemoryUpdate {
                        key: memory_key,
                        value: v,
                    })
                })
                .collect::<Result<Vec<_>, pyo3::PyErr>>()?;
            memory
                .store_many_async(memory_updates)
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
        })
    }

    /// Create a snapshot of the current memory state (async).
    ///
    /// Returns:
    ///     JSON string containing all key-value pairs
    pub fn snapshot<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = inner.lock().await;
            memory
                .snapshot_async()
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
        })
    }

    /// Restore memory state from a snapshot (async).
    ///
    /// Args:
    ///     snapshot: JSON string from a previous snapshot() call
    pub fn restore<'py>(&self, py: Python<'py>, snapshot: String) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = inner.lock().await;
            memory
                .restore_async(&snapshot)
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))
        })
    }

    /// Perform a health check on the Redis connection (async).
    ///
    /// Returns:
    ///     Dictionary with health status information
    pub fn health_check<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let inner = self.inner.clone();
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            let memory = inner.lock().await;
            let health = memory
                .health_check()
                .await
                .map_err(|e| crate::errors::MemoryError::new_err(e.to_string()))?;

            // Convert health to a simple representation
            let status = match health {
                skreaver_memory::redis::RedisHealth::Healthy { .. } => "healthy",
                skreaver_memory::redis::RedisHealth::Unhealthy { .. } => "unhealthy",
            };
            Ok(status.to_string())
        })
    }

    fn __repr__(&self) -> String {
        format!("RedisMemory(url='{}')", self.url)
    }

    fn __str__(&self) -> String {
        self.__repr__()
    }
}
