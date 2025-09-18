//! Runtime utilities for Redis operations
//!
//! This module provides thread-local runtime management for synchronous
//! wrapper functions around async Redis operations.

use skreaver_core::error::MemoryError;
use skreaver_core::memory::MemoryKey;

// Sync trait implementations using thread-local runtime
#[cfg(feature = "redis")]
thread_local! {
    pub static REDIS_RUNTIME: std::cell::RefCell<Option<tokio::runtime::Runtime>> =
        const { std::cell::RefCell::new(None) };
}

/// Execute an async function using the thread-local runtime
#[cfg(feature = "redis")]
pub fn with_redis_runtime<F, R>(f: F) -> Result<R, MemoryError>
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, MemoryError>>>>,
{
    REDIS_RUNTIME.with(|rt_cell| {
        let mut rt_ref = rt_cell.borrow_mut();
        if rt_ref.is_none() {
            *rt_ref =
                Some(
                    tokio::runtime::Runtime::new().map_err(|e| MemoryError::LoadFailed {
                        key: MemoryKey::new("runtime").unwrap(),
                        reason: format!("Failed to create async runtime: {}", e),
                    })?,
                );
        }
        let rt = rt_ref.as_ref().unwrap();
        rt.block_on(f())
    })
}
