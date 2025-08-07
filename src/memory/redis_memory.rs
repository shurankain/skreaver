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
