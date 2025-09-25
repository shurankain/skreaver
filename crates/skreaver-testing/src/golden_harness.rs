//! # Golden Test Harness
//!
//! This module provides a specialized test harness for golden testing of tools,
//! extending the existing AgentTestHarness with snapshot management capabilities.

use crate::golden::{
    GoldenTestError, SnapshotComparison, SnapshotManager, ToolCapture, ToolSnapshot,
    compare_snapshots,
};
use skreaver_core::{StandardTool, ToolCall};
use skreaver_tools::ToolRegistry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Golden test configuration options
#[derive(Debug, Clone)]
pub struct GoldenTestConfig {
    /// Directory for storing snapshots
    pub snapshot_dir: PathBuf,
    /// Whether to auto-update snapshots when tests fail
    pub auto_update: bool,
    /// Whether to enable cross-platform normalization
    pub normalize_outputs: bool,
    /// Maximum allowed execution time difference (percentage)
    pub max_time_variance: f64,
    /// Whether to validate execution timing
    pub validate_timing: bool,
    /// Custom snapshot file prefix
    pub snapshot_prefix: String,
}

impl Default for GoldenTestConfig {
    fn default() -> Self {
        Self {
            snapshot_dir: PathBuf::from("tests/golden"),
            auto_update: false,
            normalize_outputs: true,
            max_time_variance: 0.5, // 50% variance allowed
            validate_timing: false, // Off by default for CI stability
            snapshot_prefix: "snapshot".to_string(),
        }
    }
}

/// A specialized test harness for golden testing
pub struct GoldenTestHarness {
    snapshot_manager: SnapshotManager,
    tool_capture: ToolCapture,
    config: GoldenTestConfig,
    test_results: Vec<GoldenTestResult>,
}

/// Result of a golden test execution
#[derive(Debug, Clone)]
pub struct GoldenTestResult {
    pub test_id: String,
    pub scenario_name: String,
    pub passed: bool,
    pub execution_time: Duration,
    pub snapshot_comparison: Option<SnapshotComparison>,
    pub error: Option<GoldenTestError>,
    pub action_taken: GoldenTestAction,
}

/// Actions that can be taken during golden testing
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GoldenTestAction {
    /// Compared against existing snapshot
    Compared,
    /// Created new snapshot
    Created,
    /// Updated existing snapshot
    Updated,
    /// Skipped due to configuration
    Skipped,
    /// Failed due to error
    Failed,
}

impl GoldenTestHarness {
    /// Create a new golden test harness
    pub fn new(
        registry: Box<dyn ToolRegistry + Send + Sync>,
        config: GoldenTestConfig,
    ) -> Result<Self, GoldenTestError> {
        let snapshot_manager = SnapshotManager::new(&config.snapshot_dir)?;
        let mut tool_capture = ToolCapture::new(registry);
        tool_capture.set_normalization(config.normalize_outputs);

        // Set up standard directory structure
        snapshot_manager.setup_standard_directories()?;

        Ok(Self {
            snapshot_manager,
            tool_capture,
            config,
            test_results: Vec::new(),
        })
    }

    /// Create a new golden test harness with mock tools for testing
    pub fn new_for_testing() -> Result<Self, GoldenTestError> {
        let config = GoldenTestConfig {
            snapshot_dir: tempfile::tempdir()?.path().to_path_buf(),
            ..Default::default()
        };

        let tool_capture = ToolCapture::new_with_mocks();
        let snapshot_manager = SnapshotManager::new(&config.snapshot_dir)?;

        Ok(Self {
            snapshot_manager,
            tool_capture,
            config,
            test_results: Vec::new(),
        })
    }

