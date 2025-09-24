//! Agent instance management with proper state tracking

use crate::runtime::agent_status::AgentStatus;
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
    pub status: Arc<RwLock<AgentStatus>>,
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
    /// Optional metadata for the agent
    pub metadata: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

/// Trait for agent coordinators to allow dynamic dispatch
pub trait CoordinatorTrait {
    fn step(&mut self, input: String) -> String;
    fn get_agent_type(&self) -> &'static str;
}

impl AgentInstance {
    /// Create a new agent instance
    pub fn new(
        id: AgentId,
        agent_type: String,
        coordinator: Box<dyn CoordinatorTrait + Send + Sync>,
    ) -> Self {
        let now = Utc::now();

        Self {
            id,
            agent_type,
            status: Arc::new(RwLock::new(AgentStatus::Ready)),
            created_at: now,
            last_activity: Arc::new(RwLock::new(now)),
            observation_count: Arc::new(AtomicU64::new(0)),
            tool_call_count: Arc::new(AtomicU64::new(0)),
            coordinator,
            metadata: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Update agent status
    pub async fn set_status(&self, status: AgentStatus) {
        let mut current_status = self.status.write().await;
        *current_status = status;
        self.update_last_activity().await;
    }

    /// Get current agent status
    pub async fn get_status(&self) -> AgentStatus {
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
    pub async fn set_metadata(&self, key: String, value: String) {
        let mut metadata = self.metadata.write().await;
        metadata.insert(key, value);
    }

    /// Get metadata value
    pub async fn get_metadata(&self, key: &str) -> Option<String> {
        let metadata = self.metadata.read().await;
        metadata.get(key).cloned()
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
        self.set_status(AgentStatus::Processing {
            current_task: "observing".to_string(),
            started_at: chrono::Utc::now(),
        })
        .await;
        self.increment_observations();

        // Execute the step
        let result = self.coordinator.step(input);

        // Set status back to ready
        self.set_status(AgentStatus::Ready).await;

        Ok(result)
    }
}

/// Errors during agent execution
#[derive(Debug, Clone)]
pub enum AgentExecutionError {
    InvalidState {
        current_status: AgentStatus,
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
        assert_eq!(instance.get_status().await, AgentStatus::Ready);
        assert!(instance.can_accept_observations().await);

        // Set to processing
        instance
            .set_status(AgentStatus::Processing {
                current_task: "test".to_string(),
                started_at: chrono::Utc::now(),
            })
            .await;

        // Check that it's in processing state
        assert!(matches!(
            instance.get_status().await,
            AgentStatus::Processing { .. }
        ));
        assert!(!instance.can_accept_observations().await);
    }
}
