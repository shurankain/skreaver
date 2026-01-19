//! Protocol-agnostic types for the unified agent interface.
//!
//! These types provide a common abstraction over MCP and A2A protocols,
//! allowing agents to work with either protocol seamlessly.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Protocol identifier for agent communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// Model Context Protocol (Anthropic)
    Mcp,
    /// Agent2Agent Protocol (Google)
    A2a,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Mcp => write!(f, "mcp"),
            Protocol::A2a => write!(f, "a2a"),
        }
    }
}

/// Unified message role across protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// Message from a user/client
    User,
    /// Message from an agent/assistant
    Agent,
    /// System message
    System,
}

/// Content part of a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    /// Plain text content
    Text { text: String },
    /// Binary data with MIME type
    Data {
        data: String, // Base64 encoded
        mime_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// File reference
    File {
        uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// Tool call request
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    /// Tool call result
    ToolResult {
        id: String,
        result: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl ContentPart {
    /// Create a text content part.
    pub fn text(text: impl Into<String>) -> Self {
        ContentPart::Text { text: text.into() }
    }

    /// Create a data content part.
    pub fn data(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        ContentPart::Data {
            data: data.into(),
            mime_type: mime_type.into(),
            name: None,
        }
    }

    /// Create a file content part.
    pub fn file(uri: impl Into<String>) -> Self {
        ContentPart::File {
            uri: uri.into(),
            mime_type: None,
            name: None,
        }
    }

    /// Create a tool call content part.
    pub fn tool_call(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        ContentPart::ToolCall {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }

    /// Create a tool result content part.
    pub fn tool_result(id: impl Into<String>, result: serde_json::Value) -> Self {
        ContentPart::ToolResult {
            id: id.into(),
            result,
            is_error: None,
        }
    }

    /// Get text content if this is a text part.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            ContentPart::Text { text } => Some(text),
            _ => None,
        }
    }
}

/// A unified message type that works across protocols.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedMessage {
    /// Unique message identifier
    pub id: String,
    /// Role of the message sender
    pub role: MessageRole,
    /// Message content parts
    pub content: Vec<ContentPart>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp of when the message was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl UnifiedMessage {
    /// Create a new message with the given role and text content.
    pub fn new(role: MessageRole, text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            content: vec![ContentPart::text(text)],
            metadata: HashMap::new(),
            timestamp: Some(chrono::Utc::now()),
        }
    }

    /// Create a user message.
    pub fn user(text: impl Into<String>) -> Self {
        Self::new(MessageRole::User, text)
    }

    /// Create an agent message.
    pub fn agent(text: impl Into<String>) -> Self {
        Self::new(MessageRole::Agent, text)
    }

    /// Create a system message.
    pub fn system(text: impl Into<String>) -> Self {
        Self::new(MessageRole::System, text)
    }

    /// Add a content part to the message.
    pub fn with_part(mut self, part: ContentPart) -> Self {
        self.content.push(part);
        self
    }

    /// Add metadata to the message.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get all text content concatenated.
    pub fn text_content(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| p.as_text())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Status of a task/operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Task is waiting to be processed
    Pending,
    /// Task is currently being processed
    Working,
    /// Task requires user input
    InputRequired,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
}

impl TaskStatus {
    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }
}

/// An artifact/output produced by a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Unique artifact identifier
    pub id: String,
    /// Name of the artifact
    pub name: String,
    /// MIME type of the artifact
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Artifact content parts
    pub content: Vec<ContentPart>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Artifact {
    /// Create a new artifact with text content.
    pub fn text(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            mime_type: Some("text/plain".to_string()),
            content: vec![ContentPart::text(text)],
            metadata: HashMap::new(),
        }
    }

    /// Create a new artifact with data content.
    pub fn data(
        name: impl Into<String>,
        data: impl Into<String>,
        mime_type: impl Into<String>,
    ) -> Self {
        let mime = mime_type.into();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            mime_type: Some(mime.clone()),
            content: vec![ContentPart::data(data, mime)],
            metadata: HashMap::new(),
        }
    }
}

/// A unified task type that works across protocols.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedTask {
    /// Unique task identifier
    pub id: String,
    /// Optional session identifier for grouping related tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Current status of the task
    pub status: TaskStatus,
    /// Messages in the conversation
    pub messages: Vec<UnifiedMessage>,
    /// Artifacts produced by the task
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp of task creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Timestamp of last update
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl UnifiedTask {
    /// Create a new task with the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: id.into(),
            session_id: None,
            status: TaskStatus::Pending,
            messages: Vec::new(),
            artifacts: Vec::new(),
            metadata: HashMap::new(),
            created_at: Some(now),
            updated_at: Some(now),
        }
    }

    /// Create a new task with a generated UUID.
    pub fn new_with_uuid() -> Self {
        Self::new(Uuid::new_v4().to_string())
    }

    /// Set the session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add a message to the task.
    pub fn add_message(&mut self, message: UnifiedMessage) {
        self.messages.push(message);
        self.updated_at = Some(chrono::Utc::now());
        // Auto-transition from pending to working when first message added
        if self.status == TaskStatus::Pending {
            self.status = TaskStatus::Working;
        }
    }

    /// Add an artifact to the task.
    pub fn add_artifact(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
        self.updated_at = Some(chrono::Utc::now());
    }

    /// Set the task status.
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Some(chrono::Utc::now());
    }

    /// Check if the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.status.is_terminal()
    }
}

