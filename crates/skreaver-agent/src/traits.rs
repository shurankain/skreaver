//! Unified agent trait definitions.
//!
//! These traits define the common interface for working with agents
//! across different protocols (MCP, A2A).

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

use crate::error::AgentResult;
use crate::types::{
    AgentInfo, Artifact, Capability, ContentPart, Protocol, StreamEvent, TaskStatus,
    UnifiedMessage, UnifiedTask,
};

/// A unified interface for interacting with agents across protocols.
///
/// This trait abstracts over the differences between MCP and A2A protocols,
/// providing a common API for agent operations.
#[async_trait]
pub trait UnifiedAgent: Send + Sync {
    /// Get information about this agent.
    fn info(&self) -> &AgentInfo;

    /// Get the protocols supported by this agent.
    fn supported_protocols(&self) -> &[Protocol] {
        &self.info().protocols
    }

    /// Check if a protocol is supported.
    fn supports_protocol(&self, protocol: Protocol) -> bool {
        self.supported_protocols().contains(&protocol)
    }

    /// Get the capabilities offered by this agent.
    fn capabilities(&self) -> &[Capability] {
        &self.info().capabilities
    }

    /// Check if streaming is supported.
    fn supports_streaming(&self) -> bool {
        self.info().supports_streaming
    }

    /// Send a message and get a response.
    ///
    /// This is the primary method for interacting with an agent.
    /// It creates a task, sends the message, and waits for completion.
    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask>;

    /// Send a message with an existing task context.
    ///
    /// Use this for multi-turn conversations within the same task.
    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask>;

    /// Send a message and receive streaming updates.
    ///
    /// Returns a stream of events as the agent processes the request.
    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>>;

    /// Get the current state of a task.
    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask>;

    /// Cancel a running task.
    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask>;
}

/// Trait for agents that can invoke tools/capabilities.
#[async_trait]
pub trait ToolInvoker: Send + Sync {
    /// Invoke a tool/capability by name.
    async fn invoke_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> AgentResult<serde_json::Value>;

    /// List available tools/capabilities.
    fn list_tools(&self) -> Vec<Capability>;
}

/// Trait for agents that can be hosted as a server.
#[async_trait]
pub trait AgentServer: Send + Sync {
    /// Handle an incoming message request.
    async fn handle_message(
        &self,
        task: &mut UnifiedTask,
        message: UnifiedMessage,
    ) -> AgentResult<()>;

    /// Handle a cancel request.
    async fn handle_cancel(&self, task: &mut UnifiedTask) -> AgentResult<()>;

    /// Get the agent info for discovery.
    fn agent_info(&self) -> AgentInfo;
}

/// Extension trait for agents that support streaming server responses.
#[async_trait]
pub trait StreamingAgentServer: AgentServer {
    /// Handle a message with streaming response.
    async fn handle_message_streaming(
        &self,
        task: &mut UnifiedTask,
        message: UnifiedMessage,
        event_sender: tokio::sync::broadcast::Sender<StreamEvent>,
    ) -> AgentResult<()>;
}

/// Builder for creating unified messages from protocol-specific data.
pub struct MessageBuilder {
    message: UnifiedMessage,
}

impl MessageBuilder {
    /// Create a new user message builder.
    pub fn user() -> Self {
        Self {
            message: UnifiedMessage::user(""),
        }
    }

    /// Create a new agent message builder.
    pub fn agent() -> Self {
        Self {
            message: UnifiedMessage::agent(""),
        }
    }

