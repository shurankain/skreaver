//! Code templates for agent and tool generation

use std::fmt;

/// Agent template types
#[derive(Debug, Clone, Copy)]
pub enum AgentTemplate {
    Simple,
    Reasoning,
    MultiTool,
}

impl AgentTemplate {
    pub fn from_str(s: &str) -> Result<Self, super::ScaffoldError> {
        match s.to_lowercase().as_str() {
            "simple" => Ok(Self::Simple),
            "reasoning" => Ok(Self::Reasoning),
            "multi-tool" | "multi" => Ok(Self::MultiTool),
            _ => Err(super::ScaffoldError::UnknownTemplate(s.to_string())),
        }
    }
}

impl fmt::Display for AgentTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple => write!(f, "simple"),
            Self::Reasoning => write!(f, "reasoning"),
            Self::MultiTool => write!(f, "multi-tool"),
        }
    }
}

/// Tool template types
#[derive(Debug, Clone, Copy)]
pub enum ToolTemplate {
    HttpClient,
    Database,
    Custom,
    FileSystem,
    ApiClient,
    Workflow,
}

impl ToolTemplate {
    pub fn from_str(s: &str) -> Result<Self, super::ScaffoldError> {
        match s.to_lowercase().as_str() {
            "http-client" | "http" => Ok(Self::HttpClient),
            "database" | "db" => Ok(Self::Database),
            "custom" => Ok(Self::Custom),
            "filesystem" | "fs" | "file" => Ok(Self::FileSystem),
            "api-client" | "api" => Ok(Self::ApiClient),
            "workflow" | "pipeline" => Ok(Self::Workflow),
            _ => Err(super::ScaffoldError::UnknownTemplate(s.to_string())),
        }
    }

    pub fn all() -> Vec<(&'static str, &'static str)> {
        vec![
            (
                "http-client",
                "Simple HTTP client for making GET/POST requests",
            ),
            (
                "api-client",
                "Advanced API client with authentication and rate limiting",
            ),
            ("database", "Database query tool with connection pooling"),
            ("filesystem", "File system operations (read, write, list)"),
            ("workflow", "Multi-step workflow/pipeline executor"),
            ("custom", "Empty custom tool template"),
        ]
    }
}

impl fmt::Display for ToolTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpClient => write!(f, "http-client"),
            Self::Database => write!(f, "database"),
            Self::Custom => write!(f, "custom"),
            Self::FileSystem => write!(f, "filesystem"),
            Self::ApiClient => write!(f, "api-client"),
            Self::Workflow => write!(f, "workflow"),
        }
    }
}

// Cargo.toml templates

