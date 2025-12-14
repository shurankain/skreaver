//! Redis transactional operations with MULTI/EXEC support
//!
//! This module provides transactional wrapper functionality for Redis operations
//! with proper atomic commits using Redis MULTI/EXEC commands.

#[cfg(feature = "redis")]
use redis::RedisError;

use skreaver_core::error::{MemoryError, TransactionError};
use skreaver_core::memory::{MemoryKey, MemoryUpdate, MemoryWriter};

use super::config::ValidRedisConfig;

/// Transactional memory wrapper for Redis operations
#[cfg(feature = "redis")]
pub struct RedisTransactionalMemory {
    operations: Vec<TransactionOperation>,
    config: ValidRedisConfig,
}

/// Types of operations that can be performed in a Redis transaction
#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum TransactionOperation {
    Set { key: String, value: String },
    Del { key: String },
}

#[cfg(feature = "redis")]
impl RedisTransactionalMemory {
    /// Create a new transactional memory wrapper
    pub fn new(config: ValidRedisConfig) -> Self {
        Self {
            operations: Vec::new(),
            config,
        }
    }

    /// Apply key prefix if configured
    fn prefixed_key(&self, key: &MemoryKey) -> String {
        match &self.config.key_prefix {
            Some(prefix) => format!("{}:{}", prefix, key.as_str()),
            None => key.as_str().to_string(),
        }
    }

    /// Commit the transaction
    pub async fn commit<T>(&mut self, memory: &T) -> Result<(), RedisError>
    where
        T: RedisConnectionProvider,
    {
        if self.operations.is_empty() {
            return Ok(());
        }

        let mut conn = memory.get_connection().await.map_err(|e| {
            RedisError::from((
                redis::ErrorKind::IoError,
                "Connection failed",
                format!("{}", e),
            ))
        })?;

        // Start transaction
        let _: () = redis::cmd("MULTI").query_async(&mut *conn).await?;

        // Execute all operations
        for operation in &self.operations {
            match operation {
                TransactionOperation::Set { key, value } => {
                    let _: () = redis::cmd("SET")
                        .arg(key)
                        .arg(value)
                        .query_async(&mut *conn)
                        .await?;
                }
                TransactionOperation::Del { key } => {
                    let _: () = redis::cmd("DEL").arg(key).query_async(&mut *conn).await?;
                }
            }
        }

        // Execute transaction
        let results: Vec<redis::Value> = redis::cmd("EXEC").query_async(&mut *conn).await?;

        // Check if transaction was aborted
        if results.is_empty() {
            return Err(RedisError::from((
                redis::ErrorKind::ExecAbortError,
                "Transaction aborted",
            )));
        }

        Ok(())
    }
}

#[cfg(feature = "redis")]
impl MemoryWriter for RedisTransactionalMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let prefixed_key = self.prefixed_key(&update.key);
        self.operations.push(TransactionOperation::Set {
            key: prefixed_key,
            value: update.value,
        });
        Ok(())
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        for update in updates {
            self.store(update)?;
        }
        Ok(())
    }
}

/// Trait for types that can provide Redis connections (needed for transaction commit)
#[cfg(feature = "redis")]
pub trait RedisConnectionProvider {
    fn get_connection(
        &self,
    ) -> impl std::future::Future<Output = Result<deadpool_redis::Connection, MemoryError>> + Send;
}

/// Transaction execution utility for Redis memory
#[cfg(feature = "redis")]
pub struct RedisTransactionExecutor;

#[cfg(feature = "redis")]
impl RedisTransactionExecutor {
    /// Execute a Redis transaction with proper runtime handling
    pub fn execute_transaction<T, F, R>(
        memory: &mut T,
        runtime_cell: &std::cell::RefCell<crate::redis::runtime::RuntimeState>,
        f: F,
    ) -> Result<R, TransactionError>
    where
        T: RedisConnectionProvider + ConfigProvider,
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        use crate::redis::runtime::RuntimeState;

        // Use a simplified synchronous approach similar to PostgreSQL backend
        let rt = tokio::runtime::Handle::try_current()
            .or_else(|_| {
                let mut rt_ref = runtime_cell.borrow_mut();
                if matches!(&*rt_ref, RuntimeState::Uninitialized) {
                    *rt_ref = RuntimeState::Ready(tokio::runtime::Runtime::new().map_err(|e| {
                        TransactionError::TransactionFailed {
                            reason: format!("Failed to create async runtime: {}", e),
                        }
                    })?);
                }
                match &*rt_ref {
                    RuntimeState::Ready(runtime) => Ok(runtime.handle().clone()),
                    RuntimeState::Uninitialized => Err(TransactionError::TransactionFailed {
                        reason: "Runtime initialization failed".to_string(),
                    }),
                }
            })
            .map_err(|e: TransactionError| e)?;

        rt.block_on(async {
            // Create transactional memory wrapper
            let mut tx_memory = RedisTransactionalMemory {
                operations: Vec::new(),
                config: memory.get_config().clone(),
            };

            let result = f(&mut tx_memory);

            match result {
                Ok(value) => {
                    // Execute Redis transaction
                    tx_memory.commit(memory).await.map_err(|e| {
                        TransactionError::TransactionFailed {
                            reason: format!(
                                "Failed to commit Redis transaction: {}",
                                crate::redis::pool::RedisPoolUtils::sanitize_error(&e)
                            ),
                        }
                    })?;
                    Ok(value)
                }
                Err(tx_error) => Err(tx_error),
            }
        })
    }
}

/// Trait for types that can provide configuration (needed for transaction setup)
#[cfg(feature = "redis")]
pub trait ConfigProvider {
    fn get_config(&self) -> &ValidRedisConfig;
}
