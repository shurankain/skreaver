//! A2A Protocol Core Types
//!
//! This module defines the core data types for the Agent2Agent (A2A) protocol,
//! based on the official A2A specification.
//!
//! The A2A protocol enables communication and interoperability between AI agents,
//! allowing them to discover each other's capabilities, exchange messages, and
//! collaborate on tasks.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Task Types
// ============================================================================

/// A task represents a unit of work in the A2A protocol.
///
/// Tasks have a lifecycle that progresses through various states, and they
/// contain messages exchanged between agents and artifacts produced as output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique identifier for the task
    pub id: String,

    /// Optional context ID for grouping related tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,

    /// Current status of the task
    pub status: TaskStatus,

    /// Messages exchanged during the task
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<Message>,

    /// Artifacts produced by the task
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,

    /// When the task was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,

    /// When the task was last updated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Task {
    /// Create a new task with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            context_id: None,
            status: TaskStatus::Working,
            messages: Vec::new(),
            artifacts: Vec::new(),
            created_at: Some(Utc::now()),
            updated_at: Some(Utc::now()),
            metadata: HashMap::new(),
        }
    }

    /// Create a new task with a generated UUID
    pub fn new_with_uuid() -> Self {
        Self::new(Uuid::new_v4().to_string())
    }

    /// Add a message to the task
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Some(Utc::now());
    }

    /// Add an artifact to the task
    pub fn add_artifact(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
        self.updated_at = Some(Utc::now());
    }

    /// Update the task status
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Some(Utc::now());
    }

    /// Check if the task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            TaskStatus::Completed
                | TaskStatus::Failed
                | TaskStatus::Cancelled
                | TaskStatus::Rejected
        )
    }

    /// Check if the task requires input
    pub fn requires_input(&self) -> bool {
        self.status == TaskStatus::InputRequired
    }
}

/// Task status indicating the current state in the task lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TaskStatus {
    /// Task is actively being processed
    Working,

    /// Task completed successfully
    Completed,

    /// Task failed due to an error
    Failed,

    /// Task was cancelled by the user
    Cancelled,

    /// Task was rejected by the system
    Rejected,

    /// Task requires additional input to proceed
    InputRequired,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Working => write!(f, "working"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
            TaskStatus::Rejected => write!(f, "rejected"),
            TaskStatus::InputRequired => write!(f, "input-required"),
        }
    }
}

// ============================================================================
// Message Types
// ============================================================================

/// A message exchanged between agents during task execution
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Optional message identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Role of the message sender
    pub role: Role,

    /// Content parts of the message
    pub parts: Vec<Part>,

    /// References to related tasks
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_task_ids: Vec<String>,

    /// When the message was sent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Message {
    /// Create a new user message with text content
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            id: Some(Uuid::new_v4().to_string()),
            role: Role::User,
            parts: vec![Part::Text(TextPart {
                text: text.into(),
                metadata: HashMap::new(),
            })],
            reference_task_ids: Vec::new(),
            timestamp: Some(Utc::now()),
            metadata: HashMap::new(),
        }
    }

    /// Create a new agent message with text content
    pub fn agent(text: impl Into<String>) -> Self {
        Self {
            id: Some(Uuid::new_v4().to_string()),
            role: Role::Agent,
            parts: vec![Part::Text(TextPart {
                text: text.into(),
                metadata: HashMap::new(),
            })],
            reference_task_ids: Vec::new(),
            timestamp: Some(Utc::now()),
            metadata: HashMap::new(),
        }
    }

    /// Add a part to the message
    pub fn with_part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    /// Add a reference to another task
    pub fn with_reference(mut self, task_id: impl Into<String>) -> Self {
        self.reference_task_ids.push(task_id.into());
        self
    }
}

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Message from a user (or client agent acting on behalf of a user)
    User,

    /// Message from an agent
    Agent,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Agent => write!(f, "agent"),
        }
    }
}

// ============================================================================
// Part Types
// ============================================================================

