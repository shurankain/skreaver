//! Integration tests for A2A client/server interaction
//!
//! These tests verify that the A2A client and server work correctly together,
//! testing agent discovery, message exchange, task management, and streaming.

#![cfg(all(feature = "client", feature = "server"))]

use async_trait::async_trait;
use futures::StreamExt;
use skreaver_a2a::{
    A2aClient, A2aServer, AgentCard, AgentHandler, AgentSkill, Artifact, Message, Task, TaskStatus,
    send_artifact_update, send_status_update,
};
use std::net::TcpListener;
use std::time::Duration;
use tokio::sync::broadcast;

// =============================================================================
// Test Agent Handlers
// =============================================================================

/// A simple echo agent that echoes messages back
struct EchoAgent;

#[async_trait]
impl AgentHandler for EchoAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new("echo-agent", "Echo Agent", "http://localhost")
            .with_description("An agent that echoes messages back")
            .with_skill(
                AgentSkill::new("echo", "Echo").with_description("Echoes the input message"),
            )
    }

    async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
        let text = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("(no text)");

        task.add_message(Message::agent(format!("Echo: {}", text)));
        Ok(())
    }
}

/// An agent that requires multiple inputs
struct MultiTurnAgent {
    required_turns: usize,
}

impl MultiTurnAgent {
    fn new(required_turns: usize) -> Self {
        Self { required_turns }
    }
}

#[async_trait]
impl AgentHandler for MultiTurnAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new("multi-turn-agent", "Multi-Turn Agent", "http://localhost")
            .with_description("An agent that requires multiple turns to complete")
            .with_skill(AgentSkill::new("collect", "Collect Inputs"))
    }

    async fn handle_message(&self, task: &mut Task, _message: Message) -> Result<(), String> {
        // Count user messages
        let user_messages = task
            .messages
            .iter()
            .filter(|m| matches!(m.role, skreaver_a2a::Role::User))
            .count();

        if user_messages < self.required_turns {
            let remaining = self.required_turns - user_messages;
            task.add_message(Message::agent(format!(
                "Received message {}. Need {} more inputs.",
                user_messages, remaining
            )));
            task.set_status(TaskStatus::InputRequired);
        } else {
            task.add_message(Message::agent(format!(
                "Received all {} inputs. Task complete!",
                user_messages
            )));
            task.set_status(TaskStatus::Completed);
        }

        Ok(())
    }
}

/// An agent that produces artifacts
struct ArtifactAgent;

#[async_trait]
impl AgentHandler for ArtifactAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new("artifact-agent", "Artifact Agent", "http://localhost")
            .with_description("An agent that produces artifacts")
            .with_skill(
                AgentSkill::new("generate", "Generate Artifacts")
                    .with_description("Generates text artifacts"),
            )
    }

    async fn handle_message(&self, task: &mut Task, message: Message) -> Result<(), String> {
        let text = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("default");

        // Create an artifact
        let artifact = Artifact::text(
            format!("artifact-{}", uuid::Uuid::new_v4()),
            format!("Generated content for: {}", text),
        )
        .with_label("Generated Document");

        task.add_artifact(artifact);
        task.add_message(Message::agent("I've generated an artifact for you."));

        Ok(())
    }
}

/// An agent that supports streaming
struct StreamingAgent;

#[async_trait]
impl AgentHandler for StreamingAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new("streaming-agent", "Streaming Agent", "http://localhost")
            .with_description("An agent that streams responses")
            .with_streaming()
            .with_skill(AgentSkill::new("stream", "Stream Response"))
    }

    async fn handle_message(&self, task: &mut Task, _message: Message) -> Result<(), String> {
        task.add_message(Message::agent("Processing complete (non-streaming)."));
        Ok(())
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn handle_message_streaming(
        &self,
        task: &mut Task,
        message: Message,
        event_tx: broadcast::Sender<skreaver_a2a::StreamingEvent>,
    ) -> Result<(), String> {
        let text = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("default");

        // Send status updates as we "process"
        send_status_update(&event_tx, &task.id, TaskStatus::Working, None);

        // Simulate processing with multiple artifact updates
        for i in 1..=3 {
            tokio::time::sleep(Duration::from_millis(10)).await;

            let artifact = Artifact::text(
                format!("part-{}", i),
                format!("Part {} of response for: {}", i, text),
            )
            .with_label(format!("Part {}", i));

            send_artifact_update(&event_tx, &task.id, artifact.clone(), i == 3);
            task.add_artifact(artifact);
        }

        task.add_message(Message::agent("Streaming complete!"));
        task.set_status(TaskStatus::Completed);

        Ok(())
    }
}

/// An agent that can fail
struct FailingAgent;

#[async_trait]
impl AgentHandler for FailingAgent {
    fn agent_card(&self) -> AgentCard {
        AgentCard::new("failing-agent", "Failing Agent", "http://localhost")
            .with_description("An agent that fails on demand")
            .with_skill(AgentSkill::new("fail", "Fail"))
    }

