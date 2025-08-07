use super::{Memory, MemoryUpdate};
use redis::{Commands, Connection, RedisResult};

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

#[test]
fn redis_memory_works() {
    let mut mem = RedisMemory::new("redis://127.0.0.1/").unwrap();
    mem.store(MemoryUpdate {
        key: "foo".into(),
        value: "bar".into(),
    });
    let value = mem.load("foo");
    assert_eq!(value, Some("bar".into()));
}

#[test]
fn redis_memory_with_ttl() {
    let mut mem = RedisMemory::new("redis://127.0.0.1/").unwrap();
    mem.store_with_ttl(
        MemoryUpdate {
            key: "temp".into(),
            value: "short-lived".into(),
        },
        2,
    );

    println!("Stored. Wait 3s...");
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("Loaded: {:?}", mem.load("temp"));
}