/// A content part within a message or artifact
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Part {
    /// Text content
    #[serde(rename = "text")]
    Text(TextPart),

    /// File reference
    #[serde(rename = "file")]
    File(FilePart),

    /// Structured data
    #[serde(rename = "data")]
    Data(DataPart),
}

impl Part {
    /// Create a text part
    pub fn text(content: impl Into<String>) -> Self {
        Part::Text(TextPart {
            text: content.into(),
            metadata: HashMap::new(),
        })
    }

    /// Create a file part
    pub fn file(uri: impl Into<String>, media_type: impl Into<String>) -> Self {
        Part::File(FilePart {
            uri: uri.into(),
            media_type: media_type.into(),
            name: None,
            metadata: HashMap::new(),
        })
    }

    /// Create a data part
    pub fn data(data: serde_json::Value, media_type: impl Into<String>) -> Self {
        Part::Data(DataPart {
            data,
            media_type: media_type.into(),
            metadata: HashMap::new(),
        })
    }

    /// Get the text content if this is a text part
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Part::Text(t) => Some(&t.text),
            _ => None,
        }
    }
}

/// Text content part
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextPart {
    /// The text content
    pub text: String,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// File reference part
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePart {
    /// URI to the file
    pub uri: String,

    /// MIME type of the file
    pub media_type: String,

    /// Optional file name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Structured data part
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataPart {
    /// The structured data
    pub data: serde_json::Value,

    /// MIME type of the data (e.g., "application/json")
    pub media_type: String,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

// ============================================================================
// Artifact Types
// ============================================================================

/// An artifact produced as output from a task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// Unique identifier for the artifact
    pub id: String,

    /// Content parts of the artifact
    pub parts: Vec<Part>,

    /// MIME type of the artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,

    /// Human-readable label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Description of the artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Artifact {
    /// Create a new artifact with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            parts: Vec::new(),
            media_type: None,
            label: None,
            description: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a new artifact with a generated UUID
    pub fn new_with_uuid() -> Self {
        Self::new(Uuid::new_v4().to_string())
    }

    /// Create a text artifact
    pub fn text(id: impl Into<String>, content: impl Into<String>) -> Self {
        let mut artifact = Self::new(id);
        artifact.parts.push(Part::text(content));
        artifact.media_type = Some("text/plain".to_string());
        artifact
    }

    /// Create a JSON artifact
    pub fn json(id: impl Into<String>, data: serde_json::Value) -> Self {
        let mut artifact = Self::new(id);
        artifact.parts.push(Part::data(data, "application/json"));
        artifact.media_type = Some("application/json".to_string());
        artifact
    }

    /// Add a part to the artifact
    pub fn with_part(mut self, part: Part) -> Self {
        self.parts.push(part);
        self
    }

    /// Set the label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

// ============================================================================
// Agent Card Types
// ============================================================================

/// Agent Card for capability discovery
///
/// The Agent Card is a JSON document that describes an agent's capabilities,
/// skills, and how to interact with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    /// Unique identifier for the agent
    pub agent_id: String,

    /// Human-readable name of the agent
    pub name: String,

    /// Description of the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Provider information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProvider>,

    /// Agent capabilities
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Skills the agent can perform
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<AgentSkill>,

    /// Security schemes for authentication
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security_schemes: Vec<SecurityScheme>,

    /// Interfaces for interacting with the agent
    pub interfaces: Vec<AgentInterface>,

    /// Supported protocol versions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_versions: Vec<String>,

    /// Extensions supported by the agent
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<AgentExtension>,

    /// Optional signature for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<AgentCardSignature>,
}

impl AgentCard {
    /// Create a new agent card with required fields
    pub fn new(
        agent_id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            name: name.into(),
            description: None,
            provider: None,
            capabilities: AgentCapabilities::default(),
            skills: Vec::new(),
            security_schemes: Vec::new(),
            interfaces: vec![AgentInterface::http(base_url)],
            protocol_versions: vec!["0.3".to_string()],
            extensions: Vec::new(),
            signature: None,
        }
    }

    /// Add a skill to the agent card
    pub fn with_skill(mut self, skill: AgentSkill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Enable streaming capability
    pub fn with_streaming(mut self) -> Self {
        self.capabilities.streaming = true;
        self
    }

    /// Enable push notifications
    pub fn with_push_notifications(mut self) -> Self {
        self.capabilities.push_notifications = true;
        self
    }
}

/// Information about the agent provider
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProvider {
    /// Provider name
    pub name: String,

    /// Provider URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Agent capabilities
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    /// Whether the agent supports streaming responses
    #[serde(default)]
    pub streaming: bool,

    /// Whether the agent supports push notifications
    #[serde(default)]
    pub push_notifications: bool,

    /// Whether the agent provides an extended agent card
    #[serde(default)]
    pub extended_agent_card: bool,
}

/// A skill that the agent can perform
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    /// Unique identifier for the skill
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of what the skill does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Input schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,

    /// Output schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,

    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl AgentSkill {
    /// Create a new skill
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            input_schema: None,
            output_schema: None,
            tags: Vec::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the input schema
    pub fn with_input_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Set the output schema
    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}

/// Security scheme for authentication
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SecurityScheme {
    /// API key authentication
    #[serde(rename = "apiKey")]
    ApiKey {
        /// Name of the header or query parameter
        name: String,
        /// Where the key is sent
        #[serde(rename = "in")]
        location: ApiKeyLocation,
    },

    /// HTTP authentication (Bearer, Basic, etc.)
    #[serde(rename = "http")]
    Http {
        /// Authentication scheme (bearer, basic, etc.)
        scheme: String,
    },

    /// OAuth2 authentication
    #[serde(rename = "oauth2")]
    OAuth2 {
        /// OAuth2 flows
        flows: OAuth2Flows,
    },
}

/// Location of API key
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyLocation {
    Header,
    Query,
}

/// OAuth2 flows configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Flows {
    /// Authorization code flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_code: Option<OAuth2Flow>,

    /// Client credentials flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_credentials: Option<OAuth2Flow>,
}

/// OAuth2 flow configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Flow {
    /// Authorization URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,

    /// Token URL
    pub token_url: String,

    /// Available scopes
    #[serde(default)]
    pub scopes: HashMap<String, String>,
}

