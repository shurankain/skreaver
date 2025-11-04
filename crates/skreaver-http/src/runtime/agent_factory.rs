//! Agent Factory Pattern Implementation
//!
//! This module provides a type-safe agent factory pattern that enables
//! dynamic agent creation from specifications. The factory maintains
//! a registry of agent builders and handles the complete agent lifecycle.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::runtime::{
    agent_instance::{AgentId, AgentInstance, CoordinatorTrait},
    agent_status::AgentStatusEnum,
    api_types::{AgentEndpoints, AgentSpec, AgentType, CreateAgentResponse},
};
use skreaver_core::IdValidationError;

/// Factory error types
#[derive(Debug, Clone)]
pub enum AgentFactoryError {
    /// Agent type not registered in factory
    UnknownAgentType(AgentType),
    /// Invalid agent ID format
    InvalidAgentId(IdValidationError),
    /// Agent already exists with the same ID
    AgentAlreadyExists(String),
    /// Agent creation failed
    CreationFailed {
        agent_type: AgentType,
        reason: String,
    },
    /// Agent not found
    AgentNotFound(String),
    /// Invalid agent configuration
    InvalidConfiguration { field: String, reason: String },
}

impl std::fmt::Display for AgentFactoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownAgentType(agent_type) => {
                write!(f, "Unknown agent type: {}", agent_type)
            }
            Self::InvalidAgentId(err) => write!(f, "Invalid agent ID: {}", err),
            Self::AgentAlreadyExists(id) => {
                write!(f, "Agent with ID '{}' already exists", id)
            }
            Self::CreationFailed { agent_type, reason } => {
                write!(f, "Failed to create {} agent: {}", agent_type, reason)
            }
            Self::AgentNotFound(id) => write!(f, "Agent '{}' not found", id),
            Self::InvalidConfiguration { field, reason } => {
                write!(f, "Invalid configuration for field '{}': {}", field, reason)
            }
        }
    }
}

impl std::error::Error for AgentFactoryError {}

/// Trait for building specific agent types
pub trait AgentBuilder: Send + Sync {
    /// Get the agent type this builder handles
    fn agent_type(&self) -> AgentType;

    /// Create a new coordinator instance from specification
    fn build_coordinator(
        &self,
        spec: &AgentSpec,
    ) -> Result<Box<dyn CoordinatorTrait + Send + Sync>, AgentFactoryError>;

    /// Validate agent specification before creation
    fn validate_spec(&self, spec: &AgentSpec) -> Result<(), AgentFactoryError> {
        // Default validation - can be overridden
        if spec.agent_type != self.agent_type() {
            return Err(AgentFactoryError::InvalidConfiguration {
                field: "agent_type".to_string(),
                reason: format!(
                    "Builder for {} cannot handle {}",
                    self.agent_type(),
                    spec.agent_type
                ),
            });
        }
        Ok(())
    }

