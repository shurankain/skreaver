//! # Skreaver Testing Framework
//!
//! This crate provides comprehensive testing utilities for Skreaver agents and tools.
//! It includes mock tools, test harnesses, and integration testing capabilities.
//!
//! ## Components
//!
//! - **Mock Tools**: Predictable tool implementations for testing
//! - **Agent Test Harness**: Controlled environments for agent testing
//! - **Integration Tests**: End-to-end testing utilities
//! - **Performance Benchmarks**: Basic performance testing framework
//!
//! ## Usage
//!
//! ```rust
//! use skreaver_testing::{MockTool, MockToolRegistry, TestHarnessBuilder, TestScenario};
//! use skreaver_core::{Agent, MemoryUpdate, ExecutionResult, ToolCall, MemoryReader, MemoryWriter};
//! use skreaver_core::InMemoryMemory;
//!
//! // Example agent implementation
//! struct TestAgent { memory: InMemoryMemory }
//! impl Agent for TestAgent {
//!     type Observation = String;
//!     type Action = String;
//!     fn observe(&mut self, _input: String) {}
//!     fn act(&mut self) -> String { "response".to_string() }
//!     fn call_tools(&self) -> Vec<ToolCall> { Vec::new() }
//!     fn handle_result(&mut self, _result: ExecutionResult) {}
//!     fn update_context(&mut self, update: MemoryUpdate) { let _ = self.memory_writer().store(update); }
//!     fn memory_reader(&self) -> &dyn MemoryReader { &self.memory }
//!     fn memory_writer(&mut self) -> &mut dyn MemoryWriter { &mut self.memory }
//! }
//!
//! let agent = TestAgent { memory: InMemoryMemory::new() };
//! let mut harness = TestHarnessBuilder::new()
//!     .with_mock_tools()
//!     .build_with_agent(agent);
//!
//! let result = harness.run_scenario(TestScenario::simple_observation("test input"));
//! assert!(result.is_success());
//! ```

/// Performance testing framework
pub mod benchmarks;
/// Command-line interface for regression detection
pub mod cli;
/// Criterion benchmark output parser
pub mod criterion_parser;
/// Golden test framework for tool output validation
pub mod golden;
/// Golden test harness for comprehensive tool testing
pub mod golden_harness;
/// Integration test utilities
pub mod integration;
/// Convenient macros for golden test creation
pub mod macros;
/// Mock tools for predictable testing
pub mod mock_tools;
/// Performance regression detection system
pub mod regression;
/// Agent test harness for controlled testing environments
pub mod test_harness;

pub use benchmarks::{BenchmarkRunner, PerformanceTest};
pub use cli::{CliRunner, RegressionCli};
pub use criterion_parser::{CriterionCli, CriterionParser};
pub use golden::{
    GoldenTestError, SnapshotCollection, SnapshotComparison, SnapshotManager, ToolCapture,
    ToolSnapshot, compare_snapshots,
};
pub use golden_harness::{
    GoldenTestConfig, GoldenTestHarness, GoldenTestHarnessBuilder, GoldenTestResult,
    GoldenTestScenario, GoldenTestSummary,
};
pub use integration::{HttpRuntimeTester, IntegrationTest};
pub use mock_tools::{MockTool, MockToolRegistry};
pub use regression::{
    BaselineManager, PerformanceBaseline, PerformanceMeasurement, RegressionAnalysis,
    RegressionConfig, RegressionError,
};
pub use test_harness::{
    AgentTestHarness, CombinedTestSummary, TestHarnessBuilder, TestResult, TestRunner,
    TestScenario, TestSummary,
};

// Re-export commonly used types from skreaver-core for convenience
pub use skreaver_core::{StandardTool, ToolCall, ToolDispatch};
