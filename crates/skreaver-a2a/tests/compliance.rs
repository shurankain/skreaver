//! A2A Protocol Compliance Test Suite
//!
//! Verifies compliance with the A2A Protocol specification.
//! Reference: https://a2a-protocol.org/latest/specification/

use chrono::Utc;
use serde_json::json;
use skreaver_a2a::{
    AgentCard, AgentSkill, ApiKeyLocation, Artifact, CancelTaskRequest, ErrorResponse,
    GetTaskRequest, Message, OAuth2Flow, OAuth2Flows, Part, Role, SecurityScheme,
    SendMessageRequest, SendMessageResponse, StreamingEvent, Task, TaskArtifactUpdateEvent,
    TaskStatus, TaskStatusUpdateEvent,
};
use std::collections::HashMap;

// =============================================================================
// Agent Card Compliance
// =============================================================================

#[test]
fn test_agent_card_structure_and_serialization() {
    let card = AgentCard::new("test-agent", "Test Agent", "https://agent.example.com")
        .with_description("A test agent")
        .with_streaming()
        .with_push_notifications()
        .with_skill(
            AgentSkill::new("echo", "Echo")
                .with_description("Echoes input")
                .with_input_schema(json!({"type": "object"})),
        );

    // Required fields
    assert!(!card.agent_id.is_empty());
    assert!(!card.name.is_empty());
    assert!(!card.interfaces.is_empty());
    assert!(!card.protocol_versions.is_empty());

    // Serialization: camelCase, roundtrip
    let json = serde_json::to_string(&card).unwrap();
    assert!(json.contains("\"agentId\""));
    assert!(json.contains("\"protocolVersions\""));
    assert!(!json.contains("\"agent_id\""));

    let parsed: AgentCard = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.agent_id, card.agent_id);
    assert_eq!(parsed.skills.len(), 1);
}

// =============================================================================
// Task Lifecycle Compliance
// =============================================================================

#[test]
fn test_task_status_serialization() {
    // All states serialize to kebab-case
    let cases = [
        (TaskStatus::Working, "\"working\""),
        (TaskStatus::InputRequired, "\"input-required\""),
        (TaskStatus::Completed, "\"completed\""),
        (TaskStatus::Failed, "\"failed\""),
        (TaskStatus::Cancelled, "\"cancelled\""),
        (TaskStatus::Rejected, "\"rejected\""),
    ];

    for (status, expected) in cases {
        assert_eq!(serde_json::to_string(&status).unwrap(), expected);
        // Round-trip
        let parsed: TaskStatus = serde_json::from_str(expected).unwrap();
        assert_eq!(parsed, status);
    }
}

#[test]
fn test_task_terminal_states() {
    let mut task = Task::new("t1");

    // Non-terminal
    assert!(!task.is_terminal());
    task.set_status(TaskStatus::InputRequired);
    assert!(!task.is_terminal());

    // Terminal
    for status in [
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
        TaskStatus::Rejected,
    ] {
        task.set_status(status);
        assert!(task.is_terminal(), "{:?} should be terminal", status);
    }
}

#[test]
fn test_task_structure_and_serialization() {
    let mut task = Task::new("task-123");
    task.context_id = Some("ctx-1".to_string());
    task.add_message(Message::user("Hello"));
    task.add_artifact(Artifact::text("art-1", "Output"));

    // Required fields and initial state
    assert_eq!(task.status, TaskStatus::Working);
    assert!(task.created_at.is_some());

    // Serialization: camelCase
    let json = serde_json::to_string(&task).unwrap();
    assert!(json.contains("\"contextId\""));
    assert!(json.contains("\"createdAt\""));
    assert!(!json.contains("\"context_id\""));

    // Round-trip
    let parsed: Task = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.messages.len(), 1);
    assert_eq!(parsed.artifacts.len(), 1);
}

// =============================================================================
// Message & Part Compliance
// =============================================================================

#[test]
fn test_message_and_role() {
    let user_msg = Message::user("Hello");
    let agent_msg = Message::agent("Hi");

    assert_eq!(user_msg.role, Role::User);
    assert_eq!(agent_msg.role, Role::Agent);
    assert!(user_msg.id.is_some());
    assert!(user_msg.timestamp.is_some());

    // Role serialization
    assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
    assert_eq!(serde_json::to_string(&Role::Agent).unwrap(), "\"agent\"");
}

#[test]
fn test_part_types_and_serialization() {
    let parts = [
        (Part::text("Hello"), "text"),
        (Part::file("https://example.com/doc.pdf", "application/pdf"), "file"),
        (Part::data(json!({"key": "value"}), "application/json"), "data"),
    ];

    for (part, expected_type) in parts {
        let json = serde_json::to_value(&part).unwrap();
        assert_eq!(json["type"], expected_type);

        // Round-trip
        let serialized = serde_json::to_string(&part).unwrap();
        let parsed: Part = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parsed, part);
    }

    // Text extraction
    assert_eq!(Part::text("Hello").as_text(), Some("Hello"));
    assert_eq!(Part::file("url", "type").as_text(), None);
}

// =============================================================================
// Artifact Compliance
// =============================================================================

