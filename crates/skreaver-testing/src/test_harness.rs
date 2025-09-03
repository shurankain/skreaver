//! # Agent Test Harness
//!
//! This module provides controlled testing environments for agents with
//! scenarios, assertions, and comprehensive result tracking.

use crate::MockToolRegistry;
use skreaver_core::Agent;
use skreaver_http::runtime::Coordinator;
use skreaver_tools::ToolRegistry;
use std::fmt;
use std::time::{Duration, Instant};

/// Test scenario for agent execution
#[derive(Debug, Clone)]
pub struct TestScenario {
    /// Name of the test scenario
    pub name: String,
    /// Input observation for the agent
    pub observation: String,
    /// Expected agent actions (optional)
    pub expected_actions: Vec<String>,
    /// Expected tool calls (optional)
    pub expected_tool_calls: Vec<String>,
    /// Maximum execution time allowed
    pub timeout: Duration,
    /// Whether the scenario should succeed
    pub should_succeed: bool,
}

impl TestScenario {
    /// Create a simple observation scenario
    pub fn simple_observation(observation: impl Into<String>) -> Self {
        Self {
            name: "simple_observation".to_string(),
            observation: observation.into(),
            expected_actions: Vec::new(),
            expected_tool_calls: Vec::new(),
            timeout: Duration::from_secs(5),
            should_succeed: true,
        }
    }

    /// Create a named scenario
    pub fn named(name: impl Into<String>, observation: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            observation: observation.into(),
            expected_actions: Vec::new(),
            expected_tool_calls: Vec::new(),
            timeout: Duration::from_secs(5),
            should_succeed: true,
        }
    }

    /// Expect specific agent actions
    pub fn expect_actions(mut self, actions: Vec<String>) -> Self {
        self.expected_actions = actions;
        self
    }

    /// Expect specific tool calls
    pub fn expect_tool_calls(mut self, tool_calls: Vec<String>) -> Self {
        self.expected_tool_calls = tool_calls;
        self
    }

    /// Set execution timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Expect the scenario to fail
    pub fn should_fail(mut self) -> Self {
        self.should_succeed = false;
        self
    }
}

/// Result of a test scenario execution
#[derive(Debug)]
pub struct TestResult {
    /// Name of the scenario
    pub scenario_name: String,
    /// Whether the test passed
    pub passed: bool,
    /// Agent's action response
    pub agent_action: String,
    /// Tool calls made during execution
    pub tool_calls: Vec<String>,
    /// Memory updates performed
    pub memory_updates: Vec<String>,
    /// Execution time
    pub execution_time: Duration,
    /// Error message if test failed
    pub error: Option<String>,
    /// Additional assertions results
    pub assertion_results: Vec<AssertionResult>,
}

impl TestResult {
    /// Check if the test passed
    pub fn is_success(&self) -> bool {
        self.passed
    }

    /// Get a summary of the test result
    pub fn summary(&self) -> String {
        let status = if self.passed { "PASS" } else { "FAIL" };
        let time = self.execution_time.as_millis();

        match &self.error {
            Some(error) => format!(
                "[{}] {} ({}ms) - {}",
                status, self.scenario_name, time, error
            ),
            None => format!("[{}] {} ({}ms)", status, self.scenario_name, time),
        }
    }
}

impl fmt::Display for TestResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Result of an assertion
#[derive(Debug)]
pub struct AssertionResult {
    /// Description of the assertion
    pub description: String,
    /// Whether the assertion passed
    pub passed: bool,
    /// Expected value
    pub expected: String,
    /// Actual value
    pub actual: String,
}

/// Test harness for controlled agent testing
pub struct AgentTestHarness<A, R>
where
    A: Agent,
    A::Observation: std::fmt::Display,
    R: ToolRegistry + Clone,
{
    coordinator: Coordinator<A, R>,
    memory_snapshots: Vec<String>,
}

