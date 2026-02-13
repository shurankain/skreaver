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