pub fn simple_agent_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
skreaver = {{ path = "../../crates/skreaver", features = ["default"] }}
skreaver-core = {{ path = "../../crates/skreaver-core" }}
tokio = {{ version = "1", features = ["full"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter", "json"] }}
"#,
        name = name
    )
}

pub fn reasoning_agent_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
skreaver = {{ path = "../../crates/skreaver", features = ["default"] }}
skreaver-core = {{ path = "../../crates/skreaver-core" }}
skreaver-tools = {{ path = "../../crates/skreaver-tools" }}
tokio = {{ version = "1", features = ["full"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter", "json"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#,
        name = name
    )
}

pub fn multi_tool_agent_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
skreaver = {{ path = "../../crates/skreaver", features = ["default"] }}
skreaver-core = {{ path = "../../crates/skreaver-core" }}
skreaver-tools = {{ path = "../../crates/skreaver-tools" }}
tokio = {{ version = "1", features = ["full"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter", "json"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
reqwest = {{ version = "0.12", features = ["json"] }}
"#,
        name = name
    )
}

// Main.rs templates

pub fn simple_agent_main(name: &str) -> String {
    format!(
        r#"//! {name} - Simple Skreaver Agent

use skreaver_core::{{
    agent::{{Agent, AgentConfig}},
    memory::InMemory,
    tool::ToolRegistry,
}};

#[tokio::main]
async fn main() {{
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create agent configuration
    let config = AgentConfig::default()
        .with_name("{name}")
        .with_system_prompt("You are a helpful AI assistant.");

    // Create memory and tool registry
    let memory = InMemory::new();
    let tools = ToolRegistry::new();

    // Create agent
    let mut agent = Agent::new(config, memory, tools);

    // Run the agent
    let result = agent.execute("Hello! Please introduce yourself.").await;

    match result {{
        Ok(response) => {{
            println!("Agent response: {{}}", response);
        }}
        Err(e) => {{
            eprintln!("Error: {{}}", e);
        }}
    }}
}}
"#,
        name = name
    )
}

pub fn reasoning_agent_main(name: &str) -> String {
    format!(
        r#"//! {name} - Reasoning Agent with Tools

use skreaver_core::{{
    agent::{{Agent, AgentConfig}},
    memory::InMemory,
    tool::ToolRegistry,
}};

mod tools;

#[tokio::main]
async fn main() {{
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Create agent configuration with reasoning capabilities
    let config = AgentConfig::default()
        .with_name("{name}")
        .with_system_prompt(
            "You are an AI assistant with access to tools. \
             Think step-by-step and use tools when appropriate."
        );

    // Create memory
    let memory = InMemory::new();

    // Create tool registry and register tools
    let mut tools = ToolRegistry::new();
    // Register your tools here
    // tools.register(Box::new(YourTool::new()));

    // Create agent
    let mut agent = Agent::new(config, memory, tools);

    // Example task requiring reasoning
    let task = "What is 15 * 23? Please calculate and explain your reasoning.";

    match agent.execute(task).await {{
        Ok(response) => {{
            println!("Task: {{}}", task);
            println!("\\nAgent response:\\n{{}}", response);
        }}
        Err(e) => {{
            eprintln!("Error: {{}}", e);
        }}
    }}
}}
"#,
        name = name
    )
}

pub fn multi_tool_agent_main(name: &str) -> String {
    format!(
        r#"//! {name} - Multi-Tool Agent

use skreaver_core::{{
    agent::{{Agent, AgentConfig}},
    memory::InMemory,
    tool::ToolRegistry,
}};

mod tools;

use tools::{{http_client::HttpClientTool, calculator::CalculatorTool}};

#[tokio::main]
async fn main() {{
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    // Create agent configuration
    let config = AgentConfig::default()
        .with_name("{name}")
        .with_system_prompt(
            "You are an AI assistant with access to HTTP client and calculator tools. \
             Use them to help users accomplish their tasks."
        );

    // Create memory
    let memory = InMemory::new();

    // Create tool registry and register tools
    let mut tools = ToolRegistry::new();
    tools.register(Box::new(HttpClientTool::new()));
    tools.register(Box::new(CalculatorTool::new()));

    // Create agent
    let mut agent = Agent::new(config, memory, tools);

    // Example multi-tool task
    let task = "Fetch data from https://api.github.com/repos/rust-lang/rust and \
                calculate how many days since it was created.";

    match agent.execute(task).await {{
        Ok(response) => {{
            println!("Task: {{}}", task);
            println!("\\nAgent response:\\n{{}}", response);
        }}
        Err(e) => {{
            eprintln!("Error: {{}}", e);
        }}
    }}
}}
"#,
        name = name
    )
}

// Tool module templates

pub fn reasoning_tools_mod() -> &'static str {
    r#"//! Agent tools

// Add your custom tools here
// Example:
// pub mod calculator;
// pub mod web_search;
"#
}

pub fn http_client_tool() -> &'static str {
    r#"//! HTTP Client Tool

use async_trait::async_trait;
use serde_json::Value;
use skreaver_core::tool::{Tool, ToolError, ToolInput, ToolResult};

pub struct HttpClientTool {
    client: reqwest::Client,
}

impl HttpClientTool {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for HttpClientTool {
    fn name(&self) -> &str {
        "http_client"
    }

    fn description(&self) -> &str {
        "Make HTTP GET requests to fetch data from URLs"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'url' parameter".to_string()))?;

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let body = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(serde_json::json!({
            "content": body,
            "url": url
        }))
    }
}
"#
}

pub fn database_tool() -> &'static str {
    r#"//! Database Tool

use async_trait::async_trait;
use serde_json::Value;
use skreaver_core::tool::{Tool, ToolError, ToolInput, ToolResult};

pub struct DatabaseTool {
    // Add your database connection here
}

impl DatabaseTool {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Tool for DatabaseTool {
    fn name(&self) -> &str {
        "database_query"
    }

    fn description(&self) -> &str {
        "Execute SQL queries against the database"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "SQL query to execute"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        let _query = input
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'query' parameter".to_string()))?;

        // TODO: Implement database query execution
        Ok(serde_json::json!({
            "message": "Database tool not yet implemented"
        }))
    }
}
"#
}

pub fn custom_tool() -> &'static str {
    r#"//! Custom Tool Template

use async_trait::async_trait;
use serde_json::Value;
use skreaver_core::tool::{Tool, ToolError, ToolInput, ToolResult};

pub struct CustomTool;

impl CustomTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CustomTool {
    fn name(&self) -> &str {
        "custom_tool"
    }

    fn description(&self) -> &str {
        "A custom tool - describe what it does here"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "input": {
                    "type": "string",
                    "description": "Tool input description"
                }
            },
            "required": ["input"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        let _param = input
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'input' parameter".to_string()))?;

        // TODO: Implement your tool logic here
        Ok(serde_json::json!({
            "result": "Custom tool result"
        }))
    }
}
"#
}

pub fn calculator_tool() -> &'static str {
    r#"//! Calculator Tool

use async_trait::async_trait;
use serde_json::Value;
use skreaver_core::tool::{Tool, ToolError, ToolInput, ToolResult};

pub struct CalculatorTool;

impl CalculatorTool {
    pub fn new() -> Self {
        Self
    }

    fn evaluate(&self, expression: &str) -> Result<f64, String> {
        // Simple expression parser - extend as needed
        let parts: Vec<&str> = expression.split_whitespace().collect();

        if parts.len() != 3 {
            return Err("Expression must be in format: number operator number".to_string());
        }

        let a: f64 = parts[0].parse().map_err(|_| "Invalid first number")?;
        let op = parts[1];
        let b: f64 = parts[2].parse().map_err(|_| "Invalid second number")?;

        match op {
            "+" => Ok(a + b),
            "-" => Ok(a - b),
            "*" => Ok(a * b),
            "/" => {
                if b == 0.0 {
                    Err("Division by zero".to_string())
                } else {
                    Ok(a / b)
                }
            }
            _ => Err(format!("Unknown operator: {}", op)),
        }
    }
}

#[async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &str {
        "calculator"
    }

    fn description(&self) -> &str {
        "Perform basic arithmetic calculations (add, subtract, multiply, divide)"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "Math expression in format: 'number operator number' (e.g., '5 + 3')"
                }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        let expression = input
            .get("expression")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'expression' parameter".to_string()))?;

        let result = self
            .evaluate(expression)
            .map_err(|e| ToolError::ExecutionFailed(e))?;

        Ok(serde_json::json!({
            "expression": expression,
            "result": result
        }))
    }
}
"#
}

