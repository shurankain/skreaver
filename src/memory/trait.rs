#[derive(Debug, Clone)]
pub struct MemoryUpdate {
    pub key: String,
    pub value: String,
}

pub trait Memory {
    fn load(&self, key: &str) -> Option<String>;
    fn store(&mut self, update: MemoryUpdate);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyMemory {
        store: std::collections::HashMap<String, String>,
    }

    impl Memory for DummyMemory {
        fn load(&self, key: &str) -> Option<String> {
            self.store.get(key).cloned()
        }

        fn store(&mut self, update: MemoryUpdate) {
            self.store.insert(update.key, update.value);
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
}
