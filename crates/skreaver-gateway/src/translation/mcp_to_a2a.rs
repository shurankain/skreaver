//! MCP to A2A Translation
//!
//! This module translates MCP (Model Context Protocol) messages to A2A format.

use serde_json::{Value, json};
use tracing::debug;

use crate::error::{GatewayError, GatewayResult};

/// Translator for MCP to A2A protocol conversion
#[derive(Debug, Clone, Default)]
pub struct McpToA2aTranslator {
    /// Default context ID for translated tasks
    default_context_id: Option<String>,
}

impl McpToA2aTranslator {
    /// Create a new MCP to A2A translator
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default context ID for translated tasks
    pub fn with_context_id(mut self, context_id: impl Into<String>) -> Self {
        self.default_context_id = Some(context_id.into());
        self
    }

    /// Translate an MCP message to A2A format
    pub fn translate(&self, message: Value) -> GatewayResult<Value> {
        let obj = message
            .as_object()
            .ok_or_else(|| GatewayError::InvalidMessage("Expected JSON object".to_string()))?;

        // Check if this is a JSON-RPC message
        if obj.get("jsonrpc") == Some(&json!("2.0")) {
            // Determine message type
            if obj.contains_key("method") {
                if obj.contains_key("id") {
                    // Request
                    self.translate_request(&message)
                } else {
                    // Notification
                    self.translate_notification(&message)
                }
            } else if obj.contains_key("result") || obj.contains_key("error") {
                // Response
                self.translate_response(&message)
            } else {
                Err(GatewayError::InvalidMessage(
                    "Unknown JSON-RPC message type".to_string(),
                ))
            }
        } else {
            Err(GatewayError::InvalidMessage(
                "Not a valid MCP message (missing jsonrpc field)".to_string(),
            ))
        }
    }

    /// Translate an MCP request to A2A SendMessageRequest
    fn translate_request(&self, message: &Value) -> GatewayResult<Value> {
        let method = message["method"].as_str().unwrap_or_default();
        let params = &message["params"];
        let id = &message["id"];

        debug!(method = %method, "Translating MCP request to A2A");

        match method {
            // Tool call becomes A2A task with message
            "tools/call" => {
                let tool_name = params["name"].as_str().unwrap_or("unknown");
                let arguments = &params["arguments"];

                // Create A2A SendMessageRequest
                let task_id = format!("mcp-{}", id);
                let text_content = format!(
                    "Tool call: {} with arguments: {}",
                    tool_name,
                    serde_json::to_string(arguments).unwrap_or_default()
                );

                let mut a2a_request = json!({
                    "taskId": task_id,
                    "message": {
                        "role": "user",
                        "parts": [
                            {
                                "type": "text",
                                "text": text_content
                            }
                        ]
                    },
                    "metadata": {
                        "mcp_method": method,
                        "mcp_id": id,
                        "tool_name": tool_name,
                        "arguments": arguments
                    }
                });

                // Add context ID if configured
                if let Some(ref ctx) = self.default_context_id {
                    a2a_request["contextId"] = json!(ctx);
                }

                Ok(a2a_request)
            }

            // List tools becomes agent card request (no body needed for GET)
            "tools/list" => Ok(json!({
                "type": "agentCardRequest",
                "metadata": {
                    "mcp_method": method,
                    "mcp_id": id
                }
            })),

            // Sampling request becomes A2A task creation
            "sampling/createMessage" => {
                let messages = params.get("messages").cloned().unwrap_or(json!([]));
                let task_id = format!("mcp-sample-{}", id);

                // Convert MCP messages to A2A format
                let a2a_messages = self.convert_mcp_messages_to_a2a(&messages)?;

                let mut task = json!({
                    "id": task_id,
                    "status": "working",
                    "messages": a2a_messages,
                    "metadata": {
                        "mcp_method": method,
                        "mcp_id": id,
                        "sampling_params": params
                    }
                });

                if let Some(ref ctx) = self.default_context_id {
                    task["contextId"] = json!(ctx);
                }

                Ok(task)
            }

            // Generic request handling
            _ => {
                let task_id = format!("mcp-{}-{}", method.replace('/', "-"), id);

                let mut request = json!({
                    "taskId": task_id,
                    "message": {
                        "role": "user",
                        "parts": [
                            {
                                "type": "text",
                                "text": format!("MCP request: {}", method)
                            },
                            {
                                "type": "data",
                                "mimeType": "application/json",
                                "data": params
                            }
                        ]
                    },
                    "metadata": {
                        "mcp_method": method,
                        "mcp_id": id
                    }
                });

                if let Some(ref ctx) = self.default_context_id {
                    request["contextId"] = json!(ctx);
                }

                Ok(request)
            }
        }
    }

