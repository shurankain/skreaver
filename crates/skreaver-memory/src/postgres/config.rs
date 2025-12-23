//! PostgreSQL connection configuration
//!
//! This module provides configuration structures and validation for PostgreSQL connections.

use skreaver_core::database::{DatabaseName, HostAddress, PoolSize};
use skreaver_core::error::MemoryError;
use std::time::Duration;
use tokio_postgres::{Config, Error as PgError};

/// PostgreSQL connection configuration
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    /// Database host
    pub host: String,
    /// Database port
    pub port: u16,
    /// Database name
    pub database: String,
    /// Username for authentication
    pub user: String,
    /// Password for authentication
    pub password: Option<String>,
    /// Connection timeout in seconds
    pub connect_timeout: u64,
    /// Connection pool size (validated 1-100)
    pub pool_size: PoolSize,
    /// Application name for connection identification
    pub application_name: String,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 5432,
            database: "skreaver".to_string(),
            user: "skreaver".to_string(),
            password: None,
            connect_timeout: 30,
            pool_size: PoolSize::default_size(),
            application_name: "skreaver-memory".to_string(),
        }
    }
}

impl PostgresConfig {
    /// Create a new config with database URL
    pub fn from_url(url: &str) -> Result<Self, MemoryError> {
        let config: Config = url
            .parse()
            .map_err(|e: PgError| MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: format!("Invalid database URL: {}", e),
                },
            })?;

        Ok(Self {
            host: config
                .get_hosts()
                .first()
                .map(|h| match h {
                    tokio_postgres::config::Host::Tcp(s) => s.clone(),
                    tokio_postgres::config::Host::Unix(path) => {
                        // LOW-3: Log lossy conversion for Unix socket paths
                        let lossy = path.to_string_lossy();
                        if matches!(lossy, std::borrow::Cow::Owned(_)) {
                            tracing::warn!(
                                path_debug = ?path,
                                "Unix socket path contains invalid UTF-8"
                            );
                        }
                        lossy.to_string()
                    }
                })
                .unwrap_or_else(|| "localhost".to_string()),
            port: config.get_ports().first().copied().unwrap_or(5432),
            database: config.get_dbname().unwrap_or("skreaver").to_string(),
            user: config.get_user().unwrap_or("skreaver").to_string(),
            password: config
                .get_password()
                .map(|s| String::from_utf8_lossy(s).to_string()),
            connect_timeout: 30,
            pool_size: PoolSize::default_size(),
            application_name: config
                .get_application_name()
                .unwrap_or("skreaver-memory")
                .to_string(),
        })
    }

    /// Validate configuration for security
    pub fn validate(&self) -> Result<(), MemoryError> {
        // Basic validation
        if self.host.is_empty() {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: "Host cannot be empty".to_string(),
                },
            });
        }

        if self.database.is_empty() {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: "Database name cannot be empty".to_string(),
                },
            });
        }

        if self.user.is_empty() {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: "Username cannot be empty".to_string(),
                },
            });
        }

        // Pool size validation is now handled by the PoolSize type itself

        // Security validations
        if self.host.contains("..") || self.host.contains("//") {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: "Invalid host: potential path traversal detected".to_string(),
                },
            });
        }

        // Validate database name contains only safe characters
        if !self
            .database
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(MemoryError::ConnectionFailed {
                backend: skreaver_core::error::MemoryBackend::Postgres,
                kind: skreaver_core::error::MemoryErrorKind::InvalidKey {
                    validation_error: "Database name contains invalid characters".to_string(),
                },
            });
        }

        Ok(())
    }

    /// Build tokio_postgres Config
    pub fn build_pg_config(&self) -> Config {
        let mut config = Config::new();
        config
            .host(&self.host)
            .port(self.port)
            .dbname(&self.database)
            .user(&self.user)
            .application_name(&self.application_name)
            .connect_timeout(Duration::from_secs(self.connect_timeout));

        if let Some(ref password) = self.password {
            config.password(password);
        }

        config
    }
}
