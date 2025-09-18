//! Redis connection configuration
//!
//! This module provides configuration structures and validation for Redis connections,
//! supporting standalone, cluster, and sentinel deployments.

use skreaver_core::error::MemoryError;
use std::time::Duration;

/// Redis deployment configuration types
#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
pub enum RedisDeployment {
    /// Single Redis instance
    Standalone { url: String },
    /// Redis Cluster deployment
    Cluster { nodes: Vec<String> },
    /// Redis Sentinel for high availability
    Sentinel {
        sentinels: Vec<String>,
        service_name: String,
    },
}

/// Enhanced Redis configuration with enterprise features
#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis deployment configuration
    pub deployment: RedisDeployment,
    /// Connection pool size
    pub pool_size: usize,
    /// Connection timeout in seconds
    pub connect_timeout: Duration,
    /// Command timeout in seconds
    pub command_timeout: Duration,
    /// Health check interval
    pub health_check_interval: Duration,
    /// Maximum retries for failed operations
    pub max_retries: usize,
    /// Username for AUTH (Redis 6+)
    pub username: Option<String>,
    /// Password for AUTH
    pub password: Option<String>,
    /// Enable TLS
    pub tls: bool,
    /// Database number (0-15)
    pub database: u8,
    /// Key prefix for namespace isolation
    pub key_prefix: Option<String>,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            deployment: RedisDeployment::Standalone {
                url: "redis://localhost:6379".to_string(),
            },
            pool_size: 10,
            connect_timeout: Duration::from_secs(5),
            command_timeout: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
            max_retries: 3,
            username: None,
            password: None,
            tls: false,
            database: 0,
            key_prefix: None,
        }
    }
}

#[cfg(feature = "redis")]
impl RedisConfig {
    /// Create config for standalone Redis
    pub fn standalone(url: &str) -> Self {
        Self {
            deployment: RedisDeployment::Standalone {
                url: url.to_string(),
            },
            ..Default::default()
        }
    }

    /// Create config for Redis Cluster
    pub fn cluster(nodes: Vec<String>) -> Self {
        Self {
            deployment: RedisDeployment::Cluster { nodes },
            ..Default::default()
        }
    }

    /// Create config for Redis Sentinel
    pub fn sentinel(sentinels: Vec<String>, service_name: String) -> Self {
        Self {
            deployment: RedisDeployment::Sentinel {
                sentinels,
                service_name,
            },
            ..Default::default()
        }
    }

    /// Set authentication credentials
    pub fn with_auth(mut self, username: Option<String>, password: String) -> Self {
        self.username = username;
        self.password = Some(password);
        self
    }

    /// Enable TLS encryption
    pub fn with_tls(mut self) -> Self {
        self.tls = true;
        self
    }

    /// Set connection pool size
    pub fn with_pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Set key prefix for namespace isolation
    pub fn with_key_prefix(mut self, prefix: String) -> Self {
        self.key_prefix = Some(prefix);
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), MemoryError> {
        if self.pool_size == 0 {
            return Err(MemoryError::ConnectionFailed {
                backend: "redis".to_string(),
                reason: "Pool size cannot be zero".to_string(),
            });
        }

        if self.pool_size > 100 {
            return Err(MemoryError::ConnectionFailed {
                backend: "redis".to_string(),
                reason: "Pool size too large (max 100)".to_string(),
            });
        }

        match &self.deployment {
            RedisDeployment::Standalone { url } => {
                if url.is_empty() {
                    return Err(MemoryError::ConnectionFailed {
                        backend: "redis".to_string(),
                        reason: "Redis URL cannot be empty".to_string(),
                    });
                }
            }
            RedisDeployment::Cluster { nodes } => {
                if nodes.is_empty() {
                    return Err(MemoryError::ConnectionFailed {
                        backend: "redis".to_string(),
                        reason: "Redis cluster nodes cannot be empty".to_string(),
                    });
                }
            }
            RedisDeployment::Sentinel {
                sentinels,
                service_name,
            } => {
                if sentinels.is_empty() {
                    return Err(MemoryError::ConnectionFailed {
                        backend: "redis".to_string(),
                        reason: "Redis sentinel nodes cannot be empty".to_string(),
                    });
                }
                if service_name.is_empty() {
                    return Err(MemoryError::ConnectionFailed {
                        backend: "redis".to_string(),
                        reason: "Redis sentinel service name cannot be empty".to_string(),
                    });
                }
            }
        }

        Ok(())
    }
}
