use super::{Memory, MemoryUpdate};
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
    fn load(&mut self, key: &str) -> Option<String> {
        self.conn.get::<_, Option<String>>(key).ok().flatten()
    }

    fn store(&mut self, update: MemoryUpdate) {
        let _ = self.conn.set::<_, _, ()>(update.key, update.value);
    }
}

impl RedisMemory {
    /// Store a key-value pair with TTL in seconds.
    pub fn store_with_ttl(&mut self, update: MemoryUpdate, ttl_secs: u64) {
        let _: redis::RedisResult<()> = self.conn.set_ex(&update.key, &update.value, ttl_secs);
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

    fn restore(&mut self, snapshot: &str) -> Result<(), String> {
        let data: HashMap<String, String> =
            serde_json::from_str(snapshot).map_err(|e| format!("Parse failed: {e}"))?;

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
        mem.store(MemoryUpdate {
            key: key.clone(),
            value: "bar".into(),
        });
        let value = mem.load(&key);
        assert_eq!(value, Some("bar".into()));
    }

    #[test]
    fn redis_memory_with_ttl() {
        let mut mem = RedisMemory::new("redis://127.0.0.1/").unwrap();
        clear_test_keys(&mut mem);

        let key = format!("test:{}:foo", Uuid::new_v4());
        mem.store_with_ttl(
            MemoryUpdate {
                key: key.clone(),
                value: "short-lived".into(),
            },
            2,
        );

        println!("Stored. Wait 3s...");
        std::thread::sleep(std::time::Duration::from_secs(3));
        println!("Loaded: {:?}", mem.load(&key));
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
        mem.store(MemoryUpdate {
            key: "target".into(),
            value: "Eliminate the client".into(),
        });

        // Should be retrievable using just "target"
        assert_eq!(mem.load("target"), Some("Eliminate the client".into()));

        // Confirm that the actual Redis key is prefixed
        let full_key = "agent:47:target";
        let raw_value = mem.inner().load(full_key);
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
        mem.store(MemoryUpdate {
            key: "goal".into(),
            value: "Eat cake".into(),
        });

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
        let full_key = "agent:47:goal";
        let value = redis_restored.load(full_key);
        assert_eq!(value, Some("Eat cake".into()));
    }

    fn clear_test_keys(mem: &mut RedisMemory) {
        let keys: Vec<String> = mem.conn.keys("test:*").unwrap();
        for k in keys {
            let _: () = mem.conn.del(&k).unwrap();
        }
    }
}
