//! Agent instance management with proper state tracking

use crate::runtime::agent_status::AgentStatusEnum;
use crate::runtime::api_types::AgentInstanceMetadata;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Unique identifier for agent instances
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new agent ID with validation
    pub fn new(id: String) -> Result<Self, AgentIdError> {
        if id.is_empty() {
            return Err(AgentIdError::Empty);
        }

        if id.len() > 128 {
            return Err(AgentIdError::TooLong);
        }

        // Validate characters (alphanumeric, hyphens, underscores only)
        if !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AgentIdError::InvalidCharacters);
        }

        Ok(Self(id))
    }

    /// Get the string representation of the ID
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<AgentId> for String {
    fn from(id: AgentId) -> Self {
        id.0
    }
}

/// Errors when creating agent IDs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentIdError {
    Empty,
    TooLong,
    InvalidCharacters,
}

impl std::fmt::Display for AgentIdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "Agent ID cannot be empty"),
            Self::TooLong => write!(f, "Agent ID cannot exceed 128 characters"),
            Self::InvalidCharacters => write!(
                f,
                "Agent ID can only contain alphanumeric characters, hyphens, and underscores"
            ),
        }
    }
}

impl std::error::Error for AgentIdError {}

/// Comprehensive agent instance with state tracking
pub struct AgentInstance {
    /// Agent identifier
    pub id: AgentId,
    /// Agent type name
    pub agent_type: String,
    /// Current execution status
    pub status: Arc<RwLock<AgentStatusEnum>>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last activity timestamp
    pub last_activity: Arc<RwLock<DateTime<Utc>>>,
    /// Number of observations processed
    pub observation_count: Arc<AtomicU64>,
    /// Number of tool calls made
    pub tool_call_count: Arc<AtomicU64>,
    /// Agent coordinator (boxed trait object)
    pub coordinator: Box<dyn CoordinatorTrait + Send + Sync>,
    /// Structured instance metadata for comprehensive tracking
    pub instance_metadata: Arc<RwLock<AgentInstanceMetadata>>,
}

/// Trait for agent coordinators to allow dynamic dispatch
pub trait CoordinatorTrait {
    fn step(&mut self, input: String) -> String;
    fn get_agent_type(&self) -> &'static str;
}

impl AgentInstance {
    /// Create a new agent instance with default metadata
    pub fn new(
        id: AgentId,
        agent_type: String,
        coordinator: Box<dyn CoordinatorTrait + Send + Sync>,
    ) -> Self {
        let now = Utc::now();

        Self {
            id,
            agent_type,
            status: Arc::new(RwLock::new(AgentStatusEnum::Ready)),
            created_at: now,
            last_activity: Arc::new(RwLock::new(now)),
            observation_count: Arc::new(AtomicU64::new(0)),
            tool_call_count: Arc::new(AtomicU64::new(0)),
            coordinator,
            instance_metadata: Arc::new(RwLock::new(AgentInstanceMetadata::default())),
        }
    }

    /// Create a new agent instance with custom metadata
    pub fn new_with_metadata(
        id: AgentId,
        agent_type: String,
        coordinator: Box<dyn CoordinatorTrait + Send + Sync>,
        instance_metadata: AgentInstanceMetadata,
    ) -> Self {
        let now = Utc::now();

        Self {
            id,
            agent_type,
            status: Arc::new(RwLock::new(AgentStatusEnum::Ready)),
            created_at: now,
            last_activity: Arc::new(RwLock::new(now)),
            observation_count: Arc::new(AtomicU64::new(0)),
            tool_call_count: Arc::new(AtomicU64::new(0)),
            coordinator,
            instance_metadata: Arc::new(RwLock::new(instance_metadata)),
        }
    }

    /// Update agent status
    pub async fn set_status(&self, status: AgentStatusEnum) {
        let mut current_status = self.status.write().await;
        *current_status = status;
        self.update_last_activity().await;
    }

    /// Get current agent status
    pub async fn get_status(&self) -> AgentStatusEnum {
        self.status.read().await.clone()
    }

    /// Check if agent can accept new observations
    pub async fn can_accept_observations(&self) -> bool {
        let status = self.get_status().await;
        status.can_accept_observations()
    }

    /// Update last activity timestamp
    pub async fn update_last_activity(&self) {
        let mut last_activity = self.last_activity.write().await;
        *last_activity = Utc::now();
    }

    /// Get last activity timestamp
    pub async fn get_last_activity(&self) -> DateTime<Utc> {
        *self.last_activity.read().await
    }

