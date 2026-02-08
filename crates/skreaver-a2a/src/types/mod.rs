//! A2A Protocol Core Types
//!
//! This module defines the core data types for the Agent2Agent (A2A) protocol,
//! based on the official A2A specification.
//!
//! The A2A protocol enables communication and interoperability between AI agents,
//! allowing them to discover each other's capabilities, exchange messages, and
//! collaborate on tasks.
//!
//! ## Module Structure
//!
//! - [`task`] - Task lifecycle and status types
//! - [`message`] - Message and role types
//! - [`part`] - Content part types (text, file, data)
//! - [`artifact`] - Task output artifacts
//! - [`agent_card`] - Agent capability discovery
//! - [`streaming`] - Streaming event types
//! - [`request`] - Request/response types

mod agent_card;
mod artifact;
mod message;
mod part;
mod request;
mod streaming;
mod task;

// Re-export all types for convenience
pub use agent_card::{
    AgentCapabilities, AgentCard, AgentCardSignature, AgentExtension, AgentInterface,
    AgentProvider, AgentSkill, ApiKeyLocation, OAuth2Flow, OAuth2Flows, SecurityScheme,
};
pub use artifact::Artifact;
pub use message::{Message, Role};
pub use part::{DataPart, FilePart, Part, TextPart};
pub use request::{
    CancelTaskRequest, GetTaskRequest, PushNotificationConfig, SendMessageRequest,
    SendMessageResponse,
};
pub use streaming::{StreamingEvent, TaskArtifactUpdateEvent, TaskStatusUpdateEvent};
pub use task::{Task, TaskStatus};

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