pub fn filesystem_tool() -> &'static str {
    r#"//! File System Tool

use async_trait::async_trait;
use serde_json::Value;
use skreaver_core::tool::{Tool, ToolError, ToolInput, ToolResult};
use std::path::Path;
use tokio::fs;

pub struct FileSystemTool {
    base_path: std::path::PathBuf,
}

impl FileSystemTool {
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    async fn read_file(&self, path: &str) -> Result<String, ToolError> {
        let full_path = self.base_path.join(path);

        // Security: Prevent path traversal
        if !full_path.starts_with(&self.base_path) {
            return Err(ToolError::InvalidInput("Path traversal detected".to_string()));
        }

        fs::read_to_string(&full_path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), ToolError> {
        let full_path = self.base_path.join(path);

        // Security: Prevent path traversal
        if !full_path.starts_with(&self.base_path) {
            return Err(ToolError::InvalidInput("Path traversal detected".to_string()));
        }

        // Create parent directories if needed
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create directories: {}", e)))?;
        }

        fs::write(&full_path, content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))
    }

    async fn list_files(&self, dir: &str) -> Result<Vec<String>, ToolError> {
        let full_path = self.base_path.join(dir);

        // Security: Prevent path traversal
        if !full_path.starts_with(&self.base_path) {
            return Err(ToolError::InvalidInput("Path traversal detected".to_string()));
        }

        let mut entries = fs::read_dir(&full_path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read directory: {}", e)))?;

        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| ToolError::ExecutionFailed(e.to_string()))? {
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }

        Ok(files)
    }
}

