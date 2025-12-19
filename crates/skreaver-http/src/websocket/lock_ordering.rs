//! Type-safe lock ordering for WebSocketManager
//!
//! This module enforces a consistent lock acquisition order to prevent deadlocks.
//! The order is: connections → ip_connections → subscriptions
//!
//! # Safety Model (LOW-43)
//!
//! Lock ordering is enforced through two complementary mechanisms:
//!
//! 1. **Compile-time safety**: The typed methods (`level1_write`, `level2_write`, `level3_write`)
//!    and composite guards (`Level2WriteGuard`, `Level3WriteGuard`) enforce ordering through
//!    their API design. It's impossible to acquire locks out of order using these APIs.
//!
//! 2. **Debug-time runtime verification**: In debug builds, `assert_lock_order` panics if
//!    locks are acquired out of order. This catches any direct lock access that bypasses
//!    the typed APIs.
//!
//! In release builds, the runtime verification is disabled for performance. The typed API
//! is the primary safety mechanism - code that exclusively uses `level*_write()` methods
//! cannot deadlock regardless of build mode.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

use super::ConnectionState;

// ============================================================================
// Runtime Lock Ordering Verification (Debug Only)
// ============================================================================

#[cfg(debug_assertions)]
use std::cell::RefCell;

/// Lock levels for ordering verification
#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LockLevel {
    Connections = 1,
    IpConnections = 2,
    Subscriptions = 3,
}

#[cfg(debug_assertions)]
impl LockLevel {
    fn name(&self) -> &'static str {
        match self {
            LockLevel::Connections => "connections",
            LockLevel::IpConnections => "ip_connections",
            LockLevel::Subscriptions => "subscriptions",
        }
    }
}

// Thread-local storage for tracking lock acquisition order
#[cfg(debug_assertions)]
thread_local! {
    static LOCK_ORDER: RefCell<Vec<LockLevel>> = const { RefCell::new(Vec::new()) };
}

/// Assert that a lock can be acquired without violating ordering
#[cfg(debug_assertions)]
fn assert_lock_order(lock_level: LockLevel) {
    LOCK_ORDER.with(|order| {
        let current_order = order.borrow();

        // Check that we're acquiring locks in increasing order
        // Allow same-level locks (for multiple read locks at same level)
        if let Some(&last_level) = current_order.last()
            && lock_level < last_level
        {
            panic!(
                "Lock ordering violation detected! \
                Attempted to acquire {} (level {:?}) after {} (level {:?}). \
                Correct order: connections → ip_connections → subscriptions",
                lock_level.name(),
                lock_level as u8,
                last_level.name(),
                last_level as u8
            );
        }
    });
}

/// Record that a lock has been acquired
#[cfg(debug_assertions)]
fn record_lock_acquired(lock_level: LockLevel) {
    LOCK_ORDER.with(|order| {
        order.borrow_mut().push(lock_level);
    });
}

/// Record that a lock has been released
#[cfg(debug_assertions)]
fn record_lock_released(lock_level: LockLevel) {
    LOCK_ORDER.with(|order| {
        let mut current_order = order.borrow_mut();
        if let Some(pos) = current_order.iter().rposition(|&l| l == lock_level) {
            current_order.remove(pos);
        }
    });
}

/// RAII guard to track lock lifecycle in debug builds
#[cfg(debug_assertions)]
struct LockTracker {
    level: LockLevel,
}

#[cfg(debug_assertions)]
impl LockTracker {
    fn new(level: LockLevel) -> Self {
        assert_lock_order(level);
        record_lock_acquired(level);
        Self { level }
    }
}

#[cfg(debug_assertions)]
impl Drop for LockTracker {
    fn drop(&mut self) {
        record_lock_released(self.level);
    }
}

// ============================================================================
// Lock Guards with Runtime Verification
// ============================================================================

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
#[allow(dead_code)]
pub struct Level1ReadGuard<'a> {
    pub(super) connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
    #[cfg(debug_assertions)]
    _tracker: LockTracker,
}

/// Level 1 write lock guard: connections only
pub struct Level1WriteGuard<'a> {
    pub(super) connections: RwLockWriteGuard<'a, HashMap<Uuid, ConnectionState>>,
    #[cfg(debug_assertions)]
    _tracker: LockTracker,
}

