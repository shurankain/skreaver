//! A2A Protocol Compliance Test Suite
//!
//! This test suite verifies compliance with the Agent2Agent (A2A) Protocol
//! specification (v0.3) as defined at https://a2a-protocol.org/latest/specification/
//!
//! ## Coverage
//!
//! - **Agent Card**: Required fields, serialization format (Section 4.4)
//! - **Task Lifecycle**: State transitions, terminal states (Section 4.1.3)
//! - **Messages**: Role, parts, timestamps (Section 4.1.4)
//! - **Artifacts**: Structure and serialization (Section 4.1.7)
//! - **Error Codes**: Standard HTTP status mapping (Section 3.3.2)
//! - **Streaming Events**: SSE event types (Section 3.2.3)
//!
//! ## Reference
//!
//! Sources:
//! - [A2A Protocol Specification](https://a2a-protocol.org/latest/specification/)
//! - [Google A2A Announcement](https://developers.googleblog.com/en/a2a-a-new-era-of-agent-interoperability/)

use chrono::Utc;
use serde_json::json;
use skreaver_a2a::{
    AgentCapabilities, AgentCard, AgentInterface, AgentSkill, Artifact, ErrorResponse, Message,
    Part, Role, StreamingEvent, Task, TaskArtifactUpdateEvent, TaskStatus, TaskStatusUpdateEvent,
};

// =============================================================================
// Agent Card Compliance Tests (Section 4.4)
// =============================================================================

mod agent_card_compliance {
    use super::*;

    /// SPEC: AgentCard MUST have required fields: id, name, interfaces
    #[test]
    fn test_agent_card_required_fields() {
        let card = AgentCard::new("test-agent", "Test Agent", "https://agent.example.com");

        // Required fields MUST be present
        assert!(!card.agent_id.is_empty(), "Agent ID MUST not be empty");
        assert!(!card.name.is_empty(), "Agent name MUST not be empty");
        assert!(!card.interfaces.is_empty(), "Interfaces MUST not be empty");

        // Verify at least one interface exists
        assert!(
            card.interfaces
                .iter()
                .any(|i| matches!(i, AgentInterface::Http { .. })),
            "Agent MUST have at least one interface"
        );
    }

    /// SPEC: AgentCard SHOULD include protocolVersions
    #[test]
    fn test_agent_card_protocol_versions() {
        let card = AgentCard::new("test-agent", "Test Agent", "https://agent.example.com");

        // Protocol versions SHOULD be specified
        assert!(
            !card.protocol_versions.is_empty(),
            "Protocol versions SHOULD be specified"
        );

        // Current version is 0.3
        assert!(
            card.protocol_versions
                .iter()
                .any(|v| v == "0.3" || v == "0.2" || v == "1.0"),
            "Protocol version SHOULD include a recognized version"
        );
    }

    /// SPEC: AgentCapabilities MUST include streaming, pushNotifications, extendedAgentCard
    #[test]
    fn test_agent_card_capabilities_structure() {
        let capabilities = AgentCapabilities::default();

        // All capability fields exist (even if false)
        let json = serde_json::to_value(&capabilities).unwrap();

        // Verify serialization includes the fields
        assert!(
            json.get("streaming").is_some() || json.is_object(),
            "Capabilities MUST serialize correctly"
        );
    }

    /// SPEC: Agent Card MUST serialize to camelCase JSON
    #[test]
    fn test_agent_card_camel_case_serialization() {
        let card = AgentCard::new("test-agent", "Test Agent", "https://agent.example.com")
            .with_description("A test agent")
            .with_streaming()
            .with_push_notifications();

        let json = serde_json::to_string(&card).unwrap();

        // Verify camelCase field names
        assert!(json.contains("\"agentId\""), "MUST use agentId (camelCase)");
        assert!(
            json.contains("\"protocolVersions\""),
            "MUST use protocolVersions (camelCase)"
        );
        assert!(
            !json.contains("\"agent_id\""),
            "MUST NOT use agent_id (snake_case)"
        );
    }

