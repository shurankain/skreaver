use std::sync::Arc;

use skreaver::Memory;
use skreaver::ToolCall;
use skreaver::agent::Agent;
use skreaver::memory::{InMemoryMemory, MemoryUpdate};
use skreaver::runtime::Coordinator;
use skreaver::tool::registry::InMemoryToolRegistry;
use skreaver::tool::{ExecutionResult, Tool};

struct EchoAgent {
    memory: InMemoryMemory,
    last_input: Option<String>,
}

impl Agent for EchoAgent {
    type Observation = String;
    type Action = String;
    type Memory = InMemoryMemory;

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
        memory: InMemoryMemory::new(),
        last_input: None,
    };

    let registry = InMemoryToolRegistry::new().with_tool("uppercase", Arc::new(UppercaseTool));

    let mut coordinator = Coordinator::new(agent, registry);

    let output = coordinator.step("Skreaver".into());

    println!("Agent said: {output}");
}