    /// Run a golden test for a single tool call
    pub fn run_golden_test(
        &mut self,
        test_id: &str,
        tool_call: ToolCall,
    ) -> Result<GoldenTestResult, GoldenTestError> {
        let start_time = Instant::now();

        // Capture current tool execution
        let current_snapshot = self
            .tool_capture
            .capture_tool_execution(tool_call.clone())?;

        // Check if we have an existing snapshot
        match self.snapshot_manager.get_snapshot(test_id) {
            Some(expected_snapshot) => {
                // Compare with existing snapshot
                let comparison = compare_snapshots(expected_snapshot, &current_snapshot);

                let passed = comparison.matches;
                let action = if passed {
                    GoldenTestAction::Compared
                } else if self.config.auto_update {
                    // Auto-update the snapshot
                    self.snapshot_manager
                        .update_snapshot(test_id, current_snapshot)?;
                    GoldenTestAction::Updated
                } else {
                    GoldenTestAction::Compared
                };

                let result = GoldenTestResult {
                    test_id: test_id.to_string(),
                    scenario_name: format!("{}({})", tool_call.name(), tool_call.input),
                    passed,
                    execution_time: start_time.elapsed(),
                    snapshot_comparison: Some(comparison),
                    error: None,
                    action_taken: action,
                };

                self.test_results.push(result.clone());
                Ok(result)
            }
            None => {
                // Create new snapshot
                self.snapshot_manager
                    .store_snapshot(test_id, current_snapshot)?;

                let result = GoldenTestResult {
                    test_id: test_id.to_string(),
                    scenario_name: format!("{}({})", tool_call.name(), tool_call.input),
                    passed: true, // New snapshots always "pass"
                    execution_time: start_time.elapsed(),
                    snapshot_comparison: None,
                    error: None,
                    action_taken: GoldenTestAction::Created,
                };

                self.test_results.push(result.clone());
                Ok(result)
            }
        }
    }

