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
///
/// # Errors
///
/// Returns `MemoryError` if:
/// - The runtime cannot be created (resource exhaustion)
/// - The runtime is already borrowed (nested calls)
/// - The async operation fails
///
/// # Panics
///
/// This function will NOT panic. All error conditions are handled gracefully.
#[cfg(feature = "redis")]
pub fn with_redis_runtime<F, R>(f: F) -> Result<R, MemoryError>
where
    F: FnOnce() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<R, MemoryError>>>>,
{
    REDIS_RUNTIME.with(|rt_cell| {
        // Try to borrow mutably - this can fail if we have nested calls
        let mut rt_ref = match rt_cell.try_borrow_mut() {
            Ok(r) => r,
            Err(_) => {
                return Err(MemoryError::LoadFailed {
                    key: MemoryKeys::runtime(),
                    backend: skreaver_core::error::MemoryBackend::Redis,
                    kind: skreaver_core::error::MemoryErrorKind::InternalError {
                        backend_error:
                            "Redis runtime already borrowed - nested runtime calls not supported"
                                .to_string(),
                    },
                });
            }
        };

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

        // Execute with the runtime - guaranteed to be Ready at this point
        match &*rt_ref {
            RuntimeState::Ready(rt) => rt.block_on(f()),
            RuntimeState::Uninitialized => {
                // This is truly unreachable after the initialization above succeeds,
                // but we return an error instead of panicking for safety
                Err(MemoryError::LoadFailed {
                    key: MemoryKeys::runtime(),
                    backend: skreaver_core::error::MemoryBackend::Redis,
                    kind: skreaver_core::error::MemoryErrorKind::InternalError {
                        backend_error: "Runtime initialization succeeded but state is inconsistent"
                            .to_string(),
                    },
                })
            }
        }
    })
}