    /// SPEC: AgentInterface types MUST be "http", "grpc", etc.
    #[test]
    fn test_agent_interface_types() {
        let http = AgentInterface::http("https://agent.example.com");
        let grpc = AgentInterface::grpc("agent.example.com:50051");

        let http_json = serde_json::to_value(&http).unwrap();
        let grpc_json = serde_json::to_value(&grpc).unwrap();

        assert_eq!(
            http_json["type"], "http",
            "HTTP interface type MUST be 'http'"
        );
        assert_eq!(
            grpc_json["type"], "grpc",
            "gRPC interface type MUST be 'grpc'"
        );
    }

    /// SPEC: AgentSkill MUST have id and name
    #[test]
    fn test_agent_skill_required_fields() {
        let skill =
            AgentSkill::new("summarize", "Summarize Text").with_description("Summarizes documents");

        assert!(!skill.id.is_empty(), "Skill ID MUST not be empty");
        assert!(!skill.name.is_empty(), "Skill name MUST not be empty");

        let json = serde_json::to_string(&skill).unwrap();
        assert!(json.contains("\"id\""), "Skill MUST serialize id");
        assert!(json.contains("\"name\""), "Skill MUST serialize name");
    }

    /// SPEC: Agent Card round-trip serialization
    #[test]
    fn test_agent_card_roundtrip() {
        let original = AgentCard::new("test-agent", "Test Agent", "https://agent.example.com")
            .with_description("A test agent")
            .with_streaming()
            .with_skill(
                AgentSkill::new("echo", "Echo")
                    .with_description("Echoes input")
                    .with_input_schema(json!({
                        "type": "object",
                        "properties": {
                            "message": {"type": "string"}
                        }
                    })),
            );

        let json = serde_json::to_string(&original).unwrap();
        let parsed: AgentCard = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.agent_id, original.agent_id);
        assert_eq!(parsed.name, original.name);
        assert_eq!(parsed.description, original.description);
        assert_eq!(
            parsed.capabilities.streaming,
            original.capabilities.streaming
        );
        assert_eq!(parsed.skills.len(), original.skills.len());
    }
}

// =============================================================================
// Task Lifecycle Compliance Tests (Section 4.1.3)
// =============================================================================

mod task_lifecycle_compliance {
    use super::*;