    /// Get default configuration for this agent type
    fn default_config(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
}

/// Agent factory for creating and managing agent instances
pub struct AgentFactory {
    /// Registry of agent builders by type
    builders: HashMap<AgentType, Box<dyn AgentBuilder>>,
    /// Created agent instances (using AgentId as key for type safety)
    agents: Arc<RwLock<HashMap<AgentId, AgentInstance>>>,
}

impl AgentFactory {
    /// Create a new agent factory
    pub fn new() -> Self {
        Self {
            builders: HashMap::new(),
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an agent builder for a specific type
    pub fn register_builder(&mut self, builder: Box<dyn AgentBuilder>) {
        let agent_type = builder.agent_type();
        self.builders.insert(agent_type, builder);
    }

    /// Get list of supported agent types
    pub fn supported_types(&self) -> Vec<AgentType> {
        self.builders.keys().cloned().collect()
    }

    /// Check if an agent type is supported
    pub fn supports_type(&self, agent_type: &AgentType) -> bool {
        self.builders.contains_key(agent_type)
    }

    /// Create a new agent from specification
    pub async fn create_agent(
        &self,
        spec: AgentSpec,
        custom_id: Option<String>,
    ) -> Result<CreateAgentResponse, AgentFactoryError> {
        // Get builder for agent type
        let builder = self
            .builders
            .get(&spec.agent_type)
            .ok_or_else(|| AgentFactoryError::UnknownAgentType(spec.agent_type.clone()))?;

        // Validate specification
        builder.validate_spec(&spec)?;

        // Generate or validate agent ID
        let agent_id_str = match custom_id {
            Some(id) => id,
            None => self.generate_agent_id(&spec),
        };

        let agent_id = AgentId::parse(&agent_id_str).map_err(AgentFactoryError::InvalidAgentId)?;

        // Check if agent already exists
        {
            let agents = self.agents.read().await;
            if agents.contains_key(&agent_id) {
                return Err(AgentFactoryError::AgentAlreadyExists(agent_id_str));
            }
        }

        // Build coordinator
        let coordinator = builder.build_coordinator(&spec)?;

        // Create agent instance
        let agent_instance =
            AgentInstance::new(agent_id.clone(), spec.agent_type.to_string(), coordinator);

        // Set agent to ready state
        agent_instance.set_status(AgentStatusEnum::Ready).await;

        // Add metadata from spec
        if let Some(ref name) = spec.name {
            agent_instance
                .add_tag("name".to_string(), name.clone())
                .await;
        }

        // Store spec configuration as custom metadata
        for (key, value) in &spec.config {
            agent_instance
                .add_custom_metadata(format!("config.{}", key), value.clone())
                .await;
        }

        let created_at = agent_instance.created_at;
        let current_status = agent_instance.get_status().await;

        // Store agent instance
        {
            let mut agents = self.agents.write().await;
            agents.insert(agent_id.clone(), agent_instance);
        }

        // Create response
        Ok(CreateAgentResponse {
            agent_id: agent_id_str.clone(),
            spec: spec.clone(),
            status: current_status,
            created_at,
            endpoints: AgentEndpoints::for_agent(&agent_id_str, &spec.agent_type),
        })
    }

    /// Check if an agent exists
    pub async fn has_agent(&self, agent_id: &str) -> bool {
        if let Ok(agent_id) = AgentId::parse(agent_id) {
            let agents = self.agents.read().await;
            agents.contains_key(&agent_id)
        } else {
            false
        }
    }

    /// Remove an agent by ID
    pub async fn remove_agent(&self, agent_id: &str) -> Result<(), AgentFactoryError> {
        let agent_id = AgentId::parse(agent_id).map_err(AgentFactoryError::InvalidAgentId)?;
        let mut agents = self.agents.write().await;
        agents
            .remove(&agent_id)
            .ok_or_else(|| AgentFactoryError::AgentNotFound(agent_id.to_string()))?;
        Ok(())
    }

    /// List all agent IDs
    pub async fn list_agent_ids(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().map(|id| id.to_string()).collect()
    }

    /// Get agent count
    pub async fn agent_count(&self) -> usize {
        let agents = self.agents.read().await;
        agents.len()
    }

    /// Shutdown all agents gracefully, calling cleanup hooks
    ///
    /// This method should be called during graceful shutdown to ensure
    /// all agents have a chance to cleanup resources, close connections, etc.
    ///
    /// # Returns
    ///
    /// The number of agents that were cleaned up
    pub async fn shutdown_all_agents(&self) -> usize {
        let mut agents = self.agents.write().await;
        let count = agents.len();

        // Clear the agents map - this will drop all AgentInstance values,
        // which will in turn drop their coordinators, triggering the Drop
        // implementations that call cleanup()
        agents.clear();

        tracing::info!("Cleaned up {} agents during shutdown", count);
        count
    }

    /// Get agents map reference for external access
    pub fn agents(&self) -> Arc<RwLock<HashMap<AgentId, AgentInstance>>> {
        Arc::clone(&self.agents)
    }

    /// Generate a unique agent ID
    fn generate_agent_id(&self, spec: &AgentSpec) -> String {
        let prefix = match &spec.name {
            Some(name) => {
                // Sanitize name for use in ID
                let sanitized = name
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-')
                    .collect::<String>()
                    .to_lowercase();
                if sanitized.is_empty() {
                    format!("{}", spec.agent_type)
                } else {
                    format!("{}-{}", spec.agent_type, sanitized)
                }
            }
            None => format!("{}", spec.agent_type),
        };

        // Add unique suffix
        let uuid = Uuid::new_v4().to_string();
        let short_uuid = &uuid[..8];
        format!("{}-{}", prefix, short_uuid)
    }
}

impl Default for AgentFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience macro for implementing simple agent builders
#[macro_export]
macro_rules! impl_agent_builder {
    ($builder_name:ident, $agent_type:expr, $coordinator_type:ty) => {
        pub struct $builder_name;

        impl AgentBuilder for $builder_name {
            fn agent_type(&self) -> AgentType {
                $agent_type
            }

            fn build_coordinator(
                &self,
                spec: &AgentSpec,
            ) -> Result<Box<dyn CoordinatorTrait + Send + Sync>, AgentFactoryError> {
                let coordinator = <$coordinator_type>::new(spec.config.clone()).map_err(|e| {
                    AgentFactoryError::CreationFailed {
                        agent_type: self.agent_type(),
                        reason: e.to_string(),
                    }
                })?;
                Ok(Box::new(coordinator))
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::api_types::AgentLimits;
    use std::collections::HashMap;

    // Mock coordinator for testing
    struct MockCoordinator {
        name: String,
    }

    impl MockCoordinator {
        fn new(config: HashMap<String, serde_json::Value>) -> Result<Self, String> {
            let name = config
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("mock")
                .to_string();
            Ok(Self { name })
        }
    }

    impl CoordinatorTrait for MockCoordinator {
        fn step(&mut self, input: String) -> String {
            format!("{}: {}", self.name, input)
        }

        fn get_agent_type(&self) -> &'static str {
            "mock"
        }
    }

    // Mock builder
    struct MockBuilder;

    impl AgentBuilder for MockBuilder {
        fn agent_type(&self) -> AgentType {
            AgentType::Echo
        }

        fn build_coordinator(
            &self,
            spec: &AgentSpec,
        ) -> Result<Box<dyn CoordinatorTrait + Send + Sync>, AgentFactoryError> {
            let coordinator = MockCoordinator::new(spec.config.clone()).map_err(|e| {
                AgentFactoryError::CreationFailed {
                    agent_type: self.agent_type(),
                    reason: e,
                }
            })?;
            Ok(Box::new(coordinator))
        }
    }

    #[tokio::test]
    async fn test_agent_factory_creation() {
        let mut factory = AgentFactory::new();
        factory.register_builder(Box::new(MockBuilder));

        assert!(factory.supports_type(&AgentType::Echo));
        assert!(!factory.supports_type(&AgentType::Advanced));

        let spec = AgentSpec {
            agent_type: AgentType::Echo,
            name: Some("test-agent".to_string()),
            config: HashMap::new(),
            limits: AgentLimits::default(),
        };

        let response = factory.create_agent(spec, None).await.unwrap();

        assert!(response.agent_id.contains("echo"));
        assert!(response.agent_id.contains("test-agent"));
        assert_eq!(response.spec.agent_type, AgentType::Echo);
        assert_eq!(response.status, AgentStatusEnum::Ready);
    }

    #[tokio::test]
    async fn test_agent_factory_errors() {
        let factory = AgentFactory::new();

        // Test unknown agent type
        let spec = AgentSpec {
            agent_type: AgentType::Advanced,
            name: None,
            config: HashMap::new(),
            limits: AgentLimits::default(),
        };

        let result = factory.create_agent(spec, None).await;
        assert!(matches!(
            result,
            Err(AgentFactoryError::UnknownAgentType(_))
        ));

        // Test invalid agent ID
        let mut factory = AgentFactory::new();
        factory.register_builder(Box::new(MockBuilder));
        let spec = AgentSpec {
            agent_type: AgentType::Echo,
            name: None,
            config: HashMap::new(),
            limits: AgentLimits::default(),
        };

        let result = factory
            .create_agent(spec, Some("invalid@id".to_string()))
            .await;
        assert!(matches!(result, Err(AgentFactoryError::InvalidAgentId(_))));
    }

    #[tokio::test]
    async fn test_agent_management() {
        let mut factory = AgentFactory::new();
        factory.register_builder(Box::new(MockBuilder));

        let spec = AgentSpec {
            agent_type: AgentType::Echo,
            name: Some("managed-agent".to_string()),
            config: HashMap::new(),
            limits: AgentLimits::default(),
        };

        let response = factory.create_agent(spec, None).await.unwrap();
        let agent_id = response.agent_id;

        // Test has agent
        assert!(factory.has_agent(&agent_id).await);

        // Test agent count
        assert_eq!(factory.agent_count().await, 1);

        // Test list agents
        let agent_ids = factory.list_agent_ids().await;
        assert_eq!(agent_ids.len(), 1);
        assert!(agent_ids.contains(&agent_id));

        // Test remove agent
        factory.remove_agent(&agent_id).await.unwrap();
        assert_eq!(factory.agent_count().await, 0);

        // Test removed agent doesn't exist
        assert!(!factory.has_agent(&agent_id).await);
    }

    #[tokio::test]
    async fn test_duplicate_agent_id() {
        let mut factory = AgentFactory::new();
        factory.register_builder(Box::new(MockBuilder));

        let spec = AgentSpec {
            agent_type: AgentType::Echo,
            name: None,
            config: HashMap::new(),
            limits: AgentLimits::default(),
        };

        // Create first agent
        factory
            .create_agent(spec.clone(), Some("duplicate-id".to_string()))
            .await
            .unwrap();

        // Try to create second agent with same ID
        let result = factory
            .create_agent(spec, Some("duplicate-id".to_string()))
            .await;
        assert!(matches!(
            result,
            Err(AgentFactoryError::AgentAlreadyExists(_))
        ));
    }
}
