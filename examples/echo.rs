use skreaver::ToolCall;
use skreaver::agent::Agent;
use skreaver::memory::{Memory, MemoryUpdate};
use skreaver::runtime::Coordinator;
use skreaver::tool::{ExecutionResult, Tool};

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

struct EchoAgent {
    memory: DummyMemory,
    last_input: Option<String>,
}

impl Agent for EchoAgent {
    type Observation = String;
    type Action = String;
    type Memory = DummyMemory;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        self.memory.store(MemoryUpdate {
            key: "input".into(),
            value: input,
        });
    }

    fn act(&mut self) -> Self::Action {
        self.last_input
            .as_ref()
            .map(|s| format!("Echo: {s}"))
            .unwrap_or_else(|| "No input".into())
    }

    fn call_tool(&self) -> Option<ToolCall> {
        self.last_input.as_ref().map(|input| ToolCall {
            name: "uppercase".into(),
            input: input.clone(),
        })
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.success {
            self.last_input = Some(result.output.clone());
        }
    }

    fn update_context(&mut self, _update: MemoryUpdate) {}
}

// Simple tool that uppercases input
struct UppercaseTool;

impl Tool for UppercaseTool {
    fn name(&self) -> &str {
        "uppercase"
    }

    fn call(&self, input: String) -> ExecutionResult {
        ExecutionResult {
            output: input.to_uppercase(),
            success: true,
        }
    }
}

fn main() {
    let agent = EchoAgent {
        memory: DummyMemory {
            store: Default::default(),
        },
        last_input: None,
    };

    let mut coordinator = Coordinator::new(agent, UppercaseTool);
    let output = coordinator.step("Skreaver".into());

    println!("Agent said: {output}");
}