/// Capability descriptor for an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    /// Unique capability identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what this capability does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Input schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Capability {
    /// Create a new capability.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            input_schema: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add an input schema.
    pub fn with_input_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Add a tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// Information about an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Unique agent identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Version string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Protocols supported by this agent
    pub protocols: Vec<Protocol>,
    /// Capabilities/skills offered by this agent
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<Capability>,
    /// Whether the agent supports streaming
    #[serde(default)]
    pub supports_streaming: bool,
    /// Agent endpoint URL (if remote)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AgentInfo {
    /// Create new agent info.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            version: None,
            protocols: Vec::new(),
            capabilities: Vec::new(),
            supports_streaming: false,
            url: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Add a version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Add a supported protocol.
    pub fn with_protocol(mut self, protocol: Protocol) -> Self {
        if !self.protocols.contains(&protocol) {
            self.protocols.push(protocol);
        }
        self
    }

    /// Add a capability.
    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Enable streaming support.
    pub fn with_streaming(mut self) -> Self {
        self.supports_streaming = true;
        self
    }

    /// Set the agent URL.
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }
}

/// Event types for streaming updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Task status changed
    StatusUpdate {
        task_id: String,
        status: TaskStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// New message added
    MessageAdded {
        task_id: String,
        message: UnifiedMessage,
    },
    /// Partial message content (for streaming responses)
    MessageDelta {
        task_id: String,
        message_id: String,
        delta: ContentPart,
    },
    /// New artifact produced
    ArtifactAdded { task_id: String, artifact: Artifact },
    /// Error occurred
    Error {
        task_id: String,
        code: String,
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_message_creation() {
        let msg = UnifiedMessage::user("Hello, agent!");
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.text_content(), "Hello, agent!");
        assert!(msg.timestamp.is_some());
    }

    #[test]
    fn test_unified_message_with_parts() {
        let msg = UnifiedMessage::agent("Here's an image")
            .with_part(ContentPart::data("base64data", "image/png"))
            .with_metadata("source", serde_json::json!("camera"));

        assert_eq!(msg.content.len(), 2);
        assert!(msg.metadata.contains_key("source"));
    }

    #[test]
    fn test_unified_task_lifecycle() {
        let mut task = UnifiedTask::new("task-001");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(!task.is_terminal());

        task.add_message(UnifiedMessage::user("Do something"));
        assert_eq!(task.status, TaskStatus::Working);

        task.set_status(TaskStatus::Completed);
        assert!(task.is_terminal());
    }

    #[test]
    fn test_artifact_creation() {
        let artifact = Artifact::text("result.txt", "Hello, world!");
        assert_eq!(artifact.name, "result.txt");
        assert_eq!(artifact.mime_type, Some("text/plain".to_string()));
    }

    #[test]
    fn test_capability_builder() {
        let cap = Capability::new("search", "Web Search")
            .with_description("Search the web for information")
            .with_tag("search")
            .with_tag("web");

        assert_eq!(cap.id, "search");
        assert_eq!(cap.tags.len(), 2);
    }

    #[test]
    fn test_agent_info_builder() {
        let info = AgentInfo::new("agent-1", "My Agent")
            .with_description("A helpful agent")
            .with_protocol(Protocol::Mcp)
            .with_protocol(Protocol::A2a)
            .with_streaming()
            .with_capability(Capability::new("chat", "Chat"));

        assert_eq!(info.protocols.len(), 2);
        assert!(info.supports_streaming);
        assert_eq!(info.capabilities.len(), 1);
    }

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent::StatusUpdate {
            task_id: "task-1".to_string(),
            status: TaskStatus::Completed,
            message: Some("Done!".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("status_update"));
        assert!(json.contains("completed"));
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(Protocol::Mcp.to_string(), "mcp");
        assert_eq!(Protocol::A2a.to_string(), "a2a");
    }

    #[test]
    fn test_task_status_terminal() {
        assert!(!TaskStatus::Pending.is_terminal());
        assert!(!TaskStatus::Working.is_terminal());
        assert!(!TaskStatus::InputRequired.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
    }
}
