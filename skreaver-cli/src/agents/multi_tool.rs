use std::path::PathBuf;
use std::sync::Arc;

use skreaver::ToolCall;
use skreaver::agent::Agent;
use skreaver::memory::{FileMemory, Memory, MemoryUpdate};
use skreaver::runtime::Coordinator;
use skreaver::tool::registry::InMemoryToolRegistry;
use skreaver::tool::{ExecutionResult, Tool};

pub fn run_multi_agent() {
    let memory_path = PathBuf::from("multi_memory.json");

    let agent = MultiToolAgent {
        memory: Box::new(FileMemory::new(memory_path)),
        last_input: None,
        tool_results: vec![],
    };

    let registry = InMemoryToolRegistry::new()
        .with_tool("uppercase", Arc::new(UppercaseTool))
        .with_tool("reverse", Arc::new(ReverseTool));

    let mut coordinator = Coordinator::new(agent, registry);

    let output = coordinator.step("Skreaver".into());

    println!("Agent said: {output}");
}

struct MultiToolAgent {
    memory: Box<dyn Memory>,
    last_input: Option<String>,
    tool_results: Vec<String>,
}

impl Agent for MultiToolAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        self.memory.store(MemoryUpdate {
            key: "input".into(),
            value: input,
        });
        self.tool_results.clear();
    }

    fn act(&mut self) -> Self::Action {
        let base = self
            .last_input
            .as_ref()
            .cloned()
            .unwrap_or_else(|| "No input".into());

        if self.tool_results.is_empty() {
            format!("Echo: {base}")
        } else {
            format!("Echo: {base} -> [{}]", self.tool_results.join(", "))
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        self.last_input
            .as_ref()
            .map(|input| {
                vec![
                    ToolCall {
                        name: "uppercase".into(),
                        input: input.clone(),
                    },
                    ToolCall {
                        name: "reverse".into(),
                        input: input.clone(),
                    },
                ]
            })
            .unwrap_or_default()
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.success {
            self.tool_results.push(result.output);
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        self.memory.store(update);
    }

    fn memory(&mut self) -> &mut dyn Memory {
        &mut *self.memory
    }
}

struct UppercaseTool;

impl Tool for UppercaseTool {
    fn name(&self) -> &str {
        "uppercase"
    }

    fn call(&self, input: String) -> ExecutionResult {
        ExecutionResult::success(input.to_uppercase())
    }
}

struct ReverseTool;

impl Tool for ReverseTool {
    fn name(&self) -> &str {
        "reverse"
    }

    fn call(&self, input: String) -> ExecutionResult {
        ExecutionResult::success(input.chars().rev().collect())
    }
}
