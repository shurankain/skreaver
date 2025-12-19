//! Runtime utilities for Redis operations
//!
//! This module provides thread-local runtime management for synchronous
//! wrapper functions around async Redis operations.
//!
//! # Thread Safety (MEDIUM-32)
//!
//! This module uses `thread_local!` storage, which means:
//!
//! - **Each thread has its own runtime**: If your code moves between threads
//!   (e.g., via `tokio::spawn` or thread pool), each thread will create
//!   its own Tokio runtime on first access.
//!
//! - **Not Send/Sync**: The `REDIS_RUNTIME` static is not thread-safe and
//!   cannot be shared across threads. This is by design - each thread needs
//!   its own runtime to avoid blocking other threads.
//!
//! - **Debug tracking**: Debug builds track thread IDs to help diagnose
//!   unexpected multi-thread runtime creation patterns.
//!
//! ## Recommended Usage
//!
//! For async code, prefer using the async Redis client directly with the
//! existing Tokio runtime rather than creating blocking wrappers.
//!
//! For sync code that must call async Redis operations, this module provides
//! a safe way to do so on a per-thread basis.

use skreaver_core::error::MemoryError;
use skreaver_core::memory::MemoryKeys;

/// Runtime state for type-safe initialization tracking
///
/// MEDIUM-32: Includes thread ID tracking in debug builds to help diagnose
/// unexpected multi-thread runtime creation patterns.
#[cfg(feature = "redis")]
pub enum RuntimeState {
    Uninitialized,
    Ready {
        runtime: tokio::runtime::Runtime,
        /// Thread ID that created this runtime (debug only)
        #[cfg(debug_assertions)]
        created_by_thread: std::thread::ThreadId,
    },
}

// Sync trait implementations using thread-local runtime
//
// MEDIUM-32: Note that thread_local! ensures each thread gets its own
// independent RuntimeState. This is intentional but can lead to multiple
// runtimes being created if code runs on multiple threads.
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
///
/// # Thread Safety (MEDIUM-32)
///
/// This function uses thread-local storage, so each thread will have its own
/// Tokio runtime. In debug builds, the thread ID is tracked and logged when
/// a new runtime is created to help diagnose unexpected multi-thread patterns.
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

            // MEDIUM-32: Track thread ID in debug builds to help diagnose
            // unexpected multi-thread runtime creation patterns
            #[cfg(debug_assertions)]
            {
                let thread_id = std::thread::current().id();
                tracing::debug!(
                    ?thread_id,
                    "Created new Redis runtime for thread (MEDIUM-32 tracking)"
                );
                *rt_ref = RuntimeState::Ready {
                    runtime,
                    created_by_thread: thread_id,
                };
            }

            #[cfg(not(debug_assertions))]
            {
                *rt_ref = RuntimeState::Ready { runtime };
            }
        }

        // Execute with the runtime - guaranteed to be Ready at this point
        match &*rt_ref {
            RuntimeState::Ready { runtime, .. } => runtime.block_on(f()),
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
