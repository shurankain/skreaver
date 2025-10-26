use std::path::PathBuf;
use std::sync::Arc;

use skreaver::{
    ExecutionResult, FileMemory, InMemoryToolRegistry, MemoryReader, MemoryUpdate, MemoryWriter,
    Tool, ToolCall, agent::Agent, runtime::Coordinator,
};

pub fn run_multi_agent() {
    let memory_path = PathBuf::from("multi_memory.json");

    let agent = MultiToolAgent {
        memory: FileMemory::new(memory_path),
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
    memory: FileMemory,
    last_input: Option<String>,
    tool_results: Vec<String>,
}

impl Agent for MultiToolAgent {
    type Observation = String;
    type Action = String;
    type Error = std::convert::Infallible;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        if let Ok(update) = MemoryUpdate::new("input", &input) {
            let _ = self.memory_writer().store(update);
        }
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
                    ToolCall::new("uppercase", input).expect("Valid tool name"),
                    ToolCall::new("reverse", input).expect("Valid tool name"),
                ]
            })
            .unwrap_or_default()
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.is_success() {
            self.tool_results.push(result.output().to_string());
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

struct ReverseTool;

impl Tool for ReverseTool {
    fn name(&self) -> &str {
        "reverse"
    }

    fn call(&self, input: String) -> ExecutionResult {
        ExecutionResult::success(input.chars().rev().collect())
    }
}
