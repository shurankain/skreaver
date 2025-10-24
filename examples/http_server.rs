//! # HTTP Server Example
//!
//! This example demonstrates how to run Skreaver agents through an HTTP server
//! using Axum. It shows agent lifecycle management, remote observations, and
//! RESTful API interactions.

use skreaver::{
    Agent, ExecutionResult, FileReadTool, HttpGetTool, InMemoryMemory, InMemoryToolRegistry,
    JsonParseTool, MemoryReader, MemoryUpdate, MemoryWriter, TextUppercaseTool, ToolCall,
    runtime::{HttpAgentRuntime, HttpRuntimeConfigBuilder, shutdown_signal},
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
                return vec![ToolCall::new("text_uppercase", text).unwrap()];
            }

            if let Some(url) = input.strip_prefix("fetch:") {
                return vec![ToolCall::new("http_get", url).unwrap()];
            }

            if input == "demo_json" {
                return vec![ToolCall::new("json_parse", r#"{"message": "Hello from HTTP agent!", "timestamp": "2024-01-01T00:00:00Z"}"#).unwrap()];
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

    // Load configuration from environment variables
    // This allows runtime configuration without rebuilding the application.
    // See config module documentation for all available environment variables.
    let config = HttpRuntimeConfigBuilder::from_env()
        .map_err(|e| format!("Failed to load config from environment: {}", e))?
        .build()
        .map_err(|e| format!("Failed to validate config: {}", e))?;

    println!("📋 Configuration loaded:");
    println!(
        "   - Request timeout: {} seconds",
        config.request_timeout_secs
    );
    println!(
        "   - Max body size: {} MB",
        config.max_body_size / (1024 * 1024)
    );
    println!("   - CORS enabled: {}", config.enable_cors);
    println!(
        "   - Rate limit (global): {} req/min",
        config.rate_limit.global_rpm
    );
    println!(
        "   - Rate limit (per IP): {} req/min",
        config.rate_limit.per_ip_rpm
    );
    println!(
        "   - Backpressure max queue: {}",
        config.backpressure.max_queue_size
    );
    println!();

    // Create tool registry with standard tools
    let registry = InMemoryToolRegistry::new()
        .with_tool("http_get", Arc::new(HttpGetTool::new()))
        .with_tool("file_read", Arc::new(FileReadTool::new()))
        .with_tool("text_uppercase", Arc::new(TextUppercaseTool::new()))
        .with_tool("json_parse", Arc::new(JsonParseTool::new()));

    // Create HTTP runtime with environment-based configuration
    let runtime = HttpAgentRuntime::with_config(registry, config);

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

    // Start server with graceful shutdown
    // This handles SIGTERM (from Kubernetes) and SIGINT (Ctrl+C)
    println!();
    println!("💡 Graceful shutdown enabled:");
    println!("   - Send SIGTERM (kill <pid>) for graceful shutdown");
    println!("   - Press Ctrl+C for graceful shutdown");
    println!("   - All in-flight requests will be completed before shutdown");

    // Serve with ConnectInfo to enable IP tracking for connection limits
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    println!("✅ Server shutdown complete");
    Ok(())
}
