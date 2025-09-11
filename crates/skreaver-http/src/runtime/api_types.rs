//! Improved API types with better ergonomics and type safety

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// Validated response delivery method with compile-time safety
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseDelivery {
    /// Return complete response after processing
    Complete,
    /// Stream response in real-time via Server-Sent Events
    /// Only available for streaming-capable agents
    Streaming {
        /// Buffer size for streaming chunks
        #[serde(default = "default_stream_buffer_size")]
        buffer_size: u32,
    },
    /// Return both immediate acknowledgment and stream results
    /// Only available for streaming-capable agents
    Hybrid {
        /// Buffer size for streaming chunks
        #[serde(default = "default_stream_buffer_size")]
        buffer_size: u32,
        /// Include processing metadata in acknowledgment
        #[serde(default)]
        include_metadata: bool,
    },
}

impl ResponseDelivery {
    /// Create a complete delivery mode
    pub fn complete() -> Self {
        Self::Complete
    }

    /// Create a streaming delivery mode with validation
    pub fn streaming(agent_type: &AgentType) -> Result<Self, DeliveryError> {
        if !agent_type.supports_streaming() {
            return Err(DeliveryError::StreamingNotSupported(agent_type.clone()));
        }
        Ok(Self::Streaming {
            buffer_size: default_stream_buffer_size(),
        })
    }

    /// Create a hybrid delivery mode with validation
    pub fn hybrid(agent_type: &AgentType) -> Result<Self, DeliveryError> {
        if !agent_type.supports_streaming() {
            return Err(DeliveryError::StreamingNotSupported(agent_type.clone()));
        }
        Ok(Self::Hybrid {
            buffer_size: default_stream_buffer_size(),
            include_metadata: false,
        })
    }

    /// Create a streaming delivery with custom buffer size
    pub fn streaming_with_buffer(
        agent_type: &AgentType,
        buffer_size: u32,
    ) -> Result<Self, DeliveryError> {
        if !agent_type.supports_streaming() {
            return Err(DeliveryError::StreamingNotSupported(agent_type.clone()));
        }
        if buffer_size == 0 || buffer_size > MAX_STREAM_BUFFER_SIZE {
            return Err(DeliveryError::InvalidBufferSize {
                size: buffer_size,
                max: MAX_STREAM_BUFFER_SIZE,
            });
        }
        Ok(Self::Streaming { buffer_size })
    }

    /// Check if this delivery mode requires streaming support
    pub fn requires_streaming(&self) -> bool {
        matches!(self, Self::Streaming { .. } | Self::Hybrid { .. })
    }

    /// Validate delivery mode against agent capabilities
    pub fn validate_for_agent(&self, agent_type: &AgentType) -> Result<(), DeliveryError> {
        if self.requires_streaming() && !agent_type.supports_streaming() {
            return Err(DeliveryError::StreamingNotSupported(agent_type.clone()));
        }
        Ok(())
    }
}

impl Default for ResponseDelivery {
    fn default() -> Self {
        Self::Complete
    }
}

/// Default streaming buffer size (4KB)
fn default_stream_buffer_size() -> u32 {
    4 * 1024
}

/// Maximum streaming buffer size (64KB)
const MAX_STREAM_BUFFER_SIZE: u32 = 64 * 1024;

/// Errors when configuring response delivery
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryError {
    StreamingNotSupported(AgentType),
    InvalidBufferSize { size: u32, max: u32 },
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StreamingNotSupported(agent_type) => {
                write!(f, "Agent type '{}' does not support streaming", agent_type)
            }
            Self::InvalidBufferSize { size, max } => {
                write!(f, "Invalid buffer size: {} (must be 1-{})", size, max)
            }
        }
    }
}

impl std::error::Error for DeliveryError {}