    /// Run golden tests for all standard tools with given inputs
    pub fn test_all_standard_tools(
        &mut self,
        inputs: HashMap<StandardTool, Vec<String>>,
    ) -> Result<Vec<GoldenTestResult>, GoldenTestError> {
        let mut results = Vec::new();

        for (tool, test_inputs) in inputs {
            for (idx, input) in test_inputs.iter().enumerate() {
                let test_id = format!("standard_{}_{}", tool.name(), idx);
                let tool_call = ToolCall::from_standard(tool, input.clone());

                match self.run_golden_test(&test_id, tool_call) {
                    Ok(result) => results.push(result),
                    Err(error) => {
                        let error_result = GoldenTestResult {
                            test_id: test_id.clone(),
                            scenario_name: format!("{}({})", tool.name(), input),
                            passed: false,
                            execution_time: Duration::default(),
                            snapshot_comparison: None,
                            error: Some(error.clone()),
                            action_taken: GoldenTestAction::Failed,
                        };
                        self.test_results.push(error_result.clone());
                        results.push(error_result);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Run golden tests based on test scenarios
    pub fn run_golden_scenarios(
        &mut self,
        scenarios: Vec<GoldenTestScenario>,
    ) -> Result<Vec<GoldenTestResult>, GoldenTestError> {
        let mut results = Vec::new();

        for scenario in scenarios {
            let result = self.run_golden_test(&scenario.test_id, scenario.tool_call)?;
            results.push(result);
        }

        Ok(results)
    }

    /// Update all snapshots (useful for mass updates)
    pub fn update_all_snapshots(&mut self) -> Result<usize, GoldenTestError> {
        let old_auto_update = self.config.auto_update;
        self.config.auto_update = true;

        let snapshot_ids: Vec<_> = self.snapshot_manager.list_snapshots();
        let mut updated_count = 0;

        for test_id in snapshot_ids {
            if let Some(snapshot) = self.snapshot_manager.get_snapshot(&test_id) {
                // Recreate the tool call and re-execute
                let tool_call = self.recreate_tool_call_from_snapshot(snapshot)?;
                let _ = self.run_golden_test(&test_id, tool_call)?;
                updated_count += 1;
            }
        }

        self.config.auto_update = old_auto_update;
        Ok(updated_count)
    }

    /// Recreate ToolCall from snapshot (for updates)
    fn recreate_tool_call_from_snapshot(
        &self,
        snapshot: &ToolSnapshot,
    ) -> Result<ToolCall, GoldenTestError> {
        match &snapshot.tool_type {
            crate::golden::ToolDispatchType::Standard(tool_name) => {
                if let Some(standard_tool) = StandardTool::from_name(tool_name) {
                    Ok(ToolCall::from_standard(
                        standard_tool,
                        snapshot.input.clone(),
                    ))
                } else {
                    Err(GoldenTestError::ValidationError(format!(
                        "Unknown standard tool: {}",
                        tool_name
                    )))
                }
            }
            crate::golden::ToolDispatchType::Custom(tool_name) => {
                ToolCall::new(tool_name, &snapshot.input)
                    .map_err(|e| GoldenTestError::ValidationError(e.to_string()))
            }
        }
    }

    /// Get test summary
    pub fn get_summary(&self) -> GoldenTestSummary {
        let total = self.test_results.len();
        let passed = self.test_results.iter().filter(|r| r.passed).count();
        let failed = total - passed;

        let mut actions = HashMap::new();
        for result in &self.test_results {
            *actions.entry(result.action_taken.clone()).or_insert(0) += 1;
        }

        let total_time: Duration = self.test_results.iter().map(|r| r.execution_time).sum();

        GoldenTestSummary {
            total,
            passed,
            failed,
            actions,
            total_time,
            snapshot_count: self.snapshot_manager.list_snapshots().len(),
        }
    }

    /// Print detailed test results
    pub fn print_results(&self) {
        println!("Golden Test Results:");
        println!("===================");

        for result in &self.test_results {
            let status = if result.passed {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };
            let action = match result.action_taken {
                GoldenTestAction::Created => "[NEW]",
                GoldenTestAction::Updated => "[UPD]",
                GoldenTestAction::Compared => "[CMP]",
                GoldenTestAction::Skipped => "[SKIP]",
                GoldenTestAction::Failed => "[ERR]",
            };

            println!(
                "{} {} {} ({}ms)",
                status,
                action,
                result.scenario_name,
                result.execution_time.as_millis()
            );

            if let Some(ref error) = result.error {
                println!("    Error: {}", error);
            }

            if let Some(ref comparison) = result.snapshot_comparison
                && !comparison.matches
            {
                println!("    {}", comparison.summary());
            }
        }

        let summary = self.get_summary();
        println!("\n{}", summary);
    }

    /// Clear all test results
    pub fn clear_results(&mut self) {
        self.test_results.clear();
    }

    /// Get access to snapshot manager (for advanced operations)
    pub fn snapshot_manager(&mut self) -> &mut SnapshotManager {
        &mut self.snapshot_manager
    }

    /// Get current configuration
    pub fn config(&self) -> &GoldenTestConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: GoldenTestConfig) -> Result<(), GoldenTestError> {
        // If snapshot directory changed, create new manager
        if self.config.snapshot_dir != config.snapshot_dir {
            self.snapshot_manager = SnapshotManager::new(&config.snapshot_dir)?;
            self.snapshot_manager.setup_standard_directories()?;
        }

        // Update tool capture normalization
        self.tool_capture
            .set_normalization(config.normalize_outputs);

        self.config = config;
        Ok(())
    }
}

/// Golden test scenario definition
#[derive(Debug, Clone)]
pub struct GoldenTestScenario {
    pub test_id: String,
    pub tool_call: ToolCall,
    pub description: Option<String>,
    pub expected_to_pass: bool,
}

impl GoldenTestScenario {
    /// Create a new golden test scenario
    pub fn new(test_id: impl Into<String>, tool_call: ToolCall) -> Self {
        Self {
            test_id: test_id.into(),
            tool_call,
            description: None,
            expected_to_pass: true,
        }
    }

    /// Add a description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Mark as expected to fail
    pub fn expect_failure(mut self) -> Self {
        self.expected_to_pass = false;
        self
    }

    /// Create a scenario for a standard tool
    pub fn for_standard_tool(
        test_id: impl Into<String>,
        tool: StandardTool,
        input: impl Into<String>,
    ) -> Self {
        let tool_call = ToolCall::from_standard(tool, input.into());
        Self::new(test_id, tool_call)
    }

    /// Create a scenario for a custom tool
    pub fn for_custom_tool(
        test_id: impl Into<String>,
        tool_name: impl AsRef<str>,
        input: impl Into<String>,
    ) -> Result<Self, GoldenTestError> {
        let tool_call = ToolCall::new(tool_name.as_ref(), &input.into())
            .map_err(|e| GoldenTestError::ValidationError(e.to_string()))?;
        Ok(Self::new(test_id, tool_call))
    }
}

/// Summary of golden test execution
#[derive(Debug)]
pub struct GoldenTestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub actions: HashMap<GoldenTestAction, usize>,
    pub total_time: Duration,
    pub snapshot_count: usize,
}

impl std::fmt::Display for GoldenTestSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Golden Test Summary:")?;
        writeln!(f, "  Total tests: {}", self.total)?;
        writeln!(f, "  Passed: {}", self.passed)?;
        writeln!(f, "  Failed: {}", self.failed)?;
        writeln!(f, "  Total time: {}ms", self.total_time.as_millis())?;
        writeln!(f, "  Snapshots: {}", self.snapshot_count)?;

        if !self.actions.is_empty() {
            writeln!(f, "  Actions:")?;
            for (action, count) in &self.actions {
                writeln!(f, "    {:?}: {}", action, count)?;
            }
        }

        Ok(())
    }
}

/// Builder for creating golden test harnesses
pub struct GoldenTestHarnessBuilder {
    config: GoldenTestConfig,
    registry: Option<Box<dyn ToolRegistry + Send + Sync>>,
}

impl GoldenTestHarnessBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: GoldenTestConfig::default(),
            registry: None,
        }
    }

    /// Set the snapshot directory
    pub fn snapshot_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.config.snapshot_dir = dir.into();
        self
    }

    /// Enable auto-update of snapshots
    pub fn auto_update(mut self, enabled: bool) -> Self {
        self.config.auto_update = enabled;
        self
    }

    /// Enable output normalization
    pub fn normalize_outputs(mut self, enabled: bool) -> Self {
        self.config.normalize_outputs = enabled;
        self
    }

    /// Set timing validation
    pub fn validate_timing(mut self, enabled: bool) -> Self {
        self.config.validate_timing = enabled;
        self
    }

    /// Set the tool registry
    pub fn with_registry(mut self, registry: Box<dyn ToolRegistry + Send + Sync>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Use mock tools
    pub fn with_mock_tools(self) -> Self {
        // For now, we'll handle mock tools in the build method
        self
    }

    /// Build the golden test harness
    pub fn build(self) -> Result<GoldenTestHarness, GoldenTestError> {
        let registry = self.registry.ok_or_else(|| {
            GoldenTestError::ValidationError(
                "No tool registry provided. Use with_registry() or with_mock_tools().".to_string(),
            )
        })?;

        GoldenTestHarness::new(registry, self.config)
    }

    /// Build with mock tools (convenience method)
    pub fn build_with_mocks(self) -> Result<GoldenTestHarness, GoldenTestError> {
        GoldenTestHarness::new_for_testing()
    }
}

