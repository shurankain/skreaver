use std::path::PathBuf;
use std::sync::Arc;

use skreaver::agent::Agent;
use skreaver::memory::{FileMemory, MemoryReader, MemoryUpdate, MemoryWriter};
use skreaver::runtime::Coordinator;
use skreaver::{ExecutionResult, InMemoryToolRegistry, Tool};
use skreaver::{ToolCall, ToolName};

pub fn run_echo_agent() {
    let memory_path = PathBuf::from("echo_memory.json");

    let agent = EchoAgent {
        memory: FileMemory::new(memory_path),
        last_input: None,
    };

    let registry = InMemoryToolRegistry::new().with_tool("uppercase", Arc::new(UppercaseTool));
    let mut coordinator = Coordinator::new(agent, registry);

    loop {
        let mut input = String::new();
        if let Err(e) = std::io::stdin().read_line(&mut input) {
            tracing::error!(error = %e, "Failed to read user input");
            continue;
        }
        let output = coordinator.step(input.trim().to_string());
        println!("Agent said: {output}");
    }
}

struct EchoAgent {
    memory: FileMemory,
    last_input: Option<String>,
}

impl Agent for EchoAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        if let Ok(update) = MemoryUpdate::new("input", &input) {
            let _ = self.memory_writer().store(update);
        }
    }

    fn act(&mut self) -> Self::Action {
        self.last_input
            .as_ref()
            .map(|s| format!("Echo: {s}"))
            .unwrap_or_else(|| "No input".into())
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(input) = &self.last_input {
            vec![ToolCall {
                name: ToolName::new("uppercase").expect("Valid tool name"),
                input: input.clone(),
            }]
        } else {
            Vec::new()
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.is_success() {
            self.last_input = Some(result.output().to_string());
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let _ = self.memory_writer().store(update);
    }

    fn memory_reader(&self) -> &dyn MemoryReader {
        &self.memory
    }

    fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
        &mut self.memory
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
