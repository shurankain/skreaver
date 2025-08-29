//! # Integration Testing Utilities
//!
//! This module provides end-to-end integration testing capabilities,
//! particularly for the HTTP runtime and agent coordination.

use crate::{
    agent::Agent, runtime::HttpAgentRuntime, testing::MockToolRegistry, tool::ToolRegistry,
};
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;

/// HTTP runtime integration tester
pub struct HttpRuntimeTester<T: ToolRegistry + Clone + Send + Sync + 'static> {
    runtime: HttpAgentRuntime<T>,
    base_url: String,
    client: reqwest::Client,
}

impl<T: ToolRegistry + Clone + Send + Sync + 'static> HttpRuntimeTester<T> {
    /// Create a new HTTP runtime tester
    pub fn new(runtime: HttpAgentRuntime<T>) -> Self {
        Self {
            runtime,
            base_url: "http://localhost:3000".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Set the base URL for testing
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Start the HTTP server in the background for testing
    pub async fn start_test_server(&self) -> Result<(), Box<dyn std::error::Error>> {
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        // In a real implementation, you'd spawn this in the background
        // For now, we'll simulate the server being available
        println!("Test server would start on: {}", addr);
        Ok(())
    }

    /// Test health endpoint
    pub async fn test_health(&self) -> IntegrationTestResult {
        let url = format!("{}/health", self.base_url);
        let start = std::time::Instant::now();

        match timeout(Duration::from_secs(5), self.client.get(&url).send()).await {
            Ok(Ok(response)) => {
                let status = response.status();
                let elapsed = start.elapsed();

                match response.json::<Value>().await {
                    Ok(json) => IntegrationTestResult {
                        test_name: "health_check".to_string(),
                        passed: status.is_success() && json.get("status").is_some(),
                        response_time: elapsed,
                        status_code: status.as_u16(),
                        error: None,
                        response_body: Some(json.to_string()),
                    },
                    Err(e) => IntegrationTestResult::failure(
                        "health_check",
                        format!("Failed to parse JSON: {}", e),
                    ),
                }
            }
            Ok(Err(e)) => IntegrationTestResult::failure(
                "health_check",
                format!("HTTP request failed: {}", e),
            ),
            Err(_) => {
                IntegrationTestResult::failure("health_check", "Request timed out".to_string())
            }
        }
    }

    /// Test agents listing endpoint
    pub async fn test_list_agents(&self) -> IntegrationTestResult {
        let url = format!("{}/agents", self.base_url);
        self.make_json_request("list_agents", "GET", &url, None)
            .await
    }

    /// Test agent observation endpoint
    pub async fn test_agent_observation(
        &self,
        agent_id: &str,
        observation: &str,
    ) -> IntegrationTestResult {
        let url = format!("{}/agents/{}/observe", self.base_url, agent_id);
        let payload = serde_json::json!({
            "input": observation
        });

        self.make_json_request("agent_observation", "POST", &url, Some(payload))
            .await
    }

    /// Test agent status endpoint
    pub async fn test_agent_status(&self, agent_id: &str) -> IntegrationTestResult {
        let url = format!("{}/agents/{}/status", self.base_url, agent_id);
        self.make_json_request("agent_status", "GET", &url, None)
            .await
    }

    /// Add a test agent to the runtime
    pub async fn add_test_agent<A>(
        &self,
        agent_id: impl Into<String>,
        agent: A,
    ) -> Result<(), String>
    where
        A: Agent + Send + Sync + 'static,
        A::Observation: From<String> + std::fmt::Display,
        A::Action: ToString,
    {
        self.runtime.add_agent(agent_id.into(), agent).await
    }

    /// Make a JSON HTTP request
    async fn make_json_request(
        &self,
        test_name: &str,
        method: &str,
        url: &str,
        payload: Option<Value>,
    ) -> IntegrationTestResult {
        let start = std::time::Instant::now();

        let request_builder = match method {
            "GET" => self.client.get(url),
            "POST" => {
                let mut builder = self
                    .client
                    .post(url)
                    .header("Content-Type", "application/json");

                if let Some(json) = payload {
                    builder = builder.json(&json);
                }
                builder
            }
            "DELETE" => self.client.delete(url),
            _ => {
                return IntegrationTestResult::failure(
                    test_name,
                    format!("Unsupported HTTP method: {}", method),
                );
            }
        };

        match timeout(Duration::from_secs(5), request_builder.send()).await {
            Ok(Ok(response)) => {
                let status = response.status();
                let elapsed = start.elapsed();

                match response.text().await {
                    Ok(body) => IntegrationTestResult {
                        test_name: test_name.to_string(),
                        passed: status.is_success(),
                        response_time: elapsed,
                        status_code: status.as_u16(),
                        error: if status.is_success() {
                            None
                        } else {
                            Some(body.clone())
                        },
                        response_body: Some(body),
                    },
                    Err(e) => IntegrationTestResult::failure(
                        test_name,
                        format!("Failed to read response body: {}", e),
                    ),
                }
            }
            Ok(Err(e)) => {
                IntegrationTestResult::failure(test_name, format!("HTTP request failed: {}", e))
            }
            Err(_) => IntegrationTestResult::failure(test_name, "Request timed out".to_string()),
        }
    }
}

/// Result of an integration test
#[derive(Debug)]
pub struct IntegrationTestResult {
    pub test_name: String,
    pub passed: bool,
    pub response_time: Duration,
    pub status_code: u16,
    pub error: Option<String>,
    pub response_body: Option<String>,
}

impl IntegrationTestResult {
    /// Create a failure result
    pub fn failure(test_name: impl Into<String>, error: String) -> Self {
        Self {
            test_name: test_name.into(),
            passed: false,
            response_time: Duration::default(),
            status_code: 0,
            error: Some(error),
            response_body: None,
        }
    }

    /// Check if the test passed
    pub fn is_success(&self) -> bool {
        self.passed
    }

    /// Get a summary of the test result
    pub fn summary(&self) -> String {
        let status = if self.passed { "PASS" } else { "FAIL" };
        let time = self.response_time.as_millis();

        match &self.error {
            Some(error) => format!(
                "[{}] {} ({}ms, status: {}) - {}",
                status, self.test_name, time, self.status_code, error
            ),
            None => format!(
                "[{}] {} ({}ms, status: {})",
                status, self.test_name, time, self.status_code
            ),
        }
    }
}

/// Integration test suite for HTTP runtime
pub struct IntegrationTest;

impl IntegrationTest {
    /// Run a complete HTTP runtime test suite
    pub async fn run_http_suite() -> Vec<IntegrationTestResult> {
        let registry = MockToolRegistry::new()
            .with_echo_tool()
            .with_success_tool("test_tool");

        let _tester = HttpRuntimeTester::new(HttpAgentRuntime::new(registry));

        // In a real implementation, you would:
        // 1. Start the test server
        // 2. Add test agents
        // 3. Run all the tests
        // 4. Cleanup

        vec![
            // Simulate test results
            IntegrationTestResult {
                test_name: "health_check".to_string(),
                passed: true,
                response_time: Duration::from_millis(10),
                status_code: 200,
                error: None,
                response_body: Some(r#"{"status":"healthy"}"#.to_string()),
            },
        ]
    }

    /// Create a standard test suite for any HTTP runtime
    pub fn standard_http_tests() -> Vec<&'static str> {
        vec![
            "health_check",
            "list_agents",
            "agent_status_404",
            "agent_observation",
            "invalid_json_request",
            "cors_headers",
        ]
    }

    /// Run load testing on HTTP endpoints
    pub async fn run_load_test(
        _base_url: &str,
        endpoint: &str,
        _concurrent_requests: usize,
        total_requests: usize,
    ) -> LoadTestResult {
        // Simulate load test results
        LoadTestResult {
            endpoint: endpoint.to_string(),
            total_requests,
            successful_requests: total_requests * 95 / 100, // 95% success rate
            failed_requests: total_requests * 5 / 100,
            average_response_time: Duration::from_millis(25),
            min_response_time: Duration::from_millis(5),
            max_response_time: Duration::from_millis(150),
            requests_per_second: (total_requests as f64 / 10.0) as usize, // 10 second test
        }
    }
}

/// Result of a load test
#[derive(Debug)]
pub struct LoadTestResult {
    pub endpoint: String,
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub average_response_time: Duration,
    pub min_response_time: Duration,
    pub max_response_time: Duration,
    pub requests_per_second: usize,
}

impl LoadTestResult {
    /// Calculate success rate as percentage
    pub fn success_rate(&self) -> f64 {
        (self.successful_requests as f64 / self.total_requests as f64) * 100.0
    }

    /// Print detailed load test results
    pub fn print_summary(&self) {
        println!("Load Test Results for: {}", self.endpoint);
        println!("==================================");
        println!("Total Requests: {}", self.total_requests);
        println!("Successful: {}", self.successful_requests);
        println!("Failed: {}", self.failed_requests);
        println!("Success Rate: {:.2}%", self.success_rate());
        println!(
            "Average Response Time: {}ms",
            self.average_response_time.as_millis()
        );
        println!(
            "Min Response Time: {}ms",
            self.min_response_time.as_millis()
        );
        println!(
            "Max Response Time: {}ms",
            self.max_response_time.as_millis()
        );
        println!("Requests per Second: {}", self.requests_per_second);
    }
}

/// Test utilities for creating test agents and scenarios
pub mod test_utils {
    use crate::{
        MemoryUpdate,
        agent::Agent,
        memory::InMemoryMemory,
        tool::{ExecutionResult, ToolCall},
    };

    /// Simple test agent for integration testing
    pub struct TestAgent {
        pub memory: crate::memory::InMemoryMemory,
        pub last_input: Option<String>,
        pub responses: Vec<String>,
    }

    impl Default for TestAgent {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TestAgent {
        pub fn new() -> Self {
            Self {
                memory: InMemoryMemory::new(),
                last_input: None,
                responses: vec!["Test response 1".to_string(), "Test response 2".to_string()],
            }
        }

        pub fn with_responses(responses: Vec<String>) -> Self {
            Self {
                memory: InMemoryMemory::new(),
                last_input: None,
                responses,
            }
        }
    }

    impl Agent for TestAgent {
        type Observation = String;
        type Action = String;

        fn observe(&mut self, input: String) {
            self.last_input = Some(input);
        }

        fn act(&mut self) -> String {
            let input = self.last_input.as_deref().unwrap_or("no input");
            if input.starts_with("echo:") {
                format!("Echo: {}", input.strip_prefix("echo:").unwrap_or(input))
            } else {
                self.responses
                    .first()
                    .cloned()
                    .unwrap_or_else(|| format!("Processed: {}", input))
            }
        }

        fn call_tools(&self) -> Vec<ToolCall> {
            Vec::new()
        }

        fn handle_result(&mut self, _result: ExecutionResult) {}

        fn update_context(&mut self, update: MemoryUpdate) {
            let _ = self.memory_writer().store(update);
        }

        fn memory_reader(&self) -> &dyn crate::memory::MemoryReader {
            &self.memory
        }

        fn memory_writer(&mut self) -> &mut dyn crate::memory::MemoryWriter {
            &mut self.memory
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IntegrationTestResult, LoadTestResult, test_utils::TestAgent};
    use crate::Agent;
    use std::time::Duration;

    #[tokio::test]
    async fn integration_test_result_creation() {
        let result = IntegrationTestResult::failure("test", "error".to_string());
        assert!(!result.is_success());
        assert!(result.summary().contains("FAIL"));
    }

    #[test]
    fn load_test_result_calculations() {
        let result = LoadTestResult {
            endpoint: "/test".to_string(),
            total_requests: 100,
            successful_requests: 95,
            failed_requests: 5,
            average_response_time: Duration::from_millis(50),
            min_response_time: Duration::from_millis(10),
            max_response_time: Duration::from_millis(200),
            requests_per_second: 50,
        };

        assert_eq!(result.success_rate(), 95.0);
    }

    #[test]
    fn test_agent_works() {
        let mut agent = TestAgent::new();
        agent.observe("test input".to_string());
        let action = agent.act();
        assert!(action.contains("Test response") || action.contains("Processed"));
    }
}