    async fn handle_message(&self, _task: &mut Task, message: Message) -> Result<(), String> {
        let text = message
            .parts
            .first()
            .and_then(|p| p.as_text())
            .unwrap_or("");

        if text.contains("fail") {
            Err("Intentional failure triggered".to_string())
        } else {
            Ok(())
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Find an available port for testing
fn find_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// Start a test server and return the address
async fn start_test_server<H: AgentHandler>(handler: H) -> String {
    let port = find_available_port();
    let addr = format!("127.0.0.1:{}", port);
    let server = A2aServer::new(handler);
    let router = server.router();

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    let actual_addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Give the server a moment to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    format!("http://{}", actual_addr)
}

// =============================================================================
// Tests: Agent Discovery
// =============================================================================

#[tokio::test]
async fn test_agent_card_discovery() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let card = client.get_agent_card().await.unwrap();

    assert_eq!(card.agent_id, "echo-agent");
    assert_eq!(card.name, "Echo Agent");
    assert_eq!(card.skills.len(), 1);
    assert_eq!(card.skills[0].id, "echo");
}

#[tokio::test]
async fn test_agent_card_with_streaming() {
    let base_url = start_test_server(StreamingAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let card = client.get_agent_card().await.unwrap();

    assert!(card.capabilities.streaming);
}

#[tokio::test]
async fn test_client_discover_and_cache() {
    let base_url = start_test_server(EchoAgent).await;
    let mut client = A2aClient::new(&base_url).unwrap();

    // First call should fetch
    let card1 = client.discover().await.unwrap();
    assert_eq!(card1.agent_id, "echo-agent");

    // Second call should return cached
    let card2 = client.agent_card().await.unwrap();
    assert_eq!(card1.agent_id, card2.agent_id);
}

// =============================================================================
// Tests: Basic Message Exchange
// =============================================================================

#[tokio::test]
async fn test_send_simple_message() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let task = client.send_message("Hello, Agent!").await.unwrap();

    assert_eq!(task.status, TaskStatus::Completed);
    assert!(!task.messages.is_empty());

    // Find the agent's response
    let response = task
        .messages
        .iter()
        .find(|m| matches!(m.role, skreaver_a2a::Role::Agent))
        .unwrap();

    let response_text = response.parts[0].as_text().unwrap();
    assert!(response_text.contains("Echo: Hello, Agent!"));
}

#[tokio::test]
async fn test_send_full_message() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let message = Message::user("Full message test");
    let task = client.send(message, None, None).await.unwrap();

    assert_eq!(task.status, TaskStatus::Completed);
}

#[tokio::test]
async fn test_get_task() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    // Create a task
    let created_task = client.send_message("Create task").await.unwrap();

    // Fetch the same task
    let fetched_task = client.get_task(&created_task.id).await.unwrap();

    assert_eq!(created_task.id, fetched_task.id);
    assert_eq!(created_task.status, fetched_task.status);
}

#[tokio::test]
async fn test_task_not_found() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let result = client.get_task("non-existent-task-id").await;

    assert!(result.is_err());
}

// =============================================================================
// Tests: Multi-Turn Conversations
// =============================================================================

#[tokio::test]
async fn test_multi_turn_conversation() {
    let base_url = start_test_server(MultiTurnAgent::new(3)).await;
    let client = A2aClient::new(&base_url).unwrap();

    // First turn
    let task = client.send_message("Input 1").await.unwrap();
    assert_eq!(task.status, TaskStatus::InputRequired);

    // Second turn
    let task = client.continue_task(&task.id, "Input 2").await.unwrap();
    assert_eq!(task.status, TaskStatus::InputRequired);

    // Third turn
    let task = client.continue_task(&task.id, "Input 3").await.unwrap();
    assert_eq!(task.status, TaskStatus::Completed);
}

#[tokio::test]
async fn test_continue_with_context_id() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    // Send with context ID
    let message = Message::user("Message with context");
    let task = client
        .send(message, None, Some("context-123".to_string()))
        .await
        .unwrap();

    assert_eq!(task.context_id, Some("context-123".to_string()));
}

// =============================================================================
// Tests: Task Cancellation
// =============================================================================

#[tokio::test]
async fn test_cancel_task() {
    let base_url = start_test_server(MultiTurnAgent::new(5)).await;
    let client = A2aClient::new(&base_url).unwrap();

    // Create a task that requires multiple inputs (won't complete immediately)
    let task = client.send_message("Start").await.unwrap();
    assert_eq!(task.status, TaskStatus::InputRequired);

    // Cancel it
    let cancelled_task = client.cancel_task(&task.id, None).await.unwrap();
    assert_eq!(cancelled_task.status, TaskStatus::Cancelled);

    // Verify it's really cancelled
    let fetched_task = client.get_task(&task.id).await.unwrap();
    assert_eq!(fetched_task.status, TaskStatus::Cancelled);
}