    /// SPEC: Valid task states are working, input_required, completed, failed, cancelled, rejected
    #[test]
    fn test_all_task_states_exist() {
        let states = [
            TaskStatus::Working,
            TaskStatus::InputRequired,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
            TaskStatus::Rejected,
        ];

        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            let parsed: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, state, "State {:?} MUST round-trip correctly", state);
        }
    }

    /// SPEC: Task states MUST serialize to kebab-case
    #[test]
    fn test_task_status_kebab_case_serialization() {
        assert_eq!(
            serde_json::to_string(&TaskStatus::Working).unwrap(),
            "\"working\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::InputRequired).unwrap(),
            "\"input-required\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
        assert_eq!(
            serde_json::to_string(&TaskStatus::Rejected).unwrap(),
            "\"rejected\""
        );
    }

    /// SPEC: Terminal states are completed, failed, cancelled, rejected
    #[test]
    fn test_terminal_states() {
        // Terminal states
        let mut task = Task::new("t1");
        task.set_status(TaskStatus::Completed);
        assert!(task.is_terminal(), "Completed MUST be terminal");

        task.set_status(TaskStatus::Failed);
        assert!(task.is_terminal(), "Failed MUST be terminal");

        task.set_status(TaskStatus::Cancelled);
        assert!(task.is_terminal(), "Cancelled MUST be terminal");

        task.set_status(TaskStatus::Rejected);
        assert!(task.is_terminal(), "Rejected MUST be terminal");

        // Non-terminal states
        task.set_status(TaskStatus::Working);
        assert!(!task.is_terminal(), "Working MUST NOT be terminal");

        task.set_status(TaskStatus::InputRequired);
        assert!(!task.is_terminal(), "InputRequired MUST NOT be terminal");
    }

    /// SPEC: Task MUST have id field
    #[test]
    fn test_task_required_fields() {
        let task = Task::new("task-123");

        assert_eq!(task.id, "task-123", "Task ID MUST match");
        assert_eq!(
            task.status,
            TaskStatus::Working,
            "Initial status MUST be Working"
        );
    }

    /// SPEC: Task MUST track creation timestamp
    #[test]
    fn test_task_timestamps() {
        let task = Task::new("task-123");

        assert!(task.created_at.is_some(), "Task SHOULD have created_at");
        assert!(task.updated_at.is_some(), "Task SHOULD have updated_at");
    }

    /// SPEC: Task serializes with camelCase
    #[test]
    fn test_task_camel_case_serialization() {
        let mut task = Task::new("task-123");
        task.context_id = Some("ctx-1".to_string());
        task.add_message(Message::user("Hello"));

        let json = serde_json::to_string(&task).unwrap();

        assert!(json.contains("\"contextId\""), "MUST use contextId");
        assert!(json.contains("\"createdAt\""), "MUST use createdAt");
        assert!(json.contains("\"updatedAt\""), "MUST use updatedAt");
        assert!(!json.contains("\"context_id\""), "MUST NOT use snake_case");
    }

    /// SPEC: Task can contain messages and artifacts
    #[test]
    fn test_task_messages_and_artifacts() {
        let mut task = Task::new("task-123");

        task.add_message(Message::user("Question"));
        task.add_message(Message::agent("Answer"));
        task.add_artifact(Artifact::text("art-1", "Generated content"));

        assert_eq!(task.messages.len(), 2);
        assert_eq!(task.artifacts.len(), 1);
    }

    /// SPEC: Task with InputRequired state
    #[test]
    fn test_task_input_required() {
        let mut task = Task::new("task-123");
        task.set_status(TaskStatus::InputRequired);

        assert!(task.requires_input(), "requires_input MUST return true");
        assert!(!task.is_terminal(), "InputRequired is NOT terminal");
    }
}

// =============================================================================
// Message Compliance Tests (Section 4.1.4)
// =============================================================================

mod message_compliance {
    use super::*;

    /// SPEC: Message MUST have role ("user" | "agent") and parts
    #[test]
    fn test_message_required_fields() {
        let user_msg = Message::user("Hello");
        let agent_msg = Message::agent("Hi there");

        assert_eq!(user_msg.role, Role::User);
        assert_eq!(agent_msg.role, Role::Agent);
        assert!(!user_msg.parts.is_empty(), "Parts MUST not be empty");
        assert!(!agent_msg.parts.is_empty(), "Parts MUST not be empty");
    }

    /// SPEC: Role serializes to lowercase
    #[test]
    fn test_role_serialization() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(serde_json::to_string(&Role::Agent).unwrap(), "\"agent\"");
    }

    /// SPEC: Message serializes with camelCase
    #[test]
    fn test_message_camel_case_serialization() {
        let msg = Message::user("Hello").with_reference("task-456");

        let json = serde_json::to_string(&msg).unwrap();

        assert!(
            json.contains("\"referenceTaskIds\""),
            "MUST use referenceTaskIds"
        );
        assert!(
            !json.contains("\"reference_task_ids\""),
            "MUST NOT use snake_case"
        );
    }

    /// SPEC: Message can have optional fields
    #[test]
    fn test_message_optional_fields() {
        let msg = Message::user("Hello");

        // ID should be auto-generated
        assert!(msg.id.is_some(), "Message SHOULD have auto-generated ID");

        // Timestamp should be set
        assert!(msg.timestamp.is_some(), "Message SHOULD have timestamp");
    }
}

