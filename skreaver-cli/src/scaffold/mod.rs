//! Agent and tool scaffolding
//!
//! This module provides code generation for creating new agents and tools
//! with best-practice templates.

use std::fs;
use std::path::{Path, PathBuf};

pub mod loader;
pub mod templates;

pub use templates::{AgentTemplate, ToolTemplate};

/// List available templates
pub fn list_templates(category: &str) {
    match category.to_lowercase().as_str() {
        "tools" => {
            println!("📦 Available Tool Templates:\n");
            for (name, description) in ToolTemplate::all() {
                println!("  • {:<15} - {}", name, description);
            }
            println!("\nUsage:");
            println!("  skreaver generate --type tool --template <template> --output <path>");
            println!("\nExample:");
            println!(
                "  skreaver generate --type tool --template http-client --output src/tools/http.rs"
            );
        }
        "agents" => {
            println!("🤖 Available Agent Templates:\n");
            println!("  • simple        - Basic agent with minimal configuration");
            println!("  • reasoning     - Agent with reasoning capabilities and tool support");
            println!("  • multi-tool    - Agent with multiple pre-configured tools");
            println!("\nUsage:");
            println!("  skreaver new --name <name> --template <template>");
            println!("\nExample:");
            println!("  skreaver new --name my-agent --template reasoning");
        }
        _ => {
            println!("Unknown category: {}", category);
            println!("Available categories: tools, agents");
        }
    }
}

/// Generate a new agent from template
pub fn generate_agent(
    name: &str,
    template: &str,
    output_dir: Option<&str>,
) -> Result<(), ScaffoldError> {
    let template = template.parse::<AgentTemplate>()?;
    let output_path = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // Create agent directory
    let agent_dir = output_path.join(name);
    fs::create_dir_all(&agent_dir)?;

    // Generate files based on template
    match template {
        AgentTemplate::Simple => generate_simple_agent(&agent_dir, name)?,
        AgentTemplate::Reasoning => generate_reasoning_agent(&agent_dir, name)?,
        AgentTemplate::MultiTool => generate_multi_tool_agent(&agent_dir, name)?,
    }

    println!("✅ Generated {} agent: {}", template, agent_dir.display());
    println!("\nNext steps:");
    println!("  cd {}", name);
    println!("  cargo build");
    println!("  cargo run");

    Ok(())
}

/// Generate a new tool from template
pub fn generate_tool(_tool_type: &str, template: &str, output: &str) -> Result<(), ScaffoldError> {
    let template = template.parse::<ToolTemplate>()?;
    let output_path = PathBuf::from(output);

    // Create output directory
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Generate tool file
    let content = match template {
        ToolTemplate::HttpClient => templates::http_client_tool(),
        ToolTemplate::Database => templates::database_tool(),
        ToolTemplate::Custom => templates::custom_tool(),
        ToolTemplate::FileSystem => templates::filesystem_tool(),
        ToolTemplate::ApiClient => templates::api_client_tool(),
        ToolTemplate::Workflow => templates::workflow_tool(),
        ToolTemplate::Calculator => templates::calculator_tool(),
    };

    fs::write(&output_path, content)?;

    println!("✅ Generated {} tool: {}", template, output_path.display());
    println!("\nNext steps:");
    println!("  1. Review the generated tool code");
    println!("  2. Implement any TODOs marked in the code");
    println!("  3. Add the tool to your agent's ToolRegistry");
    println!("\nExample usage:");
    println!("  let tool = YourTool::new();");
    println!("  tools.register(Box::new(tool));");

    Ok(())
}

fn generate_simple_agent(dir: &Path, name: &str) -> Result<(), ScaffoldError> {
    // Cargo.toml
    fs::write(
        dir.join("Cargo.toml"),
        templates::simple_agent_cargo_toml(name),
    )?;

    // src/main.rs
    fs::create_dir_all(dir.join("src"))?;
    fs::write(dir.join("src/main.rs"), templates::simple_agent_main(name))?;

    // README.md
    fs::write(
        dir.join("README.md"),
        templates::project_readme(name, "simple"),
    )?;

    // .gitignore
    fs::write(dir.join(".gitignore"), templates::project_gitignore())?;

    Ok(())
}

fn generate_reasoning_agent(dir: &Path, name: &str) -> Result<(), ScaffoldError> {
    // Cargo.toml
    fs::write(
        dir.join("Cargo.toml"),
        templates::reasoning_agent_cargo_toml(name),
    )?;

    // src/main.rs
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("src/main.rs"),
        templates::reasoning_agent_main(name),
    )?;

    // Create tools directory
    fs::create_dir_all(dir.join("src/tools"))?;
    fs::write(
        dir.join("src/tools/mod.rs"),
        templates::reasoning_tools_mod(),
    )?;

    // README.md
    fs::write(
        dir.join("README.md"),
        templates::project_readme(name, "reasoning"),
    )?;

    // .gitignore
    fs::write(dir.join(".gitignore"), templates::project_gitignore())?;

    Ok(())
}

fn generate_multi_tool_agent(dir: &Path, name: &str) -> Result<(), ScaffoldError> {
    // Cargo.toml
    fs::write(
        dir.join("Cargo.toml"),
        templates::multi_tool_agent_cargo_toml(name),
    )?;

    // src/main.rs
    fs::create_dir_all(dir.join("src"))?;
    fs::write(
        dir.join("src/main.rs"),
        templates::multi_tool_agent_main(name),
    )?;

    // Create tools directory with examples
    fs::create_dir_all(dir.join("src/tools"))?;
    fs::write(
        dir.join("src/tools/mod.rs"),
        "pub mod http_client;\npub mod calculator;\n",
    )?;
    fs::write(
        dir.join("src/tools/http_client.rs"),
        templates::http_client_tool(),
    )?;
    fs::write(
        dir.join("src/tools/calculator.rs"),
        templates::calculator_tool(),
    )?;

    // README.md
    fs::write(
        dir.join("README.md"),
        templates::project_readme(name, "multi-tool"),
    )?;

    // .gitignore
    fs::write(dir.join(".gitignore"), templates::project_gitignore())?;

    Ok(())
}

/// Scaffolding errors
#[derive(Debug, thiserror::Error)]
pub enum ScaffoldError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unknown template: {0}")]
    UnknownTemplate(String),
}
