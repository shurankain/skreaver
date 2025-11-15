//! RAII guards for automatic resource cleanup in WebSocket connections
//!
//! This module provides [`ConnectionGuard`], an RAII (Resource Acquisition Is Initialization)
//! guard that guarantees WebSocket connection cleanup even in the presence of panics.
//!
//! # Problem: Resource Leaks on Panic
//!
//! Without RAII guards, connection cleanup depends on explicit calls that might not execute:
//!
//! ```ignore
//! async fn handle_socket(...) {
//!     manager.add_connection(conn_id, info).await?;
//!
//!     tokio::select! {
//!         _ = task1 => {}  // If task1 panics...
//!         _ = task2 => {}  // If task2 panics...
//!     }
//!
//!     manager.remove_connection(conn_id).await;  // ← This might not run!
//! }
//! ```
//!
//! # Solution: Automatic Cleanup
//!
//! ConnectionGuard uses Rust's Drop trait to guarantee cleanup:
//!
//! ```ignore
//! async fn handle_socket(...) {
//!     let _guard = manager.register_connection(conn_id, info).await?;
//!
//!     tokio::select! {
//!         _ = task1 => {}  // Even if this panics...
//!         _ = task2 => {}  // Or this panics...
//!     }
//!
//!     // Guard drops here - cleanup ALWAYS happens!
//! }
//! ```

use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

use super::manager::WebSocketManager;

/// RAII guard that ensures WebSocket connection cleanup on drop.
///
/// This guard guarantees that connection cleanup happens even if:
/// - A task panics
/// - An early return occurs
/// - The connection handler exits normally
///
/// # Panic Safety
///
/// The Drop implementation spawns a new async task for cleanup to avoid
/// blocking during drop. This ensures cleanup runs even if Drop is called
/// from a panic context.
///
/// # Example
///
/// ```ignore
/// async fn handle_connection(manager: Arc<WebSocketManager>, conn_id: Uuid) {
///     // Create guard - connection is registered
///     let _guard = ConnectionGuard::new(conn_id, Arc::clone(&manager));
///
///     // ... handle connection ...
///     // If this panics, guard still drops and cleans up!
///
///     // Guard drops here - connection is automatically removed
/// }
/// ```
pub struct ConnectionGuard {
    conn_id: Uuid,
    manager: Arc<WebSocketManager>,
    /// Whether cleanup has already been performed
    cleaned: bool,
}

impl ConnectionGuard {
    /// Create a new connection guard
    ///
    /// # Arguments
    ///
    /// * `conn_id` - The connection ID to track
    /// * `manager` - The WebSocket manager that owns the connection
    ///
    /// # Note
    ///
    /// This does NOT register the connection - call `WebSocketManager::add_connection`
    /// before creating the guard.
    pub fn new(conn_id: Uuid, manager: Arc<WebSocketManager>) -> Self {
        Self {
            conn_id,
            manager,
            cleaned: false,
        }
    }

    /// Get the connection ID
    pub fn conn_id(&self) -> Uuid {
        self.conn_id
    }