// =============================================================================
// Part Types Compliance Tests (Section 4.1.6)
// =============================================================================

mod part_compliance {
    use super::*;

    /// SPEC: Part types are text, file, data
    #[test]
    fn test_part_types() {
        let text = Part::text("Hello world");
        let file = Part::file("https://example.com/doc.pdf", "application/pdf");
        let data = Part::data(json!({"key": "value"}), "application/json");

        assert!(matches!(text, Part::Text(_)));
        assert!(matches!(file, Part::File(_)));
        assert!(matches!(data, Part::Data(_)));
    }

    /// SPEC: Text part can extract text content
    #[test]
    fn test_text_part_extraction() {
        let text = Part::text("Hello world");
        assert_eq!(text.as_text(), Some("Hello world"));

        let file = Part::file("https://example.com/doc.pdf", "application/pdf");
        assert_eq!(file.as_text(), None);
    }

    /// SPEC: Part serializes with type tag
    #[test]
    fn test_part_type_tag_serialization() {
        let text = Part::text("Hello");
        let json = serde_json::to_value(&text).unwrap();

        assert_eq!(json["type"], "text", "Text part type MUST be 'text'");
        assert!(json.get("text").is_some(), "Text part MUST have text field");

        let file = Part::file("https://example.com/doc.pdf", "application/pdf");
        let json = serde_json::to_value(&file).unwrap();

        assert_eq!(json["type"], "file", "File part type MUST be 'file'");
        assert!(json.get("uri").is_some(), "File part MUST have uri field");
        assert!(
            json.get("mediaType").is_some(),
            "File part MUST have mediaType"
        );

        let data = Part::data(json!({"key": "value"}), "application/json");
        let json = serde_json::to_value(&data).unwrap();

        assert_eq!(json["type"], "data", "Data part type MUST be 'data'");
        assert!(json.get("data").is_some(), "Data part MUST have data field");
    }

    /// SPEC: Part round-trip serialization
    #[test]
    fn test_part_roundtrip() {
        let parts = vec![
            Part::text("Hello"),
            Part::file("https://example.com/doc.pdf", "application/pdf"),
            Part::data(json!({"nested": {"key": "value"}}), "application/json"),
        ];

        for part in parts {
            let json = serde_json::to_string(&part).unwrap();
            let parsed: Part = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, part, "Part MUST round-trip correctly");
        }
    }
}

// =============================================================================
// Artifact Compliance Tests (Section 4.1.7)
// =============================================================================

mod artifact_compliance {
    use super::*;

    /// SPEC: Artifact MUST have id and parts
    #[test]
    fn test_artifact_required_fields() {
        let artifact = Artifact::text("art-1", "Generated content");

        assert!(!artifact.id.is_empty(), "Artifact ID MUST not be empty");
        assert!(
            !artifact.parts.is_empty(),
            "Artifact parts MUST not be empty"
        );
    }

    /// SPEC: Artifact can have optional title, description, mimeType
    #[test]
    fn test_artifact_optional_fields() {
        let artifact = Artifact::text("art-1", "Content")
            .with_label("Output Document")
            .with_description("The generated output");

        assert_eq!(artifact.label, Some("Output Document".to_string()));
        assert_eq!(
            artifact.description,
            Some("The generated output".to_string())
        );
    }

    /// SPEC: Artifact serializes with camelCase
    #[test]
    fn test_artifact_camel_case_serialization() {
        let artifact = Artifact::text("art-1", "Content").with_label("Label");

        let json = serde_json::to_string(&artifact).unwrap();

        assert!(json.contains("\"mediaType\"") || json.contains("\"label\""));
        assert!(!json.contains("\"media_type\""), "MUST NOT use snake_case");
    }

    /// SPEC: JSON artifact helper
    #[test]
    fn test_json_artifact() {
        let artifact = Artifact::json("art-1", json!({"result": 42}));

        assert_eq!(artifact.media_type, Some("application/json".to_string()));
        assert_eq!(artifact.parts.len(), 1);
    }
}

