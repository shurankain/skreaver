//! PostgreSQL connection pool with health monitoring
//!
//! This module provides connection pooling functionality with RAII resource management,
//! health monitoring, and connection validation.

use skreaver_core::error::MemoryError;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_postgres::{Client, Error as PgError, NoTls};

use super::config::PostgresConfig;
use super::health::PostgresPoolHealth;

/// A pooled PostgreSQL connection with RAII cleanup
///
/// This type uses ManuallyDrop to ensure the client is always valid.
/// The client is only taken during Drop to return it to the pool.
pub struct PooledConnection {
    client: std::mem::ManuallyDrop<Client>,
    pool: Arc<Mutex<Vec<Client>>>,
    pool_size: usize,
}

impl PooledConnection {
    pub(crate) fn new(client: Client, pool: Arc<Mutex<Vec<Client>>>, pool_size: usize) -> Self {
        Self {
            client: std::mem::ManuallyDrop::new(client),
            pool,
            pool_size,
        }
    }
}

// Implement Deref to transparently access the Client
// This is safe because ManuallyDrop guarantees the client is always valid
// until Drop explicitly takes ownership
impl Deref for PooledConnection {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        // SAFETY (HIGH-1): ManuallyDrop::take is safe here under the following invariants:
        //
        // 1. **Single Ownership**: Drop is guaranteed by Rust to be called exactly once per value,
        //    so we will never attempt to take() the same ManuallyDrop twice.
        //
        // 2. **No Prior Take**: The ManuallyDrop was initialized in new() and has never been
        //    taken before this point. The only way to access the inner Client is through
        //    Deref/DerefMut, which borrows but doesn't take ownership.
        //
        // 3. **Exclusive Access**: &mut self ensures we have exclusive access to self.client,
        //    preventing concurrent access during the take operation.
        //
        // 4. **Valid State**: The Client inside ManuallyDrop is always in a valid state
        //    (initialized in new(), never moved out except here).
        //
        // Violating these invariants would require unsafe code elsewhere in this module
        // or transmuting between incompatible types, both of which we don't do.
        let client = unsafe { std::mem::ManuallyDrop::take(&mut self.client) };

        let pool = Arc::clone(&self.pool);
        let pool_size = self.pool_size;

        // Return connection to pool synchronously using try_lock to avoid blocking
        if let Ok(mut pool_guard) = pool.try_lock()
            && pool_guard.len() < pool_size
        {
            pool_guard.push(client);
        }
        // If pool is full or locked, connection will be dropped here
        // This prevents deadlocks in Drop implementations
    }
}

/// PostgreSQL connection pool with health monitoring
pub struct PostgresPool {
    config: PostgresConfig,
    connections: Arc<Mutex<Vec<Client>>>,
    active_count: Arc<RwLock<usize>>,
}

impl PostgresPool {
    /// Create a new PostgreSQL connection pool
    pub async fn new(config: PostgresConfig) -> Result<Self, MemoryError> {
        config.validate()?;

        let pg_config = config.build_pg_config();
        let pool_size = config.pool_size.get();
        let mut connections = Vec::with_capacity(pool_size);

        // Create initial pool of connections
        for _ in 0..pool_size {
            let (client, connection) =
                pg_config
                    .connect(NoTls)
                    .await
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: skreaver_core::error::MemoryBackend::Postgres,
                        kind: skreaver_core::error::MemoryErrorKind::IoError {
                            details: Self::sanitize_error(&e),
                        },
                    })?;

            // Spawn connection task
            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("PostgreSQL connection error: {}", e);
                }
            });

            // Validate connection health
            Self::validate_connection(&client).await?;
            connections.push(client);
        }

        Ok(Self {
            config,
            connections: Arc::new(Mutex::new(connections)),
            active_count: Arc::new(RwLock::new(0)),
        })
    }

    /// Sanitize PostgreSQL errors for security
    fn sanitize_error(error: &PgError) -> String {
        use skreaver_core::sanitization::DatabaseErrorSanitizer;
        DatabaseErrorSanitizer::sanitize(error)
    }

    /// Validate connection health
    async fn validate_connection(client: &Client) -> Result<(), MemoryError> {
        // Simple health check query
        client
            .query_one("SELECT 1", &[])
            .await
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::IoError {
                    details: Self::sanitize_error(&e),
                },
            })?;

        Ok(())
    }

    /// Acquire a connection from the pool
    pub async fn acquire(&self) -> Result<PooledConnection, MemoryError> {
        // Try to get available connection - use a separate scope to release locks quickly
        let existing_client = {
            let mut pool = self.connections.lock().await;
            pool.pop()
        };

        if let Some(client) = existing_client {
            // Validate connection before returning
            if Self::validate_connection(&client).await.is_ok() {
                // Update active count atomically
                {
                    let mut active = self.active_count.write().await;
                    *active += 1;
                }

                return Ok(PooledConnection::new(
                    client,
                    Arc::clone(&self.connections),
                    self.config.pool_size,
                ));
            }
            // Connection is bad, drop it and create a new one below
        }

        // Check if we can create new connection - atomic read
        let current_active = *self.active_count.read().await;
        if current_active >= self.config.pool_size {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::ResourceExhausted {
                    resource: "connection_pool".to_string(),
                    limit: format!(
                        "{} active connections (max: {})",
                        current_active, self.config.pool_size
                    ),
                },
            });
        }

        // Create new connection
        let pg_config = self.config.build_pg_config();
        let (client, connection) =
            pg_config
                .connect(NoTls)
                .await
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: skreaver_core::error::MemoryBackend::Postgres,
                    kind: skreaver_core::error::MemoryErrorKind::IoError {
                        details: Self::sanitize_error(&e),
                    },
                })?;

        // Spawn connection task
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {}", e);
            }
        });

        // Validate new connection
        Self::validate_connection(&client).await?;

        // Update active count atomically
        {
            let mut active = self.active_count.write().await;
            *active += 1;
        }

        Ok(PooledConnection::new(
            client,
            Arc::clone(&self.connections),
            self.config.pool_size,
        ))
    }

    /// Check pool health
    pub async fn health_check(&self) -> Result<PostgresPoolHealth, MemoryError> {
        let available_connections = {
            let pool = self.connections.lock().await;
            pool.len()
        };

        let active_connections = *self.active_count.read().await;

        // Get server version for health info
        let server_version = if let Ok(conn) = self.acquire().await {
            conn.query_one("SELECT version()", &[])
                .await
                .map(|row| row.get::<_, String>(0))
                .unwrap_or_else(|_| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        };

        Ok(PostgresPoolHealth {
            available_connections,
            total_connections: self.config.pool_size,
            active_connections,
            server_version,
            last_check: std::time::Instant::now(),
        })
    }
}