/// Interface for interacting with the agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentInterface {
    /// HTTP/REST interface
    #[serde(rename = "http")]
    Http {
        /// Base URL for the agent
        base_url: String,
    },

    /// gRPC interface
    #[serde(rename = "grpc")]
    Grpc {
        /// Host and port for gRPC
        host: String,
    },
}

impl AgentInterface {
    /// Create an HTTP interface
    pub fn http(base_url: impl Into<String>) -> Self {
        AgentInterface::Http {
            base_url: base_url.into(),
        }
    }

    /// Create a gRPC interface
    pub fn grpc(host: impl Into<String>) -> Self {
        AgentInterface::Grpc { host: host.into() }
    }
}

/// Extension supported by the agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentExtension {
    /// Extension identifier
    pub id: String,

    /// Extension version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Extension configuration
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub config: HashMap<String, serde_json::Value>,
}

/// Signature for agent card verification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardSignature {
    /// Algorithm used for signing
    pub algorithm: String,

    /// The signature value
    pub value: String,

    /// Key ID for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
}

// ============================================================================
// Streaming Event Types
// ============================================================================

/// Event for task status updates during streaming
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdateEvent {
    /// Task ID
    pub task_id: String,

    /// New status
    pub status: TaskStatus,

    /// Optional message with the update
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,

    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
}

/// Event for artifact updates during streaming
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskArtifactUpdateEvent {
    /// Task ID
    pub task_id: String,

    /// The artifact being added or updated
    pub artifact: Artifact,

    /// Whether this is the final update for this artifact
    #[serde(default)]
    pub is_final: bool,

    /// Timestamp of the event
    pub timestamp: DateTime<Utc>,
}