    /// Explicitly clean up the connection
    ///
    /// This allows manual cleanup before the guard drops. Useful if you want
    /// to handle cleanup errors explicitly rather than silently in Drop.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut guard = ConnectionGuard::new(conn_id, manager);
    /// // ... use connection ...
    /// guard.cleanup().await;  // Explicit cleanup
    /// // Drop is now a no-op since cleanup already happened
    /// ```
    pub async fn cleanup(&mut self) {
        if !self.cleaned {
            info!(
                "Cleaning up WebSocket connection {} (explicit)",
                self.conn_id
            );
            self.manager.remove_connection(self.conn_id).await;
            self.cleaned = true;
        }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if !self.cleaned {
            info!("Cleaning up WebSocket connection {} (RAII)", self.conn_id);

            // SAFETY: We need to spawn cleanup as an async task because Drop is synchronous.
            // The cleanup will complete even if the current task is being torn down.
            //
            // We clone the Arc before spawning to ensure the manager stays alive for cleanup.
            let manager = Arc::clone(&self.manager);
            let conn_id = self.conn_id;

            // Spawn cleanup task - this will run even if we're dropping due to panic
            tokio::spawn(async move {
                manager.remove_connection(conn_id).await;
            });

            self.cleaned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websocket::{ConnectionInfo, WebSocketConfig};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn create_test_manager() -> Arc<WebSocketManager> {
        let config = WebSocketConfig::default();
        Arc::new(WebSocketManager::new(config))
    }

    fn create_test_addr() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
    }

    #[tokio::test]
    async fn test_guard_cleanup_on_normal_drop() {
        let manager = create_test_manager();
        let conn_info = ConnectionInfo::new(create_test_addr());
        let conn_id = conn_info.id();

        // Register connection
        let _tx = manager.add_connection(conn_id, conn_info).await.unwrap();

        // Verify connection exists
        assert_eq!(manager.connection_count().await, 1);

        {
            let _guard = ConnectionGuard::new(conn_id, Arc::clone(&manager));
            // Guard created - connection still exists
            assert_eq!(manager.connection_count().await, 1);
        } // Guard drops here

        // Give cleanup task time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Connection should be removed
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_guard_cleanup_on_panic() {
        let manager = create_test_manager();
        let conn_info = ConnectionInfo::new(create_test_addr());
        let conn_id = conn_info.id();

        // Register connection
        let _tx = manager.add_connection(conn_id, conn_info).await.unwrap();
        assert_eq!(manager.connection_count().await, 1);

        // Simulate panic scenario
        let result = tokio::spawn({
            let manager = Arc::clone(&manager);
            async move {
                let _guard = ConnectionGuard::new(conn_id, Arc::clone(&manager));
                // Simulate panic
                panic!("Simulated panic!");
            }
        })
        .await;

        // Task should have panicked
        assert!(result.is_err());

        // Give cleanup task time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Connection should still be cleaned up despite panic
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_explicit_cleanup() {
        let manager = create_test_manager();
        let conn_info = ConnectionInfo::new(create_test_addr());
        let conn_id = conn_info.id();

        let _tx = manager.add_connection(conn_id, conn_info).await.unwrap();
        assert_eq!(manager.connection_count().await, 1);

        {
            let mut guard = ConnectionGuard::new(conn_id, Arc::clone(&manager));

            // Explicit cleanup
            guard.cleanup().await;
            assert_eq!(manager.connection_count().await, 0);

            // Drop should be a no-op now
        }

        // Connection should still be removed (cleanup happened explicitly)
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_double_cleanup_is_safe() {
        let manager = create_test_manager();
        let conn_info = ConnectionInfo::new(create_test_addr());
        let conn_id = conn_info.id();

        let _tx = manager.add_connection(conn_id, conn_info).await.unwrap();

        {
            let mut guard = ConnectionGuard::new(conn_id, Arc::clone(&manager));

            // First cleanup
            guard.cleanup().await;
            assert_eq!(manager.connection_count().await, 0);

            // Second cleanup should be safe (no-op)
            guard.cleanup().await;
            assert_eq!(manager.connection_count().await, 0);
        }

        // Drop cleanup should also be safe (no-op)
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_guard_with_early_return() {
        let manager = create_test_manager();
        let conn_info = ConnectionInfo::new(create_test_addr());
        let conn_id = conn_info.id();

        let _tx = manager.add_connection(conn_id, conn_info).await.unwrap();

        async fn simulate_early_return(
            manager: Arc<WebSocketManager>,
            conn_id: Uuid,
        ) -> Result<(), &'static str> {
            let _guard = ConnectionGuard::new(conn_id, Arc::clone(&manager));

            // Early return - guard still drops!
            return Err("early return");

            #[allow(unreachable_code)]
            {
                Ok(())
            }
        }

        let result = simulate_early_return(Arc::clone(&manager), conn_id).await;
        assert!(result.is_err());

        // Give cleanup time to run
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Connection should be cleaned up despite early return
        assert_eq!(manager.connection_count().await, 0);
    }
}
