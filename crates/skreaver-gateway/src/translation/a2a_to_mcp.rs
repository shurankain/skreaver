//! A2A to MCP Translation
//!
//! This module translates A2A (Agent-to-Agent) messages to MCP format.

use serde_json::{Value, json};
use tracing::debug;

use crate::error::{GatewayError, GatewayResult};

/// Translator for A2A to MCP protocol conversion
#[derive(Debug, Default)]
pub struct A2aToMcpTranslator {
    /// Counter for generating JSON-RPC IDs
    next_id: std::sync::atomic::AtomicU64,
}

impl Clone for A2aToMcpTranslator {
    fn clone(&self) -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU64::new(
                self.next_id.load(std::sync::atomic::Ordering::SeqCst),
            ),
        }
    }
}

impl A2aToMcpTranslator {
    /// Create a new A2A to MCP translator
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the next JSON-RPC ID
    fn next_id(&self) -> u64 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Translate an A2A message to MCP format
    pub fn translate(&self, message: Value) -> GatewayResult<Value> {
        let obj = message
            .as_object()
            .ok_or_else(|| GatewayError::InvalidMessage("Expected JSON object".to_string()))?;

        // Detect A2A message type and translate accordingly

        // Streaming events (check first as they may have taskId)
        if let Some(event_type) = obj.get("type").and_then(|t| t.as_str()) {
            return self.translate_streaming_event(&message, event_type);
        }

        // Task with messages (full task object)
        if obj.contains_key("id") && obj.contains_key("status") && obj.contains_key("messages") {
            return self.translate_task(&message);
        }

        // SendMessageRequest
        if obj.contains_key("taskId") || obj.contains_key("task_id") {
            if obj.contains_key("message") {
                return self.translate_send_message_request(&message);
            }
            // GetTask or CancelTask request
            return self.translate_task_query(&message);
        }

        // AgentCard
        if (obj.contains_key("agentId") || obj.contains_key("agent_id"))
            && (obj.contains_key("skills") || obj.contains_key("capabilities"))
        {
            return self.translate_agent_card(&message);
        }

        // Single message (from A2A task message)
        if obj.contains_key("role") && obj.contains_key("parts") {
            return self.translate_single_message(&message);
        }

        Err(GatewayError::InvalidMessage(
            "Could not determine A2A message type".to_string(),
        ))
    }

