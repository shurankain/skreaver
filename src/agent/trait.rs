use crate::memory::{Memory, MemoryUpdate};
use crate::tool::{ExecutionResult, ToolCall};

pub trait Agent {
    type Observation;
    type Action;

    fn memory(&mut self) -> &mut dyn Memory;

    fn observe(&mut self, input: Self::Observation);
    fn act(&mut self) -> Self::Action;
    fn update_context(&mut self, update: MemoryUpdate);

    fn call_tools(&self) -> Vec<ToolCall> {
        self.call_tool().into_iter().collect()
    }

    fn call_tool(&self) -> Option<ToolCall> {
        None
    }

    fn handle_result(&mut self, _result: ExecutionResult) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Memory, MemoryUpdate};

    struct DummyMemory {
        store: Vec<(String, String)>,
    }

    impl Memory for DummyMemory {
        fn load(&self, key: &str) -> Option<String> {
            self.store
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v.clone())
        }

        fn store(&mut self, update: MemoryUpdate) {
            self.store.push((update.key, update.value));
        }
    }

    struct DummyAgent {
        mem: Box<dyn Memory>,
        last_observation: Option<String>,
    }

    impl Agent for DummyAgent {
        type Observation = String;
        type Action = String;

        fn memory(&mut self) -> &mut dyn Memory {
            &mut *self.mem
        }

        fn observe(&mut self, input: Self::Observation) {
            self.last_observation = Some(input);
        }

        fn act(&mut self) -> Self::Action {
            self.last_observation
                .as_ref()
                .map(|s| format!("echo: {s}"))
                .unwrap_or_else(|| "no input".into())
        }

        fn update_context(&mut self, update: MemoryUpdate) {
            self.memory().store(update);
        }
    }

    #[test]
    fn agent_can_store_memory_through_boxed_trait() {
        let mut agent = DummyAgent {
            mem: Box::new(DummyMemory { store: vec![] }),
            last_observation: None,
        };

        agent.update_context(MemoryUpdate {
            key: "k".into(),
            value: "v".into(),
        });

        assert_eq!(agent.memory().load("k"), Some("v".into()));
    }
}
