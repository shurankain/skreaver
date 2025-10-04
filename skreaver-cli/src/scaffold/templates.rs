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
}

impl ToolTemplate {
    pub fn from_str(s: &str) -> Result<Self, super::ScaffoldError> {
        match s.to_lowercase().as_str() {
            "http-client" | "http" => Ok(Self::HttpClient),
            "database" | "db" => Ok(Self::Database),
            "custom" => Ok(Self::Custom),
            _ => Err(super::ScaffoldError::UnknownTemplate(s.to_string())),
        }
    }
}

impl fmt::Display for ToolTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpClient => write!(f, "http-client"),
            Self::Database => write!(f, "database"),
            Self::Custom => write!(f, "custom"),
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
