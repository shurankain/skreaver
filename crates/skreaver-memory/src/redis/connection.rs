//! Type-safe Redis connection state tracking with phantom types
//!
//! This module provides compile-time guarantees for Redis connection lifecycle
//! management, making it impossible to use disconnected connections.

use std::marker::PhantomData;
use std::time::{Duration, Instant};

#[cfg(feature = "redis")]
use deadpool_redis::{Connection as PooledConnection, Pool};

use skreaver_core::error::MemoryError;
use skreaver_core::memory::MemoryKeys;

// === Connection State Phantom Types ===

/// Marker trait for connection states
pub trait ConnectionState {}

/// Connection is established and ready for operations
#[derive(Debug, Clone)]
pub struct Connected;

/// Connection is not available or has been disconnected
#[derive(Debug, Clone)]
pub struct Disconnected;

impl ConnectionState for Connected {}
impl ConnectionState for Disconnected {}

// === Type-Safe Connection Wrapper ===

/// Connection data holder based on state
#[cfg(feature = "redis")]
enum ConnectionData {
    /// Disconnected state - no connection data
    Disconnected {
        attempt_count: usize,
        last_activity: Option<Instant>,
    },
    /// Connected state - guaranteed valid connection
    Connected {
        connection: PooledConnection,
        connected_at: Instant,
        last_activity: Instant,
        attempt_count: usize,
    },
}

/// Type-safe Redis connection wrapper with state tracking
///
/// This uses an enum internally to ensure that Connected state always has
/// a valid connection, making the typestate pattern truly compile-time safe.
#[cfg(feature = "redis")]
pub struct RedisConnection<State: ConnectionState> {
    data: ConnectionData,
    _state: PhantomData<State>,
}

/// Type alias for connected Redis connection
#[cfg(feature = "redis")]
pub type ConnectedRedis = RedisConnection<Connected>;

/// Type alias for disconnected Redis connection
#[cfg(feature = "redis")]
pub type DisconnectedRedis = RedisConnection<Disconnected>;

// === Connection Implementation ===

#[cfg(feature = "redis")]
impl RedisConnection<Disconnected> {
    /// Create a new disconnected connection
    pub fn new_disconnected() -> Self {
        Self {
            data: ConnectionData::Disconnected {
                attempt_count: 0,
                last_activity: None,
            },
            _state: PhantomData,
        }
    }

    /// Attempt to establish connection from pool
    pub async fn connect(
        self,
        pool: &Pool,
    ) -> Result<RedisConnection<Connected>, (Self, MemoryError)> {
        let (attempt_count, last_activity) = match self.data {
            ConnectionData::Disconnected {
                attempt_count,
                last_activity,
            } => {
                // MEDIUM-28: Use saturating_add to prevent integer overflow
                // If connection fails usize::MAX times, counter stays at max instead of wrapping
                (attempt_count.saturating_add(1), last_activity)
            }
            _ => panic!("INVARIANT VIOLATION: Disconnected state must have Disconnected data"),
        };

        match pool.get().await {
            Ok(conn) => {
                let now = Instant::now();
                Ok(RedisConnection {
                    data: ConnectionData::Connected {
                        connection: conn,
                        connected_at: now,
                        last_activity: now,
                        attempt_count,
                    },
                    _state: PhantomData,
                })
            }
            Err(e) => {
                let error = MemoryError::ConnectionFailed {
                    backend: skreaver_core::error::MemoryBackend::Redis,
                    kind: skreaver_core::error::MemoryErrorKind::InternalError {
                        backend_error: format!("Failed to get connection from pool: {}", e),
                    },
                };
                let disconnected = RedisConnection {
                    data: ConnectionData::Disconnected {
                        attempt_count,
                        last_activity,
                    },
                    _state: PhantomData,
                };
                Err((disconnected, error))
            }
        }
    }

