#[derive(Debug, Clone)]
pub struct MemoryUpdate {
    pub key: String,
    pub value: String,
}

/// Basic trait for agent memory
pub trait Memory {
    fn load(&mut self, key: &str) -> Option<String>;
    fn store(&mut self, update: MemoryUpdate);
}

/// Optional extension for memory types that support snapshot/restore
pub trait SnapshotableMemory: Memory {
    fn snapshot(&mut self) -> Option<String>;
    fn restore(&mut self, snapshot: &str) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct DummyMemory {
        store: HashMap<String, String>,
    }

    impl Memory for DummyMemory {
        fn load(&mut self, key: &str) -> Option<String> {
            self.store.get(key).cloned()
        }

        fn store(&mut self, update: MemoryUpdate) {
            self.store.insert(update.key, update.value);
        }
    }

    impl SnapshotableMemory for DummyMemory {
        fn snapshot(&mut self) -> Option<String> {
            serde_json::to_string_pretty(&self.store).ok()
        }

        fn restore(&mut self, snapshot: &str) -> Result<(), String> {
            match serde_json::from_str::<HashMap<String, String>>(snapshot) {
                Ok(data) => {
                    self.store = data;
                    Ok(())
                }
                Err(err) => Err(format!("Restore failed: {err}")),
            }
        }
    }

    #[test]
    fn memory_can_store_and_load() {
        let mut mem = DummyMemory {
            store: Default::default(),
        };
        mem.store(MemoryUpdate {
            key: "foo".into(),
            value: "bar".into(),
        });
        assert_eq!(mem.load("foo"), Some("bar".into()));
    }

    #[test]
    fn memory_can_snapshot_and_restore() {
        let mut mem = DummyMemory {
            store: HashMap::from([("a".into(), "1".into())]),
        };

        let snap = mem.snapshot().unwrap();
        let mut new_mem = DummyMemory {
            store: Default::default(),
        };
        new_mem.restore(&snap).unwrap();

        assert_eq!(new_mem.load("a"), Some("1".into()));
    }
}
