//! # HTTP Client Tools
//!
//! This module provides HTTP client tools for making REST API requests with
//! authentication support, error handling, and flexible configuration.

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

/// HTTP GET tool for retrieving resources
pub struct HttpGetTool {
    client: Client,
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
        "http_get"
    }

    fn call(&self, input: String) -> ExecutionResult {
        run_async(|| self.execute_async(input))
    }
}

impl HttpGetTool {
    async fn execute_async(&self, input: String) -> ExecutionResult {
        let config: HttpConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(_) => HttpConfig::new(input), // Fallback to simple URL
        };

        let mut request = self.client.get(&config.url);

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Set timeout
        if let Some(timeout) = config.timeout_secs {
            request = request.timeout(Duration::from_secs(timeout));
        }

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
                    Err(e) => {
                        ExecutionResult::failure(format!("Failed to read response body: {}", e))
                    }
                }
            }
            Err(e) => ExecutionResult::failure(format!("HTTP request failed: {}", e)),
        }
    }
}

/// HTTP POST tool for creating resources
pub struct HttpPostTool {
    client: Client,
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
        "http_post"
    }

    fn call(&self, input: String) -> ExecutionResult {
        run_async(|| self.execute_async(input))
    }
}

impl HttpPostTool {
    async fn execute_async(&self, input: String) -> ExecutionResult {
        let config: HttpConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(e) => return ExecutionResult::failure(format!("Invalid JSON config: {}", e)),
        };

        let mut request = self.client.post(&config.url);

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Add body
        if let Some(body) = &config.body {
            request = request.body(body.clone());
        }

        // Set timeout
        if let Some(timeout) = config.timeout_secs {
            request = request.timeout(Duration::from_secs(timeout));
        }

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
                    Err(e) => {
                        ExecutionResult::failure(format!("Failed to read response body: {}", e))
                    }
                }
            }
            Err(e) => ExecutionResult::failure(format!("HTTP request failed: {}", e)),
        }
    }
}

/// HTTP PUT tool for updating resources
pub struct HttpPutTool {
    client: Client,
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
        "http_put"
    }

    fn call(&self, input: String) -> ExecutionResult {
        run_async(|| self.execute_async(input))
    }
}

impl HttpPutTool {
    async fn execute_async(&self, input: String) -> ExecutionResult {
        let config: HttpConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(e) => return ExecutionResult::failure(format!("Invalid JSON config: {}", e)),
        };

        let mut request = self.client.put(&config.url);

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Add body
        if let Some(body) = &config.body {
            request = request.body(body.clone());
        }

        // Set timeout
        if let Some(timeout) = config.timeout_secs {
            request = request.timeout(Duration::from_secs(timeout));
        }

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
                    Err(e) => {
                        ExecutionResult::failure(format!("Failed to read response body: {}", e))
                    }
                }
            }
            Err(e) => ExecutionResult::failure(format!("HTTP request failed: {}", e)),
        }
    }
}

/// HTTP DELETE tool for removing resources
pub struct HttpDeleteTool {
    client: Client,
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
        "http_delete"
    }

    fn call(&self, input: String) -> ExecutionResult {
        run_async(|| self.execute_async(input))
    }
}

impl HttpDeleteTool {
    async fn execute_async(&self, input: String) -> ExecutionResult {
        let config: HttpConfig = match serde_json::from_str(&input) {
            Ok(config) => config,
            Err(_) => HttpConfig::new(input), // Fallback to simple URL
        };

        let mut request = self.client.delete(&config.url);

        // Add headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Set timeout
        if let Some(timeout) = config.timeout_secs {
            request = request.timeout(Duration::from_secs(timeout));
        }

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
                    Err(e) => {
                        ExecutionResult::failure(format!("Failed to read response body: {}", e))
                    }
                }
            }
            Err(e) => ExecutionResult::failure(format!("HTTP request failed: {}", e)),
        }
    }
}