/// Validated agent observation with size and content limits
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentObservation {
    /// The observation content (validated for size and format)
    #[schema(example = "Analyze the user behavior patterns")]
    pub content: String,
    /// Response delivery mode
    #[serde(default)]
    pub delivery: ResponseDelivery,
    /// Optional context metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl AgentObservation {
    /// Create a new observation with validation
    pub fn new(content: String) -> Result<Self, ObservationError> {
        Self::with_delivery(content, ResponseDelivery::default())
    }

    /// Create observation with specific delivery mode
    pub fn with_delivery(
        content: String,
        delivery: ResponseDelivery,
    ) -> Result<Self, ObservationError> {
        // Validate content
        if content.is_empty() {
            return Err(ObservationError::EmptyContent);
        }

        if content.len() > MAX_OBSERVATION_LENGTH {
            return Err(ObservationError::ContentTooLarge {
                actual_size: content.len(),
                max_size: MAX_OBSERVATION_LENGTH,
            });
        }

        // Check for potentially problematic content
        if content
            .chars()
            .filter(|c| c.is_control() && *c != '\n' && *c != '\t')
            .count()
            > 0
        {
            return Err(ObservationError::InvalidCharacters);
        }

        Ok(Self {
            content,
            delivery,
            metadata: HashMap::new(),
        })
    }

    /// Create observation with validated streaming delivery for specific agent type
    pub fn with_streaming(
        content: String,
        agent_type: &AgentType,
    ) -> Result<Self, ObservationError> {
        let delivery =
            ResponseDelivery::streaming(agent_type).map_err(ObservationError::InvalidDelivery)?;
        Self::with_delivery(content, delivery)
    }

    /// Create observation with validated hybrid delivery for specific agent type
    pub fn with_hybrid(content: String, agent_type: &AgentType) -> Result<Self, ObservationError> {
        let delivery =
            ResponseDelivery::hybrid(agent_type).map_err(ObservationError::InvalidDelivery)?;
        Self::with_delivery(content, delivery)
    }

    /// Validate the observation's delivery mode against an agent type
    pub fn validate_for_agent(&self, agent_type: &AgentType) -> Result<(), ObservationError> {
        self.delivery
            .validate_for_agent(agent_type)
            .map_err(ObservationError::InvalidDelivery)
    }

    /// Add metadata to the observation
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Maximum observation content length (256KB)
const MAX_OBSERVATION_LENGTH: usize = 256 * 1024;

/// Errors when creating observations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationError {
    EmptyContent,
    ContentTooLarge { actual_size: usize, max_size: usize },
    InvalidCharacters,
    InvalidDelivery(DeliveryError),
}

impl std::fmt::Display for ObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyContent => write!(f, "Observation content cannot be empty"),
            Self::ContentTooLarge {
                actual_size,
                max_size,
            } => {
                write!(
                    f,
                    "Observation content too large: {} bytes (max: {} bytes)",
                    actual_size, max_size
                )
            }
            Self::InvalidCharacters => write!(f, "Observation contains invalid control characters"),
            Self::InvalidDelivery(err) => write!(f, "Invalid delivery mode: {}", err),
        }
    }
}

impl std::error::Error for ObservationError {}

/// Strongly-typed agent creation specification
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentSpec {
    /// Agent type identifier
    pub agent_type: AgentType,
    /// Human-readable agent name
    #[schema(example = "my-analysis-agent")]
    pub name: Option<String>,
    /// Initial configuration parameters
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub config: HashMap<String, serde_json::Value>,
    /// Resource limits for the agent
    #[serde(default)]
    pub limits: AgentLimits,
}

/// Supported agent types with compile-time guarantees
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    /// Simple echo agent for testing
    Echo,
    /// Advanced processing agent
    Advanced,
    /// Analytics-focused agent
    Analytics,
    /// Custom agent type (for extensibility)
    Custom(String),
}

impl AgentType {
    /// Get the implementation class name for this agent type
    pub fn implementation_name(&self) -> &str {
        match self {
            Self::Echo => "EchoAgent",
            Self::Advanced => "AdvancedDemoAgent",
            Self::Analytics => "AnalyticsAgent",
            Self::Custom(name) => name,
        }
    }

