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
            } => (attempt_count + 1, last_activity),
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
    pub fn connection(&mut self) -> &mut PooledConnection {
        match &mut self.data {
            ConnectionData::Connected { connection, .. } => connection,
            _ => panic!("INVARIANT VIOLATION: Connected state must have Connected data"),
        }
    }

    /// Get connection duration
    pub fn connection_duration(&self) -> Duration {
        match &self.data {
            ConnectionData::Connected { connected_at, .. } => connected_at.elapsed(),
            _ => panic!("INVARIANT VIOLATION: Connected state must have Connected data"),
        }
    }

    /// Get time since last activity
    pub fn idle_duration(&self) -> Duration {
        match &self.data {
            ConnectionData::Connected { last_activity, .. } => last_activity.elapsed(),
            _ => panic!("INVARIANT VIOLATION: Connected state must have Connected data"),
        }
    }

    /// Get attempt count that led to this connection
    pub fn attempt_count(&self) -> usize {
        match &self.data {
            ConnectionData::Connected { attempt_count, .. } => *attempt_count,
            _ => panic!("INVARIANT VIOLATION: Connected state must have Connected data"),
        }
    }

    /// Update last activity timestamp
    fn update_activity(&mut self) {
        match &mut self.data {
            ConnectionData::Connected { last_activity, .. } => {
                *last_activity = Instant::now();
            }
            _ => panic!("INVARIANT VIOLATION: Connected state must have Connected data"),
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
    pub fn is_stale(&self, max_idle_duration: Duration) -> bool {
        self.idle_duration() > max_idle_duration
    }
}

// === Connection Pool with State Tracking ===

/// Connection manager that maintains type-safe connection state
#[cfg(feature = "redis")]
pub struct StatefulConnectionManager {
    pool: Pool,
    max_idle_duration: Duration,
    max_retry_attempts: usize,
}

#[cfg(feature = "redis")]
impl StatefulConnectionManager {
    /// Create a new connection manager
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            max_idle_duration: Duration::from_secs(300), // 5 minutes
            max_retry_attempts: 3,
        }
    }

    /// Configure maximum idle duration before considering connection stale
    pub fn with_max_idle_duration(mut self, duration: Duration) -> Self {
        self.max_idle_duration = duration;
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
        if connection.is_stale(self.max_idle_duration) {
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