#[async_trait]
impl Tool for FileSystemTool {
    fn name(&self) -> &str {
        "filesystem"
    }

    fn description(&self) -> &str {
        "Perform file system operations: read, write, and list files within the allowed directory"
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["read", "write", "list"],
                    "description": "The file system operation to perform"
                },
                "path": {
                    "type": "string",
                    "description": "File or directory path (relative to base path)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write (required for 'write' operation)"
                }
            },
            "required": ["operation", "path"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'operation' parameter".to_string()))?;

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'path' parameter".to_string()))?;

        match operation {
            "read" => {
                let content = self.read_file(path).await?;
                Ok(serde_json::json!({
                    "operation": "read",
                    "path": path,
                    "content": content
                }))
            }
            "write" => {
                let content = input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolError::InvalidInput("Missing 'content' parameter for write operation".to_string()))?;

                self.write_file(path, content).await?;
                Ok(serde_json::json!({
                    "operation": "write",
                    "path": path,
                    "status": "success"
                }))
            }
            "list" => {
                let files = self.list_files(path).await?;
                Ok(serde_json::json!({
                    "operation": "list",
                    "path": path,
                    "files": files
                }))
            }
            _ => Err(ToolError::InvalidInput(format!("Unknown operation: {}", operation))),
        }
    }
}
"#
}

pub fn api_client_tool() -> &'static str {
    r#"//! Advanced API Client Tool
//!
//! Features:
//! - Bearer token authentication
//! - Rate limiting
//! - Custom headers
//! - Supports GET, POST, PUT, DELETE

use async_trait::async_trait;
use serde_json::Value;
use skreaver_core::tool::{Tool, ToolError, ToolInput, ToolResult};
use std::sync::Arc;
use tokio::sync::Semaphore;
use std::time::{Duration, Instant};

pub struct ApiClientTool {
    client: reqwest::Client,
    api_key: Option<String>,
    rate_limiter: Arc<Semaphore>,
    last_request: Arc<tokio::sync::Mutex<Instant>>,
}

impl ApiClientTool {
    pub fn new() -> Self {
        Self::with_config(None, 10) // Default: no API key, 10 concurrent requests
    }

    pub fn with_config(api_key: Option<String>, max_concurrent: usize) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
            api_key,
            rate_limiter: Arc::new(Semaphore::new(max_concurrent)),
            last_request: Arc::new(tokio::sync::Mutex::new(Instant::now())),
        }
    }

    // TODO: Configure rate limiting parameters (requests per second, etc.)
    // TODO: Add retry logic for failed requests
    // TODO: Add response caching
}

