use super::{Memory, MemoryKey, MemoryUpdate};
use crate::memory::SnapshotableMemory;
use redis::{Commands, Connection, RedisResult};
use std::collections::HashMap;

/// Redis-based memory backend.
pub struct RedisMemory {
    conn: Connection,
}

impl RedisMemory {
    /// Creates a new RedisMemory with the given connection string (e.g., "redis://127.0.0.1/")
    pub fn new(redis_url: &str) -> RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = client.get_connection()?;
        Ok(Self { conn })
    }
}

impl Memory for RedisMemory {
    fn load(&mut self, key: &MemoryKey) -> Result<Option<String>, crate::error::MemoryError> {
        self.conn
            .get::<_, Option<String>>(key.as_str())
            .map_err(|e| crate::error::MemoryError::LoadFailed {
                key: key.as_str().to_string(),
                reason: e.to_string(),
            })
    }

    fn store(&mut self, update: MemoryUpdate) -> Result<(), crate::error::MemoryError> {
        self.conn
            .set::<_, _, ()>(update.key.as_str(), update.value)
            .map_err(|e| crate::error::MemoryError::StoreFailed {
                key: update.key.as_str().to_string(),
                reason: e.to_string(),
            })
    }
}

impl RedisMemory {
    /// Store a key-value pair with TTL in seconds.
    pub fn store_with_ttl(
        &mut self,
        update: MemoryUpdate,
        ttl_secs: u64,
    ) -> Result<(), crate::error::MemoryError> {
        self.conn
            .set_ex(update.key.as_str(), &update.value, ttl_secs)
            .map_err(|e| crate::error::MemoryError::StoreFailed {
                key: update.key.as_str().to_string(),
                reason: e.to_string(),
            })
    }
}

impl SnapshotableMemory for RedisMemory {
    fn snapshot(&mut self) -> Option<String> {
        let keys: Vec<String> = self.conn.keys("*").ok()?;
        let mut map = HashMap::new();

        for key in keys {
            if let Ok(Some(value)) = self.conn.get::<_, Option<String>>(&key) {
                map.insert(key, value);
            }
        }

        serde_json::to_string_pretty(&map).ok()
    }

    fn restore(&mut self, snapshot: &str) -> Result<(), crate::error::MemoryError> {
        let data: HashMap<String, String> = serde_json::from_str(snapshot).map_err(|e| {
            crate::error::MemoryError::RestoreFailed {
                reason: format!("JSON parsing failed: {}", e),
            }
        })?;

        for (key, value) in data {
            let _ = self.conn.set::<_, _, ()>(key, value);
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
        mem.store(MemoryUpdate {
            key: memory_key.clone(),
            value: "bar".into(),
        })
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
        use crate::memory::NamespacedMemory;

        // Create a Redis backend
        let mut redis = RedisMemory::new("redis://127.0.0.1/").unwrap();

        // Define a namespace for Agent 47 (Hitman)
        let prefix = "agent:47";

        // Clean up any leftover keys from previous test runs
        let keys: Vec<String> = redis.conn.keys(format!("{}:*", prefix)).unwrap();
        for k in keys {
            let _: () = redis.conn.del(&k).unwrap();
        }

        // Wrap Redis with NamespacedMemory for agent:47
        let mut mem = NamespacedMemory::new(prefix, redis);

        // Store a memory entry using logical key (without prefix)
        let target_key = MemoryKey::new("target").unwrap();
        mem.store(MemoryUpdate {
            key: target_key.clone(),
            value: "Eliminate the client".into(),
        })
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
        use crate::memory::{NamespacedMemory, SnapshotableMemory};

        // Step 1: Write data to a namespaced Redis memory for Agent 47
        let mut redis = RedisMemory::new("redis://127.0.0.1/").unwrap();
        let prefix = "agent:47";

        // Clean up before test
        let keys: Vec<String> = redis.conn.keys(format!("{}:*", prefix)).unwrap();
        for k in keys {
            let _: () = redis.conn.del(&k).unwrap();
        }

        let mut mem = NamespacedMemory::new(prefix, redis);

        // Store agent's current objective
        let goal_key = MemoryKey::new("goal").unwrap();
        mem.store(MemoryUpdate {
            key: goal_key,
            value: "Eat cake".into(),
        })
        .unwrap();

        // Take a snapshot of all keys in Redis (not just this agent)
        let snapshot = mem.inner().snapshot().unwrap();
        println!("Snapshot: {}", snapshot);

        // Step 2: Restore that snapshot into a fresh Redis connection
        let mut redis_restored = RedisMemory::new("redis://127.0.0.1/").unwrap();

        // Clear before restore just in case
        let keys: Vec<String> = redis_restored.conn.keys(format!("{}:*", prefix)).unwrap();
        for k in keys {
            let _: () = redis_restored.conn.del(&k).unwrap();
        }

        redis_restored.restore(&snapshot).unwrap();

        // Confirm the restored value exists
        let full_key = MemoryKey::new("agent:47:goal").unwrap();
        let value = redis_restored.load(&full_key).unwrap();
        assert_eq!(value, Some("Eat cake".into()));
    }

    fn clear_test_keys(mem: &mut RedisMemory) {
        let keys: Vec<String> = mem.conn.keys("test:*").unwrap();
        for k in keys {
            let _: () = mem.conn.del(&k).unwrap();
        }
    }
}
