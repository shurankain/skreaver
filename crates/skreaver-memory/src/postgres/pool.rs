//! PostgreSQL connection pool with health monitoring
//!
//! This module provides connection pooling functionality with RAII resource management,
//! health monitoring, and connection validation.

use skreaver_core::error::MemoryError;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_postgres::{Client, Error as PgError, NoTls};

use super::config::PostgresConfig;
use super::health::PostgresPoolHealth;

/// A pooled PostgreSQL connection with RAII cleanup
pub struct PooledConnection {
    client: Option<Client>,
    pool: Arc<Mutex<Vec<Client>>>,
    pool_size: usize,
}

impl PooledConnection {
    pub(crate) fn new(client: Client, pool: Arc<Mutex<Vec<Client>>>, pool_size: usize) -> Self {
        Self {
            client: Some(client),
            pool,
            pool_size,
        }
    }

    /// Get reference to the underlying client
    pub fn client(&self) -> &Client {
        self.client.as_ref().expect("Client should be available")
    }

    /// Get mutable reference to the underlying client
    pub fn client_mut(&mut self) -> &mut Client {
        self.client.as_mut().expect("Client should be available")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let pool = Arc::clone(&self.pool);
            let pool_size = self.pool_size;

            // Return connection to pool synchronously using try_lock to avoid blocking
            if let Ok(mut pool_guard) = pool.try_lock() {
                if pool_guard.len() < pool_size {
                    pool_guard.push(client);
                }
                // If pool is full or locked, connection will be dropped
            }
            // If we can't get the lock immediately, just drop the connection
            // This prevents deadlocks in Drop implementations
        }
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
        let mut connections = Vec::with_capacity(config.pool_size);

        // Create initial pool of connections
        for _ in 0..config.pool_size {
            let (client, connection) =
                pg_config
                    .connect(NoTls)
                    .await
                    .map_err(|e| MemoryError::ConnectionFailed {
                        backend: "postgres".to_string(),
                        reason: Self::sanitize_error(&e),
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
        // Map specific PostgreSQL errors to safe messages
        if error.as_db_error().is_some() {
            "Database operation failed".to_string()
        } else if error.to_string().contains("connection") {
            "Connection failed".to_string()
        } else if error.to_string().contains("authentication") {
            "Authentication failed".to_string()
        } else if error.to_string().contains("timeout") {
            "Operation timed out".to_string()
        } else {
            "Database error occurred".to_string()
        }
    }

    /// Validate connection health
    async fn validate_connection(client: &Client) -> Result<(), MemoryError> {
        // Simple health check query
        client
            .query_one("SELECT 1", &[])
            .await
            .map_err(|e| MemoryError::ConnectionFailed {
                backend: "postgres".to_string(),
                reason: Self::sanitize_error(&e),
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
                backend: "postgres".to_string(),
                reason: format!(
                    "Connection pool exhausted: {} active connections (max: {})",
                    current_active, self.config.pool_size
                ),
            });
        }

        // Create new connection
        let pg_config = self.config.build_pg_config();
        let (client, connection) =
            pg_config
                .connect(NoTls)
                .await
                .map_err(|e| MemoryError::ConnectionFailed {
                    backend: "postgres".to_string(),
                    reason: Self::sanitize_error(&e),
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
            conn.client()
                .query_one("SELECT version()", &[])
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
