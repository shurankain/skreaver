//! # Standard Tools Agent
//!
//! This agent demonstrates the use of Skreaver's standard tool library
//! for processing various types of data and performing common operations.

use std::path::PathBuf;
use std::sync::Arc;

use skreaver::ToolCall;
use skreaver::agent::Agent;
use skreaver::memory::{FileMemory, Memory, MemoryUpdate};
use skreaver::runtime::Coordinator;
use skreaver::tool::registry::InMemoryToolRegistry;
use skreaver::tool::{ExecutionResult, standard::*};

pub fn run_standard_tools_agent() {
    let memory_path = PathBuf::from("standard_tools_memory.json");

    let agent = StandardToolsAgent {
        memory: Box::new(FileMemory::new(memory_path)),
        last_input: None,
    };

    let registry = InMemoryToolRegistry::new()
        .with_tool("text_uppercase", Arc::new(TextUppercaseTool::new()))
        .with_tool("text_reverse", Arc::new(TextReverseTool::new()))
        .with_tool("text_analyze", Arc::new(TextAnalyzeTool::new()))
        .with_tool("text_search", Arc::new(TextSearchTool::new()))
        .with_tool("json_parse", Arc::new(JsonParseTool::new()))
        .with_tool("json_transform", Arc::new(JsonTransformTool::new()))
        .with_tool("file_read", Arc::new(FileReadTool::new()))
        .with_tool("file_write", Arc::new(FileWriteTool::new()))
        .with_tool("http_get", Arc::new(HttpGetTool::new()));

    let mut coordinator = Coordinator::new(agent, registry);

    println!("🔧 Standard Tools Agent - Type your commands!");
    println!("Available commands:");
    println!("  text <text>     - Analyze and transform text");
    println!("  json <json>     - Parse and validate JSON");
    println!("  file <path>     - Read file content");
    println!("  http <url>      - Make HTTP GET request");
    println!("  help           - Show this help");
    println!("  quit           - Exit");
    println!();

    loop {
        let mut input = String::new();
        print!("skreaver> ");
        use std::io::{self, Write};
        io::stdout().flush().unwrap();

        match std::io::stdin().read_line(&mut input) {
            Ok(0) => {
                // EOF reached
                println!("\nGoodbye! 👋");
                break;
            }
            Ok(_) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == "quit" {
                    println!("Goodbye! 👋");
                    break;
                }

                if trimmed == "help" {
                    println!("Available commands:");
                    println!("  text <text>     - Analyze and transform text");
                    println!("  json <json>     - Parse and validate JSON");
                    println!("  file <path>     - Read file content");
                    println!("  http <url>      - Make HTTP GET request");
                    println!("  quit           - Exit");
                    continue;
                }

                let output = coordinator.step(trimmed.to_string());
                println!("Agent: {output}");
                println!();
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to read user input");
                break;
            }
        }
    }
}

struct StandardToolsAgent {
    memory: Box<dyn Memory>,
    last_input: Option<String>,
}

impl Agent for StandardToolsAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        if let Ok(update) = MemoryUpdate::new("input", &input) {
            let _ = self.memory.store(update);
        }
    }

    fn act(&mut self) -> Self::Action {
        match self.last_input.as_ref() {
            Some(input) => {
                if let Some(text) = input.strip_prefix("text ") {
                    format!("Processing text: '{}'", text)
                } else if input.starts_with("json ") {
                    "Parsing JSON data...".to_string()
                } else if let Some(file) = input.strip_prefix("file ") {
                    format!("Reading file: '{}'", file)
                } else if let Some(url) = input.strip_prefix("http ") {
                    format!("Making HTTP request to: '{}'", url)
                } else {
                    format!(
                        "Unknown command: '{}'. Type 'help' for available commands.",
                        input
                    )
                }
            }
            None => "No input received".to_string(),
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(input) = &self.last_input {
            if let Some(text) = input.strip_prefix("text ") {
                vec![
                    ToolCall {
                        name: skreaver::tool::ToolName::new("text_analyze")
                            .expect("Valid tool name"),
                        input: text.to_string(),
                    },
                    ToolCall {
                        name: skreaver::tool::ToolName::new("text_uppercase")
                            .expect("Valid tool name"),
                        input: text.to_string(),
                    },
                    ToolCall {
                        name: skreaver::tool::ToolName::new("text_reverse")
                            .expect("Valid tool name"),
                        input: text.to_string(),
                    },
                ]
            } else if let Some(json) = input.strip_prefix("json ") {
                vec![ToolCall {
                    name: skreaver::tool::ToolName::new("json_parse").expect("Valid tool name"),
                    input: json.to_string(),
                }]
            } else if let Some(path) = input.strip_prefix("file ") {
                vec![ToolCall {
                    name: skreaver::tool::ToolName::new("file_read").expect("Valid tool name"),
                    input: serde_json::json!({"path": path}).to_string(),
                }]
            } else if let Some(url) = input.strip_prefix("http ") {
                vec![ToolCall {
                    name: skreaver::tool::ToolName::new("http_get").expect("Valid tool name"),
                    input: url.to_string(),
                }]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.is_success() {
            println!("✅ Tool result: {}", result.output());
        } else {
            println!("❌ Tool failed: {}", result.output());
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let _ = self.memory.store(update);
    }

    fn memory(&mut self) -> &mut dyn Memory {
        &mut *self.memory
    }
}