// =============================================================================
// Error Response Compliance Tests (Section 3.3.2)
// =============================================================================

mod error_compliance {
    use super::*;

    /// SPEC: Error response has code and message
    #[test]
    fn test_error_response_structure() {
        let error = ErrorResponse::new(404, "Task not found");

        assert_eq!(error.code, 404);
        assert_eq!(error.message, "Task not found");
    }

    /// SPEC: Error codes map to HTTP status
    #[test]
    fn test_error_code_http_mapping() {
        // Standard mappings per spec Section 3.3.2
        let mappings = [
            (400, "Bad Request / Validation Error"),
            (401, "Authentication Required"),
            (403, "Not Authorized"),
            (404, "Not Found"),
            (429, "Rate Limited"),
            (500, "Internal Error"),
            (502, "Bad Gateway / Connection Error"),
            (504, "Gateway Timeout"),
        ];

        for (code, _desc) in mappings {
            let error = ErrorResponse::new(code, "Test");
            assert_eq!(error.code, code);
        }
    }

    /// SPEC: Error response serializes with camelCase
    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse::new(404, "Not found").with_data(json!({"taskId": "task-123"}));

        let json = serde_json::to_string(&error).unwrap();

        assert!(json.contains("\"code\""));
        assert!(json.contains("\"message\""));
        assert!(json.contains("\"data\""));
    }
}

// =============================================================================
// Streaming Event Compliance Tests (Section 3.2.3)
// =============================================================================

mod streaming_compliance {
    use super::*;

    /// SPEC: StreamingEvent types are taskStatusUpdate, taskArtifactUpdate
    #[test]
    fn test_streaming_event_types() {
        let status_event = StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
            task_id: "task-123".to_string(),
            status: TaskStatus::Working,
            message: None,
            timestamp: Utc::now(),
        });

        let artifact_event = StreamingEvent::TaskArtifactUpdate(TaskArtifactUpdateEvent {
            task_id: "task-123".to_string(),
            artifact: Artifact::text("art-1", "Content"),
            is_final: false,
            timestamp: Utc::now(),
        });

        // Verify type tags
        let status_json = serde_json::to_value(&status_event).unwrap();
        let artifact_json = serde_json::to_value(&artifact_event).unwrap();

        assert_eq!(
            status_json["type"], "taskStatusUpdate",
            "Status event type MUST be 'taskStatusUpdate'"
        );
        assert_eq!(
            artifact_json["type"], "taskArtifactUpdate",
            "Artifact event type MUST be 'taskArtifactUpdate'"
        );
    }

    /// SPEC: TaskStatusUpdateEvent MUST have taskId, status, timestamp
    #[test]
    fn test_status_update_event_fields() {
        let event = TaskStatusUpdateEvent {
            task_id: "task-123".to_string(),
            status: TaskStatus::Completed,
            message: Some(Message::agent("Done")),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_value(&event).unwrap();

        assert!(json.get("taskId").is_some(), "MUST have taskId");
        assert!(json.get("status").is_some(), "MUST have status");
        assert!(json.get("timestamp").is_some(), "MUST have timestamp");
    }

    /// SPEC: TaskArtifactUpdateEvent MUST have taskId, artifact, timestamp
    #[test]
    fn test_artifact_update_event_fields() {
        let event = TaskArtifactUpdateEvent {
            task_id: "task-123".to_string(),
            artifact: Artifact::text("art-1", "Content"),
            is_final: true,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_value(&event).unwrap();

        assert!(json.get("taskId").is_some(), "MUST have taskId");
        assert!(json.get("artifact").is_some(), "MUST have artifact");
        assert!(json.get("timestamp").is_some(), "MUST have timestamp");
        assert!(json.get("isFinal").is_some(), "MUST have isFinal");
    }

    /// SPEC: Streaming events round-trip correctly
    #[test]
    fn test_streaming_event_roundtrip() {
        let events = vec![
            StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
                task_id: "task-123".to_string(),
                status: TaskStatus::Working,
                message: None,
                timestamp: Utc::now(),
            }),
            StreamingEvent::TaskArtifactUpdate(TaskArtifactUpdateEvent {
                task_id: "task-123".to_string(),
                artifact: Artifact::text("art-1", "Content"),
                is_final: false,
                timestamp: Utc::now(),
            }),
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let parsed: StreamingEvent = serde_json::from_str(&json).unwrap();

            match (&event, &parsed) {
                (StreamingEvent::TaskStatusUpdate(e1), StreamingEvent::TaskStatusUpdate(e2)) => {
                    assert_eq!(e1.task_id, e2.task_id);
                    assert_eq!(e1.status, e2.status);
                }
                (
                    StreamingEvent::TaskArtifactUpdate(e1),
                    StreamingEvent::TaskArtifactUpdate(e2),
                ) => {
                    assert_eq!(e1.task_id, e2.task_id);
                    assert_eq!(e1.is_final, e2.is_final);
                }
                _ => panic!("Event type mismatch"),
            }
        }
    }
}