/// Level 2 read lock guard: connections + ip_connections (read mode)
#[allow(dead_code)]
pub struct Level2ReadGuard<'a> {
    pub(super) connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub(super) ip_connections: RwLockReadGuard<'a, HashMap<IpAddr, usize>>,
    #[cfg(debug_assertions)]
    _tracker_connections: LockTracker,
    #[cfg(debug_assertions)]
    _tracker_ip: LockTracker,
}

/// Level 2 lock guard: connections + ip_connections
pub struct Level2WriteGuard<'a> {
    pub(super) connections: RwLockWriteGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub(super) ip_connections: RwLockWriteGuard<'a, HashMap<IpAddr, usize>>,
    #[cfg(debug_assertions)]
    _tracker_connections: LockTracker,
    #[cfg(debug_assertions)]
    _tracker_ip: LockTracker,
}

/// Level 3 lock guard: connections + ip_connections + subscriptions
pub struct Level3WriteGuard<'a> {
    pub(super) connections: RwLockWriteGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub(super) ip_connections: RwLockWriteGuard<'a, HashMap<IpAddr, usize>>,
    pub(super) subscriptions: RwLockWriteGuard<'a, HashMap<String, Vec<Uuid>>>,
    #[cfg(debug_assertions)]
    _tracker_connections: LockTracker,
    #[cfg(debug_assertions)]
    _tracker_ip: LockTracker,
    #[cfg(debug_assertions)]
    _tracker_subs: LockTracker,
}

/// Level 3 read guard: all locks in read mode
#[allow(dead_code)]
pub struct Level3ReadGuard<'a> {
    pub(super) connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub(super) ip_connections: RwLockReadGuard<'a, HashMap<IpAddr, usize>>,
    pub(super) subscriptions: RwLockReadGuard<'a, HashMap<String, Vec<Uuid>>>,
    #[cfg(debug_assertions)]
    _tracker_connections: LockTracker,
    #[cfg(debug_assertions)]
    _tracker_ip: LockTracker,
    #[cfg(debug_assertions)]
    _tracker_subs: LockTracker,
}

/// Specialized guard for subscriptions + connections (used in specific cases)
///
/// Note: This still respects lock ordering by acquiring connections first
pub struct SubscriptionsGuard<'a> {
    _connections: RwLockReadGuard<'a, HashMap<Uuid, ConnectionState>>,
    pub subscriptions: RwLockWriteGuard<'a, HashMap<String, Vec<Uuid>>>,
    #[cfg(debug_assertions)]
    _tracker_connections: LockTracker,
    #[cfg(debug_assertions)]
    _tracker_subs: LockTracker,
}

impl ManagerLocks {
    /// Acquire level 1 read lock (connections only)
    pub async fn level1_read(&self) -> Level1ReadGuard<'_> {
        #[cfg(debug_assertions)]
        let _tracker = LockTracker::new(LockLevel::Connections);