    /// Check if agent type supports specific features
    pub fn supports_streaming(&self) -> bool {
        matches!(self, Self::Advanced | Self::Analytics)
    }

    /// Get default resource limits for agent type
    pub fn default_limits(&self) -> AgentLimits {
        match self {
            Self::Echo => AgentLimits {
                max_memory_mb: 16,
                max_observation_size_kb: 64,
                max_concurrent_tools: 1,
                execution_timeout_secs: 10,
            },
            Self::Advanced | Self::Analytics => AgentLimits {
                max_memory_mb: 512,
                max_observation_size_kb: 1024,
                max_concurrent_tools: 5,
                execution_timeout_secs: 60,
            },
            Self::Custom(_) => AgentLimits::default(),
        }
    }
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Echo => write!(f, "echo"),
            Self::Advanced => write!(f, "advanced"),
            Self::Analytics => write!(f, "analytics"),
            Self::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Resource limits for agent execution
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentLimits {
    /// Maximum memory usage in megabytes
    #[schema(example = 128)]
    pub max_memory_mb: u32,
    /// Maximum observation size in kilobytes
    #[schema(example = 512)]
    pub max_observation_size_kb: u32,
    /// Maximum number of concurrent tool calls
    #[schema(example = 3)]
    pub max_concurrent_tools: u32,
    /// Execution timeout in seconds
    #[schema(example = 30)]
    pub execution_timeout_secs: u32,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 64,
            max_observation_size_kb: 256,
            max_concurrent_tools: 2,
            execution_timeout_secs: 30,
        }
    }
}

/// Comprehensive agent status with detailed information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentStatusResponse {
    /// Agent identifier
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Agent type
    pub agent_type: AgentType,
    /// Current execution status
    pub status: crate::runtime::agent_status::AgentStatus,
    /// Resource usage information
    pub resource_usage: ResourceUsage,
    /// Performance metrics
    pub metrics: AgentMetrics,
    /// Agent creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last activity timestamp
    pub last_activity: chrono::DateTime<chrono::Utc>,
    /// Agent configuration
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub config: HashMap<String, serde_json::Value>,
}

/// Current resource usage for an agent
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ResourceUsage {
    /// Current memory usage in bytes
    #[schema(example = 1048576)]
    pub memory_bytes: u64,
    /// Memory usage percentage of limit
    #[schema(example = 25.5)]
    pub memory_percent: f32,
    /// Number of active tool calls
    #[schema(example = 1)]
    pub active_tools: u32,
}

