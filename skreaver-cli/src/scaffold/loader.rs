//! Template loader and renderer using Handlebars

use handlebars::Handlebars;
use serde_json::json;
use std::sync::OnceLock;

/// Global template loader instance
static TEMPLATE_LOADER: OnceLock<TemplateLoader> = OnceLock::new();

/// Template loader error types
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("Template registration failed: {0}")]
    RegistrationFailed(#[from] handlebars::TemplateError),

    #[error("Template rendering failed: {0}")]
    RenderingFailed(#[from] handlebars::RenderError),

    #[error("Template not found: {0}")]
    #[allow(dead_code)]
    NotFound(String),
}

/// Template loader with embedded Handlebars templates
pub struct TemplateLoader {
    handlebars: Handlebars<'static>,
}

impl TemplateLoader {
    /// Create a new template loader with all templates registered
    pub fn new() -> Result<Self, TemplateError> {
        let mut hbs = Handlebars::new();

        // Configure Handlebars
        hbs.set_strict_mode(true);

        // Register agent templates
        hbs.register_template_string(
            "agents/simple/cargo",
            include_str!("../../templates/agents/simple/Cargo.toml.hbs"),
        )?;
        hbs.register_template_string(
            "agents/simple/main",
            include_str!("../../templates/agents/simple/src/main.rs.hbs"),
        )?;

        hbs.register_template_string(
            "agents/reasoning/cargo",
            include_str!("../../templates/agents/reasoning/Cargo.toml.hbs"),
        )?;
        hbs.register_template_string(
            "agents/reasoning/main",
            include_str!("../../templates/agents/reasoning/src/main.rs.hbs"),
        )?;
        hbs.register_template_string(
            "agents/reasoning/tools_mod",
            include_str!("../../templates/agents/reasoning/src/tools/mod.rs.hbs"),
        )?;

        hbs.register_template_string(
            "agents/multi-tool/cargo",
            include_str!("../../templates/agents/multi-tool/Cargo.toml.hbs"),
        )?;
        hbs.register_template_string(
            "agents/multi-tool/main",
            include_str!("../../templates/agents/multi-tool/src/main.rs.hbs"),
        )?;

        // Register tool templates
        hbs.register_template_string(
            "tools/http_client",
            include_str!("../../templates/tools/http_client.rs.hbs"),
        )?;
        hbs.register_template_string(
            "tools/database",
            include_str!("../../templates/tools/database.rs.hbs"),
        )?;
        hbs.register_template_string(
            "tools/custom",
            include_str!("../../templates/tools/custom.rs.hbs"),
        )?;
        hbs.register_template_string(
            "tools/calculator",
            include_str!("../../templates/tools/calculator.rs.hbs"),
        )?;
        hbs.register_template_string(
            "tools/filesystem",
            include_str!("../../templates/tools/filesystem.rs.hbs"),
        )?;
        hbs.register_template_string(
            "tools/api_client",
            include_str!("../../templates/tools/api_client.rs.hbs"),
        )?;
        hbs.register_template_string(
            "tools/workflow",
            include_str!("../../templates/tools/workflow.rs.hbs"),
        )?;

        // Register project templates
        hbs.register_template_string(
            "project/readme",
            include_str!("../../templates/project/README.md.hbs"),
        )?;
        hbs.register_template_string(
            "project/gitignore",
            include_str!("../../templates/project/.gitignore"),
        )?;

        Ok(Self { handlebars: hbs })
    }

    /// Get the global template loader instance
    pub fn global() -> &'static Self {
        TEMPLATE_LOADER.get_or_init(|| Self::new().expect("Failed to initialize template loader"))
    }

    /// Render a template with the given name and data
    pub fn render(&self, name: &str, data: &serde_json::Value) -> Result<String, TemplateError> {
        self.handlebars
            .render(name, data)
            .map_err(TemplateError::from)
    }

    /// Render agent Cargo.toml
    pub fn render_agent_cargo(
        &self,
        agent_type: &str,
        name: &str,
    ) -> Result<String, TemplateError> {
        let template_name = format!("agents/{}/cargo", agent_type);
        self.render(&template_name, &json!({ "name": name }))
    }

    /// Render agent main.rs
    pub fn render_agent_main(&self, agent_type: &str, name: &str) -> Result<String, TemplateError> {
        let template_name = format!("agents/{}/main", agent_type);
        self.render(&template_name, &json!({ "name": name }))
    }

    /// Render reasoning agent tools/mod.rs
    pub fn render_tools_mod(&self) -> Result<String, TemplateError> {
        self.render("agents/reasoning/tools_mod", &json!({}))
    }

    /// Render tool template
    pub fn render_tool(&self, tool_type: &str) -> Result<String, TemplateError> {
        let template_name = format!("tools/{}", tool_type);
        self.render(&template_name, &json!({}))
    }

    /// Render project README
    pub fn render_readme(&self, name: &str, template: &str) -> Result<String, TemplateError> {
        self.render(
            "project/readme",
            &json!({
                "name": name,
                "template": template
            }),
        )
    }

    /// Render project .gitignore
    pub fn render_gitignore(&self) -> Result<String, TemplateError> {
        self.render("project/gitignore", &json!({}))
    }
}

impl Default for TemplateLoader {
    fn default() -> Self {
        Self::new().expect("Failed to initialize template loader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loader_initialization() {
        let loader = TemplateLoader::new();
        assert!(loader.is_ok());
    }

    #[test]
    fn test_render_simple_agent_cargo() {
        let loader = TemplateLoader::new().unwrap();
        let result = loader.render_agent_cargo("simple", "test-agent");
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("test-agent"));
        assert!(content.contains("[package]"));
    }

    #[test]
    fn test_render_simple_agent_main() {
        let loader = TemplateLoader::new().unwrap();
        let result = loader.render_agent_main("simple", "test-agent");
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("test-agent"));
        assert!(content.contains("async fn main"));
    }

    #[test]
    fn test_render_tool() {
        let loader = TemplateLoader::new().unwrap();
        let result = loader.render_tool("calculator");
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("CalculatorTool"));
    }

    #[test]
    fn test_render_readme() {
        let loader = TemplateLoader::new().unwrap();
        let result = loader.render_readme("my-agent", "simple");
        assert!(result.is_ok());
        let content = result.unwrap();
        assert!(content.contains("my-agent"));
        assert!(content.contains("simple"));
    }

    #[test]
    fn test_global_instance() {
        let loader1 = TemplateLoader::global();
        let loader2 = TemplateLoader::global();
        // Both should point to the same instance
        assert!(std::ptr::eq(loader1, loader2));
    }
}