    /// Get connection attempt count for retry logic
    pub fn attempt_count(&self) -> usize {
        match &self.data {
            ConnectionData::Disconnected { attempt_count, .. } => *attempt_count,
            _ => panic!("INVARIANT VIOLATION: Disconnected state must have Disconnected data"),
        }
    }

    /// Reset attempt count
    pub fn reset_attempts(self) -> Self {
        let last_activity = match self.data {
            ConnectionData::Disconnected { last_activity, .. } => last_activity,
            _ => panic!("INVARIANT VIOLATION: Disconnected state must have Disconnected data"),
        };

        Self {
            data: ConnectionData::Disconnected {
                attempt_count: 0,
                last_activity,
            },
            _state: PhantomData,
        }
    }
}

#[cfg(feature = "redis")]
impl RedisConnection<Connected> {
    /// Get the underlying connection (guaranteed to be available via typestate)
    ///
    /// # Safety Guarantee
    ///
    /// The typestate pattern ensures that `RedisConnection<Connected>` can ONLY be
    /// constructed via the `connect()` method on `RedisConnection<Disconnected>`,
    /// which always initializes `ConnectionData::Connected`. The `Disconnected` and
    /// `Connected` state markers prevent direct construction of invalid states.
    ///
    /// The non-Connected match arm is marked `unreachable!()` rather than returning
    /// an error because:
    /// 1. The typestate pattern makes this state impossible in safe Rust
    /// 2. Only `unsafe` code (like mem::transmute) could violate this invariant
    /// 3. Returning Result would add overhead and complexity for an impossible case
    pub fn connection(&mut self) -> &mut PooledConnection {
        match &mut self.data {
            ConnectionData::Connected { connection, .. } => connection,
            // SAFETY: Typestate pattern guarantees Connected state has Connected data
            // Only unsafe code (transmute) could reach this - that's undefined behavior anyway
            _ => unreachable!("Typestate violation: Connected marker with non-Connected data"),
        }
    }

    /// Get connection duration
    ///
    /// See `connection()` for safety guarantee documentation.
    pub fn connection_duration(&self) -> Duration {
        match &self.data {
            ConnectionData::Connected { connected_at, .. } => connected_at.elapsed(),
            _ => unreachable!("Typestate violation: Connected marker with non-Connected data"),
        }
    }

    /// Get time since last activity
    ///
    /// See `connection()` for safety guarantee documentation.
    pub fn idle_duration(&self) -> Duration {
        match &self.data {
            ConnectionData::Connected { last_activity, .. } => last_activity.elapsed(),
            _ => unreachable!("Typestate violation: Connected marker with non-Connected data"),
        }
    }

    /// Get attempt count that led to this connection
    ///
    /// See `connection()` for safety guarantee documentation.
    pub fn attempt_count(&self) -> usize {
        match &self.data {
            ConnectionData::Connected { attempt_count, .. } => *attempt_count,
            _ => unreachable!("Typestate violation: Connected marker with non-Connected data"),
        }
    }

    /// Update last activity timestamp
    ///
    /// See `connection()` for safety guarantee documentation.
    fn update_activity(&mut self) {
        match &mut self.data {
            ConnectionData::Connected { last_activity, .. } => {
                *last_activity = Instant::now();
            }
            _ => unreachable!("Typestate violation: Connected marker with non-Connected data"),
        }
    }

