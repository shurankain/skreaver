#[cfg(feature = "redis")]
use redis::{Client, Commands, Connection, RedisResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use skreaver_core::error::{MemoryError, TransactionError};
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};

/// Redis-based memory backend with connection sharing for concurrent access.
#[cfg(feature = "redis")]
#[derive(Clone)]
pub struct RedisMemory {
    client: Arc<Client>,
    conn: Arc<Mutex<Connection>>,
}

#[cfg(feature = "redis")]
impl RedisMemory {
    /// Creates a new RedisMemory with the given connection string (e.g., "redis://127.0.0.1/")
    pub fn new(redis_url: &str) -> RedisResult<Self> {
        let client = Arc::new(redis::Client::open(redis_url)?);
        let conn = Arc::new(Mutex::new(client.get_connection()?));
        Ok(Self { client, conn })
    }

    /// Get a new connection from the client (for batch operations or transactions)
    fn get_connection(&self) -> RedisResult<Connection> {
        self.client.get_connection()
    }
}

#[cfg(feature = "redis")]
impl MemoryReader for RedisMemory {
    fn load(&self, key: &MemoryKey) -> Result<Option<String>, MemoryError> {
        let mut conn = self.conn.lock().map_err(|e| MemoryError::LoadFailed {
            key: key.clone(),
            reason: format!("Lock poisoned: {}", e),
        })?;

        conn.get::<_, Option<String>>(key.as_str())
            .map_err(|e| MemoryError::LoadFailed {
                key: key.clone(),
                reason: e.to_string(),
            })
    }

    fn load_many(&self, keys: &[MemoryKey]) -> Result<Vec<Option<String>>, MemoryError> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let batch_key = MemoryKey::new("batch").unwrap();
        let mut conn = self.conn.lock().map_err(|e| MemoryError::LoadFailed {
            key: batch_key.clone(),
            reason: format!("Lock poisoned: {}", e),
        })?;

        let key_strs: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
        let values: Vec<Option<String>> =
            conn.get(key_strs).map_err(|e| MemoryError::LoadFailed {
                key: batch_key,
                reason: e.to_string(),
            })?;

        Ok(values)
    }
}

#[cfg(feature = "redis")]
impl MemoryWriter for RedisMemory {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        let mut conn = self.conn.lock().map_err(|e| MemoryError::StoreFailed {
            key: update.key.clone(),
            reason: format!("Lock poisoned: {}", e),
        })?;

        conn.set::<_, _, ()>(update.key.as_str(), update.value)
            .map_err(|e| MemoryError::StoreFailed {
                key: update.key.clone(),
                reason: e.to_string(),
            })
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        if updates.is_empty() {
            return Ok(());
        }

        let batch_key = MemoryKey::new("batch").unwrap();
        let mut conn = self.conn.lock().map_err(|e| MemoryError::StoreFailed {
            key: batch_key,
            reason: format!("Lock poisoned: {}", e),
        })?;

        // Use Redis pipeline for efficient batch writes
        let mut pipe = redis::pipe();
        for update in updates {
            pipe.set(update.key.as_str(), update.value);
        }

        pipe.execute(&mut *conn);
        Ok(())
    }
}

#[cfg(feature = "redis")]
impl TransactionalMemory for RedisMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        // Get a new connection for the transaction
        let mut tx_conn =
            self.get_connection()
                .map_err(|e| TransactionError::TransactionFailed {
                    reason: format!("Failed to get Redis connection: {}", e),
                })?;

        // Start Redis transaction
        let _: () = redis::cmd("MULTI").query(&mut tx_conn).map_err(|e| {
            TransactionError::TransactionFailed {
                reason: format!("Failed to start Redis transaction: {}", e),
            }
        })?;

        // Create a temporary in-memory storage for transaction operations
        let mut tx_memory = crate::InMemoryMemory::new();

        // Execute the transaction function
        let result = f(&mut tx_memory);

        match result {
            Ok(value) => {
                // If successful, execute the Redis transaction
                let _: Vec<redis::Value> = redis::cmd("EXEC").query(&mut tx_conn).map_err(|e| {
                    TransactionError::TransactionFailed {
                        reason: format!("Failed to execute Redis transaction: {}", e),
                    }
                })?;
                Ok(value)
            }
            Err(err) => {
                // If failed, discard the Redis transaction
                let _: () = redis::cmd("DISCARD").query(&mut tx_conn).map_err(|_| {
                    TransactionError::TransactionFailed {
                        reason: "Failed to discard Redis transaction".to_string(),
                    }
                })?;
                Err(err)
            }
        }
    }
}

#[cfg(feature = "redis")]
impl SnapshotableMemory for RedisMemory {
    fn snapshot(&mut self) -> Option<String> {
        let mut conn = self.conn.lock().ok()?;

        // Get all keys using SCAN to handle large datasets
        let keys: Vec<String> = redis::cmd("KEYS").arg("*").query(&mut *conn).ok()?;

        if keys.is_empty() {
            return Some("{}".to_string());
        }

        // Get all values for the keys
        let values: Vec<Option<String>> = conn.get(&keys).ok()?;

        // Build a HashMap for JSON serialization
        let mut snapshot_data = HashMap::new();
        for (key, value) in keys.into_iter().zip(values) {
            if let Some(val) = value {
                snapshot_data.insert(key, val);
            }
        }

        serde_json::to_string(&snapshot_data).ok()
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        // Parse the JSON snapshot
        let snapshot_data: HashMap<String, String> =
            serde_json::from_str(snapshot).map_err(|e| MemoryError::RestoreFailed {
                reason: format!("JSON parsing failed: {}", e),
            })?;

        let mut conn = self.conn.lock().map_err(|e| MemoryError::RestoreFailed {
            reason: format!("Lock poisoned: {}", e),
        })?;

        // Clear existing data
        let _: () =
            redis::cmd("FLUSHDB")
                .query(&mut *conn)
                .map_err(|e| MemoryError::RestoreFailed {
                    reason: format!("Failed to clear Redis database: {}", e),
                })?;

        // Restore data using pipeline for efficiency
        let mut pipe = redis::pipe();
        for (key, value) in snapshot_data {
            pipe.set(&key, &value);
        }

        pipe.execute(&mut *conn);

        Ok(())
    }
}
