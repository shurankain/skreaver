//! Concrete agent builders for different agent types
//!
//! This module provides ready-to-use agent builders for the standard
//! agent types supported by the platform.

#![allow(clippy::collapsible_if)]

use crate::runtime::agent_error::{AgentBuildError, ConfigExt};
use serde_json::Value;
use std::collections::HashMap;

use skreaver_core::memory::{MemoryKeys, MemoryReader, MemoryWriter};
use skreaver_core::{Agent, ExecutionResult, InMemoryMemory, MemoryUpdate, Tool, ToolCall};
use skreaver_tools::InMemoryToolRegistry;
use std::sync::Arc;

use crate::runtime::{
    agent_factory::{AgentBuilder, AgentFactoryError},
    agent_instance::CoordinatorTrait,
    api_types::{AgentSpec, AgentType},
    coordinator::Coordinator,
};

/// Simple mock tool for testing
struct MockTool {
    name: String,
}

impl MockTool {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Tool for MockTool {
    fn name(&self) -> &str {
        &self.name
    }

    fn call(&self, input: String) -> ExecutionResult {
        ExecutionResult::success(format!("[{}] processed: {}", self.name, input))
    }
}

/// Echo agent implementation - simple agent that echoes input
pub struct EchoAgent {
    memory: InMemoryMemory,
    last_input: Option<String>,
}

impl EchoAgent {
    pub fn new(_config: HashMap<String, Value>) -> Result<Self, AgentBuildError> {
        Ok(Self {
            memory: InMemoryMemory::default(),
            last_input: None,
        })
    }
}

impl Agent for EchoAgent {
    type Observation = String;
    type Action = String;
    type Error = std::convert::Infallible;

    fn observe(&mut self, input: Self::Observation) {
        self.last_input = Some(input.clone());
        let update = MemoryUpdate::from_validated(MemoryKeys::last_input(), input);
        if let Err(e) = self.memory_writer().store(update) {
            tracing::warn!(error = %e, "Failed to store last_input to memory");
        }
    }

    fn act(&mut self) -> Self::Action {
        match &self.last_input {
            Some(input) => format!("Echo: {}", input),
            None => "Echo: (no input received)".to_string(),
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        // Echo agent doesn't use tools by default
        Vec::new()
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        // Echo agent can optionally modify output based on tool results
        if result.is_success() {
            let update = MemoryUpdate::from_validated(
                MemoryKeys::last_tool_result(),
                result.output().to_string(),
            );
            if let Err(e) = self.memory_writer().store(update) {
                tracing::warn!(error = %e, "Failed to store last_tool_result to memory");
            }
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let key = update.key.clone();
        if let Err(e) = self.memory_writer().store(update) {
            tracing::warn!(error = %e, key = %key, "Failed to store context update to memory");
        }
    }

    fn memory_reader(&self) -> &dyn MemoryReader {
        &self.memory
    }

    fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
        &mut self.memory
    }
}

/// Advanced processing agent with tool capabilities
pub struct AdvancedAgent {
    memory: InMemoryMemory,
    context: String,
    processing_mode: ProcessingMode,
    use_tools: bool,
}

#[derive(Debug, Clone)]
enum ProcessingMode {
    Simple,
    Analytical,
    Creative,
}

impl AdvancedAgent {
    pub fn new(config: HashMap<String, Value>) -> Result<Self, AgentBuildError> {
        // Use the ConfigExt trait for type-safe config extraction
        let mode_str = config.get_string_or("mode", "simple");

        let processing_mode = match mode_str.as_str() {
            "analytical" => ProcessingMode::Analytical,
            "creative" => ProcessingMode::Creative,
            "simple" => ProcessingMode::Simple,
            other => {
                return Err(AgentBuildError::invalid_mode(
                    other,
                    vec!["simple".into(), "analytical".into(), "creative".into()],
                ));
            }
        };

        let use_tools = config.get_bool_or("use_tools", true);

        Ok(Self {
            memory: InMemoryMemory::default(),
            context: String::new(),
            processing_mode,
            use_tools,
        })
    }
}

impl Agent for AdvancedAgent {
    type Observation = String;
    type Action = String;
    type Error = std::convert::Infallible;

    fn observe(&mut self, input: Self::Observation) {
        self.context = input.clone();
        let update = MemoryUpdate::from_validated(MemoryKeys::context(), input);
        if let Err(e) = self.memory_writer().store(update) {
            tracing::warn!(error = %e, "Failed to store context to memory");
        }
    }

