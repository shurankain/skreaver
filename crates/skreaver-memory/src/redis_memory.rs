use super::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};
use redis::{Client, Commands, Connection, RedisResult};
use skreaver_core::error::{MemoryError, TransactionError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Redis-based memory backend with connection sharing for concurrent access.
#[derive(Clone)]
pub struct RedisMemory {
    client: Arc<Client>,
    conn: Arc<Mutex<Connection>>,
}

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

// Implement new trait hierarchy
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

        let mut conn = self.conn.lock().map_err(|e| MemoryError::LoadFailed {
            key: MemoryKey::new("batch").unwrap(),
            reason: format!("Lock poisoned: {}", e),
        })?;

        let key_strs: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
        let values: Vec<Option<String>> =
            conn.get(key_strs).map_err(|e| MemoryError::LoadFailed {
                key: MemoryKey::new("batch").unwrap(),
                reason: e.to_string(),
            })?;

        Ok(values)
    }
}

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

        let mut conn = self.conn.lock().map_err(|e| MemoryError::StoreFailed {
            key: MemoryKey::new("batch").unwrap(),
            reason: format!("Lock poisoned: {}", e),
        })?;

        // Use Redis pipeline for efficient batch writes
        let mut pipe = redis::pipe();
        for update in updates {
            pipe.set(update.key.as_str(), update.value);
        }

        pipe.execute(&mut *conn).unwrap();
        Ok(())
    }
}

impl TransactionalMemory for RedisMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        // Redis transactions are implemented using MULTI/EXEC
        // For this implementation, we'll use a simpler approach with a new connection
        let mut tx_conn =
            self.get_connection()
                .map_err(|e| TransactionError::TransactionFailed {
                    reason: format!("Failed to get transaction connection: {}", e),
                })?;

        // Create a transaction memory wrapper
        let tx_memory = RedisTransactionMemory { conn: &mut tx_conn };
        let tx_writer: &mut dyn MemoryWriter = &mut RedisTransactionWriter { memory: tx_memory };

        // Execute the transaction function
        f(tx_writer)
    }
}

// Helper struct for Redis transactions
struct RedisTransactionMemory<'a> {
    conn: &'a mut Connection,
}

struct RedisTransactionWriter<'a> {
    memory: RedisTransactionMemory<'a>,
}

impl<'a> MemoryWriter for RedisTransactionWriter<'a> {
    fn store(&mut self, update: MemoryUpdate) -> Result<(), MemoryError> {
        self.memory
            .conn
            .set::<_, _, ()>(update.key.as_str(), update.value)
            .map_err(|e| MemoryError::StoreFailed {
                key: update.key.clone(),
                reason: e.to_string(),
            })
    }

    fn store_many(&mut self, updates: Vec<MemoryUpdate>) -> Result<(), MemoryError> {
        let mut pipe = redis::pipe();
        for update in updates {
            pipe.set(update.key.as_str(), update.value);
        }
        pipe.execute(self.memory.conn).unwrap();
        Ok(())
    }
}

impl RedisMemory {
    /// Store a key-value pair with TTL in seconds.
    pub fn store_with_ttl(
        &mut self,
        update: MemoryUpdate,
        ttl_secs: u64,
    ) -> Result<(), MemoryError> {
        let mut conn = self.conn.lock().map_err(|e| MemoryError::StoreFailed {
            key: update.key.clone(),
            reason: format!("Lock poisoned: {}", e),
        })?;

        conn.set_ex(update.key.as_str(), &update.value, ttl_secs)
            .map_err(|e| MemoryError::StoreFailed {
                key: update.key.clone(),
                reason: e.to_string(),
            })
    }
}

