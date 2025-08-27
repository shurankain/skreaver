//! # Advanced HTTP Server Example
//!
//! This example demonstrates the production-ready HTTP runtime features of Skreaver,
//! including JWT authentication, API keys, rate limiting, streaming responses,
//! and OpenAPI documentation generation.

use skreaver::{
    Agent, MemoryUpdate,
    memory::InMemoryMemory,
    runtime::{
        HttpAgentRuntime, HttpRuntimeConfig, auth::create_jwt_token, rate_limit::RateLimitConfig,
    },
    tool::{
        ExecutionResult, ToolCall,
        registry::InMemoryToolRegistry,
        standard::{FileReadTool, HttpGetTool, JsonParseTool, TextUppercaseTool},
    },
};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Advanced demo agent that handles complex processing scenarios
struct AdvancedDemoAgent {
    memory: Box<dyn skreaver::memory::Memory>,
    last_input: Option<String>,
    processing_history: Vec<String>,
}

impl AdvancedDemoAgent {
    fn new() -> Self {
        Self {
            memory: Box::new(InMemoryMemory::new()),
            last_input: None,
            processing_history: Vec::new(),
        }
    }
}

impl Agent for AdvancedDemoAgent {
    type Observation = String;
    type Action = String;

    fn observe(&mut self, input: Self::Observation) {
        println!("🔍 Advanced Agent received: {}", input);
        self.last_input = Some(input.clone());
        self.processing_history.push(format!("INPUT: {}", input));

        if let Ok(update) = MemoryUpdate::new("last_input", &input) {
            let _ = self.memory.store(update);
        }
        if let Ok(update) =
            MemoryUpdate::new("history_count", &self.processing_history.len().to_string())
        {
            let _ = self.memory.store(update);
        }
    }