    fn act(&mut self) -> Self::Action {
        match self.processing_mode {
            ProcessingMode::Simple => {
                format!("Processed: {}", self.context)
            }
            ProcessingMode::Analytical => {
                format!(
                    "Analysis: Based on the input '{}', I observe {} patterns and {} key themes.",
                    self.context,
                    self.context.split_whitespace().count(),
                    self.context.chars().filter(|c| c.is_uppercase()).count()
                )
            }
            ProcessingMode::Creative => {
                format!(
                    "Creative response: '{}' reminds me of a story where challenges become opportunities for growth.",
                    self.context
                )
            }
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if !self.use_tools || self.context.is_empty() {
            return Vec::new();
        }

        match self.processing_mode {
            ProcessingMode::Analytical => {
                vec![
                    ToolCall::new("analyze_text", &self.context).expect("Valid tool call"),
                    ToolCall::new("count_words", &self.context).expect("Valid tool call"),
                ]
            }
            ProcessingMode::Creative => {
                vec![ToolCall::new("generate_ideas", &self.context).expect("Valid tool call")]
            }
            _ => Vec::new(),
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.is_success() {
            self.context
                .push_str(&format!(" [Tool result: {}]", result.output()));
            let update =
                MemoryUpdate::from_validated(MemoryKeys::enriched_context(), self.context.clone());
            if let Err(e) = self.memory_writer().store(update) {
                tracing::warn!(error = %e, "Failed to store enriched_context to memory");
            }
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let key = update.key.clone();
        if let Err(e) = self.memory_writer().store(update) {
            tracing::warn!(error = %e, key = %key, "Failed to store context update to memory");
        }
    }

    fn memory_reader(&self) -> &dyn MemoryReader {
        &self.memory
    }

    fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
        &mut self.memory
    }
}

/// Analytics agent for data analysis tasks
pub struct AnalyticsAgent {
    memory: InMemoryMemory,
    data: Vec<String>,
    analysis_depth: AnalysisDepth,
}

#[derive(Debug, Clone)]
enum AnalysisDepth {
    Basic,
    Detailed,
    Comprehensive,
}

impl AnalyticsAgent {
    pub fn new(config: HashMap<String, Value>) -> Result<Self, AgentBuildError> {
        let analysis_depth = match config.get("depth").and_then(|v| v.as_str()) {
            Some("detailed") => AnalysisDepth::Detailed,
            Some("comprehensive") => AnalysisDepth::Comprehensive,
            _ => AnalysisDepth::Basic,
        };

        Ok(Self {
            memory: InMemoryMemory::default(),
            data: Vec::new(),
            analysis_depth,
        })
    }
}

impl Agent for AnalyticsAgent {
    type Observation = String;
    type Action = String;
    type Error = std::convert::Infallible;

    fn observe(&mut self, input: Self::Observation) {
        self.data.push(input.clone());
        let update = MemoryUpdate::from_validated(MemoryKeys::latest_data(), input);
        if let Err(e) = self.memory_writer().store(update) {
            tracing::warn!(error = %e, "Failed to store latest_data to memory");
        }
    }

    fn act(&mut self) -> Self::Action {
        match self.analysis_depth {
            AnalysisDepth::Basic => {
                format!(
                    "Analytics: Processed {} data points. Latest: {}",
                    self.data.len(),
                    self.data.last().unwrap_or(&"None".to_string())
                )
            }
            AnalysisDepth::Detailed => {
                let total_chars: usize = self.data.iter().map(|s| s.len()).sum();
                let avg_length = if !self.data.is_empty() {
                    total_chars / self.data.len()
                } else {
                    0
                };
                format!(
                    "Detailed Analytics: {} data points, {} total characters, {} average length per point",
                    self.data.len(),
                    total_chars,
                    avg_length
                )
            }
            AnalysisDepth::Comprehensive => {
                let word_count: usize =
                    self.data.iter().map(|s| s.split_whitespace().count()).sum();
                let unique_words: std::collections::HashSet<&str> = self
                    .data
                    .iter()
                    .flat_map(|s| s.split_whitespace())
                    .collect();
                format!(
                    "Comprehensive Analytics: {} data points, {} total words, {} unique words, vocabulary richness: {:.2}",
                    self.data.len(),
                    word_count,
                    unique_words.len(),
                    if word_count > 0 {
                        unique_words.len() as f32 / word_count as f32
                    } else {
                        0.0
                    }
                )
            }
        }
    }

    fn call_tools(&self) -> Vec<ToolCall> {
        if self.data.is_empty() {
            return Vec::new();
        }

        let latest_data = self.data.last().unwrap();
        match self.analysis_depth {
            AnalysisDepth::Basic => Vec::new(),
            AnalysisDepth::Detailed => {
                vec![ToolCall::new("statistical_analysis", latest_data).expect("Valid tool call")]
            }
            AnalysisDepth::Comprehensive => {
                vec![
                    ToolCall::new("statistical_analysis", latest_data).expect("Valid tool call"),
                    ToolCall::new("pattern_detection", latest_data).expect("Valid tool call"),
                    ToolCall::new("trend_analysis", latest_data).expect("Valid tool call"),
                ]
            }
        }
    }

    fn handle_result(&mut self, result: ExecutionResult) {
        if result.is_success() {
            let update = MemoryUpdate::from_validated(
                MemoryKeys::analysis_results(),
                result.output().to_string(),
            );
            if let Err(e) = self.memory_writer().store(update) {
                tracing::warn!(error = %e, "Failed to store analysis_results to memory");
            }
        }
    }

    fn update_context(&mut self, update: MemoryUpdate) {
        let key = update.key.clone();
        if let Err(e) = self.memory_writer().store(update) {
            tracing::warn!(error = %e, key = %key, "Failed to store context update to memory");
        }
    }

    fn memory_reader(&self) -> &dyn MemoryReader {
        &self.memory
    }

    fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
        &mut self.memory
    }
}

// Coordinator wrappers that implement CoordinatorTrait
pub struct EchoCoordinator {
    coordinator: Coordinator<EchoAgent, InMemoryToolRegistry>,
}

impl EchoCoordinator {
    pub fn new(config: HashMap<String, Value>) -> Result<Self, AgentBuildError> {
        let mut agent = EchoAgent::new(config)?;

        // Initialize the agent before use
        agent
            .initialize()
            .map_err(|e| AgentBuildError::ValidationFailed {
                what: "agent initialization".to_string(),
                reason: e.to_string(),
            })?;

        let registry = InMemoryToolRegistry::new();
        Ok(Self {
            coordinator: Coordinator::new(agent, registry),
        })
    }
}

impl CoordinatorTrait for EchoCoordinator {
    fn step(&mut self, input: String) -> String {
        self.coordinator.step(input)
    }