/// Performance metrics for an agent
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentMetrics {
    /// Total observations processed
    #[schema(example = 42)]
    pub observations_processed: u64,
    /// Total tool calls made
    #[schema(example = 13)]
    pub tool_calls_made: u64,
    /// Average response time in milliseconds
    #[schema(example = 125.5)]
    pub avg_response_time_ms: f32,
    /// Success rate (0.0 to 1.0)
    #[schema(example = 0.95)]
    pub success_rate: f32,
    /// Last error (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Enhanced response for agent observation with timing and metadata
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentResponse {
    /// Agent that processed the observation
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Agent's response content
    #[schema(example = "Based on the analysis, I found 3 key patterns...")]
    pub content: String,
    /// Response delivery mode used
    pub delivery: ResponseDelivery,
    /// Processing time in milliseconds
    #[schema(example = 150)]
    pub processing_time_ms: u32,
    /// Number of tools called during processing
    #[schema(example = 2)]
    pub tools_called: u32,
    /// Response timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Response metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Request to create a new agent with validation
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAgentRequest {
    /// Agent specification
    pub spec: AgentSpec,
}

/// Response after creating an agent
#[derive(Debug, Serialize, ToSchema)]
pub struct CreateAgentResponse {
    /// Created agent identifier
    #[schema(example = "agent-12345")]
    pub agent_id: String,
    /// Agent specification that was used
    pub spec: AgentSpec,
    /// Current agent status
    pub status: crate::runtime::agent_status::AgentStatus,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Endpoints available for this agent
    pub endpoints: AgentEndpoints,
}

/// Available endpoints for an agent
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AgentEndpoints {
    /// Status endpoint URL
    #[schema(example = "/agents/agent-12345/status")]
    pub status: String,
    /// Observation endpoint URL
    #[schema(example = "/agents/agent-12345/observe")]
    pub observe: String,
    /// Streaming endpoint URL (if supported)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "/agents/agent-12345/stream")]
    pub stream: Option<String>,
}

impl AgentEndpoints {
    /// Create endpoints for an agent
    pub fn for_agent(agent_id: &str, agent_type: &AgentType) -> Self {
        let base = format!("/agents/{}", agent_id);
        Self {
            status: format!("{}/status", base),
            observe: format!("{}/observe", base),
            stream: if agent_type.supports_streaming() {
                Some(format!("{}/stream", base))
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_observation_validation() {
        // Valid observation
        let obs = AgentObservation::new("Test input".to_string()).unwrap();
        assert_eq!(obs.content, "Test input");
        assert_eq!(obs.delivery, ResponseDelivery::Complete);

        // Empty content
        assert!(matches!(
            AgentObservation::new("".to_string()),
            Err(ObservationError::EmptyContent)
        ));

        // Too large content
        let large_content = "a".repeat(MAX_OBSERVATION_LENGTH + 1);
        assert!(matches!(
            AgentObservation::new(large_content),
            Err(ObservationError::ContentTooLarge { .. })
        ));

        // Test streaming validation
        let streaming_obs =
            AgentObservation::with_streaming("Test streaming".to_string(), &AgentType::Advanced)
                .unwrap();
        assert!(streaming_obs.delivery.requires_streaming());

        // Test invalid streaming for non-streaming agent
        assert!(matches!(
            AgentObservation::with_streaming("Test".to_string(), &AgentType::Echo),
            Err(ObservationError::InvalidDelivery(
                DeliveryError::StreamingNotSupported(_)
            ))
        ));
    }

    #[test]
    fn test_agent_type_features() {
        assert!(AgentType::Advanced.supports_streaming());
        assert!(!AgentType::Echo.supports_streaming());

        assert_eq!(
            AgentType::Advanced.implementation_name(),
            "AdvancedDemoAgent"
        );
    }

    #[test]
    fn test_response_delivery() {
        // Test complete delivery
        let complete = ResponseDelivery::complete();
        assert_eq!(complete, ResponseDelivery::Complete);
        assert!(!complete.requires_streaming());

        // Test streaming delivery validation
        let streaming = ResponseDelivery::streaming(&AgentType::Advanced).unwrap();
        assert!(streaming.requires_streaming());

        // Test streaming not supported
        assert!(matches!(
            ResponseDelivery::streaming(&AgentType::Echo),
            Err(DeliveryError::StreamingNotSupported(_))
        ));

        // Test buffer size validation
        assert!(matches!(
            ResponseDelivery::streaming_with_buffer(&AgentType::Advanced, 0),
            Err(DeliveryError::InvalidBufferSize { .. })
        ));

        assert!(matches!(
            ResponseDelivery::streaming_with_buffer(
                &AgentType::Advanced,
                MAX_STREAM_BUFFER_SIZE + 1
            ),
            Err(DeliveryError::InvalidBufferSize { .. })
        ));
    }

    #[test]
    fn test_agent_endpoints() {
        let endpoints = AgentEndpoints::for_agent("test-123", &AgentType::Advanced);
        assert_eq!(endpoints.status, "/agents/test-123/status");
        assert_eq!(endpoints.observe, "/agents/test-123/observe");
        assert!(endpoints.stream.is_some());

        let echo_endpoints = AgentEndpoints::for_agent("echo-456", &AgentType::Echo);
        assert!(echo_endpoints.stream.is_none());
    }
}
