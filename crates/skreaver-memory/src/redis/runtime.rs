//! Runtime utilities for Redis operations
//!
//! This module provides thread-local runtime management for synchronous
//! wrapper functions around async Redis operations.

use skreaver_core::error::MemoryError;
use skreaver_core::memory::MemoryKeys;

/// Runtime state for type-safe initialization tracking
#[cfg(feature = "redis")]
pub enum RuntimeState {
    Uninitialized,
    Ready(tokio::runtime::Runtime),
}

// Sync trait implementations using thread-local runtime
#[cfg(feature = "redis")]
thread_local! {
    pub static REDIS_RUNTIME: std::cell::RefCell<RuntimeState> =
        const { std::cell::RefCell::new(RuntimeState::Uninitialized) };
}

/// Execute an async function using the thread-local runtime
#[cfg(feature = "redis")]
pub fn with_redis_runtime<F, R>(f: F) -> Result<R, MemoryError>
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, MemoryError>>>>,
{
    REDIS_RUNTIME.with(|rt_cell| {
        let mut rt_ref = rt_cell.borrow_mut();

        // Initialize runtime if needed
        if matches!(&*rt_ref, RuntimeState::Uninitialized) {
            let runtime = tokio::runtime::Runtime::new().map_err(|e| MemoryError::LoadFailed {
                key: MemoryKeys::runtime(),
                backend: skreaver_core::error::MemoryBackend::Redis,
                kind: skreaver_core::error::MemoryErrorKind::InternalError {
                    backend_error: format!("Failed to create async runtime: {}", e),
                },
            })?;
            *rt_ref = RuntimeState::Ready(runtime);
        }

        // SAFETY: We just ensured the runtime is in Ready state
        match &*rt_ref {
            RuntimeState::Ready(rt) => rt.block_on(f()),
            #[cfg(debug_assertions)]
            RuntimeState::Uninitialized => {
                panic!("BUG: Runtime should be Ready after initialization")
            }
            #[cfg(not(debug_assertions))]
            RuntimeState::Uninitialized => unsafe { std::hint::unreachable_unchecked() },
        }
    })
}
