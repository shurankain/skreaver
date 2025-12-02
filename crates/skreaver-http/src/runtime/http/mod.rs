//! # HTTP Runtime
//!
//! This module provides a production-ready HTTP server runtime for Skreaver agents,
//! enabling secure remote interaction through RESTful APIs with authentication,
//! rate limiting, and streaming capabilities. The runtime manages agent lifecycle,
//! handles observations, and provides real-time status information.

mod config;

#[cfg(test)]
mod tests;

pub use config::{CorsConfig, HttpRuntimeConfig, OpenApiConfig};

use crate::runtime::{
    Coordinator,
    agent_builders::{AdvancedAgentBuilder, AnalyticsAgentBuilder, EchoAgentBuilder},
    agent_factory::{AgentFactory, AgentFactoryError},
    agent_instance::{AgentInstance, CoordinatorTrait},
    api_types::{AgentSpec, CreateAgentResponse},
    backpressure::BackpressureManager,
    rate_limit::RateLimitState,
};
use skreaver_core::Agent;
use skreaver_core::auth::rbac::RoleManager;
use skreaver_core::security::SecurityConfig;
use skreaver_observability::init_observability;
use skreaver_tools::{SecureToolRegistry, ToolRegistry};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

// Re-export unified AgentId from skreaver-core
pub use skreaver_core::AgentId;

/// HTTP server state containing all running agents and security configuration
///
/// Tool registry is wrapped in `SecureToolRegistry` to enforce security policies at runtime.
#[derive(Clone)]
pub struct HttpAgentRuntime<T: ToolRegistry> {
    pub agents: Arc<RwLock<HashMap<AgentId, AgentInstance>>>,
    /// Tool registry wrapped with security policy enforcement
    pub tool_registry: Arc<SecureToolRegistry<T>>,
    pub rate_limit_state: Arc<RateLimitState>,
    pub backpressure_manager: Arc<BackpressureManager>,
    pub agent_factory: Arc<AgentFactory>,
    /// Security configuration loaded from file or defaults
    pub security_config: Arc<SecurityConfig>,
    /// Connection tracker for HTTP connection limits
    pub connection_tracker: Arc<crate::runtime::connection_limits::ConnectionTracker>,
    /// API key manager for secure key storage, rotation, and revocation
    pub api_key_manager: Arc<skreaver_core::ApiKeyManager>,
}

// AgentInstance and CoordinatorTrait are now imported from agent_instance module

impl<A: Agent + Send + Sync + 'static, T: ToolRegistry + Clone> CoordinatorTrait
    for Coordinator<A, T>
where
    A::Observation: From<String> + std::fmt::Display,
    A::Action: ToString,
{
    fn step(&mut self, input: String) -> String {
        let observation = A::Observation::from(input);
        let action = self.step(observation);
        action.to_string()
    }

    fn get_agent_type(&self) -> &'static str {
        std::any::type_name::<A>()
    }
}