    fn get_agent_type(&self) -> &'static str {
        "EchoAgent"
    }
}

impl Drop for EchoCoordinator {
    fn drop(&mut self) {
        // Call cleanup on the agent
        if let Err(e) = self.coordinator.agent.cleanup() {
            tracing::warn!("Agent cleanup failed for EchoAgent: {}", e);
        }
    }
}

pub struct AdvancedCoordinator {
    coordinator: Coordinator<AdvancedAgent, InMemoryToolRegistry>,
}

impl AdvancedCoordinator {
    pub fn new(config: HashMap<String, Value>) -> Result<Self, AgentBuildError> {
        let mut agent = AdvancedAgent::new(config)?;

        // Initialize the agent before use
        agent
            .initialize()
            .map_err(|e| AgentBuildError::ValidationFailed {
                what: "agent initialization".to_string(),
                reason: e.to_string(),
            })?;

        let registry = InMemoryToolRegistry::new();

        // Add some mock tools for demonstration
        let registry = registry
            .with_tool("analyze_text", Arc::new(MockTool::new("analyze_text")))
            .with_tool("count_words", Arc::new(MockTool::new("count_words")))
            .with_tool("generate_ideas", Arc::new(MockTool::new("generate_ideas")));

        Ok(Self {
            coordinator: Coordinator::new(agent, registry),
        })
    }
}

impl CoordinatorTrait for AdvancedCoordinator {
    fn step(&mut self, input: String) -> String {
        self.coordinator.step(input)
    }