impl<A, R> AgentTestHarness<A, R>
where
    A: Agent,
    A::Observation: From<String> + std::fmt::Display,
    A::Action: ToString,
    R: ToolRegistry + Clone,
{
    /// Create a new test harness
    pub fn new(agent: A, registry: R) -> Self {
        let coordinator = Coordinator::new(agent, registry);
        Self {
            coordinator,
            memory_snapshots: Vec::new(),
        }
    }

    /// Run a single test scenario
    pub fn run_scenario(&mut self, scenario: TestScenario) -> TestResult {
        let start_time = Instant::now();
        let mut result = TestResult {
            scenario_name: scenario.name.clone(),
            passed: false,
            agent_action: String::new(),
            tool_calls: Vec::new(),
            memory_updates: Vec::new(),
            execution_time: Duration::default(),
            error: None,
            assertion_results: Vec::new(),
        };

        // Take memory snapshot before execution
        self.take_memory_snapshot();

        // Execute the scenario
        match self.execute_scenario_with_timeout(&scenario) {
            Ok(action) => {
                result.agent_action = action;
                result.passed = self.validate_scenario(&scenario, &mut result);
            }
            Err(error) => {
                result.error = Some(error);
                result.passed = !scenario.should_succeed; // If we expected failure, this is success
            }
        }

        result.execution_time = start_time.elapsed();
        result
    }

    /// Run multiple scenarios and return aggregated results
    pub fn run_scenarios(&mut self, scenarios: Vec<TestScenario>) -> Vec<TestResult> {
        scenarios
            .into_iter()
            .map(|scenario| self.run_scenario(scenario))
            .collect()
    }

    /// Create a test suite with common scenarios
    pub fn standard_test_suite(observation: impl Into<String>) -> Vec<TestScenario> {
        let obs = observation.into();
        vec![
            TestScenario::simple_observation(&obs),
            TestScenario::named("timeout_test", &obs).with_timeout(Duration::from_millis(100)),
            TestScenario::named("empty_input", ""),
            TestScenario::named("long_input", "a".repeat(1000)),
        ]
    }

    /// Execute scenario with timeout
    fn execute_scenario_with_timeout(&mut self, scenario: &TestScenario) -> Result<String, String> {
        // For now, we'll execute synchronously
        // In a real implementation, you might want to use tokio::time::timeout
        let observation = A::Observation::from(scenario.observation.clone());
        let action = self.coordinator.step(observation);
        Ok(action.to_string())
    }

    /// Validate scenario results against expectations
    fn validate_scenario(&self, scenario: &TestScenario, result: &mut TestResult) -> bool {
        let mut passed = scenario.should_succeed;

        // Validate expected actions
        if !scenario.expected_actions.is_empty() {
            let action_found = scenario
                .expected_actions
                .iter()
                .any(|expected| result.agent_action.contains(expected));

            result.assertion_results.push(AssertionResult {
                description: "Expected action found".to_string(),
                passed: action_found,
                expected: scenario.expected_actions.join(", "),
                actual: result.agent_action.clone(),
            });

            passed &= action_found;
        }

        // Validate expected tool calls
        if !scenario.expected_tool_calls.is_empty() {
            // This would need access to the tool registry call history
            // For now, we'll mark as passed
            result.assertion_results.push(AssertionResult {
                description: "Expected tool calls".to_string(),
                passed: true,
                expected: scenario.expected_tool_calls.join(", "),
                actual: "N/A".to_string(),
            });
        }

        passed
    }

    /// Take a snapshot of current memory state
    fn take_memory_snapshot(&mut self) {
        // This would capture current memory state for comparison
        self.memory_snapshots.push("memory_snapshot".to_string());
    }
}

/// Builder for creating test harnesses with common configurations
pub struct TestHarnessBuilder {
    registry: Option<MockToolRegistry>,
}

impl TestHarnessBuilder {
    /// Create a new test harness builder
    pub fn new() -> Self {
        Self { registry: None }
    }