    /// Add text content.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.message.content.push(ContentPart::text(text));
        self
    }

    /// Add data content.
    pub fn data(mut self, data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        self.message
            .content
            .push(ContentPart::data(data, mime_type));
        self
    }

    /// Add a file reference.
    pub fn file(mut self, uri: impl Into<String>) -> Self {
        self.message.content.push(ContentPart::file(uri));
        self
    }

    /// Add a tool call.
    pub fn tool_call(
        mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        self.message
            .content
            .push(ContentPart::tool_call(id, name, arguments));
        self
    }

    /// Add a tool result.
    pub fn tool_result(mut self, id: impl Into<String>, result: serde_json::Value) -> Self {
        self.message
            .content
            .push(ContentPart::tool_result(id, result));
        self
    }

    /// Add metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.message.metadata.insert(key.into(), value);
        self
    }

    /// Build the message.
    pub fn build(mut self) -> UnifiedMessage {
        // Remove the empty text part if we added other content
        if self.message.content.len() > 1
            && let Some(ContentPart::Text { text }) = self.message.content.first()
            && text.is_empty()
        {
            self.message.content.remove(0);
        }
        self.message
    }
}

/// Builder for creating tasks.
pub struct TaskBuilder {
    task: UnifiedTask,
}

impl TaskBuilder {
    /// Create a new task builder with a generated ID.
    pub fn new() -> Self {
        Self {
            task: UnifiedTask::new_with_uuid(),
        }
    }

    /// Create a new task builder with a specific ID.
    pub fn with_id(id: impl Into<String>) -> Self {
        Self {
            task: UnifiedTask::new(id),
        }
    }

    /// Set the session ID.
    pub fn session(mut self, session_id: impl Into<String>) -> Self {
        self.task.session_id = Some(session_id.into());
        self
    }

    /// Add an initial message.
    pub fn message(mut self, message: UnifiedMessage) -> Self {
        self.task.add_message(message);
        self
    }

    /// Add metadata.
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.task.metadata.insert(key.into(), value);
        self
    }

    /// Build the task.
    pub fn build(self) -> UnifiedTask {
        self.task
    }
}

impl Default for TaskBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to send status updates.
pub fn status_update(task_id: &str, status: TaskStatus, message: Option<String>) -> StreamEvent {
    StreamEvent::StatusUpdate {
        task_id: task_id.to_string(),
        status,
        message,
    }
}

/// Helper to send message added events.
pub fn message_added(task_id: &str, message: UnifiedMessage) -> StreamEvent {
    StreamEvent::MessageAdded {
        task_id: task_id.to_string(),
        message,
    }
}

/// Helper to send artifact added events.
pub fn artifact_added(task_id: &str, artifact: Artifact) -> StreamEvent {
    StreamEvent::ArtifactAdded {
        task_id: task_id.to_string(),
        artifact,
    }
}

/// Helper to send error events.
pub fn error_event(
    task_id: &str,
    code: impl Into<String>,
    message: impl Into<String>,
) -> StreamEvent {
    StreamEvent::Error {
        task_id: task_id.to_string(),
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_builder() {
        let msg = MessageBuilder::user()
            .text("Hello")
            .text("World")
            .metadata("key", serde_json::json!("value"))
            .build();

        assert_eq!(msg.content.len(), 2);
        assert!(msg.metadata.contains_key("key"));
    }

    #[test]
    fn test_message_builder_with_tools() {
        let msg = MessageBuilder::agent()
            .text("Let me help")
            .tool_call("call-1", "search", serde_json::json!({"query": "test"}))
            .build();

        assert_eq!(msg.content.len(), 2);
    }

    #[test]
    fn test_task_builder() {
        let task = TaskBuilder::new()
            .session("session-1")
            .message(UnifiedMessage::user("Hello"))
            .metadata("source", serde_json::json!("test"))
            .build();

        assert!(task.session_id.is_some());
        assert_eq!(task.messages.len(), 1);
        assert!(task.metadata.contains_key("source"));
    }

    #[test]
    fn test_status_update_helper() {
        let event = status_update("task-1", TaskStatus::Completed, Some("Done".to_string()));
        match event {
            StreamEvent::StatusUpdate {
                task_id, status, ..
            } => {
                assert_eq!(task_id, "task-1");
                assert_eq!(status, TaskStatus::Completed);
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_artifact_added_helper() {
        let artifact = Artifact::text("result", "content");
        let event = artifact_added("task-1", artifact);
        match event {
            StreamEvent::ArtifactAdded { task_id, .. } => {
                assert_eq!(task_id, "task-1");
            }
            _ => panic!("Wrong event type"),
        }
    }
}