    /// Perform a Redis command with automatic activity tracking
    pub async fn execute<T, F, Fut>(&mut self, f: F) -> Result<T, MemoryError>
    where
        F: for<'a> FnOnce(&'a mut PooledConnection) -> Fut,
        Fut: std::future::Future<Output = Result<T, redis::RedisError>>,
    {
        let conn = self.connection();
        let result = f(conn).await.map_err(|e| MemoryError::LoadFailed {
            key: MemoryKeys::redis_operation(),
            backend: skreaver_core::error::MemoryBackend::Redis,
            kind: skreaver_core::error::MemoryErrorKind::NetworkError {
                details: format!("Redis operation failed: {}", e),
            },
        })?;

        self.update_activity();
        Ok(result)
    }

    /// Ping the connection to verify it's still alive
    pub async fn ping(mut self) -> Result<Self, (RedisConnection<Disconnected>, MemoryError)> {
        let conn = self.connection();
        let result: Result<String, redis::RedisError> = redis::cmd("PING").query_async(conn).await;

        match result {
            Ok(_) => {
                self.update_activity();
                Ok(self)
            }
            Err(redis_error) => {
                let error = MemoryError::LoadFailed {
                    key: MemoryKeys::ping(),
                    backend: skreaver_core::error::MemoryBackend::Redis,
                    kind: skreaver_core::error::MemoryErrorKind::NetworkError {
                        details: format!("Ping failed: {}", redis_error),
                    },
                };
                let (attempt_count, last_activity) = match self.data {
                    ConnectionData::Connected {
                        attempt_count,
                        last_activity,
                        ..
                    } => (attempt_count, Some(last_activity)),
                    _ => panic!("INVARIANT VIOLATION: Connected state must have Connected data"),
                };
                let disconnected = RedisConnection {
                    data: ConnectionData::Disconnected {
                        attempt_count,
                        last_activity,
                    },
                    _state: PhantomData,
                };
                Err((disconnected, error))
            }
        }
    }

    /// Gracefully disconnect the connection
    pub fn disconnect(self) -> RedisConnection<Disconnected> {
        let (attempt_count, last_activity) = match self.data {
            ConnectionData::Connected {
                attempt_count,
                last_activity,
                ..
            } => (attempt_count, Some(last_activity)),
            _ => panic!("INVARIANT VIOLATION: Connected state must have Connected data"),
        };
        RedisConnection {
            data: ConnectionData::Disconnected {
                attempt_count,
                last_activity,
            },
            _state: PhantomData,
        }
    }

    /// Check if connection should be considered stale
    ///
    /// A connection is stale if:
    /// - It has been idle longer than `max_idle_duration`, OR
    /// - It has existed longer than `max_connection_age` (if specified)
    ///
    /// This prevents long-lived connections that are only occasionally used from
    /// remaining indefinitely, which can lead to resource exhaustion or connections
    /// in an inconsistent state.
    ///
    /// # Edge Case Handling (MEDIUM-25)
    ///
    /// To prevent freshly-created connections from being immediately marked stale
    /// (e.g., if someone misconfigures `max_connection_age` to 1ms), connections
    /// less than 1 second old are never considered stale.
    pub fn is_stale(
        &self,
        max_idle_duration: Duration,
        max_connection_age: Option<Duration>,
    ) -> bool {
        // MEDIUM-25: Never mark connection stale if less than 1 second old
        // This prevents edge cases where misconfigured age limits immediately
        // mark connections as stale before they can be used
        const MIN_CONNECTION_AGE: Duration = Duration::from_secs(1);
        if self.connection_duration() < MIN_CONNECTION_AGE {
            return false;
        }

        let idle_stale = self.idle_duration() > max_idle_duration;
        let age_stale = max_connection_age
            .map(|max_age| self.connection_duration() > max_age)
            .unwrap_or(false);

        idle_stale || age_stale
    }
}

// === Connection Pool with State Tracking ===

/// Connection manager that maintains type-safe connection state
#[cfg(feature = "redis")]
pub struct StatefulConnectionManager {
    pool: Pool,
    max_idle_duration: Duration,
    max_connection_age: Option<Duration>,
    max_retry_attempts: usize,
}