    /// Increment observation count
    pub fn increment_observations(&self) {
        self.observation_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment tool call count
    pub fn increment_tool_calls(&self) {
        self.tool_call_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Get observation count
    pub fn get_observation_count(&self) -> u64 {
        self.observation_count.load(Ordering::Relaxed)
    }

    /// Get tool call count
    pub fn get_tool_call_count(&self) -> u64 {
        self.tool_call_count.load(Ordering::Relaxed)
    }

    /// Set metadata value
    ///
    /// # Deprecated
    /// Use `add_tag()` for simple key-value pairs or `add_custom_metadata()` for complex data.
    ///
    /// Migration:
    /// - `agent.set_metadata("key".to_string(), "value".to_string())` → `agent.add_tag("key".to_string(), "value".to_string())`
    #[deprecated(
        since = "0.5.0",
        note = "Use add_tag() for tags or add_custom_metadata() for complex metadata"
    )]
    pub async fn set_metadata(&self, key: String, value: String) {
        self.add_tag(key, value).await;
    }

    /// Get metadata value
    ///
    /// # Deprecated
    /// Use `get_instance_metadata().tags.get(key)` or `get_instance_metadata().custom.get(key)`.
    ///
    /// Migration:
    /// - `agent.get_metadata("key")` → `agent.get_instance_metadata().await.tags.get("key").cloned()`
    #[deprecated(
        since = "0.5.0",
        note = "Use get_instance_metadata().tags or get_instance_metadata().custom"
    )]
    pub async fn get_metadata(&self, key: &str) -> Option<String> {
        let metadata = self.instance_metadata.read().await;
        metadata.tags.get(key).cloned()
    }

    /// Get instance metadata
    pub async fn get_instance_metadata(&self) -> AgentInstanceMetadata {
        self.instance_metadata.read().await.clone()
    }

    /// Update instance metadata
    pub async fn update_instance_metadata<F>(&self, updater: F)
    where
        F: FnOnce(&mut AgentInstanceMetadata),
    {
        let mut metadata = self.instance_metadata.write().await;
        updater(&mut metadata);
    }

    /// Add a tag to instance metadata
    pub async fn add_tag(&self, key: String, value: String) {
        let mut metadata = self.instance_metadata.write().await;
        metadata.tags.insert(key, value);
    }

    /// Add custom metadata field
    pub async fn add_custom_metadata(&self, key: String, value: serde_json::Value) {
        let mut metadata = self.instance_metadata.write().await;
        metadata.custom.insert(key, value);
    }

    /// Execute a step with proper state management
    pub async fn execute_step(&mut self, input: String) -> Result<String, AgentExecutionError> {
        // Check if agent can accept observations
        if !self.can_accept_observations().await {
            let current_status = self.get_status().await;
            return Err(AgentExecutionError::InvalidState {
                current_status,
                attempted_operation: "observe".to_string(),
            });
        }

        // Set status to processing
        self.set_status(AgentStatusEnum::Processing {
            current_task: "observing".to_string(),
            started_at: chrono::Utc::now(),
        })
        .await;
        self.increment_observations();

        // Execute the step
        let result = self.coordinator.step(input);

        // Set status back to ready
        self.set_status(AgentStatusEnum::Ready).await;

        Ok(result)
    }
}

/// Errors during agent execution
#[derive(Debug, Clone)]
pub enum AgentExecutionError {
    InvalidState {
        current_status: AgentStatusEnum,
        attempted_operation: String,
    },
    ExecutionTimeout,
    CoordinatorError(String),
}

impl std::fmt::Display for AgentExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidState {
                current_status,
                attempted_operation,
            } => {
                write!(
                    f,
                    "Cannot perform '{}' operation: agent is in '{}' state",
                    attempted_operation, current_status
                )
            }
            Self::ExecutionTimeout => write!(f, "Agent execution timed out"),
            Self::CoordinatorError(msg) => write!(f, "Coordinator error: {}", msg),
        }
    }
}

impl std::error::Error for AgentExecutionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_validation() {
        // Valid IDs
        assert!(AgentId::new("test-agent".to_string()).is_ok());
        assert!(AgentId::new("agent_123".to_string()).is_ok());
        assert!(AgentId::new("a".to_string()).is_ok());