#[test]
fn test_artifact_structure() {
    let artifact = Artifact::text("art-1", "Content")
        .with_label("Label")
        .with_description("Desc");

    assert!(!artifact.id.is_empty());
    assert!(!artifact.parts.is_empty());
    assert_eq!(artifact.label, Some("Label".to_string()));

    // JSON artifact helper
    let json_artifact = Artifact::json("art-2", json!({"x": 1}));
    assert_eq!(json_artifact.media_type, Some("application/json".to_string()));
}

// =============================================================================
// Error & Request/Response Compliance
// =============================================================================

#[test]
fn test_error_response() {
    let error = ErrorResponse::new(404, "Not found").with_data(json!({"id": "123"}));

    assert_eq!(error.code, 404);
    let json = serde_json::to_string(&error).unwrap();
    assert!(json.contains("\"code\""));
    assert!(json.contains("\"message\""));
    assert!(json.contains("\"data\""));
}

#[test]
fn test_request_response_structures() {
    // SendMessageRequest
    let send_req = SendMessageRequest {
        message: Message::user("Hello"),
        task_id: Some("task-123".to_string()),
        context_id: Some("ctx-1".to_string()),
        metadata: Default::default(),
    };
    let json = serde_json::to_value(&send_req).unwrap();
    assert!(json.get("message").is_some());
    assert!(json.get("taskId").is_some());

    // SendMessageResponse
    let send_resp = SendMessageResponse { task: Task::new("t1") };
    let json = serde_json::to_value(&send_resp).unwrap();
    assert!(json.get("task").is_some());

    // GetTaskRequest / CancelTaskRequest
    let get_req = GetTaskRequest { task_id: "t1".into() };
    let cancel_req = CancelTaskRequest { task_id: "t1".into(), reason: Some("test".into()) };

    assert!(serde_json::to_value(&get_req).unwrap().get("taskId").is_some());
    assert!(serde_json::to_value(&cancel_req).unwrap().get("reason").is_some());
}

// =============================================================================
// Streaming Event Compliance
// =============================================================================

#[test]
fn test_streaming_events() {
    let status_event = StreamingEvent::TaskStatusUpdate(TaskStatusUpdateEvent {
        task_id: "task-123".to_string(),
        status: TaskStatus::Completed,
        message: Some(Message::agent("Done")),
        timestamp: Utc::now(),
    });

    let artifact_event = StreamingEvent::TaskArtifactUpdate(TaskArtifactUpdateEvent {
        task_id: "task-123".to_string(),
        artifact: Artifact::text("art-1", "Content"),
        is_final: true,
        timestamp: Utc::now(),
    });

    // Type tags
    let status_json = serde_json::to_value(&status_event).unwrap();
    let artifact_json = serde_json::to_value(&artifact_event).unwrap();

    assert_eq!(status_json["type"], "taskStatusUpdate");
    assert_eq!(artifact_json["type"], "taskArtifactUpdate");
    assert!(status_json.get("taskId").is_some());
    assert!(artifact_json.get("isFinal").is_some());

    // Round-trip
    for event in [status_event, artifact_event] {
        let json = serde_json::to_string(&event).unwrap();
        let _: StreamingEvent = serde_json::from_str(&json).unwrap();
    }
}

// =============================================================================
// Security Scheme Compliance
// =============================================================================

#[test]
fn test_security_schemes() {
    let schemes = [
        (
            SecurityScheme::ApiKey {
                name: "X-API-Key".into(),
                location: ApiKeyLocation::Header,
            },
            "apiKey",
        ),
        (
            SecurityScheme::Http { scheme: "bearer".into() },
            "http",
        ),
        (
            SecurityScheme::OAuth2 {
                flows: OAuth2Flows {
                    authorization_code: None,
                    client_credentials: Some(OAuth2Flow {
                        authorization_url: None,
                        token_url: "https://auth.example.com/token".into(),
                        scopes: HashMap::new(),
                    }),
                },
            },
            "oauth2",
        ),
    ];

    for (scheme, expected_type) in schemes {
        let json = serde_json::to_value(&scheme).unwrap();
        assert_eq!(json["type"], expected_type);
    }

    // ApiKeyLocation
    assert_eq!(serde_json::to_string(&ApiKeyLocation::Header).unwrap(), "\"header\"");
    assert_eq!(serde_json::to_string(&ApiKeyLocation::Query).unwrap(), "\"query\"");
}

// =============================================================================
// Integration: Full Lifecycle
// =============================================================================

#[test]
fn test_full_task_lifecycle() {
    // Create -> Working -> Add messages -> Add artifact -> Complete
    let mut task = Task::new_with_uuid();
    assert_eq!(task.status, TaskStatus::Working);

    task.add_message(Message::user("Process this"));
    task.add_message(Message::agent("Done"));
    task.add_artifact(Artifact::text("out-1", "Result").with_label("Output"));
    task.set_status(TaskStatus::Completed);

    assert!(task.is_terminal());
    assert_eq!(task.messages.len(), 2);
    assert_eq!(task.artifacts.len(), 1);

    // Serialization round-trip preserves everything
    let json = serde_json::to_string(&task).unwrap();
    let parsed: Task = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.status, TaskStatus::Completed);
    assert_eq!(parsed.messages.len(), 2);
}