    /// Translate an MCP notification to A2A event
    fn translate_notification(&self, message: &Value) -> GatewayResult<Value> {
        let method = message["method"].as_str().unwrap_or_default();
        let params = &message["params"];

        debug!(method = %method, "Translating MCP notification to A2A");

        match method {
            "notifications/progress" => {
                // Progress notification becomes A2A TaskProgress event
                let progress = params["progress"].as_f64().unwrap_or(0.0);
                let total = params["total"].as_f64().unwrap_or(100.0);
                let progress_pct = if total > 0.0 {
                    (progress / total * 100.0) as f32
                } else {
                    0.0
                };

                Ok(json!({
                    "type": "taskProgress",
                    "taskId": params.get("progressToken").unwrap_or(&json!("unknown")),
                    "progress": progress_pct,
                    "statusMessage": params.get("message"),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }

            "notifications/message" => {
                // Generic message notification
                Ok(json!({
                    "type": "taskMessage",
                    "message": {
                        "role": "agent",
                        "parts": [
                            {
                                "type": "text",
                                "text": params.get("message").and_then(|m| m.as_str()).unwrap_or("")
                            }
                        ]
                    },
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            }

            "notifications/cancelled" => Ok(json!({
                "type": "taskStatusUpdate",
                "status": "cancelled",
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),

            // Generic notification
            _ => Ok(json!({
                "type": "event",
                "eventType": method,
                "data": params,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
        }
    }

    /// Translate an MCP response to A2A task update
    fn translate_response(&self, message: &Value) -> GatewayResult<Value> {
        let id = &message["id"];

        if let Some(error) = message.get("error") {
            // Error response
            debug!(id = ?id, "Translating MCP error response to A2A");

            Ok(json!({
                "id": format!("mcp-{}", id),
                "status": "failed",
                "messages": [
                    {
                        "role": "agent",
                        "parts": [
                            {
                                "type": "text",
                                "text": error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error")
                            }
                        ]
                    }
                ],
                "metadata": {
                    "mcp_id": id,
                    "error_code": error.get("code"),
                    "error_data": error.get("data")
                },
                "updatedAt": chrono::Utc::now().to_rfc3339()
            }))
        } else {
            // Success response
            let result = &message["result"];

            debug!(id = ?id, "Translating MCP success response to A2A");

            // Convert MCP result to A2A message parts
            let parts = self.convert_mcp_result_to_parts(result)?;

            Ok(json!({
                "id": format!("mcp-{}", id),
                "status": "completed",
                "messages": [
                    {
                        "role": "agent",
                        "parts": parts
                    }
                ],
                "metadata": {
                    "mcp_id": id
                },
                "updatedAt": chrono::Utc::now().to_rfc3339()
            }))
        }
    }

    /// Convert MCP result content to A2A parts
    fn convert_mcp_result_to_parts(&self, result: &Value) -> GatewayResult<Value> {
        // MCP tool results have a "content" array
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            let parts: Vec<Value> = content
                .iter()
                .map(|item| {
                    let item_type = item["type"].as_str().unwrap_or("text");
                    match item_type {
                        "text" => json!({
                            "type": "text",
                            "text": item.get("text").unwrap_or(&json!(""))
                        }),
                        "image" => json!({
                            "type": "file",
                            "url": item.get("data").unwrap_or(&json!("")),
                            "mimeType": item.get("mimeType").unwrap_or(&json!("image/png"))
                        }),
                        "resource" => json!({
                            "type": "data",
                            "mimeType": "application/json",
                            "data": item
                        }),
                        _ => json!({
                            "type": "data",
                            "mimeType": "application/json",
                            "data": item
                        }),
                    }
                })
                .collect();

            Ok(json!(parts))
        } else {
            // No content array, wrap the entire result as data
            Ok(json!([
                {
                    "type": "data",
                    "mimeType": "application/json",
                    "data": result
                }
            ]))
        }
    }

    /// Convert MCP sampling messages to A2A message format
    fn convert_mcp_messages_to_a2a(&self, messages: &Value) -> GatewayResult<Value> {
        let messages_arr = messages
            .as_array()
            .ok_or_else(|| GatewayError::InvalidMessage("Messages must be an array".to_string()))?;

        let a2a_messages: Vec<Value> = messages_arr
            .iter()
            .map(|msg| {
                let role = msg["role"].as_str().unwrap_or("user");
                let a2a_role = match role {
                    "assistant" => "agent",
                    _ => "user",
                };

                // Convert content to parts
                let content = &msg["content"];
                let parts = if let Some(text) = content.as_str() {
                    vec![json!({"type": "text", "text": text})]
                } else if let Some(arr) = content.as_array() {
                    arr.iter()
                        .map(|c| {
                            let c_type = c["type"].as_str().unwrap_or("text");
                            match c_type {
                                "text" => json!({
                                    "type": "text",
                                    "text": c.get("text").unwrap_or(&json!(""))
                                }),
                                "image" => json!({
                                    "type": "file",
                                    "url": c.get("source").and_then(|s| s.get("url")).unwrap_or(&json!("")),
                                    "mimeType": c.get("source").and_then(|s| s.get("media_type")).unwrap_or(&json!("image/png"))
                                }),
                                _ => json!({
                                    "type": "data",
                                    "mimeType": "application/json",
                                    "data": c
                                }),
                            }
                        })
                        .collect()
                } else {
                    vec![json!({"type": "data", "mimeType": "application/json", "data": content})]
                };

                json!({
                    "role": a2a_role,
                    "parts": parts
                })
            })
            .collect();

        Ok(json!(a2a_messages))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_tool_call() {
        let translator = McpToA2aTranslator::new();

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "calculator",
                "arguments": {"a": 5, "b": 3}
            }
        });

        let result = translator.translate(mcp_request).unwrap();

        assert_eq!(result["taskId"], "mcp-1");
        assert_eq!(result["message"]["role"], "user");
        assert!(
            result["message"]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .contains("calculator")
        );
        assert_eq!(result["metadata"]["tool_name"], "calculator");
    }

    #[test]
    fn test_translate_tool_list() {
        let translator = McpToA2aTranslator::new();

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });

        let result = translator.translate(mcp_request).unwrap();
        assert_eq!(result["type"], "agentCardRequest");
    }

    #[test]
    fn test_translate_success_response() {
        let translator = McpToA2aTranslator::new();

        let mcp_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [
                    {"type": "text", "text": "The answer is 8"}
                ]
            }
        });

        let result = translator.translate(mcp_response).unwrap();

        assert_eq!(result["id"], "mcp-1");
        assert_eq!(result["status"], "completed");
        assert_eq!(result["messages"][0]["role"], "agent");
        assert_eq!(result["messages"][0]["parts"][0]["text"], "The answer is 8");
    }

    #[test]
    fn test_translate_error_response() {
        let translator = McpToA2aTranslator::new();

        let mcp_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid Request"
            }
        });

        let result = translator.translate(mcp_response).unwrap();

        assert_eq!(result["id"], "mcp-1");
        assert_eq!(result["status"], "failed");
        assert!(
            result["messages"][0]["parts"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Invalid Request")
        );
    }

    #[test]
    fn test_translate_progress_notification() {
        let translator = McpToA2aTranslator::new();

        let mcp_notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/progress",
            "params": {
                "progressToken": "task-123",
                "progress": 50,
                "total": 100,
                "message": "Processing..."
            }
        });

        let result = translator.translate(mcp_notification).unwrap();

        assert_eq!(result["type"], "taskProgress");
        assert_eq!(result["progress"], 50.0);
        assert_eq!(result["statusMessage"], "Processing...");
    }

    #[test]
    fn test_translate_with_context_id() {
        let translator = McpToA2aTranslator::new().with_context_id("ctx-456");

        let mcp_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": "test",
                "arguments": {}
            }
        });

        let result = translator.translate(mcp_request).unwrap();
        assert_eq!(result["contextId"], "ctx-456");
    }

    #[test]
    fn test_invalid_message() {
        let translator = McpToA2aTranslator::new();

        let invalid = json!({
            "not": "valid"
        });

        assert!(translator.translate(invalid).is_err());
    }
}
