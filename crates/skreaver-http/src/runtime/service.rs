//! Service layer for HTTP runtime with proper separation of concerns

use std::sync::Arc;
use chrono::{DateTime, Utc};

use crate::runtime::{
    agent_instance::{AgentInstance, AgentId, AgentExecutionError, CoordinatorTrait},
    agent_status::AgentStatus,
    api_types::{AgentSpec, AgentType, AgentObservation, AgentResponse, AgentStatusResponse, ResourceUsage, AgentMetrics},
    error::{RuntimeError, AgentError, RuntimeResult},
    performance::OptimizedAgentRegistry,
};
use std::collections::HashMap;

/// High-level service interface for agent operations
/// 
/// This service layer provides clean abstractions over the agent runtime,
/// handling business logic while keeping HTTP handlers thin and focused.
#[derive(Clone)]
pub struct AgentService {
    /// Optimized agent registry
    registry: Arc<OptimizedAgentRegistry>,
    /// Service configuration
    config: ServiceConfig,
    /// Service metrics
    metrics: Arc<ServiceMetrics>,
}

/// Metrics collection mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricsMode {
    /// No metrics collection
    Disabled,
    /// Basic metrics only (minimal overhead)
    Basic,
    /// Standard metrics set (balanced)
    Standard,
    /// Detailed metrics with performance impact
    Detailed,
}

impl MetricsMode {
    /// Check if metrics are enabled
    pub fn is_enabled(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// Check if using detailed metrics
    pub fn is_detailed(self) -> bool {
        matches!(self, Self::Detailed)
    }

    /// Check if basic or higher
    pub fn includes_basic(self) -> bool {
        matches!(self, Self::Basic | Self::Standard | Self::Detailed)
    }

    /// Check if standard or higher
    pub fn includes_standard(self) -> bool {
        matches!(self, Self::Standard | Self::Detailed)
    }
}

impl Default for MetricsMode {
    fn default() -> Self {
        Self::Standard
    }
}

/// Configuration for agent service
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Default execution timeout
    pub default_timeout_secs: u32,
    /// Metrics collection mode
    pub metrics: MetricsMode,
    /// Maximum number of concurrent agent operations
    pub max_concurrent_operations: usize,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            default_timeout_secs: 30,
            metrics: MetricsMode::default(),
            max_concurrent_operations: 100,
        }
    }
}

impl ServiceConfig {
    /// Create config with no metrics
    pub fn minimal() -> Self {
        Self {
            metrics: MetricsMode::Disabled,
            ..Default::default()
        }
    }

    /// Create config with basic metrics
    pub fn basic() -> Self {
        Self {
            metrics: MetricsMode::Basic,
            ..Default::default()
        }
    }

    /// Create config with detailed metrics
    pub fn detailed() -> Self {
        Self {
            metrics: MetricsMode::Detailed,
            ..Default::default()
        }
    }
}

/// Service-level metrics
#[derive(Debug, Default)]
pub struct ServiceMetrics {
    /// Total observations processed
    pub observations_processed: std::sync::atomic::AtomicU64,
    /// Total agents created
    pub agents_created: std::sync::atomic::AtomicU64,
    /// Total errors encountered
    pub errors_encountered: std::sync::atomic::AtomicU64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: std::sync::atomic::AtomicU64,
}

impl AgentService {
    /// Create a new agent service
    pub fn new(config: ServiceConfig) -> Self {
        Self {
            registry: Arc::new(OptimizedAgentRegistry::new()),
            config,
            metrics: Arc::new(ServiceMetrics::default()),
        }
    }
    
    /// Create a new agent from specification
    pub async fn create_agent(&self, spec: AgentSpec) -> RuntimeResult<CreateAgentResult> {
        let agent_id = AgentId::new(format!("agent-{}", uuid::Uuid::new_v4()));
        
        // TODO: Factory pattern for creating different agent types
        // This would require implementing an AgentFactory trait
        let _instance = self.create_agent_instance(&agent_id, &spec).await?;
        
        self.metrics.agents_created.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        
        Ok(CreateAgentResult {
            agent_id,
            spec,
            status: AgentStatus::Ready,
            created_at: Utc::now(),
        })
    }
    