        // Invalid IDs
        assert!(matches!(
            AgentId::new("".to_string()),
            Err(AgentIdError::Empty)
        ));
        assert!(matches!(
            AgentId::new("a".repeat(129)),
            Err(AgentIdError::TooLong)
        ));
        assert!(matches!(
            AgentId::new("test@agent".to_string()),
            Err(AgentIdError::InvalidCharacters)
        ));
    }

    #[tokio::test]
    async fn test_agent_status_transitions() {
        struct MockCoordinator;
        impl CoordinatorTrait for MockCoordinator {
            fn step(&mut self, _input: String) -> String {
                "response".to_string()
            }
            fn get_agent_type(&self) -> &'static str {
                "mock"
            }
        }

        let agent_id = AgentId::new("test-agent".to_string()).unwrap();
        let instance =
            AgentInstance::new(agent_id, "MockAgent".to_string(), Box::new(MockCoordinator));

        // Initially ready
        assert_eq!(instance.get_status().await, AgentStatusEnum::Ready);
        assert!(instance.can_accept_observations().await);

        // Set to processing
        instance
            .set_status(AgentStatusEnum::Processing {
                current_task: "test".to_string(),
                started_at: chrono::Utc::now(),
            })
            .await;

        // Check that it's in processing state
        assert!(matches!(
            instance.get_status().await,
            AgentStatusEnum::Processing { .. }
        ));
        assert!(!instance.can_accept_observations().await);
    }

    #[tokio::test]
    async fn test_instance_metadata_default() {
        struct MockCoordinator;
        impl CoordinatorTrait for MockCoordinator {
            fn step(&mut self, _input: String) -> String {
                "response".to_string()
            }
            fn get_agent_type(&self) -> &'static str {
                "mock"
            }
        }

        let agent_id = AgentId::new("test-agent".to_string()).unwrap();
        let instance =
            AgentInstance::new(agent_id, "MockAgent".to_string(), Box::new(MockCoordinator));

        let metadata = instance.get_instance_metadata().await;

        // Default metadata should have a generated instance_id and version
        assert!(!metadata.instance_id.is_empty());
        assert!(metadata.version.is_some());
    }

    #[tokio::test]
    async fn test_instance_metadata_custom() {
        struct MockCoordinator;
        impl CoordinatorTrait for MockCoordinator {
            fn step(&mut self, _input: String) -> String {
                "response".to_string()
            }
            fn get_agent_type(&self) -> &'static str {
                "mock"
            }
        }

        let custom_metadata = AgentInstanceMetadata::minimal("custom-instance-id".to_string())
            .with_tag("team".to_string(), "data-science".to_string())
            .with_tag("env".to_string(), "staging".to_string());

        let agent_id = AgentId::new("test-agent".to_string()).unwrap();
        let instance = AgentInstance::new_with_metadata(
            agent_id,
            "MockAgent".to_string(),
            Box::new(MockCoordinator),
            custom_metadata,
        );

        let metadata = instance.get_instance_metadata().await;

        assert_eq!(metadata.instance_id, "custom-instance-id");
        assert_eq!(metadata.tags.get("team"), Some(&"data-science".to_string()));
        assert_eq!(metadata.tags.get("env"), Some(&"staging".to_string()));
    }

    #[tokio::test]
    async fn test_add_tag() {
        struct MockCoordinator;
        impl CoordinatorTrait for MockCoordinator {
            fn step(&mut self, _input: String) -> String {
                "response".to_string()
            }
            fn get_agent_type(&self) -> &'static str {
                "mock"
            }
        }

        let agent_id = AgentId::new("test-agent".to_string()).unwrap();
        let instance =
            AgentInstance::new(agent_id, "MockAgent".to_string(), Box::new(MockCoordinator));

        // Add tags
        instance
            .add_tag("region".to_string(), "us-east-1".to_string())
            .await;
        instance
            .add_tag("purpose".to_string(), "testing".to_string())
            .await;

        let metadata = instance.get_instance_metadata().await;

        assert_eq!(metadata.tags.get("region"), Some(&"us-east-1".to_string()));
        assert_eq!(metadata.tags.get("purpose"), Some(&"testing".to_string()));
        assert_eq!(metadata.tags.len(), 2);
    }

    #[tokio::test]
    async fn test_add_custom_metadata() {
        struct MockCoordinator;
        impl CoordinatorTrait for MockCoordinator {
            fn step(&mut self, _input: String) -> String {
                "response".to_string()
            }
            fn get_agent_type(&self) -> &'static str {
                "mock"
            }
        }

        let agent_id = AgentId::new("test-agent".to_string()).unwrap();
        let instance =
            AgentInstance::new(agent_id, "MockAgent".to_string(), Box::new(MockCoordinator));

        // Add custom metadata
        instance
            .add_custom_metadata("deployment_id".to_string(), serde_json::json!(12345))
            .await;
        instance
            .add_custom_metadata("config_version".to_string(), serde_json::json!("v2.1.0"))
            .await;

        let metadata = instance.get_instance_metadata().await;

        assert_eq!(
            metadata.custom.get("deployment_id"),
            Some(&serde_json::json!(12345))
        );
        assert_eq!(
            metadata.custom.get("config_version"),
            Some(&serde_json::json!("v2.1.0"))
        );
        assert_eq!(metadata.custom.len(), 2);
    }

    #[tokio::test]
    async fn test_update_instance_metadata() {
        struct MockCoordinator;
        impl CoordinatorTrait for MockCoordinator {
            fn step(&mut self, _input: String) -> String {
                "response".to_string()
            }
            fn get_agent_type(&self) -> &'static str {
                "mock"
            }
        }

        let agent_id = AgentId::new("test-agent".to_string()).unwrap();
        let instance =
            AgentInstance::new(agent_id, "MockAgent".to_string(), Box::new(MockCoordinator));

        // Update metadata using the updater function
        instance
            .update_instance_metadata(|metadata| {
                metadata.environment = Some("production".to_string());
                metadata
                    .tags
                    .insert("critical".to_string(), "true".to_string());
            })
            .await;

        let metadata = instance.get_instance_metadata().await;

        assert_eq!(metadata.environment, Some("production".to_string()));
        assert_eq!(metadata.tags.get("critical"), Some(&"true".to_string()));
    }
}