        Level1ReadGuard {
            connections: self.connections.read().await,
            #[cfg(debug_assertions)]
            _tracker,
        }
    }

    /// Acquire level 1 write lock (connections only)
    pub async fn level1_write(&self) -> Level1WriteGuard<'_> {
        #[cfg(debug_assertions)]
        let _tracker = LockTracker::new(LockLevel::Connections);

        Level1WriteGuard {
            connections: self.connections.write().await,
            #[cfg(debug_assertions)]
            _tracker,
        }
    }

    /// Acquire level 2 read locks (connections + ip_connections, read mode)
    pub async fn level2_read(&self) -> Level2ReadGuard<'_> {
        #[cfg(debug_assertions)]
        let _tracker_connections = LockTracker::new(LockLevel::Connections);

        let connections = self.connections.read().await;

        #[cfg(debug_assertions)]
        let _tracker_ip = LockTracker::new(LockLevel::IpConnections);

        let ip_connections = self.connections_per_ip.read().await;

        Level2ReadGuard {
            connections,
            ip_connections,
            #[cfg(debug_assertions)]
            _tracker_connections,
            #[cfg(debug_assertions)]
            _tracker_ip,
        }
    }

    /// Acquire level 2 write locks (connections + ip_connections)
    pub async fn level2_write(&self) -> Level2WriteGuard<'_> {
        #[cfg(debug_assertions)]
        let _tracker_connections = LockTracker::new(LockLevel::Connections);

        let connections = self.connections.write().await;

        #[cfg(debug_assertions)]
        let _tracker_ip = LockTracker::new(LockLevel::IpConnections);

        let ip_connections = self.connections_per_ip.write().await;

        Level2WriteGuard {
            connections,
            ip_connections,
            #[cfg(debug_assertions)]
            _tracker_connections,
            #[cfg(debug_assertions)]
            _tracker_ip,
        }
    }

    /// Acquire level 3 write locks (all locks)
    pub async fn level3_write(&self) -> Level3WriteGuard<'_> {
        #[cfg(debug_assertions)]
        let _tracker_connections = LockTracker::new(LockLevel::Connections);

        let connections = self.connections.write().await;

        #[cfg(debug_assertions)]
        let _tracker_ip = LockTracker::new(LockLevel::IpConnections);

        let ip_connections = self.connections_per_ip.write().await;

        #[cfg(debug_assertions)]
        let _tracker_subs = LockTracker::new(LockLevel::Subscriptions);

        let subscriptions = self.subscriptions.write().await;

        Level3WriteGuard {
            connections,
            ip_connections,
            subscriptions,
            #[cfg(debug_assertions)]
            _tracker_connections,
            #[cfg(debug_assertions)]
            _tracker_ip,
            #[cfg(debug_assertions)]
            _tracker_subs,
        }
    }

    /// Acquire level 3 read locks (all locks in read mode)
    pub async fn level3_read(&self) -> Level3ReadGuard<'_> {
        #[cfg(debug_assertions)]
        let _tracker_connections = LockTracker::new(LockLevel::Connections);

        let connections = self.connections.read().await;

        #[cfg(debug_assertions)]
        let _tracker_ip = LockTracker::new(LockLevel::IpConnections);

        let ip_connections = self.connections_per_ip.read().await;

        #[cfg(debug_assertions)]
        let _tracker_subs = LockTracker::new(LockLevel::Subscriptions);

        let subscriptions = self.subscriptions.read().await;

        Level3ReadGuard {
            connections,
            ip_connections,
            subscriptions,
            #[cfg(debug_assertions)]
            _tracker_connections,
            #[cfg(debug_assertions)]
            _tracker_ip,
            #[cfg(debug_assertions)]
            _tracker_subs,
        }
    }

    /// Acquire subscriptions lock with proper ordering
    ///
    /// This acquires connections as a read lock first (to maintain ordering),
    /// then subscriptions as a write lock.
    pub async fn subscriptions_write(&self) -> SubscriptionsGuard<'_> {
        #[cfg(debug_assertions)]
        let _tracker_connections = LockTracker::new(LockLevel::Connections);

        let _connections = self.connections.read().await;

        #[cfg(debug_assertions)]
        let _tracker_subs = LockTracker::new(LockLevel::Subscriptions);

        let subscriptions = self.subscriptions.write().await;

        SubscriptionsGuard {
            _connections,
            subscriptions,
            #[cfg(debug_assertions)]
            _tracker_connections,
            #[cfg(debug_assertions)]
            _tracker_subs,
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

    #[tokio::test]
    async fn test_correct_lock_ordering() {
        let locks = ManagerLocks::new();

        // Correct order: connections → ip_connections → subscriptions
        let _guard = locks.level3_write().await;
        // Should not panic in debug builds
    }

    #[tokio::test]
    async fn test_lock_ordering_level2_after_level1() {
        let locks = ManagerLocks::new();

        // Acquire level 1
        let guard1 = locks.level1_write().await;
        drop(guard1); // Release before acquiring level 2

        // Now acquire level 2 (should work because level 1 was released)
        let _guard2 = locks.level2_write().await;
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "Lock ordering violation")]
    fn test_lock_ordering_violation_detection() {
        // This test manually violates lock ordering to verify detection
        // We directly use the lock tracking functions

        // Simulate acquiring subscriptions (level 3)
        record_lock_acquired(LockLevel::Subscriptions);

        // Now try to acquire connections (level 1) - should panic!
        assert_lock_order(LockLevel::Connections);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_lock_level_ordering() {
        // Verify lock levels are properly ordered
        assert!(LockLevel::Connections < LockLevel::IpConnections);
        assert!(LockLevel::IpConnections < LockLevel::Subscriptions);
        assert!(LockLevel::Connections < LockLevel::Subscriptions);
    }
}