    /// Send observation to agent and get response
    pub async fn observe_agent(
        &self,
        agent_id: &AgentId,
        observation: AgentObservation,
    ) -> RuntimeResult<AgentResponse> {
        let start_time = std::time::Instant::now();
        
        // Get agent from registry
        let agent = self.registry.get_agent(agent_id).await
            .ok_or_else(|| RuntimeError::Agent(AgentError::NotFound(agent_id.as_str().to_string())))?;
        
        // Check agent can accept observations
        if !agent.can_accept_observations().await {
            let current_status = agent.get_status().await;
            return Err(RuntimeError::Agent(AgentError::InvalidState {
                agent_id: agent_id.as_str().to_string(),
                current_state: current_status.to_string(),
                required_state: "ready".to_string(),
            }));
        }
        
        // Process observation (would need to implement execute_step properly)
        // For now, simulate processing
        let response_content = format!("Processed: {}", observation.content);
        let processing_time = start_time.elapsed();
        
        // Update metrics
        self.metrics.observations_processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.update_avg_response_time(processing_time.as_millis() as u64);
        
        Ok(AgentResponse {
            agent_id: agent_id.as_str().to_string(),
            content: response_content,
            response_mode: observation.response_mode,
            processing_time_ms: processing_time.as_millis() as u32,
            tools_called: 0, // Would be tracked by actual implementation
            timestamp: Utc::now(),
            metadata: observation.metadata.into_iter()
                .map(|(k, v)| (k, serde_json::Value::String(v)))
                .collect(),
        })
    }
    
    /// Get detailed agent status
    pub async fn get_agent_status(&self, agent_id: &AgentId) -> RuntimeResult<AgentStatusResponse> {
        let agent = self.registry.get_agent(agent_id).await
            .ok_or_else(|| RuntimeError::Agent(AgentError::NotFound(agent_id.as_str().to_string())))?;
        
        Ok(AgentStatusResponse {
            agent_id: agent_id.as_str().to_string(),
            agent_type: crate::runtime::api_types::AgentType::Advanced, // Would come from agent
            status: agent.get_status().await,
            resource_usage: ResourceUsage {
                memory_bytes: 0, // Would be measured from actual agent
                memory_percent: 0.0,
                active_tools: 0,
            },
            metrics: AgentMetrics {
                observations_processed: agent.get_observation_count(),
                tool_calls_made: agent.get_tool_call_count(),
                avg_response_time_ms: 0.0, // Would be calculated from history
                success_rate: 1.0,
                last_error: None,
            },
            created_at: agent.created_at,
            last_activity: agent.get_last_activity().await,
            config: std::collections::HashMap::new(),
            instance_metadata: Some(agent.get_instance_metadata().await),
        })
    }
    
    /// List all agents with pagination
    pub async fn list_agents(
        &self, 
        limit: Option<usize>, 
        offset: Option<usize>
    ) -> RuntimeResult<ListAgentsResult> {
        let all_ids = self.registry.get_all_ids();
        let total = all_ids.len();
        
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(50).min(100); // Cap at 100
        
        let mut agents = Vec::new();
        for agent_id in all_ids.into_iter().skip(offset).take(limit) {
            if let Ok(status) = self.get_agent_status(&agent_id).await {
                agents.push(status);
            }
        }
        
        Ok(ListAgentsResult {
            agents,
            total,
            offset,
            limit,
        })
    }
    
    /// Remove an agent
    pub async fn remove_agent(&self, agent_id: &AgentId) -> RuntimeResult<()> {
        let _removed = self.registry.remove_agent(agent_id).await
            .ok_or_else(|| RuntimeError::Agent(AgentError::NotFound(agent_id.as_str().to_string())))?;
        
        Ok(())
    }
    
