//! Code templates for agent and tool generation

use std::fmt;
use std::str::FromStr;
use super::loader::TemplateLoader;

/// Agent template types
#[derive(Debug, Clone, Copy)]
pub enum AgentTemplate {
    Simple,
    Reasoning,
    MultiTool,
}

impl FromStr for AgentTemplate {
    type Err = super::ScaffoldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "simple" => Ok(Self::Simple),
            "reasoning" => Ok(Self::Reasoning),
            "multi-tool" | "multi" => Ok(Self::MultiTool),
            _ => Err(super::ScaffoldError::UnknownTemplate(s.to_string())),
        }
    }
}

impl AgentTemplate {

    /// Get the template identifier string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Reasoning => "reasoning",
            Self::MultiTool => "multi-tool",
        }
    }
}

impl fmt::Display for AgentTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
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
    Calculator,
}

impl FromStr for ToolTemplate {
    type Err = super::ScaffoldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "http-client" | "http" => Ok(Self::HttpClient),
            "database" | "db" => Ok(Self::Database),
            "custom" => Ok(Self::Custom),
            "filesystem" | "fs" | "file" => Ok(Self::FileSystem),
            "api-client" | "api" => Ok(Self::ApiClient),
            "workflow" | "pipeline" => Ok(Self::Workflow),
            "calculator" | "calc" => Ok(Self::Calculator),
            _ => Err(super::ScaffoldError::UnknownTemplate(s.to_string())),
        }
    }
}

impl ToolTemplate {

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
            ("calculator", "Basic arithmetic calculator"),
            ("custom", "Empty custom tool template"),
        ]
    }

    /// Get the template identifier string
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HttpClient => "http_client",
            Self::Database => "database",
            Self::Custom => "custom",
            Self::FileSystem => "filesystem",
            Self::ApiClient => "api_client",
            Self::Workflow => "workflow",
            Self::Calculator => "calculator",
        }
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
            Self::Calculator => write!(f, "calculator"),
        }
    }
}

// Template rendering functions

/// Render Cargo.toml for simple agent
pub fn simple_agent_cargo_toml(name: &str) -> String {
    TemplateLoader::global()
        .render_agent_cargo("simple", name)
        .expect("Failed to render simple agent Cargo.toml")
}

/// Render Cargo.toml for reasoning agent
pub fn reasoning_agent_cargo_toml(name: &str) -> String {
    TemplateLoader::global()
        .render_agent_cargo("reasoning", name)
        .expect("Failed to render reasoning agent Cargo.toml")
}

/// Render Cargo.toml for multi-tool agent
pub fn multi_tool_agent_cargo_toml(name: &str) -> String {
    TemplateLoader::global()
        .render_agent_cargo("multi-tool", name)
        .expect("Failed to render multi-tool agent Cargo.toml")
}

/// Render main.rs for simple agent
pub fn simple_agent_main(name: &str) -> String {
    TemplateLoader::global()
        .render_agent_main("simple", name)
        .expect("Failed to render simple agent main.rs")
}

/// Render main.rs for reasoning agent
pub fn reasoning_agent_main(name: &str) -> String {
    TemplateLoader::global()
        .render_agent_main("reasoning", name)
        .expect("Failed to render reasoning agent main.rs")
}

/// Render main.rs for multi-tool agent
pub fn multi_tool_agent_main(name: &str) -> String {
    TemplateLoader::global()
        .render_agent_main("multi-tool", name)
        .expect("Failed to render multi-tool agent main.rs")
}

/// Render tools/mod.rs for reasoning agent
pub fn reasoning_tools_mod() -> String {
    TemplateLoader::global()
        .render_tools_mod()
        .expect("Failed to render tools/mod.rs")
}

/// Render HTTP client tool
pub fn http_client_tool() -> String {
    TemplateLoader::global()
        .render_tool("http_client")
        .expect("Failed to render HTTP client tool")
}

/// Render database tool
pub fn database_tool() -> String {
    TemplateLoader::global()
        .render_tool("database")
        .expect("Failed to render database tool")
}

/// Render custom tool template
pub fn custom_tool() -> String {
    TemplateLoader::global()
        .render_tool("custom")
        .expect("Failed to render custom tool")
}

/// Render calculator tool
pub fn calculator_tool() -> String {
    TemplateLoader::global()
        .render_tool("calculator")
        .expect("Failed to render calculator tool")
}

/// Render filesystem tool
pub fn filesystem_tool() -> String {
    TemplateLoader::global()
        .render_tool("filesystem")
        .expect("Failed to render filesystem tool")
}

/// Render API client tool
pub fn api_client_tool() -> String {
    TemplateLoader::global()
        .render_tool("api_client")
        .expect("Failed to render API client tool")
}

/// Render workflow tool
pub fn workflow_tool() -> String {
    TemplateLoader::global()
        .render_tool("workflow")
        .expect("Failed to render workflow tool")
}

/// Render project README
pub fn project_readme(name: &str, template: &str) -> String {
    TemplateLoader::global()
        .render_readme(name, template)
        .expect("Failed to render README.md")
}

/// Render project .gitignore
pub fn project_gitignore() -> String {
    TemplateLoader::global()
        .render_gitignore()
        .expect("Failed to render .gitignore")
}