impl<T: ToolRegistry + Clone + Send + Sync + 'static> HttpAgentRuntime<T> {
    /// Create a new HTTP agent runtime with default configuration
    pub fn new(tool_registry: T) -> Self {
        Self::with_config(tool_registry, HttpRuntimeConfig::default())
    }

    /// Create a new HTTP agent runtime with custom configuration
    pub fn with_config(tool_registry: T, config: HttpRuntimeConfig) -> Self {
        // Initialize observability framework
        if let Err(e) = init_observability(config.observability.clone()) {
            tracing::warn!("Failed to initialize observability: {}", e);
        }

        // Load security configuration with fail-fast validation
        let security_config = if let Some(config_path) = &config.security_config_path {
            match SecurityConfig::load_from_file(config_path) {
                Ok(cfg) => {
                    tracing::info!(
                        "Loaded security configuration from: {}",
                        config_path.display()
                    );
                    // Validate configuration (fail-fast on errors)
                    cfg.validate().unwrap_or_else(|e| {
                        panic!(
                            "Security configuration validation failed: {}\n\
                            Config file: {}\n\
                            CRITICAL: Invalid security configuration prevents startup. \
                            Please fix the configuration file or remove it to use defaults.",
                            e,
                            config_path.display()
                        )
                    });
                    tracing::info!("Security configuration validated successfully");
                    cfg
                }
                Err(e) => {
                    // Check if this is a "file not found" error - warn and use defaults
                    // This allows development/testing without requiring config files
                    if e.to_string().contains("No such file or directory")
                        || e.to_string().contains("cannot find the path")
                    {
                        tracing::warn!(
                            "Security configuration file not found: {} - using defaults. \
                            Error: {}",
                            config_path.display(),
                            e
                        );
                        SecurityConfig::create_default()
                    } else {
                        // Other errors (permission denied, invalid TOML, etc.) are critical
                        panic!(
                            "Failed to load security configuration from {}: {}\n\
                            CRITICAL: Could not read security configuration file. \
                            Please ensure the file exists and is readable, or remove the \
                            security_config_path setting to use defaults.",
                            config_path.display(),
                            e
                        )
                    }
                }
            }
        } else {
            tracing::info!("No security configuration file specified, using defaults");
            let default_config = SecurityConfig::create_default();
            // Validate defaults (should always pass, but check anyway)
            default_config.validate().unwrap_or_else(|e| {
                panic!(
                    "Default security configuration is invalid: {}\n\
                    This is a bug - please report it.",
                    e
                )
            });
            default_config
        };

        // Wrap tool registry with security policy and RBAC enforcement
        let security_config_arc = Arc::new(security_config);
        let role_manager = Arc::new(RoleManager::with_defaults());
        let secure_registry = SecureToolRegistry::new(
            tool_registry,
            Arc::clone(&security_config_arc),
            role_manager,
        );
        tracing::info!("Tool registry wrapped with security policy and RBAC enforcement");

        let backpressure_manager = Arc::new(BackpressureManager::new(config.backpressure.clone()));

        // Start backpressure manager in background
        let backpressure_manager_clone = Arc::clone(&backpressure_manager);
        tokio::spawn(async move {
            if let Err(e) = backpressure_manager_clone.start().await {
                tracing::error!("Failed to start backpressure manager: {}", e);
            }
        });

        // Create and configure agent factory with standard builders
        let mut agent_factory = AgentFactory::new();
        agent_factory.register_builder(Box::new(EchoAgentBuilder));
        agent_factory.register_builder(Box::new(AdvancedAgentBuilder));
        agent_factory.register_builder(Box::new(AnalyticsAgentBuilder));

        // Create connection tracker with configuration
        let connection_tracker =
            Arc::new(crate::runtime::connection_limits::ConnectionTracker::new(
                config.connection_limits.clone(),
            ));
        tracing::info!(
            "Connection limits: max={}, per_ip={}, mode={:?}",
            config.connection_limits.max_connections,
            config.connection_limits.max_connections_per_ip,
            config.connection_limits.mode
        );

        // Create API key manager for secure credential storage
        let api_key_manager = crate::runtime::auth::create_api_key_manager();
        tracing::info!("API key manager initialized with secure storage");

        Self {
            agents: agent_factory.agents(),
            tool_registry: Arc::new(secure_registry),
            rate_limit_state: Arc::new(RateLimitState::new(config.rate_limit)),
            backpressure_manager,
            agent_factory: Arc::new(agent_factory),
            security_config: security_config_arc,
            connection_tracker,
            api_key_manager,
        }
    }

    /// Create a new agent from specification using the factory pattern
    pub async fn create_agent(
        &self,
        spec: AgentSpec,
        custom_id: Option<String>,
    ) -> Result<CreateAgentResponse, AgentFactoryError> {
        self.agent_factory.create_agent(spec, custom_id).await
    }

    /// Get list of supported agent types
    pub fn supported_agent_types(&self) -> Vec<crate::runtime::api_types::AgentType> {
        self.agent_factory.supported_types()
    }

    /// Check if an agent type is supported
    pub fn supports_agent_type(&self, agent_type: &crate::runtime::api_types::AgentType) -> bool {
        self.agent_factory.supports_type(agent_type)
    }

    /// Remove an agent by ID
    pub async fn remove_agent(&self, agent_id: &str) -> Result<(), AgentFactoryError> {
        self.agent_factory.remove_agent(agent_id).await
    }

    /// Check if an agent exists by ID
    pub async fn has_agent(&self, agent_id: &str) -> bool {
        self.agent_factory.has_agent(agent_id).await
    }

    /// List all agent IDs
    pub async fn list_agent_ids(&self) -> Vec<String> {
        self.agent_factory.list_agent_ids().await
    }

    /// Shutdown all agents gracefully
    ///
    /// This method should be called during graceful shutdown to ensure
    /// all agents cleanup properly. It will call the cleanup() lifecycle hook
    /// on all agents.
    ///
    /// # Returns
    ///
    /// The number of agents that were cleaned up
    ///
    /// # Example
    ///
    /// ```no_run
    /// use skreaver_http::runtime::{HttpAgentRuntime, shutdown_signal};
    /// use skreaver_tools::InMemoryToolRegistry;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let runtime = HttpAgentRuntime::new(InMemoryToolRegistry::new());
    ///
    ///     // ... create agents and run server ...
    ///
    ///     // On shutdown signal
    ///     shutdown_signal().await;
    ///     runtime.shutdown_all_agents().await;
    /// }
    /// ```
    pub async fn shutdown_all_agents(&self) -> usize {
        self.agent_factory.shutdown_all_agents().await
    }

    /// Get agent count
    pub async fn agent_count(&self) -> usize {
        self.agent_factory.agent_count().await
    }

    /// Get the security configuration
    pub fn security_config(&self) -> &SecurityConfig {
        &self.security_config
    }

    /// Add an agent instance to the runtime (legacy method for backward compatibility)
    pub async fn add_agent<A>(&self, agent_id: impl AsRef<str>, agent: A) -> Result<(), String>
    where
        A: Agent + Send + Sync + 'static,
        A::Observation: From<String> + std::fmt::Display,
        A::Action: ToString,
    {
        let agent_id =
            AgentId::parse(agent_id.as_ref()).map_err(|e| format!("Invalid agent ID: {}", e))?;

        let coordinator = Coordinator::new(agent, (*self.tool_registry).clone());
        let agent_instance = crate::runtime::agent_instance::AgentInstance::new(
            agent_id.clone(),
            std::any::type_name::<A>().to_string(),
            Box::new(coordinator),
        );

        let mut agents = self.agents.write().await;
        agents.insert(agent_id, agent_instance);
        Ok(())
    }
}