    fn act(&mut self) -> Self::Action {
        let response = match self.last_input.as_deref() {
            Some(input) if input.starts_with("process:") => {
                let text = input.strip_prefix("process:").unwrap_or(input);
                format!(
                    "🔄 Processing complex task: '{}' (History: {} items)",
                    text,
                    self.processing_history.len()
                )
            }
            Some(input) if input.starts_with("analyze:") => {
                let text = input.strip_prefix("analyze:").unwrap_or(input);
                format!(
                    "📊 Deep analysis of: '{}' | Patterns detected: {} | Confidence: 89%",
                    text,
                    text.len() % 5 + 1
                )
            }
            Some(input) if input.starts_with("transform:") => {
                let text = input.strip_prefix("transform:").unwrap_or(input);
                format!("⚡ Transforming data: '{}' -> Enhanced format", text)
            }
            Some(input) if input.starts_with("fetch:") => {
                let url = input.strip_prefix("fetch:").unwrap_or(input);
                format!(
                    "🌐 Fetching external data from: {} (with caching enabled)",
                    url
                )
            }
            Some("status") => {
                format!(
                    "💡 Agent Status: Active | Memory items: {} | Processing history: {} entries",
                    self.processing_history.len(),
                    self.processing_history.len()
                )
            }
            Some(input) => {
                format!(
                    "🤖 Intelligent response to: '{}' | Context: {} previous interactions",
                    input,
                    self.processing_history.len()
                )
            }
            None => "❓ No input received - ready for instructions".to_string(),
        };

        println!("💭 Advanced Agent responding: {}", response);
        self.processing_history
            .push(format!("OUTPUT: {}", response));
        response
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if let Some(input) = &self.last_input {
            if let Some(text) = input.strip_prefix("transform:") {
                return vec![ToolCall {
                    name: skreaver::tool::ToolName::new("text_uppercase").unwrap(),
                    input: text.to_string(),
                }];
            }

            if let Some(url) = input.strip_prefix("fetch:") {
                return vec![ToolCall {
                    name: skreaver::tool::ToolName::new("http_get").unwrap(),
                    input: url.to_string(),
                }];
            }

            if input.starts_with("analyze:") {
                return vec![ToolCall {
                    name: skreaver::tool::ToolName::new("json_parse").unwrap(),
                    input: format!(
                        r#"{{"analysis_input": "{}", "timestamp": "{}", "agent_state": "analyzing"}}"#,
                        input,
                        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S")
                    ),
                }];
            }
        }
        Vec::new()
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        let message = if result.is_success() {
            format!("✅ Tool execution successful: {}", result.output())
        } else {
            format!("❌ Tool execution failed: {}", result.output())
        };

        println!("🔧 Tool result: {}", message);
        self.processing_history
            .push(format!("TOOL_RESULT: {}", message));

        if let Ok(update) = MemoryUpdate::new("last_tool_result", &message) {
            let _ = self.memory.store(update);
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let _ = self.memory.store(update);
    }

    fn memory(&mut self) -> &mut dyn skreaver::memory::Memory {
        &mut *self.memory
    }
}

/// Helper function to print authentication examples
fn print_auth_examples() {
    println!("🔐 Authentication Examples:");
    println!();

    // Create example JWT token
    if let Ok(token) = create_jwt_token(
        "demo-user".to_string(),
        vec!["read".to_string(), "write".to_string()],
    ) {
        println!("📋 Example JWT Token (copy for testing):");
        println!("   {}", token);
        println!();

        println!("🌐 Authenticated API Requests:");
        println!("   # List agents with JWT");
        println!("   curl -H \"Authorization: Bearer {}\" \\", token);
        println!("        http://localhost:3000/agents");
        println!();
        println!("   # Send observation with JWT");
        println!("   curl -X POST -H \"Authorization: Bearer {}\" \\", token);
        println!("        -H \"Content-Type: application/json\" \\");
        println!("        -d '{{\"input\": \"process:advanced task\"}}' \\");
        println!("        http://localhost:3000/agents/advanced-agent/observe");
        println!();
    }

    println!("   # Using API Key authentication");
    println!("   curl -H \"X-API-Key: sk-test-key-123\" \\");
    println!("        http://localhost:3000/agents");
    println!();

    println!("   # Create new JWT token");
    println!("   curl -X POST -H \"Content-Type: application/json\" \\");
    println!(
        "        -d '{{\"user_id\": \"my-user\", \"permissions\": [\"read\", \"write\"]}}' \\"
    );
    println!("        http://localhost:3000/auth/token");
}

fn print_streaming_examples() {
    println!("🔄 Streaming Examples:");
    println!("   # Stream agent execution in real-time (requires authentication)");
    println!("   curl -N -H \"Authorization: Bearer <TOKEN>\" \\");
    println!(
        "        'http://localhost:3000/agents/advanced-agent/stream?input=analyze:real-time-data'"
    );
    println!();
    println!("   # Using Server-Sent Events in browser:");
    println!(
        "   const eventSource = new EventSource('/agents/advanced-agent/stream?input=process:live-data');"
    );
    println!("   eventSource.onmessage = (event) => console.log(JSON.parse(event.data));");
}

fn print_api_documentation() {
    println!("📚 API Documentation:");
    println!("   📖 Interactive Swagger UI: http://localhost:3000/docs");
    println!("   📋 OpenAPI JSON Spec:      http://localhost:3000/api-docs/openapi.json");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Skreaver Advanced HTTP Server");
    println!("==================================");
    println!("Production-ready agent runtime with:");
    println!("  ✅ JWT Authentication & API Keys");
    println!("  ✅ Rate Limiting (1000/min global, 60/min per IP, 120/min per user)");
    println!("  ✅ Server-Sent Events Streaming");
    println!("  ✅ OpenAPI Documentation");
    println!("  ✅ CORS Support");
    println!();

    // Create advanced tool registry
    let registry = InMemoryToolRegistry::new()
        .with_tool("http_get", Arc::new(HttpGetTool::new()))
        .with_tool("file_read", Arc::new(FileReadTool::new()))
        .with_tool("text_uppercase", Arc::new(TextUppercaseTool::new()))
        .with_tool("json_parse", Arc::new(JsonParseTool::new()));

    // Configure rate limiting
    let rate_config = RateLimitConfig {
        global_rpm: 1000,  // 1000 requests per minute globally
        per_ip_rpm: 60,    // 60 requests per minute per IP
        per_user_rpm: 120, // 120 requests per minute per authenticated user
    };

    // Create HTTP runtime configuration
    let http_config = HttpRuntimeConfig {
        rate_limit: rate_config,
        request_timeout_secs: 30,
        max_body_size: 16 * 1024 * 1024, // 16MB
        enable_cors: true,
        enable_openapi: true,
    };

    // Create HTTP runtime with configuration
    let runtime = HttpAgentRuntime::with_config(registry, http_config.clone());

    // Create and add advanced demo agents
    let advanced_agent = AdvancedDemoAgent::new();
    let analytics_agent = AdvancedDemoAgent::new();
    let processing_agent = AdvancedDemoAgent::new();

    runtime
        .add_agent("advanced-agent".to_string(), advanced_agent)
        .await?;
    runtime
        .add_agent("analytics-agent".to_string(), analytics_agent)
        .await?;
    runtime
        .add_agent("processing-agent".to_string(), processing_agent)
        .await?;

    // Create router with all middleware and features
    let app = runtime.router_with_config(http_config.clone());

    // Start server
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("🌐 Server running on http://0.0.0.0:3000");
    println!();

    println!("📍 Available Endpoints:");
    println!("  Public:");
    println!("    GET  /health                           - Health check");
    println!("    POST /auth/token                       - Create JWT token");
    println!("    GET  /docs                             - Interactive API docs");
    println!("    GET  /api-docs/openapi.json           - OpenAPI specification");
    println!();
    println!("  Protected (requires authentication):");
    println!("    GET    /agents                         - List all agents");
    println!("    GET    /agents/:id/status             - Get agent status");
    println!("    POST   /agents/:id/observe            - Send observation to agent");
    println!("    GET    /agents/:id/stream             - Stream agent execution (SSE)");
    println!("    DELETE /agents/:id                    - Remove agent");
    println!("    POST   /agents                        - Create new agent (not implemented)");
    println!();

    print_auth_examples();
    print_streaming_examples();
    print_api_documentation();

    println!("💡 Advanced Usage Examples:");
    println!("   # Complex processing task");
    println!("   curl -X POST -H \"X-API-Key: sk-test-key-123\" \\");
    println!("        -H \"Content-Type: application/json\" \\");
    println!("        -d '{{\"input\": \"process:machine-learning-pipeline\"}}' \\");
    println!("        http://localhost:3000/agents/advanced-agent/observe");
    println!();
    println!("   # Data analysis request");
    println!("   curl -X POST -H \"X-API-Key: sk-test-key-123\" \\");
    println!("        -H \"Content-Type: application/json\" \\");
    println!("        -d '{{\"input\": \"analyze:user-behavior-patterns\"}}' \\");
    println!("        http://localhost:3000/agents/analytics-agent/observe");
    println!();
    println!("   # Transformation task");
    println!("   curl -X POST -H \"X-API-Key: sk-test-key-123\" \\");
    println!("        -H \"Content-Type: application/json\" \\");
    println!("        -d '{{\"input\": \"transform:raw-data-processing\"}}' \\");
    println!("        http://localhost:3000/agents/processing-agent/observe");
    println!();

    println!("🔒 Rate Limiting:");
    println!(
        "   • Global: {} requests/minute",
        http_config.rate_limit.global_rpm
    );
    println!(
        "   • Per IP: {} requests/minute",
        http_config.rate_limit.per_ip_rpm
    );
    println!(
        "   • Per authenticated user: {} requests/minute",
        http_config.rate_limit.per_user_rpm
    );
    println!();

    println!("⚡ Features Demonstrated:");
    println!("   🔐 JWT & API Key authentication");
    println!("   🚦 Rate limiting with headers");
    println!("   📡 Server-Sent Events streaming");
    println!("   📝 OpenAPI documentation generation");
    println!("   🌍 CORS support for web applications");
    println!("   ⏱️  Request timeouts and body size limits");
    println!("   🧪 Comprehensive error handling");
    println!();

    println!("Ready to serve requests! 🎯");

    axum::serve(listener, app).await?;
    Ok(())
}