impl SnapshotableMemory for RedisMemory {
    fn snapshot(&mut self) -> Option<String> {
        let mut conn = self.conn.lock().ok()?;
        let keys: Vec<String> = conn.keys("*").ok()?;
        let mut map = HashMap::new();

        for key in keys {
            if let Ok(Some(value)) = conn.get::<_, Option<String>>(&key) {
                map.insert(key, value);
            }
        }

        serde_json::to_string_pretty(&map).ok()
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), MemoryError> {
        let data: HashMap<String, String> =
            serde_json::from_str(snapshot).map_err(|e| MemoryError::RestoreFailed {
                reason: format!("JSON parsing failed: {}", e),
            })?;

        let mut conn = self.conn.lock().map_err(|e| MemoryError::RestoreFailed {
            reason: format!("Lock poisoned: {}", e),
        })?;

        for (key, value) in data {
            let _ = conn.set::<_, _, ()>(key, value);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn redis_memory_works() {
        let mut mem = RedisMemory::new("redis://127.0.0.1/").unwrap();
        clear_test_keys(&mut mem);

        let key = format!("test:{}:foo", Uuid::new_v4());
        let memory_key = MemoryKey::new(&key).unwrap();
        MemoryWriter::store(
            &mut mem,
            MemoryUpdate {
                key: memory_key.clone(),
                value: "bar".into(),
            },
        )
        .unwrap();
        let value = mem.load(&memory_key).unwrap();
        assert_eq!(value, Some("bar".into()));
    }

    #[test]
    fn redis_memory_with_ttl() {
        let mut mem = RedisMemory::new("redis://127.0.0.1/").unwrap();
        clear_test_keys(&mut mem);

        let key = format!("test:{}:foo", Uuid::new_v4());
        let memory_key = MemoryKey::new(&key).unwrap();
        mem.store_with_ttl(
            MemoryUpdate {
                key: memory_key.clone(),
                value: "short-lived".into(),
            },
            2,
        )
        .unwrap();

        println!("Stored. Wait 3s...");
        std::thread::sleep(std::time::Duration::from_secs(3));
        println!("Loaded: {:?}", mem.load(&memory_key));
    }

    #[test]
    fn redis_memory_with_namespace() {
        use crate::NamespacedMemory;

        // Create a Redis backend
        let redis = RedisMemory::new("redis://127.0.0.1/").unwrap();

        // Define a namespace for Agent 47 (Hitman)
        let prefix = "agent:47";

        // Clean up any leftover keys from previous test runs
        {
            let mut conn = redis.conn.lock().unwrap();
            let keys: Vec<String> = conn.keys(format!("{}:*", prefix)).unwrap();
            for k in keys {
                let _: () = conn.del(&k).unwrap();
            }
        }

        // Wrap Redis with NamespacedMemory for agent:47
        let mut mem = NamespacedMemory::new(prefix, redis);

        // Store a memory entry using logical key (without prefix)
        let target_key = MemoryKey::new("target").unwrap();
        MemoryWriter::store(
            &mut mem,
            MemoryUpdate {
                key: target_key.clone(),
                value: "Eliminate the client".into(),
            },
        )
        .unwrap();

        // Should be retrievable using just "target"
        assert_eq!(
            mem.load(&target_key).unwrap(),
            Some("Eliminate the client".into())
        );

        // Confirm that the actual Redis key is prefixed
        let full_key = MemoryKey::new("agent:47:target").unwrap();
        let raw_value = mem.inner().load(&full_key).unwrap();
        assert_eq!(raw_value, Some("Eliminate the client".into()));
    }

    #[test]
    fn redis_memory_snapshot_and_restore() {
        use crate::{NamespacedMemory, SnapshotableMemory};

        // Step 1: Write data to a namespaced Redis memory for Agent 47
        let redis = RedisMemory::new("redis://127.0.0.1/").unwrap();
        let prefix = "agent:47";

        // Clean up before test
        {
            let mut conn = redis.conn.lock().unwrap();
            let keys: Vec<String> = conn.keys(format!("{}:*", prefix)).unwrap();
            for k in keys {
                let _: () = conn.del(&k).unwrap();
            }
        }

        let mut mem = NamespacedMemory::new(prefix, redis);

        // Store agent's current objective
        let goal_key = MemoryKey::new("goal").unwrap();
        MemoryWriter::store(
            &mut mem,
            MemoryUpdate {
                key: goal_key,
                value: "Eat cake".into(),
            },
        )
        .unwrap();

        // Take a snapshot of all keys in Redis (not just this agent)
        let snapshot = mem.inner().snapshot().unwrap();
        println!("Snapshot: {}", snapshot);

        // Step 2: Restore that snapshot into a fresh Redis connection
        let mut redis_restored = RedisMemory::new("redis://127.0.0.1/").unwrap();

        // Clear before restore just in case
        {
            let mut conn = redis_restored.conn.lock().unwrap();
            let keys: Vec<String> = conn.keys(format!("{}:*", prefix)).unwrap();
            for k in keys {
                let _: () = conn.del(&k).unwrap();
            }
        }

        redis_restored.restore(&snapshot).unwrap();

        // Confirm the restored value exists
        let full_key = MemoryKey::new("agent:47:goal").unwrap();
        let value = redis_restored.load(&full_key).unwrap();
        assert_eq!(value, Some("Eat cake".into()));
    }

    fn clear_test_keys(mem: &mut RedisMemory) {
        let mut conn = mem.conn.lock().unwrap();
        let keys: Vec<String> = conn.keys("test:*").unwrap();
        for k in keys {
            let _: () = conn.del(&k).unwrap();
        }
    }
}
