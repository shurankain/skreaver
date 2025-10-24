//! Service layer for HTTP runtime with proper separation of concerns

use std::sync::Arc;
use chrono::{DateTime, Utc};

use crate::runtime::{
    agent_instance::{AgentInstance, AgentId, AgentExecutionError},
    agent_status::AgentStatus,
    api_types::{AgentSpec, AgentObservation, AgentResponse, AgentStatusResponse, ResourceUsage, AgentMetrics},
    error::{RuntimeError, AgentError, RuntimeResult},
    performance::OptimizedAgentRegistry,
};

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

/// Configuration for agent service
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Default execution timeout
    pub default_timeout_secs: u32,
    /// Enable detailed metrics collection
    pub enable_metrics: bool,
    /// Maximum number of concurrent agent operations
    pub max_concurrent_operations: usize,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            default_timeout_secs: 30,
            enable_metrics: true,
            max_concurrent_operations: 100,
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
            healthy: true, // Would implement actual health checks
            uptime_seconds: 0, // Would track actual uptime
            active_agents: registry_metrics.lookups.load(std::sync::atomic::Ordering::Relaxed),
            service_metrics,
            timestamp: Utc::now(),
        }
    }
    
    // Private helper methods
    
    async fn create_agent_instance(
        &self,
        _agent_id: &AgentId,
        _spec: &AgentSpec,
    ) -> RuntimeResult<Arc<AgentInstance>> {
        // TODO: Implement agent factory pattern
        // This would create different agent types based on spec.agent_type
        Err(RuntimeError::Agent(AgentError::CreationFailed(
            "Agent factory not implemented".to_string()
        )))
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

/// Health status of the service
#[derive(Debug)]
pub struct HealthStatus {
    pub healthy: bool,
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
        let secret = self.secret_manager.get_jwt_secret().await;
        
        // TODO: Implement proper JWT creation using the secret
        // This is a placeholder that would use the jsonwebtoken crate
        
        Ok(TokenResult {
            token: "generated-token".to_string(),
            expires_in: 86400,
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
                // TODO: Validate JWT using secret manager
                Ok(AuthContext {
                    user_id: "jwt-user".to_string(),
                    permissions: vec!["read".to_string(), "write".to_string()],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_service_creation() {
        let service = AgentService::new(ServiceConfig::default());
        
        // Test health status
        let health = service.get_health_status();
        assert!(health.healthy);
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
}