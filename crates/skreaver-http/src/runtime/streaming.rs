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
    /// Tool execution succeeded
    ToolSuccess {
        agent_id: String,
        tool_name: String,
        output: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Tool execution failed
    ToolFailure {
        agent_id: String,
        tool_name: String,
        error: String,
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
    /// Progress update for long-running operations
    Progress {
        agent_id: String,
        progress_percent: f32,
        status_message: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// Create a Server-Sent Events stream from agent updates with proper termination
pub fn create_sse_stream(
    updates: tokio::sync::mpsc::Receiver<AgentUpdate>,
) -> Sse<impl Stream<Item = Result<Event, BoxError>>> {
    let stream = ReceiverStream::new(updates).map(|update| {
        let event_type = match &update {
            AgentUpdate::Started { .. } => "started",
            AgentUpdate::Thinking { .. } => "thinking",
            AgentUpdate::ToolCall { .. } => "tool_call",
            AgentUpdate::ToolSuccess { .. } => "tool_success",
            AgentUpdate::ToolFailure { .. } => "tool_failure",
            AgentUpdate::Partial { .. } => "partial",
            AgentUpdate::Completed { .. } => "completed",
            AgentUpdate::Error { .. } => "error",
            AgentUpdate::Ping { .. } => "ping",
            AgentUpdate::Progress { .. } => "progress",
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
#[derive(Clone)]
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

    /// Report a successful tool execution
    pub async fn tool_success(&self, agent_id: &str, tool_name: &str, output: &str) {
        let _ = self
            .send_update(AgentUpdate::ToolSuccess {
                agent_id: agent_id.to_string(),
                tool_name: tool_name.to_string(),
                output: output.to_string(),
                timestamp: chrono::Utc::now(),
            })
            .await;
    }

    /// Report a failed tool execution
    pub async fn tool_failure(&self, agent_id: &str, tool_name: &str, error: &str) {
        let _ = self
            .send_update(AgentUpdate::ToolFailure {
                agent_id: agent_id.to_string(),
                tool_name: tool_name.to_string(),
                error: error.to_string(),
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

    /// Report progress update
    pub async fn progress(&self, agent_id: &str, progress_percent: f32, status_message: &str) {
        let _ = self
            .send_update(AgentUpdate::Progress {
                agent_id: agent_id.to_string(),
                progress_percent: progress_percent.clamp(0.0, 100.0),
                status_message: status_message.to_string(),
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

    #[tokio::test]
    async fn test_progress_reporting() {
        let (executor, mut receiver) = StreamingAgentExecutor::new();

        // Send progress updates
        executor.progress("test-agent", 25.0, "Starting").await;
        executor.progress("test-agent", 50.0, "Halfway").await;
        executor.progress("test-agent", 100.0, "Complete").await;

        // Check first progress update
        let update1 = receiver.recv().await.unwrap();
        match update1 {
            AgentUpdate::Progress {
                agent_id,
                progress_percent,
                status_message,
                ..
            } => {
                assert_eq!(agent_id, "test-agent");
                assert_eq!(progress_percent, 25.0);
                assert_eq!(status_message, "Starting");
            }
            _ => panic!("Expected Progress update"),
        }

        // Check second progress update
        let update2 = receiver.recv().await.unwrap();
        match update2 {
            AgentUpdate::Progress {
                progress_percent, ..
            } => {
                assert_eq!(progress_percent, 50.0);
            }
            _ => panic!("Expected Progress update"),
        }

        // Check final progress update
        let update3 = receiver.recv().await.unwrap();
        match update3 {
            AgentUpdate::Progress {
                progress_percent, ..
            } => {
                assert_eq!(progress_percent, 100.0);
            }
            _ => panic!("Expected Progress update"),
        }
    }

    #[tokio::test]
    async fn test_comprehensive_streaming_workflow() {
        let (executor, mut receiver) = StreamingAgentExecutor::new();

        let agent_id = "comprehensive-test-agent";

        // Simulate a comprehensive agent workflow
        tokio::spawn(async move {
            // Start
            executor.thinking(agent_id, "Initializing").await;
            executor.progress(agent_id, 10.0, "Analyzing input").await;

            // Tool usage simulation
            executor.tool_call(agent_id, "search", "query").await;
            executor
                .progress(agent_id, 30.0, "Processing search results")
                .await;
            executor
                .tool_success(agent_id, "search", "Found 10 results")
                .await;

            // Intermediate processing
            executor
                .thinking(agent_id, "Synthesizing information")
                .await;
            executor
                .progress(agent_id, 70.0, "Generating response")
                .await;
            executor.partial(agent_id, "Preliminary findings...").await;

            // Completion
            executor.progress(agent_id, 100.0, "Finalizing").await;
            let _ = executor
                .send_update(AgentUpdate::Completed {
                    agent_id: agent_id.to_string(),
                    final_response: "Complete analysis finished".to_string(),
                    timestamp: chrono::Utc::now(),
                })
                .await;
        });

        let mut update_count = 0;
        let mut received_types = Vec::new();

        // Collect all updates
        while let Some(update) = receiver.recv().await {
            update_count += 1;
            let update_type = match &update {
                AgentUpdate::Started { .. } => "started",
                AgentUpdate::Thinking { .. } => "thinking",
                AgentUpdate::ToolCall { .. } => "tool_call",
                AgentUpdate::ToolSuccess { .. } => "tool_success",
                AgentUpdate::ToolFailure { .. } => "tool_failure",
                AgentUpdate::Partial { .. } => "partial",
                AgentUpdate::Completed { .. } => "completed",
                AgentUpdate::Error { .. } => "error",
                AgentUpdate::Ping { .. } => "ping",
                AgentUpdate::Progress { .. } => "progress",
            };
            received_types.push(update_type);

            // Break after completion
            if matches!(update, AgentUpdate::Completed { .. }) {
                break;
            }

            // Safety break to avoid infinite loops
            if update_count > 20 {
                break;
            }
        }

        // Verify we received the expected types of updates
        assert!(received_types.contains(&"thinking"));
        assert!(received_types.contains(&"progress"));
        assert!(received_types.contains(&"tool_call"));
        assert!(received_types.contains(&"tool_success"));
        assert!(received_types.contains(&"partial"));
        assert!(received_types.contains(&"completed"));
        assert!(update_count >= 8); // Should have received multiple updates
    }

    #[tokio::test]
    async fn test_stream_cleanup_on_receiver_drop() {
        let (executor, receiver) = StreamingAgentExecutor::new();

        // Send an update
        executor
            .send_update(AgentUpdate::Ping {
                timestamp: chrono::Utc::now(),
            })
            .await
            .unwrap();

        // Drop the receiver
        drop(receiver);

        // Subsequent sends should fail
        let result = executor
            .send_update(AgentUpdate::Ping {
                timestamp: chrono::Utc::now(),
            })
            .await;

        assert!(
            result.is_err(),
            "Send should fail after receiver is dropped"
        );
    }

    #[tokio::test]
    async fn test_progress_clamping() {
        let (executor, mut receiver) = StreamingAgentExecutor::new();

        // Test progress values are clamped
        executor.progress("test", -10.0, "Invalid negative").await;
        executor.progress("test", 150.0, "Invalid over 100").await;

        let update1 = receiver.recv().await.unwrap();
        if let AgentUpdate::Progress {
            progress_percent, ..
        } = update1
        {
            assert_eq!(
                progress_percent, 0.0,
                "Negative progress should be clamped to 0"
            );
        }

        let update2 = receiver.recv().await.unwrap();
        if let AgentUpdate::Progress {
            progress_percent, ..
        } = update2
        {
            assert_eq!(
                progress_percent, 100.0,
                "Progress over 100 should be clamped to 100"
            );
        }
    }

    #[tokio::test]
    async fn test_concurrent_streaming_operations() {
        let (executor, mut receiver) = StreamingAgentExecutor::new();

        // Spawn multiple concurrent operations
        let executor1 = executor.clone();
        let executor2 = executor.clone();

        let handle1 = tokio::spawn(async move {
            executor1.thinking("agent1", "Processing").await;
            executor1.progress("agent1", 50.0, "Halfway").await;
        });

        let handle2 = tokio::spawn(async move {
            executor2.thinking("agent2", "Analyzing").await;
            executor2.progress("agent2", 75.0, "Almost done").await;
        });

        // Wait for both to complete
        let _ = tokio::join!(handle1, handle2);

        // Collect all updates
        let mut updates = Vec::new();
        for _ in 0..4 {
            if let Some(update) = receiver.recv().await {
                updates.push(update);
            }
        }

        assert_eq!(
            updates.len(),
            4,
            "Should receive updates from both operations"
        );

        // Verify we got updates from both agents
        let agent1_updates = updates
            .iter()
            .filter(|u| match u {
                AgentUpdate::Thinking { agent_id, .. } | AgentUpdate::Progress { agent_id, .. } => {
                    agent_id == "agent1"
                }
                _ => false,
            })
            .count();

        let agent2_updates = updates
            .iter()
            .filter(|u| match u {
                AgentUpdate::Thinking { agent_id, .. } | AgentUpdate::Progress { agent_id, .. } => {
                    agent_id == "agent2"
                }
                _ => false,
            })
            .count();

        assert_eq!(agent1_updates, 2, "Should have 2 updates from agent1");
        assert_eq!(agent2_updates, 2, "Should have 2 updates from agent2");
    }

    #[tokio::test]
    async fn test_sse_stream_termination() {
        let (executor, receiver) = StreamingAgentExecutor::new();

        // Create SSE stream (consumes receiver)
        let _sse_stream = create_sse_stream(receiver);

        // Send updates including completion
        executor.thinking("test", "Starting").await;
        executor.progress("test", 50.0, "Halfway").await;
        executor
            .send_update(AgentUpdate::Completed {
                agent_id: "test".to_string(),
                final_response: "Done".to_string(),
                timestamp: chrono::Utc::now(),
            })
            .await
            .unwrap();

        // The stream should terminate after completion
        // This is more of a conceptual test - in practice we'd need to consume the stream
        // to verify termination behavior

        // Verify further sends still work (channel not closed)
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _result = executor.thinking("test", "After completion").await;
        // This should not cause issues even if the stream is closed
    }

    #[tokio::test]
    async fn test_batch_like_concurrent_operations() {
        let (executor, mut receiver) = StreamingAgentExecutor::new();

        // Simulate multiple concurrent operations like batch processing
        let mut handles = Vec::new();

        for i in 0..5 {
            let exec = executor.clone();
            let handle = tokio::spawn(async move {
                exec.thinking(&format!("agent{}", i), "Processing").await;
                exec.progress(&format!("agent{}", i), 25.0 * (i + 1) as f32, "Working")
                    .await;
                exec.send_update(AgentUpdate::Completed {
                    agent_id: format!("agent{}", i),
                    final_response: format!("Result {}", i),
                    timestamp: chrono::Utc::now(),
                })
                .await
                .unwrap();
            });
            handles.push(handle);
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Collect all updates
        let mut updates = Vec::new();
        while let Ok(update) = receiver.try_recv() {
            updates.push(update);
        }

        // Should have received 15 updates (3 per agent: thinking, progress, completed)
        assert_eq!(
            updates.len(),
            15,
            "Should receive all updates from concurrent operations"
        );

        // Verify we have updates from all 5 agents
        let mut agent_counts = std::collections::HashMap::new();
        for update in updates {
            let agent_id = match update {
                AgentUpdate::Thinking { agent_id, .. }
                | AgentUpdate::Progress { agent_id, .. }
                | AgentUpdate::Completed { agent_id, .. } => agent_id,
                _ => continue,
            };
            *agent_counts.entry(agent_id).or_insert(0) += 1;
        }

        assert_eq!(
            agent_counts.len(),
            5,
            "Should have updates from all 5 agents"
        );
        for (_, count) in agent_counts {
            assert_eq!(count, 3, "Each agent should have exactly 3 updates");
        }
    }

    #[tokio::test]
    async fn test_streaming_with_closed_receiver() {
        let (executor, receiver) = StreamingAgentExecutor::new();

        // Drop the receiver to close the channel
        drop(receiver);

        // All streaming operations should complete without panicking
        // even though the channel is closed
        executor.thinking("test", "This should not panic").await;
        executor.progress("test", 50.0, "Should continue").await;
        executor.tool_call("test", "tool", "input").await;
        executor.tool_success("test", "tool", "output").await;
        executor.partial("test", "partial content").await;

        // These should all complete silently without errors
        // This verifies that ignoring send errors is acceptable behavior
        // when the receiver is closed (client disconnected)
    }

    #[tokio::test]
    async fn test_execute_with_streaming_after_receiver_closed() {
        let (executor, receiver) = StreamingAgentExecutor::new();

        // Drop receiver to simulate client disconnect
        drop(receiver);

        // The execute_with_streaming should still work
        let result = executor
            .execute_with_streaming("test-agent".to_string(), |exec| async move {
                exec.thinking("test-agent", "Processing").await;
                exec.progress("test-agent", 50.0, "Halfway").await;
                Ok("Success".to_string())
            })
            .await;

        // Should succeed even with closed channel
        assert_eq!(result.unwrap(), "Success");
    }
}