#[tokio::test]
async fn test_cancel_with_reason() {
    let base_url = start_test_server(MultiTurnAgent::new(5)).await;
    let client = A2aClient::new(&base_url).unwrap();

    let task = client.send_message("Start").await.unwrap();
    let cancelled_task = client
        .cancel_task(&task.id, Some("User requested".to_string()))
        .await
        .unwrap();

    assert_eq!(cancelled_task.status, TaskStatus::Cancelled);
}

#[tokio::test]
async fn test_cannot_cancel_completed_task() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    // Create and complete a task
    let task = client.send_message("Hello").await.unwrap();
    assert_eq!(task.status, TaskStatus::Completed);

    // Try to cancel it
    let result = client.cancel_task(&task.id, None).await;
    assert!(result.is_err());
}

// =============================================================================
// Tests: Artifacts
// =============================================================================

#[tokio::test]
async fn test_artifact_generation() {
    let base_url = start_test_server(ArtifactAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let task = client.send_message("Generate something").await.unwrap();

    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.artifacts.len(), 1);

    let artifact = &task.artifacts[0];
    assert!(
        artifact.parts[0]
            .as_text()
            .unwrap()
            .contains("Generate something")
    );
}

// =============================================================================
// Tests: Streaming
// =============================================================================

#[tokio::test]
async fn test_streaming_send_message() {
    let base_url = start_test_server(StreamingAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let mut stream = client.send_message_streaming("Stream this").await.unwrap();

    let mut events = Vec::new();
    while let Some(result) = stream.next().await {
        let event = result.unwrap();
        events.push(event);
    }

    // Should have received multiple events
    assert!(!events.is_empty());

    // Should have status updates and artifact updates
    let has_status = events
        .iter()
        .any(|e| matches!(e, skreaver_a2a::StreamingEvent::TaskStatusUpdate(_)));
    let has_artifact = events
        .iter()
        .any(|e| matches!(e, skreaver_a2a::StreamingEvent::TaskArtifactUpdate(_)));

    assert!(has_status, "Should have status updates");
    assert!(has_artifact, "Should have artifact updates");
}

#[tokio::test]
async fn test_streaming_final_status() {
    let base_url = start_test_server(StreamingAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let mut stream = client.send_message_streaming("Test").await.unwrap();

    let mut last_status = None;
    while let Some(result) = stream.next().await {
        if let Ok(skreaver_a2a::StreamingEvent::TaskStatusUpdate(update)) = result {
            last_status = Some(update.status);
        }
    }

    // Last status should be Completed
    assert_eq!(last_status, Some(TaskStatus::Completed));
}

// =============================================================================
// Tests: Error Handling
// =============================================================================

#[tokio::test]
async fn test_handler_error() {
    let base_url = start_test_server(FailingAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    // This should trigger a failure
    let task = client.send_message("Please fail").await.unwrap();

    assert_eq!(task.status, TaskStatus::Failed);
}

#[tokio::test]
async fn test_handler_success() {
    let base_url = start_test_server(FailingAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    // This should succeed
    let task = client.send_message("Please succeed").await.unwrap();

    assert_eq!(task.status, TaskStatus::Completed);
}

// =============================================================================
// Tests: Polling and Waiting
// =============================================================================

#[tokio::test]
async fn test_wait_for_task() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    // Create a task
    let task = client.send_message("Hello").await.unwrap();

    // Wait for it (should already be complete)
    let completed_task = client
        .wait_for_task(&task.id, Duration::from_millis(50), Duration::from_secs(5))
        .await
        .unwrap();

    assert!(completed_task.is_terminal());
}

// =============================================================================
// Tests: Router Construction
// =============================================================================

#[test]
fn test_server_router_creation() {
    let server = A2aServer::new(EchoAgent);
    let _router = server.router();
    // Just verify it doesn't panic
}

// =============================================================================
// Tests: Task State Transitions
// =============================================================================

#[tokio::test]
async fn test_task_status_progression() {
    let base_url = start_test_server(MultiTurnAgent::new(2)).await;
    let client = A2aClient::new(&base_url).unwrap();

    // Working -> InputRequired
    let task = client.send_message("First").await.unwrap();
    assert_eq!(task.status, TaskStatus::InputRequired);

    // InputRequired -> Completed
    let task = client.continue_task(&task.id, "Second").await.unwrap();
    assert_eq!(task.status, TaskStatus::Completed);

    // Completed is terminal
    assert!(task.is_terminal());
}

// =============================================================================
// Tests: Message Metadata
// =============================================================================

#[tokio::test]
async fn test_message_with_metadata() {
    let base_url = start_test_server(EchoAgent).await;
    let client = A2aClient::new(&base_url).unwrap();

    let mut message = Message::user("Test");
    message
        .metadata
        .insert("custom_key".to_string(), serde_json::json!("custom_value"));

    let request = skreaver_a2a::SendMessageRequest {
        message,
        task_id: None,
        context_id: None,
        metadata: Default::default(),
    };

    // Send using the raw endpoint
    let task = client
        .send(request.message, request.task_id, request.context_id)
        .await
        .unwrap();

    assert_eq!(task.status, TaskStatus::Completed);
}
