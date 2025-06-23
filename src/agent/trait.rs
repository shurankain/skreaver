use crate::memory::MemoryUpdate;

pub trait Agent {
    type Observation;
    type Action;
    type Memory;

    fn observe(&mut self, input: Self::Observation);
    fn act(&mut self) -> Self::Action;
    fn update_context(&mut self, update: MemoryUpdate);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAgent {
        memory: Vec<String>,
        last_observation: Option<String>,
    }

    impl Agent for DummyAgent {
        type Observation = String;
        type Action = String;
        type Memory = ();

        fn observe(&mut self, input: Self::Observation) {
            self.last_observation = Some(input);
        }

        fn act(&mut self) -> Self::Action {
            self.last_observation
                .as_ref()
                .map(|s| format!("echo: {s}"))
                .unwrap_or_else(|| "no input".into())
        }

        fn update_context(&mut self, update: crate::memory::MemoryUpdate) {
            self.memory.push(format!("{}={}", update.key, update.value));
        }
    }

    #[test]
    fn agent_can_observe_and_act() {
        let mut agent = DummyAgent {
            memory: vec![],
            last_observation: None,
        };

        agent.observe("hello".into());
        let action = agent.act();
        assert_eq!(action, "echo: hello");
    }

    #[test]
    fn agent_can_update_context() {
        let mut agent = DummyAgent {
            memory: vec![],
            last_observation: None,
        };

        agent.update_context(crate::memory::MemoryUpdate {
            key: "k".into(),
            value: "v".into(),
        });

        assert_eq!(agent.memory[0], "k=v");
    }
}
