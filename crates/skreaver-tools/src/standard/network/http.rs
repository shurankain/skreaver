//! # HTTP Client Tools
//!
//! This module provides HTTP client tools for making REST API requests with
//! authentication support, error handling, and flexible configuration.

use crate::core::ToolConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use skreaver_core::{ExecutionResult, Tool};
use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

/// Execute an async operation using the current runtime or creating a new one.
///
/// This helper safely handles runtime creation, returning an error result
/// instead of panicking if runtime creation fails.
fn run_async<F, Fut>(f: F) -> ExecutionResult
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ExecutionResult>,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(f()))
    } else {
        match tokio::runtime::Runtime::new() {
            Ok(rt) => rt.block_on(f()),
            Err(e) => ExecutionResult::failure(format!("Failed to create async runtime: {}", e)),
        }
    }
}

/// HTTP method for requests
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    /// Returns the tool name for this HTTP method
    fn tool_name(self) -> &'static str {
        match self {
            HttpMethod::Get => "http_get",
            HttpMethod::Post => "http_post",
            HttpMethod::Put => "http_put",
            HttpMethod::Delete => "http_delete",
        }
    }

    /// Whether this method supports simple URL fallback (GET/DELETE)
    fn supports_simple_url(self) -> bool {
        matches!(self, HttpMethod::Get | HttpMethod::Delete)
    }

    /// Whether this method supports a request body
    fn supports_body(self) -> bool {
        matches!(self, HttpMethod::Post | HttpMethod::Put)
    }
}

/// Configuration for HTTP requests
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpConfig {
    pub url: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default)]
    pub body: Option<String>,
}

impl HttpConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            headers: HashMap::new(),
            timeout_secs: Some(30),
            body: None,
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_secs = Some(seconds);
        self
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }
}

impl ToolConfig for HttpConfig {
    fn from_simple(input: String) -> Self {
        Self::new(input)
    }
}

/// Core HTTP execution logic shared by all HTTP tools
async fn execute_http_request(
    client: &Client,
    method: HttpMethod,
    input: String,
) -> ExecutionResult {
    // Parse config - methods that support simple URL use fallback parsing
    let config = if method.supports_simple_url() {
        HttpConfig::parse(input)
    } else {
        match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(e) => return ExecutionResult::failure(format!("Invalid JSON config: {}", e)),
        }
    };

    // Build request based on method
    let mut request = match method {
        HttpMethod::Get => client.get(&config.url),
        HttpMethod::Post => client.post(&config.url),
        HttpMethod::Put => client.put(&config.url),
        HttpMethod::Delete => client.delete(&config.url),
    };

    // Add headers
    for (key, value) in &config.headers {
        request = request.header(key, value);
    }

    // Add body for methods that support it
    if method.supports_body() && config.body.is_some() {
        request = request.body(config.body.clone().unwrap());
    }

    // Set timeout
    if let Some(timeout) = config.timeout_secs {
        request = request.timeout(Duration::from_secs(timeout));
    }

    // Execute request
    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            match response.text().await {
                Ok(body) => {
                    let result = serde_json::json!({
                        "status": status,
                        "body": body,
                        "success": (200..300).contains(&status)
                    });
                    ExecutionResult::success(result.to_string())
                }
                Err(e) => ExecutionResult::failure(format!("Failed to read response body: {}", e)),
            }
        }
        Err(e) => ExecutionResult::failure(format!("HTTP request failed: {}", e)),
    }
}

/// HTTP GET tool for retrieving resources
pub struct HttpGetTool {
    client: Client,
}

impl std::fmt::Debug for HttpGetTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpGetTool").finish_non_exhaustive()
    }
}

impl HttpGetTool {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for HttpGetTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for HttpGetTool {
    fn name(&self) -> &str {
        HttpMethod::Get.tool_name()
    }

    fn call(&self, input: String) -> ExecutionResult {
        let client = self.client.clone();
        run_async(|| execute_http_request(&client, HttpMethod::Get, input))
    }
}

/// HTTP POST tool for creating resources
pub struct HttpPostTool {
    client: Client,
}

impl std::fmt::Debug for HttpPostTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpPostTool").finish_non_exhaustive()
    }
}

impl HttpPostTool {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for HttpPostTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for HttpPostTool {
    fn name(&self) -> &str {
        HttpMethod::Post.tool_name()
    }

    fn call(&self, input: String) -> ExecutionResult {
        let client = self.client.clone();
        run_async(|| execute_http_request(&client, HttpMethod::Post, input))
    }
}

/// HTTP PUT tool for updating resources
pub struct HttpPutTool {
    client: Client,
}

impl std::fmt::Debug for HttpPutTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpPutTool").finish_non_exhaustive()
    }
}

impl HttpPutTool {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for HttpPutTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for HttpPutTool {
    fn name(&self) -> &str {
        HttpMethod::Put.tool_name()
    }

