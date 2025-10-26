//! Type-safe lock ordering for WebSocketManager
//!
//! This module enforces a consistent lock acquisition order to prevent deadlocks.
//! The order is: connections → ip_connections → subscriptions

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

use super::ConnectionState;

/// Container for all WebSocket manager locks with enforced ordering
#[derive(Clone)]
pub struct ManagerLocks {
    /// Active connections (acquire first)
    pub(super) connections: Arc<RwLock<HashMap<Uuid, ConnectionState>>>,
    /// Connections per IP address (acquire second)
    pub(super) connections_per_ip: Arc<RwLock<HashMap<IpAddr, usize>>>,
    /// Channel subscriptions (acquire third)
    pub(super) subscriptions: Arc<RwLock<HashMap<String, Vec<Uuid>>>>,
}

impl ManagerLocks {
    /// Create new manager locks
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            connections_per_ip: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for ManagerLocks {
    fn default() -> Self {
        Self::new()
    }
}

/// Level 1 lock guard: connections only
pub struct Level1ReadGuard<'a> {
    pub connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
}

/// Level 1 write lock guard: connections only
pub struct Level1WriteGuard<'a> {
    pub connections: RwLockWriteGuard<'a, HashMap<Uuid, ConnectionState>>,
}

/// Level 2 read lock guard: connections + ip_connections (read mode)
pub struct Level2ReadGuard<'a> {
    pub connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub ip_connections: RwLockReadGuard<'a, HashMap<IpAddr, usize>>,
}

/// Level 2 lock guard: connections + ip_connections
pub struct Level2WriteGuard<'a> {
    pub connections: RwLockWriteGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub ip_connections: RwLockWriteGuard<'a, HashMap<IpAddr, usize>>,
}

/// Level 3 lock guard: connections + ip_connections + subscriptions
pub struct Level3WriteGuard<'a> {
    pub connections: RwLockWriteGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub ip_connections: RwLockWriteGuard<'a, HashMap<IpAddr, usize>>,
    pub subscriptions: RwLockWriteGuard<'a, HashMap<String, Vec<Uuid>>>,
}

/// Level 3 read guard: all locks in read mode
pub struct Level3ReadGuard<'a> {
    pub connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub ip_connections: RwLockReadGuard<'a, HashMap<IpAddr, usize>>,
    pub subscriptions: RwLockReadGuard<'a, HashMap<String, Vec<Uuid>>>,
}

/// Specialized guard for subscriptions + connections (used in specific cases)
///
/// Note: This still respects lock ordering by acquiring connections first
pub struct SubscriptionsGuard<'a> {
    _connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub subscriptions: RwLockWriteGuard<'a, HashMap<String, Vec<Uuid>>>,
}

impl ManagerLocks {
    /// Acquire level 1 read lock (connections only)
    pub async fn level1_read(&self) -> Level1ReadGuard<'_> {
        Level1ReadGuard {
            connections: self.connections.read().await,
        }
    }

    /// Acquire level 1 write lock (connections only)
    pub async fn level1_write(&self) -> Level1WriteGuard<'_> {
        Level1WriteGuard {
            connections: self.connections.write().await,
        }
    }

    /// Acquire level 2 read locks (connections + ip_connections, read mode)
    pub async fn level2_read(&self) -> Level2ReadGuard<'_> {
        let connections = self.connections.read().await;
        let ip_connections = self.connections_per_ip.read().await;

        Level2ReadGuard {
            connections,
            ip_connections,
        }
    }

    /// Acquire level 2 write locks (connections + ip_connections)
    pub async fn level2_write(&self) -> Level2WriteGuard<'_> {
        let connections = self.connections.write().await;
        let ip_connections = self.connections_per_ip.write().await;

        Level2WriteGuard {
            connections,
            ip_connections,
        }
    }

    /// Acquire level 3 write locks (all locks)
    pub async fn level3_write(&self) -> Level3WriteGuard<'_> {
        let connections = self.connections.write().await;
        let ip_connections = self.connections_per_ip.write().await;
        let subscriptions = self.subscriptions.write().await;

        Level3WriteGuard {
            connections,
            ip_connections,
            subscriptions,
        }
    }

    /// Acquire level 3 read locks (all locks in read mode)
    pub async fn level3_read(&self) -> Level3ReadGuard<'_> {
        let connections = self.connections.read().await;
        let ip_connections = self.connections_per_ip.read().await;
        let subscriptions = self.subscriptions.read().await;

        Level3ReadGuard {
            connections,
            ip_connections,
            subscriptions,
        }
    }

    /// Acquire subscriptions lock with proper ordering
    ///
    /// This acquires connections as a read lock first (to maintain ordering),
    /// then subscriptions as a write lock.
    pub async fn subscriptions_write(&self) -> SubscriptionsGuard<'_> {
        let _connections = self.connections.read().await;
        let subscriptions = self.subscriptions.write().await;

        SubscriptionsGuard {
            _connections,
            subscriptions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_level1_locks() {
        let locks = ManagerLocks::new();

        let _guard = locks.level1_read().await;
        // Successfully acquired
    }

    #[tokio::test]
    async fn test_level2_locks() {
        let locks = ManagerLocks::new();

        let _guard = locks.level2_write().await;
        // Successfully acquired in correct order
    }

    #[tokio::test]
    async fn test_level3_locks() {
        let locks = ManagerLocks::new();

        let _guard = locks.level3_write().await;
        // Successfully acquired in correct order
    }

    #[tokio::test]
    async fn test_concurrent_read_locks() {
        let locks = ManagerLocks::new();

        let guard1 = locks.level1_read().await;
        let guard2 = locks.level1_read().await;

        // Multiple read locks can coexist
        assert!(guard1.connections.is_empty());
        assert!(guard2.connections.is_empty());
    }
}
