//! A2A Event Streaming
//!
//! This module implements Server-Sent Events (SSE) for real-time task updates
//! following the A2A protocol specification.

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::Stream;
use serde::{Deserialize, Serialize};
use skreaver_a2a::{Message, Task, TaskStatus};
use skreaver_tools::ToolRegistry;
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast;

use super::A2aState;

/// Query parameters for the events endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct EventsQuery {
    /// Filter events by task ID
    #[serde(rename = "taskId")]
    pub task_id: Option<String>,
    /// Only return events since this timestamp (ISO 8601)
    pub since: Option<String>,
}

/// A2A Event types for SSE streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum A2aEvent {
    /// Task was created
    TaskCreated { task_id: String, timestamp: String },
    /// Task status changed
    TaskStatusUpdate {
        task_id: String,
        status: TaskStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        final_response: Option<bool>,
        timestamp: String,
    },
    /// New message added to task
    TaskMessage {
        task_id: String,
        message: Message,
        timestamp: String,
    },
    /// Artifact produced by task
    TaskArtifact {
        task_id: String,
        artifact_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        timestamp: String,
    },
    /// Progress update for long-running tasks
    TaskProgress {
        task_id: String,
        progress: f32,
        #[serde(skip_serializing_if = "Option::is_none")]
        status_message: Option<String>,
        timestamp: String,
    },
    /// Keep-alive ping
    Ping { timestamp: String },
}