impl Default for GoldenTestHarnessBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_core::StandardTool;

    #[test]
    fn test_golden_harness_creation() {
        let harness = GoldenTestHarness::new_for_testing();
        assert!(harness.is_ok());
    }

    #[test]
    fn test_golden_test_scenario_creation() {
        let scenario = GoldenTestScenario::for_standard_tool(
            "http_get_test",
            StandardTool::HttpGet,
            "https://api.example.com/data",
        );

        assert_eq!(scenario.test_id, "http_get_test");
        assert_eq!(scenario.tool_call.name(), "http_get");
        assert!(scenario.expected_to_pass);
    }

    #[test]
    fn test_golden_test_config() {
        let config = GoldenTestConfig {
            auto_update: true,
            normalize_outputs: false,
            ..Default::default()
        };

        assert!(config.auto_update);
        assert!(!config.normalize_outputs);
    }

    #[test]
    fn test_builder_pattern() {
        let builder = GoldenTestHarnessBuilder::new()
            .snapshot_dir("custom/path")
            .auto_update(true)
            .normalize_outputs(false);

        assert_eq!(builder.config.snapshot_dir, PathBuf::from("custom/path"));
        assert!(builder.config.auto_update);
        assert!(!builder.config.normalize_outputs);
    }

    #[test]
    fn test_golden_scenario_builder() {
        let scenario = GoldenTestScenario::for_standard_tool(
            "json_parse_test",
            StandardTool::JsonParse,
            r#"{"key": "value"}"#,
        )
        .with_description("Test JSON parsing")
        .expect_failure();

        assert!(scenario.description.is_some());
        assert!(!scenario.expected_to_pass);
    }
}