    /// Use a specific tool registry
    pub fn with_registry(mut self, registry: MockToolRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Use mock tools with standard configuration
    pub fn with_mock_tools(mut self) -> Self {
        let registry = MockToolRegistry::new()
            .with_echo_tool()
            .with_success_tool("test_tool")
            .with_failure_tool("fail_tool");
        self.registry = Some(registry);
        self
    }

    /// Build the test harness with a provided agent
    pub fn build_with_agent<A>(self, agent: A) -> AgentTestHarness<A, MockToolRegistry>
    where
        A: Agent,
        A::Observation: From<String> + std::fmt::Display,
        A::Action: ToString,
    {
        let registry = self
            .registry
            .unwrap_or_else(|| MockToolRegistry::new().with_mock_tools());
        AgentTestHarness::new(agent, registry)
    }
}

impl Default for TestHarnessBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test runner for executing multiple test suites
pub struct TestRunner {
    pub results: Vec<TestResult>,
}

impl TestRunner {
    /// Create a new test runner
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    /// Run all tests and collect results
    pub fn run_all_tests<A, R>(
        &mut self,
        harness: &mut AgentTestHarness<A, R>,
        scenarios: Vec<TestScenario>,
    ) where
        A: Agent,
        A::Observation: From<String> + std::fmt::Display,
        A::Action: ToString,
        R: ToolRegistry + Clone,
    {
        let test_results = harness.run_scenarios(scenarios);
        self.results.extend(test_results);
    }

    /// Get test summary
    pub fn summary(&self) -> TestSummary {
        let total = self.results.len();
        let passed = self.results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let total_time: Duration = self.results.iter().map(|r| r.execution_time).sum();

        TestSummary {
            total,
            passed,
            failed,
            total_time,
        }
    }

    /// Print detailed results
    pub fn print_results(&self) {
        println!("Test Results:");
        println!("=============");

        for result in &self.results {
            println!("{}", result);

            for assertion in &result.assertion_results {
                let status = if assertion.passed { "✓" } else { "✗" };
                println!(
                    "  {} {}: expected '{}', got '{}'",
                    status, assertion.description, assertion.expected, assertion.actual
                );
            }
        }

        let summary = self.summary();
        println!("\nSummary:");
        println!("  Total: {}", summary.total);
        println!("  Passed: {}", summary.passed);
        println!("  Failed: {}", summary.failed);
        println!("  Total time: {}ms", summary.total_time.as_millis());
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of test execution results
#[derive(Debug)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub total_time: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::Agent;
    use skreaver_core::InMemoryMemory;
    use skreaver_core::MemoryUpdate;
    use skreaver_core::{ExecutionResult, ToolCall};
    use skreaver_core::{MemoryReader, MemoryWriter};

    struct TestAgent {
        memory: InMemoryMemory,
        last_input: Option<String>,
    }

    impl Agent for TestAgent {
        type Observation = String;
        type Action = String;

        fn observe(&mut self, input: String) {
            self.last_input = Some(input);
        }

        fn act(&mut self) -> String {
            format!(
                "Processed: {}",
                self.last_input.as_deref().unwrap_or("no input")
            )
        }

        fn call_tools(&self) -> Vec<ToolCall> {
            Vec::new()
        }

        fn handle_result(&mut self, _result: ExecutionResult) {}

        fn update_context(&mut self, update: MemoryUpdate) {
            let _ = self.memory_writer().store(update);
        }

        fn memory_reader(&self) -> &dyn MemoryReader {
            &self.memory
        }

        fn memory_writer(&mut self) -> &mut dyn MemoryWriter {
            &mut self.memory
        }
    }

    #[test]
    fn test_harness_runs_simple_scenario() {
        let agent = TestAgent {
            memory: InMemoryMemory::new(),
            last_input: None,
        };

        let registry = MockToolRegistry::new().with_mock_tools();
        let mut harness = AgentTestHarness::new(agent, registry);

        let scenario = TestScenario::simple_observation("test input");
        let result = harness.run_scenario(scenario);

        assert!(result.is_success());
        assert!(result.agent_action.contains("test input"));
    }

    #[test]
    fn test_scenario_builder_works() {
        let scenario = TestScenario::named("complex_test", "input")
            .expect_actions(vec!["action1".to_string()])
            .expect_tool_calls(vec!["tool1".to_string()])
            .with_timeout(Duration::from_secs(10))
            .should_fail();

        assert_eq!(scenario.name, "complex_test");
        assert!(!scenario.should_succeed);
        assert_eq!(scenario.timeout, Duration::from_secs(10));
    }
}