#[cfg(feature = "redis")]
impl StatefulConnectionManager {
    /// Create a new connection manager
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            max_idle_duration: Duration::from_secs(300), // 5 minutes
            max_connection_age: None,                    // No age limit by default
            max_retry_attempts: 3,
        }
    }

    /// Configure maximum idle duration before considering connection stale
    pub fn with_max_idle_duration(mut self, duration: Duration) -> Self {
        self.max_idle_duration = duration;
        self
    }

    /// Configure maximum connection age before considering connection stale
    ///
    /// This prevents long-lived connections from remaining indefinitely,
    /// which can lead to resource exhaustion. For example, setting this to
    /// 1 hour will ensure connections are refreshed at least hourly.
    pub fn with_max_connection_age(mut self, duration: Duration) -> Self {
        self.max_connection_age = Some(duration);
        self
    }

    /// Configure maximum retry attempts
    pub fn with_max_retry_attempts(mut self, attempts: usize) -> Self {
        self.max_retry_attempts = attempts;
        self
    }

    /// Get a connection, automatically handling retries
    pub async fn get_connection(&self) -> Result<ConnectedRedis, MemoryError> {
        let mut disconnected = DisconnectedRedis::new_disconnected();

        for _ in 0..self.max_retry_attempts {
            match disconnected.connect(&self.pool).await {
                Ok(connected) => return Ok(connected),
                Err((disc, error)) => {
                    disconnected = disc;
                    if disconnected.attempt_count() >= self.max_retry_attempts {
                        return Err(error);
                    }
                    // Brief delay before retry
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Err(MemoryError::ConnectionFailed {
            backend: skreaver_core::error::MemoryBackend::Redis,
            kind: skreaver_core::error::MemoryErrorKind::InternalError {
                backend_error: format!(
                    "Failed to connect after {} attempts",
                    self.max_retry_attempts
                ),
            },
        })
    }

    /// Validate connection health and reconnect if needed
    pub async fn ensure_connected(
        &self,
        connection: ConnectedRedis,
    ) -> Result<ConnectedRedis, MemoryError> {
        // Check if connection is stale
        if connection.is_stale(self.max_idle_duration, self.max_connection_age) {
            // Disconnect and reconnect
            let disconnected = connection.disconnect();
            return self.reconnect(disconnected).await;
        }

        // Ping to verify connection is alive
        match connection.ping().await {
            Ok(conn) => Ok(conn),
            Err((disconnected, _)) => self.reconnect(disconnected).await,
        }
    }

    /// Attempt to reconnect a disconnected connection
    async fn reconnect(
        &self,
        disconnected: DisconnectedRedis,
    ) -> Result<ConnectedRedis, MemoryError> {
        let reset_disconnected = disconnected.reset_attempts();
        self.get_connection_from_disconnected(reset_disconnected)
            .await
    }

    /// Get connection from existing disconnected state
    async fn get_connection_from_disconnected(
        &self,
        mut disconnected: DisconnectedRedis,
    ) -> Result<ConnectedRedis, MemoryError> {
        for _ in 0..self.max_retry_attempts {
            match disconnected.connect(&self.pool).await {
                Ok(connected) => return Ok(connected),
                Err((disc, error)) => {
                    disconnected = disc;
                    if disconnected.attempt_count() >= self.max_retry_attempts {
                        return Err(error);
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Err(MemoryError::ConnectionFailed {
            backend: skreaver_core::error::MemoryBackend::Redis,
            kind: skreaver_core::error::MemoryErrorKind::InternalError {
                backend_error: format!(
                    "Failed to reconnect after {} attempts",
                    self.max_retry_attempts
                ),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disconnected_connection_creation() {
        let conn = DisconnectedRedis::new_disconnected();
        assert_eq!(conn.attempt_count(), 0);
    }

    #[test]
    fn test_attempt_count_tracking() {
        let conn = DisconnectedRedis::new_disconnected();
        assert_eq!(conn.attempt_count(), 0);

        let reset_conn = conn.reset_attempts();
        assert_eq!(reset_conn.attempt_count(), 0);
    }

    #[cfg(feature = "redis")]
    #[tokio::test]
    async fn test_connection_manager_creation() {
        // This test would require a real Redis pool, so we'll keep it simple
        // In practice, this would test with a mock or test Redis instance
    }
}