impl A2aEvent {
    /// Get the event type as a string for SSE
    pub fn event_type(&self) -> &'static str {
        match self {
            A2aEvent::TaskCreated { .. } => "task_created",
            A2aEvent::TaskStatusUpdate { .. } => "task_status_update",
            A2aEvent::TaskMessage { .. } => "task_message",
            A2aEvent::TaskArtifact { .. } => "task_artifact",
            A2aEvent::TaskProgress { .. } => "task_progress",
            A2aEvent::Ping { .. } => "ping",
        }
    }

    /// Get the task ID if this event is related to a task
    pub fn task_id(&self) -> Option<&str> {
        match self {
            A2aEvent::TaskCreated { task_id, .. } => Some(task_id),
            A2aEvent::TaskStatusUpdate { task_id, .. } => Some(task_id),
            A2aEvent::TaskMessage { task_id, .. } => Some(task_id),
            A2aEvent::TaskArtifact { task_id, .. } => Some(task_id),
            A2aEvent::TaskProgress { task_id, .. } => Some(task_id),
            A2aEvent::Ping { .. } => None,
        }
    }

    /// Create a task created event
    pub fn task_created(task_id: &str) -> Self {
        Self::TaskCreated {
            task_id: task_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a task status update event
    pub fn task_status(task_id: &str, status: TaskStatus, is_final: bool) -> Self {
        Self::TaskStatusUpdate {
            task_id: task_id.to_string(),
            status,
            final_response: if is_final { Some(true) } else { None },
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a task message event
    pub fn task_message(task_id: &str, message: Message) -> Self {
        Self::TaskMessage {
            task_id: task_id.to_string(),
            message,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a task progress event
    pub fn task_progress(task_id: &str, progress: f32, status_message: Option<String>) -> Self {
        Self::TaskProgress {
            task_id: task_id.to_string(),
            progress: progress.clamp(0.0, 100.0),
            status_message,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create a ping event
    pub fn ping() -> Self {
        Self::Ping {
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Broadcaster for A2A events
///
/// This allows multiple SSE clients to receive task updates in real-time.
pub struct A2aEventBroadcaster {
    sender: broadcast::Sender<A2aEvent>,
}

impl A2aEventBroadcaster {
    /// Create a new event broadcaster
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<A2aEvent> {
        self.sender.subscribe()
    }

    /// Broadcast an event to all subscribers
    pub fn broadcast(&self, event: A2aEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
    }

    /// Broadcast task created event
    pub async fn broadcast_task_created(&self, task: &Task) {
        self.broadcast(A2aEvent::task_created(&task.id));
    }

    /// Broadcast task status update
    pub async fn broadcast_task_status(&self, task: &Task) {
        let is_final = task.is_terminal();
        self.broadcast(A2aEvent::task_status(&task.id, task.status, is_final));
    }

    /// Broadcast task message
    pub async fn broadcast_task_message(&self, task: &Task, message: &Message) {
        self.broadcast(A2aEvent::task_message(&task.id, message.clone()));
    }

    /// Broadcast task progress
    pub async fn broadcast_task_progress(
        &self,
        task_id: &str,
        progress: f32,
        status_message: Option<String>,
    ) {
        self.broadcast(A2aEvent::task_progress(task_id, progress, status_message));
    }

    /// Get the number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for A2aEventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// Create an event stream from a broadcast receiver
fn create_event_stream(
    receiver: broadcast::Receiver<A2aEvent>,
    task_filter: Option<String>,
) -> impl Stream<Item = Result<Event, Infallible>> {
    futures::stream::unfold((receiver, task_filter), |(mut rx, filter)| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    // Apply task filter if specified
                    if let Some(ref f) = filter
                        && let Some(event_task_id) = event.task_id()
                        && event_task_id != f
                    {
                        continue; // Skip this event and try the next one
                    }

                    let event_type = event.event_type();
                    let json_data =
                        serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());

                    let sse_event = Event::default()
                        .event(event_type)
                        .data(json_data)
                        .id(uuid::Uuid::new_v4().to_string());

                    return Some((Ok(sse_event), (rx, filter)));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Lagged, try again
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return None;
                }
            }
        }
    })
}

/// GET /a2a/events - Subscribe to Server-Sent Events
///
/// Opens an SSE stream to receive real-time task updates. The stream
/// will emit events for task creation, status changes, messages, and artifacts.
///
/// # Query Parameters
///
/// - `taskId` - Filter events for a specific task
/// - `since` - Only return events after this timestamp (ISO 8601)
///
/// # Event Format
///
/// Events are sent as JSON objects with a `type` field indicating the event type.
pub async fn events_stream<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(state): State<A2aState<T>>,
    Query(params): Query<EventsQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.event_broadcaster.subscribe();
    let stream = create_event_stream(receiver, params.task_id);

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type() {
        let event = A2aEvent::task_created("task-1");
        assert_eq!(event.event_type(), "task_created");

        let event = A2aEvent::task_status("task-1", TaskStatus::Working, false);
        assert_eq!(event.event_type(), "task_status_update");

        let event = A2aEvent::ping();
        assert_eq!(event.event_type(), "ping");
    }

    #[test]
    fn test_event_task_id() {
        let event = A2aEvent::task_created("task-123");
        assert_eq!(event.task_id(), Some("task-123"));

        let event = A2aEvent::ping();
        assert_eq!(event.task_id(), None);
    }

    #[test]
    fn test_event_serialization() {
        let event = A2aEvent::task_status("task-1", TaskStatus::Completed, true);
        let json = serde_json::to_string(&event).unwrap();

        // Verify structure - rename_all affects variant names and field names
        assert!(json.contains("\"type\":\"taskStatusUpdate\""));
        assert!(json.contains("task_id")); // field name
        assert!(json.contains("task-1")); // field value
        assert!(json.contains("\"status\":\"completed\""));
        assert!(json.contains("final_response")); // field name
    }

    #[tokio::test]
    async fn test_broadcaster_subscribe() {
        let broadcaster = A2aEventBroadcaster::new();

        let mut receiver = broadcaster.subscribe();

        broadcaster.broadcast(A2aEvent::ping());

        let event = tokio::time::timeout(Duration::from_millis(100), receiver.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, A2aEvent::Ping { .. }));
    }

    #[tokio::test]
    async fn test_broadcaster_multiple_subscribers() {
        let broadcaster = A2aEventBroadcaster::new();

        let mut receiver1 = broadcaster.subscribe();
        let mut receiver2 = broadcaster.subscribe();

        broadcaster.broadcast(A2aEvent::task_created("task-1"));

        let event1 = receiver1.recv().await.unwrap();
        let event2 = receiver2.recv().await.unwrap();

        assert_eq!(event1.task_id(), Some("task-1"));
        assert_eq!(event2.task_id(), Some("task-1"));
    }

    #[test]
    fn test_progress_clamping() {
        let event = A2aEvent::task_progress("task-1", 150.0, None);
        if let A2aEvent::TaskProgress { progress, .. } = event {
            assert_eq!(progress, 100.0);
        } else {
            panic!("Expected TaskProgress event");
        }

        let event = A2aEvent::task_progress("task-1", -10.0, None);
        if let A2aEvent::TaskProgress { progress, .. } = event {
            assert_eq!(progress, 0.0);
        } else {
            panic!("Expected TaskProgress event");
        }
    }

    #[test]
    fn test_subscriber_count() {
        let broadcaster = A2aEventBroadcaster::new();
        assert_eq!(broadcaster.subscriber_count(), 0);

        let _r1 = broadcaster.subscribe();
        assert_eq!(broadcaster.subscriber_count(), 1);

        let _r2 = broadcaster.subscribe();
        assert_eq!(broadcaster.subscriber_count(), 2);
    }
}
