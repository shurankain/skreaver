//! MCP protocol adapter for the unified agent interface.
//!
//! This module provides adapters to use MCP servers and bridges
//! through the unified agent interface.

use async_trait::async_trait;
use futures::Stream;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info};

use crate::error::{AgentError, AgentResult};
use crate::traits::{ToolInvoker, UnifiedAgent};
use crate::types::{
    AgentInfo, Capability, ContentPart, MessageRole, Protocol, StreamEvent, TaskStatus,
    UnifiedMessage, UnifiedTask,
};

use skreaver_core::tool::{ExecutionResult, Tool};
use skreaver_mcp::McpBridge;

/// Adapter that wraps an MCP bridge to provide the unified agent interface.
///
/// This allows external MCP servers to be used through the unified
/// agent abstraction.
pub struct McpAgentAdapter {
    info: AgentInfo,
    bridge: Arc<McpBridge>,
    tasks: tokio::sync::RwLock<HashMap<String, UnifiedTask>>,
}

impl std::fmt::Debug for McpAgentAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpAgentAdapter")
            .field("info", &self.info)
            .finish()
    }
}

impl McpAgentAdapter {
    /// Create a new MCP agent adapter from a bridge.
    pub fn new(bridge: McpBridge) -> Self {
        let tools = bridge.tools();

        // Build capabilities from discovered tools
        let capabilities: Vec<Capability> = tools
            .iter()
            .map(|tool| Capability::new(tool.name(), tool.name()).with_tag("mcp"))
            .collect();

        let info = AgentInfo::new(bridge.server_name(), bridge.server_name())
            .with_description(format!("MCP server with {} tools", bridge.tool_count()))
            .with_protocol(Protocol::Mcp)
            .with_capability(
                Capability::new("tool_call", "Tool Calling")
                    .with_description("Can call tools via MCP"),
            );

        let mut agent_info = info;
        for cap in capabilities {
            agent_info = agent_info.with_capability(cap);
        }

        Self {
            info: agent_info,
            bridge: Arc::new(bridge),
            tasks: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Connect to an MCP server and create an adapter.
    pub async fn connect(command: &str) -> AgentResult<Self> {
        info!(command = %command, "Connecting to MCP server");
        let bridge = McpBridge::connect_stdio(command)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;
        Ok(Self::new(bridge))
    }

    /// Connect with custom arguments.
    pub async fn connect_with_args<I, S>(program: &str, args: I) -> AgentResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let bridge = McpBridge::connect_with_args(program, args)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;
        Ok(Self::new(bridge))
    }

    /// Get the underlying bridge.
    pub fn bridge(&self) -> &McpBridge {
        &self.bridge
    }

    /// Process a message by handling tool calls.
    async fn process_message(
        &self,
        task: &mut UnifiedTask,
        message: &UnifiedMessage,
    ) -> AgentResult<()> {
        // Look for tool calls in the message
        for part in &message.content {
            if let ContentPart::ToolCall {
                id,
                name,
                arguments,
            } = part
            {
                debug!(tool = %name, id = %id, "Processing tool call");

                // Find and call the tool
                let result = self.invoke_tool(name, arguments.clone()).await;

                // Add result as a message
                let result_part = match result {
                    Ok(value) => ContentPart::ToolResult {
                        id: id.clone(),
                        result: value,
                        is_error: Some(false),
                    },
                    Err(e) => ContentPart::ToolResult {
                        id: id.clone(),
                        result: serde_json::json!({ "error": e.to_string() }),
                        is_error: Some(true),
                    },
                };

                let mut result_msg = UnifiedMessage::new(MessageRole::Agent, "");
                result_msg.content = vec![result_part];
                task.add_message(result_msg);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl UnifiedAgent for McpAgentAdapter {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        let mut task = UnifiedTask::new_with_uuid();
        task.add_message(message.clone());

        // Process the message (handle tool calls)
        self.process_message(&mut task, &message).await?;

        // Mark completed
        task.set_status(TaskStatus::Completed);

        // Store the task
        let task_id = task.id.clone();
        self.tasks.write().await.insert(task_id, task.clone());

        Ok(task)
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;

        task.add_message(message.clone());
        self.process_message(task, &message).await?;

        Ok(task.clone())
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        // MCP doesn't natively support streaming, so we simulate it
        let task = self.send_message(message).await?;
        let task_id = task.id.clone();

        let stream = async_stream::stream! {
            // Emit status working
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Working,
                message: None,
            });

            // Emit messages
            for msg in &task.messages {
                yield Ok(StreamEvent::MessageAdded {
                    task_id: task_id.clone(),
                    message: msg.clone(),
                });
            }

            // Emit artifacts
            for artifact in &task.artifacts {
                yield Ok(StreamEvent::ArtifactAdded {
                    task_id: task_id.clone(),
                    artifact: artifact.clone(),
                });
            }

            // Emit completed
            yield Ok(StreamEvent::StatusUpdate {
                task_id: task_id.clone(),
                status: TaskStatus::Completed,
                message: None,
            });
        };

        Ok(Box::pin(stream))
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        self.tasks
            .read()
            .await
            .get(task_id)
            .cloned()
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;

        task.set_status(TaskStatus::Cancelled);
        Ok(task.clone())
    }
}

#[async_trait]
impl ToolInvoker for McpAgentAdapter {
    async fn invoke_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> AgentResult<serde_json::Value> {
        let tool = self
            .bridge
            .find_tool(name)
            .ok_or_else(|| AgentError::CapabilityNotFound(name.to_string()))?;

        let input = serde_json::to_string(&arguments)?;
        let result = tool.call(input);

        match result {
            ExecutionResult::Success { output } => serde_json::from_str(&output)
                .or_else(|_| Ok(serde_json::json!({ "output": output }))),
            ExecutionResult::Failure { reason } => Err(AgentError::Internal(format!(
                "Tool execution failed: {:?}",
                reason
            ))),
        }
    }

    fn list_tools(&self) -> Vec<Capability> {
        self.bridge
            .tools()
            .iter()
            .map(|tool| Capability::new(tool.name(), tool.name()).with_tag("mcp"))
            .collect()
    }
}

/// Convert MCP tool info to unified capability.
pub fn mcp_tool_to_capability(tool: &dyn Tool) -> Capability {
    Capability::new(tool.name(), tool.name()).with_tag("mcp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_to_capability() {
        // Create a mock tool for testing
        struct MockTool;
        impl Tool for MockTool {
            fn name(&self) -> &str {
                "test_tool"
            }
            fn call(&self, _input: String) -> ExecutionResult {
                ExecutionResult::Success {
                    output: "success".to_string(),
                }
            }
        }

        let cap = mcp_tool_to_capability(&MockTool);
        assert_eq!(cap.id, "test_tool");
        assert!(cap.tags.contains(&"mcp".to_string()));
    }
}