    fn get_agent_type(&self) -> &'static str {
        "AdvancedDemoAgent"
    }
}

impl Drop for AdvancedCoordinator {
    fn drop(&mut self) {
        // Call cleanup on the agent
        if let Err(e) = self.coordinator.agent.cleanup() {
            tracing::warn!("Agent cleanup failed for AdvancedAgent: {}", e);
        }
    }
}

pub struct AnalyticsCoordinator {
    coordinator: Coordinator<AnalyticsAgent, InMemoryToolRegistry>,
}

impl AnalyticsCoordinator {
    pub fn new(config: HashMap<String, Value>) -> Result<Self, AgentBuildError> {
        let mut agent = AnalyticsAgent::new(config)?;

        // Initialize the agent before use
        agent
            .initialize()
            .map_err(|e| AgentBuildError::ValidationFailed {
                what: "agent initialization".to_string(),
                reason: e.to_string(),
            })?;

        let registry = InMemoryToolRegistry::new();

        // Add analytics-specific tools
        let registry = registry
            .with_tool(
                "statistical_analysis",
                Arc::new(MockTool::new("statistical_analysis")),
            )
            .with_tool(
                "pattern_detection",
                Arc::new(MockTool::new("pattern_detection")),
            )
            .with_tool("trend_analysis", Arc::new(MockTool::new("trend_analysis")));

        Ok(Self {
            coordinator: Coordinator::new(agent, registry),
        })
    }
}

impl CoordinatorTrait for AnalyticsCoordinator {
    fn step(&mut self, input: String) -> String {
        self.coordinator.step(input)
    }

    fn get_agent_type(&self) -> &'static str {
        "AnalyticsAgent"
    }
}

impl Drop for AnalyticsCoordinator {
    fn drop(&mut self) {
        // Call cleanup on the agent
        if let Err(e) = self.coordinator.agent.cleanup() {
            tracing::warn!("Agent cleanup failed for AnalyticsAgent: {}", e);
        }
    }
}

// Agent builders implementing AgentBuilder trait
pub struct EchoAgentBuilder;

impl AgentBuilder for EchoAgentBuilder {
    fn agent_type(&self) -> AgentType {
        AgentType::Echo
    }

    fn build_coordinator(
        &self,
        spec: &AgentSpec,
    ) -> Result<Box<dyn CoordinatorTrait + Send + Sync>, AgentFactoryError> {
        let coordinator = EchoCoordinator::new(spec.config.clone()).map_err(|e| {
            AgentFactoryError::CreationFailed {
                agent_type: self.agent_type(),
                reason: e.to_string(),
            }
        })?;
        Ok(Box::new(coordinator))
    }

    fn validate_spec(&self, spec: &AgentSpec) -> Result<(), AgentFactoryError> {
        if spec.agent_type != self.agent_type() {
            return Err(AgentFactoryError::InvalidConfiguration {
                field: "agent_type".to_string(),
                reason: format!(
                    "Expected {:?}, got {:?}",
                    self.agent_type(),
                    spec.agent_type
                ),
            });
        }
        Ok(())
    }

    fn default_config(&self) -> HashMap<String, Value> {
        HashMap::new()
    }
}

pub struct AdvancedAgentBuilder;

impl AgentBuilder for AdvancedAgentBuilder {
    fn agent_type(&self) -> AgentType {
        AgentType::Advanced
    }

    fn build_coordinator(
        &self,
        spec: &AgentSpec,
    ) -> Result<Box<dyn CoordinatorTrait + Send + Sync>, AgentFactoryError> {
        let coordinator = AdvancedCoordinator::new(spec.config.clone()).map_err(|e| {
            AgentFactoryError::CreationFailed {
                agent_type: self.agent_type(),
                reason: e.to_string(),
            }
        })?;
        Ok(Box::new(coordinator))
    }