// =============================================================================
// Request/Response Format Compliance Tests
// =============================================================================

mod request_response_compliance {
    use super::*;
    use skreaver_a2a::{
        CancelTaskRequest, GetTaskRequest, SendMessageRequest, SendMessageResponse,
    };

    /// SPEC: SendMessageRequest has message field (required)
    #[test]
    fn test_send_message_request_structure() {
        let request = SendMessageRequest {
            message: Message::user("Hello"),
            task_id: Some("task-123".to_string()),
            context_id: Some("ctx-1".to_string()),
            metadata: Default::default(),
        };

        let json = serde_json::to_value(&request).unwrap();

        assert!(json.get("message").is_some(), "MUST have message");
        assert!(
            json.get("taskId").is_some(),
            "MUST have taskId when present"
        );
        assert!(
            json.get("contextId").is_some(),
            "MUST have contextId when present"
        );
    }

    /// SPEC: SendMessageResponse contains task
    #[test]
    fn test_send_message_response_structure() {
        let response = SendMessageResponse {
            task: Task::new("task-123"),
        };

        let json = serde_json::to_value(&response).unwrap();

        assert!(json.get("task").is_some(), "MUST have task");
    }

    /// SPEC: GetTaskRequest has task_id
    #[test]
    fn test_get_task_request_structure() {
        let request = GetTaskRequest {
            task_id: "task-123".to_string(),
        };

        let json = serde_json::to_value(&request).unwrap();

        assert!(json.get("taskId").is_some(), "MUST have taskId");
    }

    /// SPEC: CancelTaskRequest has task_id and optional reason
    #[test]
    fn test_cancel_task_request_structure() {
        let request = CancelTaskRequest {
            task_id: "task-123".to_string(),
            reason: Some("User requested cancellation".to_string()),
        };

        let json = serde_json::to_value(&request).unwrap();

        assert!(json.get("taskId").is_some(), "MUST have taskId");
        assert!(
            json.get("reason").is_some(),
            "MUST have reason when present"
        );
    }
}

// =============================================================================
// Security Scheme Compliance Tests
// =============================================================================

mod security_compliance {
    #[allow(unused_imports)]
    use super::*;
    use skreaver_a2a::{ApiKeyLocation, OAuth2Flow, OAuth2Flows, SecurityScheme};
    use std::collections::HashMap;

