//! # Standard Tools Example
//!
//! This example demonstrates the usage of Skreaver's standard tool library
//! including HTTP clients, file operations, JSON processing, and text manipulation.

use skreaver::{
    ExecutionResult, FileReadTool, FileWriteTool, HttpGetTool, InMemoryMemory,
    InMemoryToolRegistry, JsonParseTool, MemoryReader, MemoryUpdate, MemoryWriter, TextAnalyzeTool,
    TextUppercaseTool, ToolCall, ToolName, agent::Agent, runtime::Coordinator,
};
use std::sync::Arc;

/// Example agent that uses standard tools
struct StandardToolsAgent {
    memory: InMemoryMemory,
    last_input: Option<String>,
}

impl Agent for StandardToolsAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        if let Ok(update) = MemoryUpdate::new("last_input", &input) {
            let _ = self.memory.store(update);
        }
    }

    fn act(&mut self) -> Self::Action {
        format!(
            "Processing: {}",
            self.last_input.as_deref().unwrap_or("No input")
        )
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(input) = &self.last_input {
            match input.as_str() {
                "test_text" => {
                    vec![
                        ToolCall {
                            name: ToolName::new("text_uppercase").unwrap(),
                            input: "hello world".to_string(),
                        },
                        ToolCall {
                            name: ToolName::new("text_analyze").unwrap(),
                            input: "hello world".to_string(),
                        },
                    ]
                }
                "test_json" => {
                    vec![ToolCall {
                        name: ToolName::new("json_parse").unwrap(),
                        input: r#"{"name": "Skreaver", "version": "0.1.0", "tools": ["http", "file", "json", "text"]}"#.to_string(),
                    }]
                }
                "test_file" => {
                    vec![
                        ToolCall {
                            name: ToolName::new("file_write").unwrap(),
                            input: serde_json::json!({
                                "path": "/tmp/skreaver_test.txt",
                                "content": "Hello from Skreaver standard tools!"
                            })
                            .to_string(),
                        },
                        ToolCall {
                            name: ToolName::new("file_read").unwrap(),
                            input: serde_json::json!({
                                "path": "/tmp/skreaver_test.txt"
                            })
                            .to_string(),
                        },
                    ]
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.is_success() {
            println!("Tool result: {}", result.output());
        } else {
            println!("Tool failed: {}", result.output());
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let _ = self.memory.store(update);
    }

    fn memory_reader(&self) -> &dyn MemoryReader {
        &self.memory
    }

    fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
        &mut self.memory
    }
}

fn main() {
    println!("🔧 Skreaver Standard Tools Example");
    println!("===================================");

    let agent = StandardToolsAgent {
        memory: InMemoryMemory::new(),
        last_input: None,
    };

    let registry = InMemoryToolRegistry::new()
        .with_tool("http_get", Arc::new(HttpGetTool::new()))
        .with_tool("file_read", Arc::new(FileReadTool::new()))
        .with_tool("file_write", Arc::new(FileWriteTool::new()))
        .with_tool("json_parse", Arc::new(JsonParseTool::new()))
        .with_tool("text_uppercase", Arc::new(TextUppercaseTool::new()))
        .with_tool("text_analyze", Arc::new(TextAnalyzeTool::new()));

    let mut coordinator = Coordinator::new(agent, registry);

    // Test text processing tools
    println!("\n📝 Testing text processing tools:");
    let output = coordinator.step("test_text".to_string());
    println!("Agent response: {}", output);

    // Test JSON processing tools
    println!("\n🔍 Testing JSON processing tools:");
    let output = coordinator.step("test_json".to_string());
    println!("Agent response: {}", output);

    // Test file operations
    println!("\n📁 Testing file operations:");
    let output = coordinator.step("test_file".to_string());
    println!("Agent response: {}", output);

    println!("\n✅ Standard tools example completed!");
}
