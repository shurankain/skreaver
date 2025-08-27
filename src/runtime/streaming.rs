//! # Streaming Responses
//!
//! This module provides streaming response capabilities for long-running
//! agent operations, allowing clients to receive real-time updates.

use axum::{
    BoxError,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

/// Agent execution update types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentUpdate {
    /// Agent started processing
    Started {
        agent_id: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Agent is thinking/processing
    Thinking {
        agent_id: String,
        step: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Agent called a tool
    ToolCall {
        agent_id: String,
        tool_name: String,
        input: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Tool execution result
    ToolResult {
        agent_id: String,
        tool_name: String,
        success: bool,
        output: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Agent produced intermediate output
    Partial {
        agent_id: String,
        content: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Agent completed processing
    Completed {
        agent_id: String,
        final_response: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Agent encountered an error
    Error {
        agent_id: String,
        error: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Keep-alive ping
    Ping {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Create a Server-Sent Events stream from agent updates
pub fn create_sse_stream(
    updates: tokio::sync::mpsc::Receiver<AgentUpdate>,
) -> Sse<impl Stream<Item = Result<Event, BoxError>>> {
    let stream = ReceiverStream::new(updates).map(|update| {
        let event_type = match &update {
            AgentUpdate::Started { .. } => "started",
            AgentUpdate::Thinking { .. } => "thinking",
            AgentUpdate::ToolCall { .. } => "tool_call",
            AgentUpdate::ToolResult { .. } => "tool_result",
            AgentUpdate::Partial { .. } => "partial",
            AgentUpdate::Completed { .. } => "completed",
            AgentUpdate::Error { .. } => "error",
            AgentUpdate::Ping { .. } => "ping",
        };

        let json_data = serde_json::to_string(&update).map_err(|e| Box::new(e) as BoxError)?;

        Ok(Event::default()
            .event(event_type)
            .data(json_data)
            .id(uuid::Uuid::new_v4().to_string()))
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    )
}

/// Streaming agent executor that sends updates via channel
pub struct StreamingAgentExecutor {
    pub update_sender: tokio::sync::mpsc::Sender<AgentUpdate>,
}

impl StreamingAgentExecutor {
    /// Create a new streaming executor
    pub fn new() -> (Self, tokio::sync::mpsc::Receiver<AgentUpdate>) {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        (Self { update_sender: tx }, rx)
    }

    /// Send an agent update
    pub async fn send_update(
        &self,
        update: AgentUpdate,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<AgentUpdate>> {
        self.update_sender.send(update).await
    }

    /// Execute an agent with streaming updates
    pub async fn execute_with_streaming<F, Fut>(
        &self,
        agent_id: String,
        operation: F,
    ) -> Result<String, String>
    where
        F: FnOnce(StreamingAgentExecutor) -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        // Send started event
        let _ = self
            .send_update(AgentUpdate::Started {
                agent_id: agent_id.clone(),
                timestamp: chrono::Utc::now(),
            })
            .await;

        // Execute the operation
        let executor = StreamingAgentExecutor {
            update_sender: self.update_sender.clone(),
        };

        match operation(executor).await {
            Ok(result) => {
                // Send completion event
                let _ = self
                    .send_update(AgentUpdate::Completed {
                        agent_id,
                        final_response: result.clone(),
                        timestamp: chrono::Utc::now(),
                    })
                    .await;
                Ok(result)
            }
            Err(error) => {
                // Send error event
                let _ = self
                    .send_update(AgentUpdate::Error {
                        agent_id,
                        error: error.clone(),
                        timestamp: chrono::Utc::now(),
                    })
                    .await;
                Err(error)
            }
        }
    }

    /// Report that agent is thinking
    pub async fn thinking(&self, agent_id: &str, step: &str) {
        let _ = self
            .send_update(AgentUpdate::Thinking {
                agent_id: agent_id.to_string(),
                step: step.to_string(),
                timestamp: chrono::Utc::now(),
            })
            .await;
    }

    /// Report a tool call
    pub async fn tool_call(&self, agent_id: &str, tool_name: &str, input: &str) {
        let _ = self
            .send_update(AgentUpdate::ToolCall {
                agent_id: agent_id.to_string(),
                tool_name: tool_name.to_string(),
                input: input.to_string(),
                timestamp: chrono::Utc::now(),
            })
            .await;
    }

    /// Report a tool result
    pub async fn tool_result(&self, agent_id: &str, tool_name: &str, success: bool, output: &str) {
        let _ = self
            .send_update(AgentUpdate::ToolResult {
                agent_id: agent_id.to_string(),
                tool_name: tool_name.to_string(),
                success,
                output: output.to_string(),
                timestamp: chrono::Utc::now(),
            })
            .await;
    }

    /// Report partial output
    pub async fn partial(&self, agent_id: &str, content: &str) {
        let _ = self
            .send_update(AgentUpdate::Partial {
                agent_id: agent_id.to_string(),
                content: content.to_string(),
                timestamp: chrono::Utc::now(),
            })
            .await;
    }
}

impl Default for StreamingAgentExecutor {
    fn default() -> Self {
        Self::new().0
    }
}

/// WebSocket message types for bidirectional communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    /// Client sends observation to agent
    Observe {
        agent_id: String,
        input: String,
        stream: bool, // Whether to stream the response
    },
    /// Client requests agent status
    Status {
        agent_id: String,
    },
    /// Client subscribes to agent updates
    Subscribe {
        agent_id: String,
    },
    /// Client unsubscribes from agent updates
    Unsubscribe {
        agent_id: String,
    },
    /// Server sends agent update (same as AgentUpdate)
    AgentUpdate(AgentUpdate),
    /// Server sends error message
    Error {
        message: String,
        code: String,
    },
    /// Ping/pong for connection health
    Ping,
    Pong,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_streaming_executor_creation() {
        let (executor, mut receiver) = StreamingAgentExecutor::new();

        // Send a test update
        executor
            .send_update(AgentUpdate::Started {
                agent_id: "test-agent".to_string(),
                timestamp: chrono::Utc::now(),
            })
            .await
            .unwrap();

        // Receive the update
        let received = receiver.recv().await.unwrap();
        match received {
            AgentUpdate::Started { agent_id, .. } => {
                assert_eq!(agent_id, "test-agent");
            }
            _ => panic!("Unexpected update type"),
        }
    }

    #[tokio::test]
    async fn test_streaming_execution() {
        let (executor, _receiver) = StreamingAgentExecutor::new();

        let result = executor
            .execute_with_streaming("test-agent".to_string(), |_exec| async {
                // Simulate some work
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok("Hello, world!".to_string())
            })
            .await;

        assert_eq!(result.unwrap(), "Hello, world!");
    }

    #[test]
    fn test_agent_update_serialization() {
        let update = AgentUpdate::Started {
            agent_id: "test".to_string(),
            timestamp: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&update).unwrap();
        let deserialized: AgentUpdate = serde_json::from_str(&json).unwrap();

        match deserialized {
            AgentUpdate::Started { agent_id, .. } => {
                assert_eq!(agent_id, "test");
            }
            _ => panic!("Unexpected deserialized type"),
        }
    }
}
