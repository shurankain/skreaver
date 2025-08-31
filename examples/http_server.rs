//! # HTTP Server Example
//!
//! This example demonstrates how to run Skreaver agents through an HTTP server
//! using Axum. It shows agent lifecycle management, remote observations, and
//! RESTful API interactions.

use skreaver::{
    Agent, ExecutionResult, FileReadTool, HttpGetTool, InMemoryMemory, InMemoryToolRegistry,
    JsonParseTool, MemoryReader, MemoryUpdate, MemoryWriter, TextUppercaseTool, ToolCall, ToolName,
    runtime::HttpAgentRuntime,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

/// Example agent that processes various types of requests
struct HttpDemoAgent {
    memory: InMemoryMemory,
    last_input: Option<String>,
}

impl Agent for HttpDemoAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        println!("🔍 Agent received: {}", input);
        self.last_input = Some(input.clone());
        if let Ok(update) = MemoryUpdate::new("last_input", &input) {
            let _ = self.memory_writer().store(update);
        }
    }

    fn act(&mut self) -> Self::Action {
        let response = match self.last_input.as_deref() {
            Some(input) if input.starts_with("uppercase:") => {
                let text = input.strip_prefix("uppercase:").unwrap_or(input);
                format!("Processing uppercase transformation for: '{}'", text)
            }
            Some(input) if input.starts_with("analyze:") => {
                let text = input.strip_prefix("analyze:").unwrap_or(input);
                format!("Analyzing text: '{}'", text)
            }
            Some(input) if input.starts_with("fetch:") => {
                let url = input.strip_prefix("fetch:").unwrap_or(input);
                format!("Fetching data from: {}", url)
            }
            Some(input) => format!("Echo: {}", input),
            None => "No input received".to_string(),
        };

        println!("💭 Agent responding: {}", response);
        response
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(input) = &self.last_input {
            if let Some(text) = input.strip_prefix("uppercase:") {
                return vec![ToolCall {
                    name: ToolName::new("text_uppercase").unwrap(),
                    input: text.to_string(),
                }];
            }

            if let Some(url) = input.strip_prefix("fetch:") {
                return vec![ToolCall {
                    name: ToolName::new("http_get").unwrap(),
                    input: url.to_string(),
                }];
            }

            if input == "demo_json" {
                return vec![ToolCall {
                    name: ToolName::new("json_parse").unwrap(),
                    input: r#"{"message": "Hello from HTTP agent!", "timestamp": "2024-01-01T00:00:00Z"}"#.to_string(),
                }];
            }
        }
        Vec::new()
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        let message = if result.is_success() {
            format!("✅ Tool succeeded: {}", result.output())
        } else {
            format!("❌ Tool failed: {}", result.output())
        };

        println!("{}", message);

        if let Ok(update) = MemoryUpdate::new("last_tool_result", &message) {
            let _ = self.memory_writer().store(update);
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting Skreaver HTTP Server Example");
    println!("========================================");

    // Create tool registry with standard tools
    let registry = InMemoryToolRegistry::new()
        .with_tool("http_get", Arc::new(HttpGetTool::new()))
        .with_tool("file_read", Arc::new(FileReadTool::new()))
        .with_tool("text_uppercase", Arc::new(TextUppercaseTool::new()))
        .with_tool("json_parse", Arc::new(JsonParseTool::new()));

    // Create HTTP runtime
    let runtime = HttpAgentRuntime::new(registry);

    // Create and add demo agents
    let demo_agent_1 = HttpDemoAgent {
        memory: InMemoryMemory::new(),
        last_input: None,
    };

    let demo_agent_2 = HttpDemoAgent {
        memory: InMemoryMemory::new(),
        last_input: None,
    };

    runtime
        .add_agent("demo-agent-1".to_string(), demo_agent_1)
        .await?;
    runtime
        .add_agent("demo-agent-2".to_string(), demo_agent_2)
        .await?;

    // Create router with CORS support
    let app = runtime.router().layer(CorsLayer::permissive());

    // Start server
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("🌐 Server listening on http://0.0.0.0:3000");
    println!();
    println!("Available endpoints:");
    println!("  GET    /health                    - Health check");
    println!("  GET    /agents                    - List all agents");
    println!("  GET    /agents/:id/status         - Get agent status");
    println!("  POST   /agents/:id/observe        - Send observation to agent");
    println!("  DELETE /agents/:id               - Remove agent");
    println!();
    println!("Example requests:");
    println!("  curl http://localhost:3000/health");
    println!("  curl http://localhost:3000/agents");
    println!("  curl -X POST http://localhost:3000/agents/demo-agent-1/observe \\");
    println!("       -H 'Content-Type: application/json' \\");
    println!("       -d '{{\"input\": \"Hello from HTTP!\"}}'");
    println!("  curl -X POST http://localhost:3000/agents/demo-agent-1/observe \\");
    println!("       -H 'Content-Type: application/json' \\");
    println!("       -d '{{\"input\": \"uppercase:hello world\"}}'");
    println!("  curl -X POST http://localhost:3000/agents/demo-agent-1/observe \\");
    println!("       -H 'Content-Type: application/json' \\");
    println!("       -d '{{\"input\": \"demo_json\"}}'");

    axum::serve(listener, app).await?;
    Ok(())
}