    /// Get service health status
    pub fn get_health_status(&self) -> HealthStatus {
        let registry_metrics = self.registry.get_metrics();
        let service_metrics = ServiceMetrics {
            observations_processed: std::sync::atomic::AtomicU64::new(
                self.metrics.observations_processed.load(std::sync::atomic::Ordering::Relaxed)
            ),
            agents_created: std::sync::atomic::AtomicU64::new(
                self.metrics.agents_created.load(std::sync::atomic::Ordering::Relaxed)
            ),
            errors_encountered: std::sync::atomic::AtomicU64::new(
                self.metrics.errors_encountered.load(std::sync::atomic::Ordering::Relaxed)
            ),
            avg_response_time_ms: std::sync::atomic::AtomicU64::new(
                self.metrics.avg_response_time_ms.load(std::sync::atomic::Ordering::Relaxed)
            ),
        };
        
        HealthStatus {
            state: HealthState::Healthy, // Would implement actual health checks
            uptime_seconds: 0, // Would track actual uptime
            active_agents: registry_metrics.lookups.load(std::sync::atomic::Ordering::Relaxed),
            service_metrics,
            timestamp: Utc::now(),
        }
    }
    
    // Private helper methods
    
    async fn create_agent_instance(
        &self,
        agent_id: &AgentId,
        spec: &AgentSpec,
    ) -> RuntimeResult<Arc<AgentInstance>> {
        // Create coordinator based on agent type using factory pattern
        let coordinator: Box<dyn CoordinatorTrait + Send + Sync> = match &spec.agent_type {
            AgentType::Echo => {
                Box::new(EchoCoordinator::new())
            }
            AgentType::Advanced => {
                Box::new(AdvancedCoordinator::new(spec.config.clone()))
            }
            AgentType::Analytics => {
                Box::new(AnalyticsCoordinator::new(spec.config.clone()))
            }
            AgentType::Custom(type_name) => {
                // For custom agent types, return an error with guidance
                return Err(RuntimeError::Agent(AgentError::CreationFailed(
                    format!(
                        "Custom agent type '{}' requires registration. \
                        Register custom agent factories using HttpAgentRuntime::register_agent_factory()",
                        type_name
                    )
                )));
            }
        };

        // Create agent instance with the coordinator
        let instance = AgentInstance::new(
            agent_id.clone(),
            spec.agent_type.implementation_name().to_string(),
            coordinator,
        );

        Ok(Arc::new(instance))
    }
    
    fn update_avg_response_time(&self, new_time_ms: u64) {
        let current_avg = self.metrics.avg_response_time_ms.load(std::sync::atomic::Ordering::Relaxed);
        let new_avg = if current_avg == 0 {
            new_time_ms
        } else {
            (current_avg + new_time_ms) / 2
        };
        self.metrics.avg_response_time_ms.store(new_avg, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Result of creating an agent
#[derive(Debug)]
pub struct CreateAgentResult {
    pub agent_id: AgentId,
    pub spec: AgentSpec,
    pub status: AgentStatus,
    pub created_at: DateTime<Utc>,
}

/// Result of listing agents
#[derive(Debug)]
pub struct ListAgentsResult {
    pub agents: Vec<AgentStatusResponse>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

/// Service health state with detailed status information
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    /// Service is fully operational
    Healthy,
    /// Service is operational but degraded (e.g., high load)
    Degraded { warning_count: u32 },
    /// Service is experiencing issues but still running
    Unhealthy { error_count: u32 },
    /// Service is not operational
    Down,
}

impl HealthState {
    /// Check if service is healthy (fully operational or degraded)
    pub fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded { .. })
    }

    /// Check if service is fully operational
    pub fn is_fully_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }

    /// Check if service is down
    pub fn is_down(self) -> bool {
        matches!(self, Self::Down)
    }

    /// Check if service is degraded
    pub fn is_degraded(self) -> bool {
        matches!(self, Self::Degraded { .. })
    }

    /// Get warning count if degraded
    pub fn warning_count(self) -> Option<u32> {
        match self {
            Self::Degraded { warning_count } => Some(warning_count),
            _ => None,
        }
    }

    /// Get error count if unhealthy
    pub fn error_count(self) -> Option<u32> {
        match self {
            Self::Unhealthy { error_count } => Some(error_count),
            _ => None,
        }
    }
}

impl Default for HealthState {
    fn default() -> Self {
        Self::Healthy
    }
}

/// Health status of the service
#[derive(Debug)]
pub struct HealthStatus {
    pub state: HealthState,
    pub uptime_seconds: u64,
    pub active_agents: u64,
    pub service_metrics: ServiceMetrics,
    pub timestamp: DateTime<Utc>,
}

/// Authentication service for handling auth operations
#[derive(Clone)]
pub struct AuthService {
    secret_manager: Arc<crate::runtime::security::SecretManager>,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new() -> RuntimeResult<Self> {
        let secret_manager = crate::runtime::security::SecretManager::new()
            .map_err(|e| RuntimeError::Auth(crate::runtime::error::AuthError::InvalidToken(
                crate::runtime::auth_token::AuthTokenError::InvalidFormat
            )))?;
        
        Ok(Self {
            secret_manager: Arc::new(secret_manager),
        })
    }
    
    /// Create a new JWT token
    pub async fn create_jwt_token(
        &self,
        user_id: String,
        permissions: Vec<String>,
    ) -> RuntimeResult<TokenResult> {
        let _secret = self.secret_manager.get_jwt_secret().await;

        // Use existing JWT implementation from auth module
        let token = crate::runtime::auth::create_jwt_token(user_id, permissions)
            .map_err(|e| RuntimeError::TokenCreationFailed {
                request_id: RequestId::new(),
                reason: e.to_string(),
            })?;

        Ok(TokenResult {
            token,
            expires_in: 86400, // 24 hours
            token_type: "Bearer".to_string(),
        })
    }
    
    /// Validate authentication token
    pub async fn validate_token(
        &self,
        token: &crate::runtime::auth_token::AuthToken,
        client_ip: std::net::IpAddr,
    ) -> RuntimeResult<AuthContext> {
        match token {
            crate::runtime::auth_token::AuthToken::Jwt(jwt) => {
                // Validate JWT using existing auth module implementation
                let token_data = crate::runtime::auth::validate_jwt_token(jwt)
                    .map_err(|e| RuntimeError::Auth(crate::runtime::error::AuthError::InvalidToken(
                        crate::runtime::auth_token::AuthTokenError::InvalidFormat(e.to_string())
                    )))?;

                Ok(AuthContext {
                    user_id: token_data.claims.sub,
                    permissions: token_data.claims.permissions,
                    auth_method: AuthMethod::Jwt,
                })
            }
            crate::runtime::auth_token::AuthToken::ApiKey(key) => {
                let key_info = self.secret_manager.validate_api_key(key, client_ip).await
                    .map_err(|e| {
                        tracing::warn!("API key validation failed: {}", e);
                        RuntimeError::Auth(crate::runtime::error::AuthError::InvalidToken(
                            crate::runtime::auth_token::AuthTokenError::InvalidFormat
                        ))
                    })?;
                
                Ok(AuthContext {
                    user_id: format!("api-key-{}", &key[3..11]),
                    permissions: key_info.permissions,
                    auth_method: AuthMethod::ApiKey,
                })
            }
        }
    }
}

/// Result of token creation
#[derive(Debug)]
pub struct TokenResult {
    pub token: String,
    pub expires_in: u64,
    pub token_type: String,
}