    /// Translate a full A2A Task to MCP response(s)
    fn translate_task(&self, task: &Value) -> GatewayResult<Value> {
        let task_id = task["id"].as_str().unwrap_or_default();
        let status = task["status"].as_str().unwrap_or("working");

        debug!(task_id = %task_id, status = %status, "Translating A2A task to MCP");

        // Extract the original MCP ID if stored in metadata
        let mcp_id = task
            .get("metadata")
            .and_then(|m| m.get("mcp_id"))
            .cloned()
            .unwrap_or_else(|| json!(self.next_id()));

        match status {
            "completed" => {
                // Convert to MCP success response
                let content = self.convert_messages_to_content(task.get("messages"))?;

                // Also include artifacts if present
                let mut result_content = content;
                if let Some(artifacts) = task.get("artifacts").and_then(|a| a.as_array()) {
                    for artifact in artifacts {
                        let artifact_content = self.convert_artifact_to_content(artifact)?;
                        if let Some(arr) = result_content.as_array_mut()
                            && let Some(art_arr) = artifact_content.as_array()
                        {
                            arr.extend(art_arr.clone());
                        }
                    }
                }

                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": mcp_id,
                    "result": {
                        "content": result_content
                    }
                }))
            }

            "failed" => {
                // Convert to MCP error response
                let error_message = task
                    .get("messages")
                    .and_then(|m| m.as_array())
                    .and_then(|arr| arr.last())
                    .and_then(|msg| msg.get("parts"))
                    .and_then(|parts| parts.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|part| part.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("Task failed");

                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": mcp_id,
                    "error": {
                        "code": -32603,
                        "message": error_message,
                        "data": {
                            "task_id": task_id,
                            "status": status
                        }
                    }
                }))
            }

            "cancelled" => Ok(json!({
                "jsonrpc": "2.0",
                "id": mcp_id,
                "error": {
                    "code": -32000,
                    "message": "Task was cancelled",
                    "data": {
                        "task_id": task_id
                    }
                }
            })),

            "working" | "input-required" => {
                // Task is still in progress - return as MCP task info
                let poll_interval = task
                    .get("metadata")
                    .and_then(|m| m.get("poll_interval"))
                    .and_then(|p| p.as_u64())
                    .unwrap_or(5000);

                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": mcp_id,
                    "result": {
                        "task": {
                            "taskId": task_id,
                            "status": if status == "input-required" { "inputRequired" } else { "working" },
                            "pollInterval": poll_interval
                        }
                    }
                }))
            }

            _ => Err(GatewayError::TranslationError(format!(
                "Unknown task status: {}",
                status
            ))),
        }
    }

    /// Translate A2A SendMessageRequest to MCP tool call
    fn translate_send_message_request(&self, request: &Value) -> GatewayResult<Value> {
        let task_id = request
            .get("taskId")
            .or_else(|| request.get("task_id"))
            .and_then(|t| t.as_str())
            .unwrap_or_default();

        let message = &request["message"];
        let parts = message.get("parts").and_then(|p| p.as_array());

        debug!(task_id = %task_id, "Translating A2A SendMessageRequest to MCP");

        // Extract tool call info from metadata if available
        let metadata = request.get("metadata");
        let tool_name = metadata
            .and_then(|m| m.get("tool_name"))
            .and_then(|t| t.as_str());
        let arguments = metadata.and_then(|m| m.get("arguments")).cloned();

        if let (Some(name), Some(args)) = (tool_name, arguments) {
            // This was originally a tool call, reconstruct it
            Ok(json!({
                "jsonrpc": "2.0",
                "id": self.next_id(),
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": args
                }
            }))
        } else {
            // Generic message - convert to sampling request
            let content = self.convert_parts_to_content(parts)?;

            Ok(json!({
                "jsonrpc": "2.0",
                "id": self.next_id(),
                "method": "sampling/createMessage",
                "params": {
                    "messages": [
                        {
                            "role": "user",
                            "content": content
                        }
                    ],
                    "metadata": {
                        "a2a_task_id": task_id
                    }
                }
            }))
        }
    }

    /// Translate A2A task query (get/cancel) to MCP
    fn translate_task_query(&self, request: &Value) -> GatewayResult<Value> {
        let task_id = request
            .get("taskId")
            .or_else(|| request.get("task_id"))
            .and_then(|t| t.as_str())
            .unwrap_or_default();

        // Check if this is a cancel request
        let is_cancel = request
            .get("action")
            .and_then(|a| a.as_str())
            .map(|a| a == "cancel")
            .unwrap_or(false);

        if is_cancel {
            Ok(json!({
                "jsonrpc": "2.0",
                "id": self.next_id(),
                "method": "tasks/cancel",
                "params": {
                    "taskId": task_id
                }
            }))
        } else {
            Ok(json!({
                "jsonrpc": "2.0",
                "id": self.next_id(),
                "method": "tasks/get",
                "params": {
                    "taskId": task_id
                }
            }))
        }
    }

    /// Translate A2A AgentCard to MCP tools/list response
    fn translate_agent_card(&self, card: &Value) -> GatewayResult<Value> {
        let skills = card.get("skills").and_then(|s| s.as_array());

        debug!("Translating A2A AgentCard to MCP tools/list response");

        let tools: Vec<Value> = skills
            .map(|arr| {
                arr.iter()
                    .map(|skill| {
                        let name = skill["id"].as_str().unwrap_or_default();
                        let description = skill
                            .get("description")
                            .and_then(|d| d.as_str())
                            .or_else(|| skill.get("name").and_then(|n| n.as_str()))
                            .unwrap_or_default();

                        json!({
                            "name": name,
                            "description": description,
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "message": {
                                        "type": "string",
                                        "description": "Message to send to the agent"
                                    }
                                },
                                "required": ["message"]
                            }
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "result": {
                "tools": tools
            }
        }))
    }

    /// Translate A2A streaming event to MCP notification
    fn translate_streaming_event(&self, event: &Value, event_type: &str) -> GatewayResult<Value> {
        debug!(event_type = %event_type, "Translating A2A streaming event to MCP");

        match event_type {
            "taskProgress" => {
                let progress = event["progress"].as_f64().unwrap_or(0.0);
                let task_id = event
                    .get("taskId")
                    .or_else(|| event.get("task_id"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_default();

                Ok(json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/progress",
                    "params": {
                        "progressToken": task_id,
                        "progress": progress,
                        "total": 100.0,
                        "message": event.get("statusMessage")
                    }
                }))
            }

            "taskStatusUpdate" => {
                let status = event["status"].as_str().unwrap_or("working");
                let task_id = event
                    .get("taskId")
                    .or_else(|| event.get("task_id"))
                    .and_then(|t| t.as_str())
                    .unwrap_or_default();

                if status == "cancelled" {
                    Ok(json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/cancelled",
                        "params": {
                            "taskId": task_id,
                            "reason": "Task was cancelled"
                        }
                    }))
                } else {
                    Ok(json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/message",
                        "params": {
                            "message": format!("Task {} status: {}", task_id, status)
                        }
                    }))
                }
            }

            "taskMessage" => {
                let message_text = event
                    .get("message")
                    .and_then(|m| m.get("parts"))
                    .and_then(|p| p.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|part| part.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("");

                Ok(json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/message",
                    "params": {
                        "message": message_text
                    }
                }))
            }

            "taskArtifact" => {
                let artifact_id = event
                    .get("artifactId")
                    .or_else(|| event.get("artifact_id"))
                    .and_then(|a| a.as_str())
                    .unwrap_or_default();

                Ok(json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {
                        "uri": format!("artifact://{}", artifact_id)
                    }
                }))
            }

            _ => Ok(json!({
                "jsonrpc": "2.0",
                "method": "notifications/message",
                "params": {
                    "message": format!("A2A event: {}", event_type),
                    "data": event
                }
            })),
        }
    }

    /// Translate a single A2A message to MCP content
    fn translate_single_message(&self, message: &Value) -> GatewayResult<Value> {
        let role = message["role"].as_str().unwrap_or("user");
        let parts = message.get("parts").and_then(|p| p.as_array());
        let content = self.convert_parts_to_content(parts)?;

        let mcp_role = match role {
            "agent" => "assistant",
            _ => "user",
        };

        Ok(json!({
            "role": mcp_role,
            "content": content
        }))
    }

    /// Convert A2A parts to MCP content array
    fn convert_parts_to_content(&self, parts: Option<&Vec<Value>>) -> GatewayResult<Value> {
        let parts = match parts {
            Some(p) => p,
            None => return Ok(json!([])),
        };

        let content: Vec<Value> = parts
            .iter()
            .map(|part| {
                let part_type = part["type"].as_str().unwrap_or("text");
                match part_type {
                    "text" => json!({
                        "type": "text",
                        "text": part.get("text").unwrap_or(&json!(""))
                    }),
                    "file" => json!({
                        "type": "image",
                        "data": part.get("url").unwrap_or(&json!("")),
                        "mimeType": part.get("mimeType").unwrap_or(&json!("application/octet-stream"))
                    }),
                    "data" => json!({
                        "type": "resource",
                        "resource": {
                            "uri": "data://embedded",
                            "mimeType": part.get("mimeType").unwrap_or(&json!("application/json")),
                            "text": serde_json::to_string(part.get("data").unwrap_or(&json!(null))).unwrap_or_default()
                        }
                    }),
                    _ => json!({
                        "type": "text",
                        "text": serde_json::to_string(part).unwrap_or_default()
                    }),
                }
            })
            .collect();

        Ok(json!(content))
    }

    /// Convert A2A messages array to MCP content
    fn convert_messages_to_content(&self, messages: Option<&Value>) -> GatewayResult<Value> {
        let messages = match messages.and_then(|m| m.as_array()) {
            Some(m) => m,
            None => return Ok(json!([])),
        };

        let mut all_content = Vec::new();

        for msg in messages {
            if let Some(parts) = msg.get("parts").and_then(|p| p.as_array()) {
                let content = self.convert_parts_to_content(Some(parts))?;
                if let Some(arr) = content.as_array() {
                    all_content.extend(arr.clone());
                }
            }
        }

        Ok(json!(all_content))
    }

    /// Convert A2A artifact to MCP content
    fn convert_artifact_to_content(&self, artifact: &Value) -> GatewayResult<Value> {
        let parts = artifact.get("parts").and_then(|p| p.as_array());
        self.convert_parts_to_content(parts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_completed_task() {
        let translator = A2aToMcpTranslator::new();

        let a2a_task = json!({
            "id": "task-123",
            "status": "completed",
            "messages": [
                {
                    "role": "agent",
                    "parts": [
                        {"type": "text", "text": "The result is 42"}
                    ]
                }
            ]
        });

        let result = translator.translate(a2a_task).unwrap();

        assert_eq!(result["jsonrpc"], "2.0");
        assert!(result.get("result").is_some());
        assert_eq!(result["result"]["content"][0]["text"], "The result is 42");
    }

    #[test]
    fn test_translate_failed_task() {
        let translator = A2aToMcpTranslator::new();

        let a2a_task = json!({
            "id": "task-456",
            "status": "failed",
            "messages": [
                {
                    "role": "agent",
                    "parts": [
                        {"type": "text", "text": "Something went wrong"}
                    ]
                }
            ]
        });

        let result = translator.translate(a2a_task).unwrap();

        assert_eq!(result["jsonrpc"], "2.0");
        assert!(result.get("error").is_some());
        assert!(
            result["error"]["message"]
                .as_str()
                .unwrap()
                .contains("Something went wrong")
        );
    }

    #[test]
    fn test_translate_working_task() {
        let translator = A2aToMcpTranslator::new();

        let a2a_task = json!({
            "id": "task-789",
            "status": "working",
            "messages": []
        });

        let result = translator.translate(a2a_task).unwrap();

        assert_eq!(result["jsonrpc"], "2.0");
        assert!(result.get("result").is_some());
        assert_eq!(result["result"]["task"]["status"], "working");
    }

    #[test]
    fn test_translate_send_message_request() {
        let translator = A2aToMcpTranslator::new();

        let a2a_request = json!({
            "taskId": "task-123",
            "message": {
                "role": "user",
                "parts": [
                    {"type": "text", "text": "Please help me"}
                ]
            }
        });

        let result = translator.translate(a2a_request).unwrap();

        assert_eq!(result["jsonrpc"], "2.0");
        assert_eq!(result["method"], "sampling/createMessage");
        assert!(
            result["params"]["messages"][0]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Please help me")
        );
    }

    #[test]
    fn test_translate_agent_card() {
        let translator = A2aToMcpTranslator::new();

        let a2a_card = json!({
            "agentId": "agent-1",
            "name": "Test Agent",
            "skills": [
                {
                    "id": "summarize",
                    "name": "Summarize",
                    "description": "Summarizes text"
                },
                {
                    "id": "translate",
                    "name": "Translate",
                    "description": "Translates text"
                }
            ]
        });

        let result = translator.translate(a2a_card).unwrap();

        assert_eq!(result["jsonrpc"], "2.0");
        let tools = result["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "summarize");
        assert_eq!(tools[1]["name"], "translate");
    }

    #[test]
    fn test_translate_progress_event() {
        let translator = A2aToMcpTranslator::new();

        let a2a_event = json!({
            "type": "taskProgress",
            "taskId": "task-123",
            "progress": 50.0,
            "statusMessage": "Halfway done"
        });

        let result = translator.translate(a2a_event).unwrap();

        assert_eq!(result["jsonrpc"], "2.0");
        assert_eq!(result["method"], "notifications/progress");
        assert_eq!(result["params"]["progressToken"], "task-123");
        assert_eq!(result["params"]["progress"], 50.0);
    }

    #[test]
    fn test_translate_cancel_request() {
        let translator = A2aToMcpTranslator::new();

        let a2a_request = json!({
            "taskId": "task-123",
            "action": "cancel"
        });

        let result = translator.translate(a2a_request).unwrap();

        assert_eq!(result["method"], "tasks/cancel");
        assert_eq!(result["params"]["taskId"], "task-123");
    }
}