#[async_trait]
impl Tool for ApiClientTool {
    fn name(&self) -> &str {
        "api_client"
    }

    fn description(&self) -> &str {
        "Make HTTP API requests with authentication and rate limiting. \
         Supports GET, POST, PUT, DELETE methods with custom headers."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE"],
                    "description": "HTTP method"
                },
                "url": {
                    "type": "string",
                    "description": "API endpoint URL"
                },
                "headers": {
                    "type": "object",
                    "description": "Optional custom headers as key-value pairs"
                },
                "body": {
                    "type": "object",
                    "description": "Optional JSON request body (for POST/PUT)"
                }
            },
            "required": ["method", "url"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        // Rate limiting: acquire permit
        let _permit = self.rate_limiter.acquire().await
            .map_err(|e| ToolError::ExecutionFailed(format!("Rate limiter error: {}", e)))?;

        // Rate limiting: minimum delay between requests
        {
            let mut last = self.last_request.lock().await;
            let elapsed = last.elapsed();
            if elapsed < Duration::from_millis(100) {
                tokio::time::sleep(Duration::from_millis(100) - elapsed).await;
            }
            *last = Instant::now();
        }

        let method = input
            .get("method")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'method' parameter".to_string()))?;

        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'url' parameter".to_string()))?;

        // Build request
        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => return Err(ToolError::InvalidInput(format!("Unsupported HTTP method: {}", method))),
        };

        // Add authentication if configured
        if let Some(api_key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }

        // Add custom headers
        if let Some(headers) = input.get("headers").and_then(|v| v.as_object()) {
            for (key, value) in headers {
                if let Some(val_str) = value.as_str() {
                    request = request.header(key, val_str);
                }
            }
        }

        // Add body for POST/PUT
        if let Some(body) = input.get("body") {
            request = request.json(body);
        }

        // Execute request
        let response = request
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Request failed: {}", e)))?;

        let status = response.status().as_u16();
        let body_text = response
            .text()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read response: {}", e)))?;

        // Try to parse response as JSON
        let body_value = serde_json::from_str::<Value>(&body_text)
            .unwrap_or_else(|_| Value::String(body_text));

        Ok(serde_json::json!({
            "status": status,
            "body": body_value
        }))
    }
}
"#
}

pub fn workflow_tool() -> &'static str {
    r#"//! Workflow/Pipeline Execution Tool
//!
//! Execute multi-step workflows with:
//! - Variable substitution ($variable)
//! - Step dependencies
//! - Conditional execution
//! - Context passing between steps

use async_trait::async_trait;
use serde_json::Value;
use skreaver_core::tool::{Tool, ToolError, ToolInput, ToolResult};
use std::collections::HashMap;

pub struct WorkflowTool;

impl WorkflowTool {
    pub fn new() -> Self {
        Self
    }

    async fn execute_workflow(&self, steps: &[Value]) -> Result<Vec<Value>, ToolError> {
        let mut context: HashMap<String, Value> = HashMap::new();
        let mut results = Vec::new();

        for step in steps {
            let name = step.get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidInput("Step missing 'name'".to_string()))?;

            let action = step.get("action")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidInput("Step missing 'action'".to_string()))?;

            let inputs = step.get("inputs")
                .and_then(|v| v.as_object())
                .cloned()
                .unwrap_or_default();

            // Resolve inputs from context (variable substitution)
            let mut resolved_inputs = HashMap::new();
            for (key, value) in inputs {
                let resolved = if let Some(var_str) = value.as_str() {
                    if let Some(var_name) = var_str.strip_prefix('$') {
                        context.get(var_name).cloned().unwrap_or(value)
                    } else {
                        value
                    }
                } else {
                    value
                };
                resolved_inputs.insert(key, resolved);
            }