/// Unified streaming event type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamingEvent {
    /// Task status update
    #[serde(rename = "taskStatusUpdate")]
    TaskStatusUpdate(TaskStatusUpdateEvent),

    /// Artifact update
    #[serde(rename = "taskArtifactUpdate")]
    TaskArtifactUpdate(TaskArtifactUpdateEvent),
}

// ============================================================================
// Request/Response Types
// ============================================================================

/// Request to send a message and create/continue a task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    /// The message to send
    pub message: Message,

    /// Optional task ID to continue an existing task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,

    /// Optional context ID for grouping tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_id: Option<String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Response containing the task state after sending a message
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResponse {
    /// The updated task
    pub task: Task,
}

/// Request to get a task by ID
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskRequest {
    /// Task ID
    pub task_id: String,
}

/// Request to cancel a task
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    /// Task ID
    pub task_id: String,

    /// Optional reason for cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Push notification configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushNotificationConfig {
    /// Configuration ID
    pub id: String,

    /// Webhook URL to receive notifications
    pub webhook_url: String,

    /// Events to notify about
    #[serde(default)]
    pub events: Vec<String>,

    /// Optional secret for webhook verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("test-task-1");
        assert_eq!(task.id, "test-task-1");
        assert_eq!(task.status, TaskStatus::Working);
        assert!(!task.is_terminal());
    }

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::InputRequired;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"input-required\"");

        let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, TaskStatus::InputRequired);
    }

    #[test]
    fn test_message_creation() {
        let msg = Message::user("Hello, agent!");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.parts.len(), 1);
        assert_eq!(msg.parts[0].as_text(), Some("Hello, agent!"));
    }

    #[test]
    fn test_part_types() {
        let text = Part::text("Hello");
        let file = Part::file("https://example.com/file.pdf", "application/pdf");
        let data = Part::data(serde_json::json!({"key": "value"}), "application/json");

        assert!(matches!(text, Part::Text(_)));
        assert!(matches!(file, Part::File(_)));
        assert!(matches!(data, Part::Data(_)));
    }

    #[test]
    fn test_artifact_creation() {
        let artifact = Artifact::text("artifact-1", "Generated content")
            .with_label("Output")
            .with_description("The generated output");

        assert_eq!(artifact.id, "artifact-1");
        assert_eq!(artifact.label, Some("Output".to_string()));
        assert_eq!(artifact.parts.len(), 1);
    }

    #[test]
    fn test_agent_card_creation() {
        let card = AgentCard::new("agent-1", "Test Agent", "https://agent.example.com")
            .with_description("A test agent")
            .with_streaming()
            .with_skill(AgentSkill::new("summarize", "Summarize Text"));

        assert_eq!(card.agent_id, "agent-1");
        assert_eq!(card.name, "Test Agent");
        assert!(card.capabilities.streaming);
        assert_eq!(card.skills.len(), 1);
    }

    #[test]
    fn test_task_serialization() {
        let mut task = Task::new("task-123");
        task.add_message(Message::user("Hello"));
        task.context_id = Some("ctx-1".to_string());

        let json = serde_json::to_string_pretty(&task).unwrap();
        assert!(json.contains("\"id\": \"task-123\""));
        assert!(json.contains("\"contextId\": \"ctx-1\""));
        assert!(json.contains("\"status\": \"working\""));

        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, task.id);
        assert_eq!(parsed.context_id, task.context_id);
    }

    #[test]
    fn test_agent_interface_serialization() {
        let http = AgentInterface::http("https://agent.example.com");
        let json = serde_json::to_string(&http).unwrap();

        // Verify it can round-trip
        let parsed: AgentInterface = serde_json::from_str(&json).unwrap();
        assert!(
            matches!(parsed, AgentInterface::Http { base_url } if base_url == "https://agent.example.com")
        );
    }
}