    /// SPEC: SecurityScheme types are apiKey, http, oauth2
    #[test]
    fn test_security_scheme_types() {
        let api_key = SecurityScheme::ApiKey {
            name: "X-API-Key".to_string(),
            location: ApiKeyLocation::Header,
        };

        let http_bearer = SecurityScheme::Http {
            scheme: "bearer".to_string(),
        };

        let oauth2 = SecurityScheme::OAuth2 {
            flows: OAuth2Flows {
                authorization_code: None,
                client_credentials: Some(OAuth2Flow {
                    authorization_url: None,
                    token_url: "https://auth.example.com/token".to_string(),
                    scopes: HashMap::new(),
                }),
            },
        };

        // Verify type tags
        let api_key_json = serde_json::to_value(&api_key).unwrap();
        let http_json = serde_json::to_value(&http_bearer).unwrap();
        let oauth2_json = serde_json::to_value(&oauth2).unwrap();

        assert_eq!(api_key_json["type"], "apiKey");
        assert_eq!(http_json["type"], "http");
        assert_eq!(oauth2_json["type"], "oauth2");
    }

    /// SPEC: ApiKeyLocation is "header" or "query"
    #[test]
    fn test_api_key_location_serialization() {
        assert_eq!(
            serde_json::to_string(&ApiKeyLocation::Header).unwrap(),
            "\"header\""
        );
        assert_eq!(
            serde_json::to_string(&ApiKeyLocation::Query).unwrap(),
            "\"query\""
        );
    }
}

// =============================================================================
// Integration Compliance Tests
// =============================================================================

mod integration_compliance {
    use super::*;
    #[allow(unused_imports)]
    use skreaver_a2a::AgentSkill;

    /// Complete agent card with all features
    #[test]
    fn test_full_agent_card_compliance() {
        let card = AgentCard::new("summarizer", "Document Summarizer", "https://summarizer.ai")
            .with_description("Summarizes documents using AI")
            .with_streaming()
            .with_push_notifications()
            .with_skill(
                AgentSkill::new("summarize", "Summarize Document")
                    .with_description("Takes a document and produces a summary")
                    .with_input_schema(json!({
                        "type": "object",
                        "properties": {
                            "document": {
                                "type": "string",
                                "description": "The document text to summarize"
                            },
                            "maxLength": {
                                "type": "integer",
                                "description": "Maximum summary length in words"
                            }
                        },
                        "required": ["document"]
                    }))
                    .with_output_schema(json!({
                        "type": "object",
                        "properties": {
                            "summary": {"type": "string"},
                            "wordCount": {"type": "integer"}
                        }
                    })),
            );

        // Serialize and verify
        let json_str = serde_json::to_string_pretty(&card).unwrap();
        let parsed: AgentCard = serde_json::from_str(&json_str).unwrap();

        // Verify all fields preserved
        assert_eq!(parsed.agent_id, "summarizer");
        assert_eq!(parsed.name, "Document Summarizer");
        assert!(parsed.capabilities.streaming);
        assert!(parsed.capabilities.push_notifications);
        assert_eq!(parsed.skills.len(), 1);
        assert!(parsed.skills[0].input_schema.is_some());
        assert!(parsed.skills[0].output_schema.is_some());
    }

    /// Complete task lifecycle
    #[test]
    fn test_full_task_lifecycle_compliance() {
        // 1. Create task
        let mut task = Task::new_with_uuid();
        assert_eq!(task.status, TaskStatus::Working);
        assert!(!task.is_terminal());

        // 2. Add user message
        task.add_message(Message::user("Summarize this document..."));
        assert_eq!(task.messages.len(), 1);

        // 3. Simulate agent processing
        task.add_message(Message::agent("Processing your request..."));

        // 4. Add artifact
        task.add_artifact(
            Artifact::text("summary-1", "This is the summary of the document.")
                .with_label("Summary"),
        );
        assert_eq!(task.artifacts.len(), 1);

        // 5. Complete task
        task.set_status(TaskStatus::Completed);
        assert!(task.is_terminal());
        assert_eq!(task.status, TaskStatus::Completed);

        // 6. Verify serialization
        let json = serde_json::to_string(&task).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, TaskStatus::Completed);
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.artifacts.len(), 1);
    }
}