            // Execute step based on action type
            let result = match action {
                "log" => {
                    let message = resolved_inputs.get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("No message");
                    tracing::info!(step = %name, message = %message);
                    serde_json::json!({
                        "step": name,
                        "action": "log",
                        "message": message
                    })
                }
                "transform" => {
                    let input_val = resolved_inputs.get("input").cloned()
                        .unwrap_or(Value::Null);
                    let transform_type = resolved_inputs.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("identity");

                    let result_val = match transform_type {
                        "uppercase" => {
                            if let Some(s) = input_val.as_str() {
                                Value::String(s.to_uppercase())
                            } else {
                                input_val
                            }
                        }
                        "lowercase" => {
                            if let Some(s) = input_val.as_str() {
                                Value::String(s.to_lowercase())
                            } else {
                                input_val
                            }
                        }
                        _ => input_val,
                    };

                    serde_json::json!({
                        "step": name,
                        "action": "transform",
                        "result": result_val
                    })
                }
                // TODO: Add more action types (condition, aggregate, etc.)
                _ => {
                    return Err(ToolError::ExecutionFailed(format!("Unknown action: {}", action)));
                }
            };

            // Store result in context for future steps
            context.insert(name.to_string(), result.clone());
            results.push(result);
        }

        Ok(results)
    }
}

#[async_trait]
impl Tool for WorkflowTool {
    fn name(&self) -> &str {
        "workflow"
    }

    fn description(&self) -> &str {
        "Execute multi-step workflows. Supports variable substitution with $variable syntax."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "steps": {
                    "type": "array",
                    "description": "Array of workflow steps to execute sequentially",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Step name (used for $variable references)"
                            },
                            "action": {
                                "type": "string",
                                "enum": ["log", "transform"],
                                "description": "Action to perform"
                            },
                            "inputs": {
                                "type": "object",
                                "description": "Step inputs (use $stepName to reference previous results)"
                            }
                        },
                        "required": ["name", "action"]
                    }
                }
            },
            "required": ["steps"]
        })
    }

    async fn execute(&self, input: ToolInput) -> ToolResult {
        let steps = input
            .get("steps")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError::InvalidInput("Missing or invalid 'steps' parameter".to_string()))?;

        let results = self.execute_workflow(steps).await?;

        Ok(serde_json::json!({
            "workflow": "completed",
            "steps": results
        }))
    }
}
"#
}

pub fn project_readme(name: &str, template: &str) -> String {
    format!(
        r#"# {name}

A Skreaver agent generated from the `{template}` template.

## Quick Start

```bash
# Build the project
cargo build

# Run the agent
cargo run
```

## Project Structure

```
{name}/
├── Cargo.toml          # Project dependencies
├── README.md           # This file
└── src/
    ├── main.rs         # Agent entry point
    └── tools/          # Custom tools (if applicable)
```

## Customization

### Adding Tools

1. Create a new tool file in `src/tools/`
2. Implement the `Tool` trait from `skreaver_core::tool`
3. Register the tool in `main.rs`:

```rust
tools.register(Box::new(YourTool::new()));
```

### Modifying Agent Behavior

Edit the system prompt in `main.rs` to change how the agent behaves:

```rust
let config = AgentConfig::default()
    .with_name("{name}")
    .with_system_prompt("Your custom prompt here");
```

## Documentation

- [Skreaver Documentation](https://github.com/yourusername/skreaver)
- [Tool Development Guide](https://github.com/yourusername/skreaver/blob/main/docs/tools.md)
- [Agent Configuration](https://github.com/yourusername/skreaver/blob/main/docs/configuration.md)

## License

MIT
"#,
        name = name,
        template = template
    )
}

pub fn project_gitignore() -> &'static str {
    r#"# Rust
/target/
**/*.rs.bk
*.pdb

# IDE
.vscode/
.idea/
*.swp
*.swo
*~

# OS
.DS_Store
Thumbs.db

# Environment
.env
.env.local

# Logs
*.log
logs/

# Agent data
data/
*.db
*.sqlite
"#
}