/// Authentication context
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub permissions: Vec<String>,
    pub auth_method: AuthMethod,
}

/// Authentication method
#[derive(Debug, Clone)]
pub enum AuthMethod {
    Jwt,
    ApiKey,
}

// Agent Coordinator Implementations (Factory Pattern)

/// Simple echo coordinator that returns the input
struct EchoCoordinator;

impl EchoCoordinator {
    fn new() -> Self {
        Self
    }
}

impl CoordinatorTrait for EchoCoordinator {
    fn step(&mut self, input: String) -> String {
        format!("Echo: {}", input)
    }

    fn get_agent_type(&self) -> &'static str {
        "EchoAgent"
    }
}

/// Advanced coordinator with configurable behavior
struct AdvancedCoordinator {
    config: HashMap<String, serde_json::Value>,
}

impl AdvancedCoordinator {
    fn new(config: HashMap<String, serde_json::Value>) -> Self {
        Self { config }
    }
}

impl CoordinatorTrait for AdvancedCoordinator {
    fn step(&mut self, input: String) -> String {
        // Process input with advanced logic (placeholder)
        let prefix = self.config
            .get("response_prefix")
            .and_then(|v| v.as_str())
            .unwrap_or("Advanced");

        format!("{}: Processed '{}'", prefix, input)
    }

    fn get_agent_type(&self) -> &'static str {
        "AdvancedDemoAgent"
    }
}

/// Analytics coordinator for data processing
struct AnalyticsCoordinator {
    config: HashMap<String, serde_json::Value>,
    request_count: usize,
}

impl AnalyticsCoordinator {
    fn new(config: HashMap<String, serde_json::Value>) -> Self {
        Self {
            config,
            request_count: 0,
        }
    }
}

impl CoordinatorTrait for AnalyticsCoordinator {
    fn step(&mut self, input: String) -> String {
        self.request_count += 1;

        // Perform analytics on input (placeholder)
        let word_count = input.split_whitespace().count();
        let char_count = input.chars().count();

        format!(
            "Analytics: Request #{} | Input: {} words, {} chars | Raw: '{}'",
            self.request_count, word_count, char_count, input
        )
    }

    fn get_agent_type(&self) -> &'static str {
        "AnalyticsAgent"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_service_creation() {
        let service = AgentService::new(ServiceConfig::default());

        // Test health status
        let health = service.get_health_status();
        assert!(health.state.is_healthy());
    }

    #[tokio::test]
    async fn test_list_agents_pagination() {
        let service = AgentService::new(ServiceConfig::default());

        let result = service.list_agents(Some(10), Some(0)).await.unwrap();
        assert_eq!(result.agents.len(), 0); // No agents created yet
        assert_eq!(result.total, 0);
        assert_eq!(result.offset, 0);
        assert_eq!(result.limit, 10);
    }

    #[test]
    fn test_echo_coordinator() {
        let mut coordinator = EchoCoordinator::new();
        let result = coordinator.step("test input".to_string());
        assert_eq!(result, "Echo: test input");
        assert_eq!(coordinator.get_agent_type(), "EchoAgent");
    }

    #[test]
    fn test_advanced_coordinator() {
        let mut config = HashMap::new();
        config.insert("response_prefix".to_string(), serde_json::json!("Custom"));

        let mut coordinator = AdvancedCoordinator::new(config);
        let result = coordinator.step("test".to_string());
        assert!(result.contains("Custom"));
        assert!(result.contains("test"));
    }

    #[test]
    fn test_analytics_coordinator() {
        let mut coordinator = AnalyticsCoordinator::new(HashMap::new());

        let result1 = coordinator.step("hello world".to_string());
        assert!(result1.contains("Request #1"));
        assert!(result1.contains("2 words"));

        let result2 = coordinator.step("test".to_string());
        assert!(result2.contains("Request #2"));
        assert!(result2.contains("1 words"));
    }
}