    fn validate_spec(&self, spec: &AgentSpec) -> Result<(), AgentFactoryError> {
        if spec.agent_type != self.agent_type() {
            return Err(AgentFactoryError::InvalidConfiguration {
                field: "agent_type".to_string(),
                reason: format!(
                    "Expected {:?}, got {:?}",
                    self.agent_type(),
                    spec.agent_type
                ),
            });
        }

        // Validate mode if specified
        if let Some(mode) = spec.config.get("mode") {
            if let Some(mode_str) = mode.as_str() {
                match mode_str {
                    "simple" | "analytical" | "creative" => {}
                    _ => {
                        return Err(AgentFactoryError::InvalidConfiguration {
                            field: "mode".to_string(),
                            reason: format!(
                                "Invalid mode '{}'. Valid modes: simple, analytical, creative",
                                mode_str
                            ),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn default_config(&self) -> HashMap<String, Value> {
        let mut config = HashMap::new();
        config.insert("mode".to_string(), Value::String("simple".to_string()));
        config.insert("use_tools".to_string(), Value::Bool(true));
        config
    }
}

pub struct AnalyticsAgentBuilder;

impl AgentBuilder for AnalyticsAgentBuilder {
    fn agent_type(&self) -> AgentType {
        AgentType::Analytics
    }

    fn build_coordinator(
        &self,
        spec: &AgentSpec,
    ) -> Result<Box<dyn CoordinatorTrait + Send + Sync>, AgentFactoryError> {
        let coordinator = AnalyticsCoordinator::new(spec.config.clone()).map_err(|e| {
            AgentFactoryError::CreationFailed {
                agent_type: self.agent_type(),
                reason: e.to_string(),
            }
        })?;
        Ok(Box::new(coordinator))
    }

    fn validate_spec(&self, spec: &AgentSpec) -> Result<(), AgentFactoryError> {
        if spec.agent_type != self.agent_type() {
            return Err(AgentFactoryError::InvalidConfiguration {
                field: "agent_type".to_string(),
                reason: format!(
                    "Expected {:?}, got {:?}",
                    self.agent_type(),
                    spec.agent_type
                ),
            });
        }

        // Validate depth if specified
        if let Some(depth) = spec.config.get("depth") {
            if let Some(depth_str) = depth.as_str() {
                match depth_str {
                    "basic" | "detailed" | "comprehensive" => {}
                    _ => {
                        return Err(AgentFactoryError::InvalidConfiguration {
                            field: "depth".to_string(),
                            reason: format!(
                                "Invalid depth '{}'. Valid depths: basic, detailed, comprehensive",
                                depth_str
                            ),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn default_config(&self) -> HashMap<String, Value> {
        let mut config = HashMap::new();
        config.insert("depth".to_string(), Value::String("basic".to_string()));
        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::api_types::AgentLimits;

    #[test]
    fn test_echo_agent() {
        let mut agent = EchoAgent::new(HashMap::new()).unwrap();

        agent.observe("Hello, world!".to_string());
        let response = agent.act();
        assert_eq!(response, "Echo: Hello, world!");

        // Test with no input
        let mut empty_agent = EchoAgent::new(HashMap::new()).unwrap();
        let response = empty_agent.act();
        assert_eq!(response, "Echo: (no input received)");
    }

    #[test]
    fn test_advanced_agent() {
        // Test simple mode
        let mut agent = AdvancedAgent::new(HashMap::new()).unwrap();
        agent.observe("Test input".to_string());
        let response = agent.act();
        assert_eq!(response, "Processed: Test input");

        // Test analytical mode
        let mut config = HashMap::new();
        config.insert("mode".to_string(), Value::String("analytical".to_string()));
        let mut agent = AdvancedAgent::new(config).unwrap();
        agent.observe("Hello World".to_string());
        let response = agent.act();
        assert!(response.contains("Analysis:"));
        assert!(response.contains("2 patterns"));
    }

    #[test]
    fn test_analytics_agent() {
        let mut agent = AnalyticsAgent::new(HashMap::new()).unwrap();

        agent.observe("First data point".to_string());
        agent.observe("Second data point".to_string());

        let response = agent.act();
        assert!(response.contains("2 data points"));
        assert!(response.contains("Second data point"));
    }

    #[test]
    fn test_agent_builders() {
        let echo_builder = EchoAgentBuilder;
        assert_eq!(echo_builder.agent_type(), AgentType::Echo);

        let advanced_builder = AdvancedAgentBuilder;
        assert_eq!(advanced_builder.agent_type(), AgentType::Advanced);

        let analytics_builder = AnalyticsAgentBuilder;
        assert_eq!(analytics_builder.agent_type(), AgentType::Analytics);
    }

    #[tokio::test]
    async fn test_coordinator_creation() {
        let spec = AgentSpec {
            agent_type: AgentType::Echo,
            name: Some("test-echo".to_string()),
            config: HashMap::new(),
            limits: AgentLimits::default(),
        };

        let builder = EchoAgentBuilder;
        let coordinator = builder.build_coordinator(&spec).unwrap();
        assert_eq!(coordinator.get_agent_type(), "EchoAgent");
    }

    #[test]
    fn test_advanced_agent_validation() {
        let builder = AdvancedAgentBuilder;

        // Valid spec
        let mut config = HashMap::new();
        config.insert("mode".to_string(), Value::String("analytical".to_string()));
        let spec = AgentSpec {
            agent_type: AgentType::Advanced,
            name: None,
            config,
            limits: AgentLimits::default(),
        };
        assert!(builder.validate_spec(&spec).is_ok());

        // Invalid mode
        let mut config = HashMap::new();
        config.insert("mode".to_string(), Value::String("invalid".to_string()));
        let spec = AgentSpec {
            agent_type: AgentType::Advanced,
            name: None,
            config,
            limits: AgentLimits::default(),
        };
        assert!(builder.validate_spec(&spec).is_err());
    }
}