    fn call(&self, input: String) -> ExecutionResult {
        let client = self.client.clone();
        run_async(|| execute_http_request(&client, HttpMethod::Put, input))
    }
}

/// HTTP DELETE tool for removing resources
pub struct HttpDeleteTool {
    client: Client,
}

impl std::fmt::Debug for HttpDeleteTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpDeleteTool").finish_non_exhaustive()
    }
}

impl HttpDeleteTool {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl Default for HttpDeleteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for HttpDeleteTool {
    fn name(&self) -> &str {
        HttpMethod::Delete.tool_name()
    }

    fn call(&self, input: String) -> ExecutionResult {
        let client = self.client.clone();
        run_async(|| execute_http_request(&client, HttpMethod::Delete, input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::Tool;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // ==================== HttpConfig Tests ====================

    #[test]
    fn test_http_config_new() {
        let config = HttpConfig::new("https://example.com/api");
        assert_eq!(config.url, "https://example.com/api");
        assert!(config.headers.is_empty());
        assert_eq!(config.timeout_secs, Some(30));
        assert!(config.body.is_none());
    }

    #[test]
    fn test_http_config_builder() {
        let config = HttpConfig::new("https://api.example.com")
            .with_header("Authorization", "Bearer token123")
            .with_header("Content-Type", "application/json")
            .with_timeout(60)
            .with_body(r#"{"key": "value"}"#);

        assert_eq!(config.url, "https://api.example.com");
        assert_eq!(config.headers.get("Authorization"), Some(&"Bearer token123".to_string()));
        assert_eq!(config.headers.get("Content-Type"), Some(&"application/json".to_string()));
        assert_eq!(config.timeout_secs, Some(60));
        assert_eq!(config.body, Some(r#"{"key": "value"}"#.to_string()));
    }

    #[test]
    fn test_http_config_from_simple() {
        let config = HttpConfig::from_simple("https://example.com".to_string());
        assert_eq!(config.url, "https://example.com");
    }

    #[test]
    fn test_http_config_parse_json() {
        let json = r#"{"url": "https://api.example.com", "timeout_secs": 10}"#;
        let config: HttpConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.url, "https://api.example.com");
        assert_eq!(config.timeout_secs, Some(10));
    }

    // ==================== HttpMethod Tests ====================

    #[test]
    fn test_http_method_tool_names() {
        assert_eq!(HttpMethod::Get.tool_name(), "http_get");
        assert_eq!(HttpMethod::Post.tool_name(), "http_post");
        assert_eq!(HttpMethod::Put.tool_name(), "http_put");
        assert_eq!(HttpMethod::Delete.tool_name(), "http_delete");
    }

    #[test]
    fn test_http_method_supports_simple_url() {
        assert!(HttpMethod::Get.supports_simple_url());
        assert!(HttpMethod::Delete.supports_simple_url());
        assert!(!HttpMethod::Post.supports_simple_url());
        assert!(!HttpMethod::Put.supports_simple_url());
    }

    #[test]
    fn test_http_method_supports_body() {
        assert!(HttpMethod::Post.supports_body());
        assert!(HttpMethod::Put.supports_body());
        assert!(!HttpMethod::Get.supports_body());
        assert!(!HttpMethod::Delete.supports_body());
    }

    // ==================== Tool Naming Tests ====================

    #[test]
    fn test_http_get_tool_name() {
        let tool = HttpGetTool::new();
        assert_eq!(tool.name(), "http_get");
    }

    #[test]
    fn test_http_post_tool_name() {
        let tool = HttpPostTool::new();
        assert_eq!(tool.name(), "http_post");
    }

    #[test]
    fn test_http_put_tool_name() {
        let tool = HttpPutTool::new();
        assert_eq!(tool.name(), "http_put");
    }

    #[test]
    fn test_http_delete_tool_name() {
        let tool = HttpDeleteTool::new();
        assert_eq!(tool.name(), "http_delete");
    }

    // ==================== HTTP GET Tests with Mock Server ====================

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_get_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/data"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"result": "success"}"#))
            .mount(&mock_server)
            .await;

        let tool = HttpGetTool::new();
        let url = format!("{}/api/data", mock_server.uri());
        let result = tool.call(url);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 200);
        assert!(output["success"].as_bool().unwrap());
        assert!(output["body"].as_str().unwrap().contains("success"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_get_with_json_config() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/resource"))
            .and(header("X-Custom-Header", "custom-value"))
            .respond_with(ResponseTemplate::new(200).set_body_string("OK"))
            .mount(&mock_server)
            .await;

        let tool = HttpGetTool::new();
        let config = serde_json::json!({
            "url": format!("{}/api/resource", mock_server.uri()),
            "headers": {
                "X-Custom-Header": "custom-value"
            }
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 200);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_get_404_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/not-found"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&mock_server)
            .await;

        let tool = HttpGetTool::new();
        let url = format!("{}/not-found", mock_server.uri());
        let result = tool.call(url);

        assert!(result.is_success()); // HTTP call succeeded, even if response is 404
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 404);
        assert!(!output["success"].as_bool().unwrap());
    }

    // ==================== HTTP POST Tests with Mock Server ====================

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_post_with_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/create"))
            .and(body_string(r#"{"name": "test"}"#))
            .respond_with(ResponseTemplate::new(201).set_body_string(r#"{"id": 123}"#))
            .mount(&mock_server)
            .await;

        let tool = HttpPostTool::new();
        let config = serde_json::json!({
            "url": format!("{}/api/create", mock_server.uri()),
            "body": r#"{"name": "test"}"#
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 201);
        assert!(output["success"].as_bool().unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_post_with_headers_and_body() {
        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/submit"))
            .and(header("Content-Type", "application/json"))
            .and(header("Authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Created"))
            .mount(&mock_server)
            .await;

        let tool = HttpPostTool::new();
        let config = serde_json::json!({
            "url": format!("{}/api/submit", mock_server.uri()),
            "headers": {
                "Content-Type": "application/json",
                "Authorization": "Bearer test-token"
            },
            "body": r#"{"data": "test"}"#
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 200);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_post_invalid_json_config() {
        let tool = HttpPostTool::new();
        let result = tool.call("not valid json".to_string());

        assert!(result.is_failure());
        assert!(result.output().contains("Invalid JSON config"));
    }

    // ==================== HTTP PUT Tests with Mock Server ====================

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_put_update() {
        let mock_server = MockServer::start().await;

        Mock::given(method("PUT"))
            .and(path("/api/resource/1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"updated": true}"#))
            .mount(&mock_server)
            .await;

        let tool = HttpPutTool::new();
        let config = serde_json::json!({
            "url": format!("{}/api/resource/1", mock_server.uri()),
            "body": r#"{"name": "updated"}"#
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 200);
        assert!(output["success"].as_bool().unwrap());
    }

    // ==================== HTTP DELETE Tests with Mock Server ====================

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_delete_success() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/api/resource/42"))
            .respond_with(ResponseTemplate::new(204).set_body_string(""))
            .mount(&mock_server)
            .await;

        let tool = HttpDeleteTool::new();
        let url = format!("{}/api/resource/42", mock_server.uri());
        let result = tool.call(url);

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 204);
        assert!(output["success"].as_bool().unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_delete_with_json_config() {
        let mock_server = MockServer::start().await;

        Mock::given(method("DELETE"))
            .and(path("/api/item/99"))
            .and(header("X-Api-Key", "secret"))
            .respond_with(ResponseTemplate::new(200).set_body_string("Deleted"))
            .mount(&mock_server)
            .await;

        let tool = HttpDeleteTool::new();
        let config = serde_json::json!({
            "url": format!("{}/api/item/99", mock_server.uri()),
            "headers": {
                "X-Api-Key": "secret"
            }
        });
        let result = tool.call(config.to_string());

        assert!(result.is_success());
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 200);
    }

    // ==================== Error Handling Tests ====================

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_get_connection_refused() {
        let tool = HttpGetTool::new();
        // Use a port that's very unlikely to be listening
        let result = tool.call("http://127.0.0.1:59999/does-not-exist".to_string());

        assert!(result.is_failure());
        assert!(result.output().contains("HTTP request failed"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_http_get_server_error() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/error"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock_server)
            .await;

        let tool = HttpGetTool::new();
        let url = format!("{}/error", mock_server.uri());
        let result = tool.call(url);

        assert!(result.is_success()); // Call succeeded, status indicates error
        let output: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
        assert_eq!(output["status"], 500);
        assert!(!output["success"].as_bool().unwrap());
    }

    // ==================== Default Implementations ====================

    #[test]
    fn test_http_tools_default() {
        let get_tool = HttpGetTool::default();
        assert_eq!(get_tool.name(), "http_get");

        let post_tool = HttpPostTool::default();
        assert_eq!(post_tool.name(), "http_post");

        let put_tool = HttpPutTool::default();
        assert_eq!(put_tool.name(), "http_put");

        let delete_tool = HttpDeleteTool::default();
        assert_eq!(delete_tool.name(), "http_delete");
    }

    // ==================== Debug Implementations ====================

    #[test]
    fn test_http_tools_debug() {
        let get_tool = HttpGetTool::new();
        let debug_str = format!("{:?}", get_tool);
        assert!(debug_str.contains("HttpGetTool"));

        let post_tool = HttpPostTool::new();
        let debug_str = format!("{:?}", post_tool);
        assert!(debug_str.contains("HttpPostTool"));

        let put_tool = HttpPutTool::new();
        let debug_str = format!("{:?}", put_tool);
        assert!(debug_str.contains("HttpPutTool"));

        let delete_tool = HttpDeleteTool::new();
        let debug_str = format!("{:?}", delete_tool);
        assert!(debug_str.contains("HttpDeleteTool"));
    }
}
