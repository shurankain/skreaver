# Golden Test Framework

The Golden Test Framework provides comprehensive tool output validation through snapshot testing, ensuring consistent behavior across different versions and platforms.

## Overview

Golden tests capture the output of tool executions and store them as "golden" snapshots. Future test runs compare against these snapshots to detect any changes in behavior. This approach is particularly valuable for:

- **Regression Detection**: Catch unintended changes in tool behavior
- **Cross-Platform Consistency**: Ensure tools work consistently across different environments
- **Performance Monitoring**: Track execution time changes over time
- **API Stability**: Validate that tool interfaces remain stable

## Quick Start

### Basic Usage

```rust
use skreaver_testing::{golden_test, GoldenTestHarnessBuilder};
use skreaver_core::StandardTool;

// Create a golden test harness
let mut harness = GoldenTestHarnessBuilder::new()
    .snapshot_dir("tests/golden")
    .build_with_mocks()?;

// Run a single golden test
let scenario = golden_test!(
    test_id: "json_parse_basic",
    tool: StandardTool::JsonParse,
    input: r#"{"key": "value"}"#,
    description: "Basic JSON parsing test"
);

let result = harness.run_golden_test(&scenario.test_id, scenario.tool_call)?;
assert!(result.passed);
```

### Using Macros for Test Suites

```rust
use skreaver_testing::golden_test_suite;

let scenarios = golden_test_suite!(
    http_tools: {
        "get_basic" => (StandardTool::HttpGet, "https://api.example.com/data"),
        "post_json" => (StandardTool::HttpPost, r#"{"data": "test"}"#),
    },
    text_tools: {
        "uppercase" => (StandardTool::TextUppercase, "hello world"),
        "analyze" => (StandardTool::TextAnalyze, "Sample text for analysis"),
    }
);

let results = harness.run_golden_scenarios(scenarios)?;
```

## Core Components

### SnapshotManager

Handles storage and retrieval of test snapshots:

```rust
use skreaver_testing::SnapshotManager;

let mut manager = SnapshotManager::new("tests/golden")?;

// List all snapshots
let snapshots = manager.list_snapshots();

// Get specific snapshot
if let Some(snapshot) = manager.get_snapshot("test_id") {
    println!("Tool: {}, Success: {}", snapshot.tool_name, snapshot.result.success);
}
```

### GoldenTestHarness

Main interface for running golden tests:

```rust
use skreaver_testing::{GoldenTestConfig, GoldenTestHarness};

let config = GoldenTestConfig {
    snapshot_dir: "tests/golden".into(),
    auto_update: false,
    normalize_outputs: true,
    max_time_variance: 0.5,
    validate_timing: false,
    ..Default::default()
};

let mut harness = GoldenTestHarness::new(registry, config)?;
```

### ToolCapture

Captures tool execution with normalization:

```rust
use skreaver_testing::ToolCapture;

let mut capture = ToolCapture::new(registry);
capture.set_normalization(true);

let snapshot = capture.capture_tool_execution(tool_call)?;
```

## Configuration Options

### GoldenTestConfig

```rust
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
```

### Cross-Platform Normalization

The framework automatically normalizes outputs for consistency:

- **File Paths**: Converts backslashes to forward slashes
- **JSON Objects**: Sorts keys for deterministic ordering
- **Timestamps**: Removes or normalizes time-sensitive data

## Macro Reference

### `golden_test!`

Creates individual golden test scenarios:

```rust
// Basic test
let test = golden_test!(
    test_id: "my_test",
    tool: StandardTool::JsonParse,
    input: r#"{"test": true}"#
);

// With description
let test = golden_test!(
    test_id: "my_test",
    tool: StandardTool::JsonParse,
    input: r#"{"test": true}"#,
    description: "Test JSON parsing"
);

// Expected to fail
let test = golden_test!(
    test_id: "my_test",
    tool: StandardTool::JsonParse,
    input: "invalid json",
    expect_failure: true
);
```

### `golden_test_suite!`

Creates multiple related tests:

```rust
let scenarios = golden_test_suite!(
    category_name: {
        "test1" => (StandardTool::HttpGet, "input1"),
        "test2" => (StandardTool::HttpPost, "input2"),
    }
);
```

### `standard_tool_inputs!`

Generates comprehensive test inputs:

```rust
// All standard tools with default inputs
let inputs = standard_tool_inputs!();

// Custom inputs for specific tools
let inputs = standard_tool_inputs!(
    HttpGet => ["url1", "url2"],
    JsonParse => [r#"{"json": "data"}"#]
);
```

### `golden_harness!`

Quick harness creation:

```rust
let harness = golden_harness!(
    snapshot_dir: "tests/golden",
    auto_update: false,
    normalize_outputs: true
);
```

## Snapshot Format

Snapshots are stored as JSON files:

```json
{
  "snapshots": {
    "test_id": {
      "tool_name": "json_parse",
      "tool_type": {
        "Standard": "json_parse"
      },
      "input": "{\"key\": \"value\"}",
      "result": {
        "success": true,
        "output": "parsed json result",
        "error": null,
        "execution_time": 15
      },
      "timestamp": 1672531200,
      "platform_info": {
        "os": "linux",
        "arch": "x86_64",
        "endianness": "little"
      },
      "duration_ms": 15,
      "version": "0.3.0"
    }
  },
  "metadata": {
    "created_at": 1672531200,
    "version": "0.3.0",
    "description": "Golden test snapshots",
    "total_snapshots": 1
  }
}
```

## CI Integration

### GitHub Actions Example

```yaml
name: Golden Tests
on: [push, pull_request]

jobs:
  golden-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run golden tests
        run: cargo test golden_tests --release

      - name: Verify snapshots unchanged
        run: |
          if [ -n "$(git status --porcelain tests/golden/)" ]; then
            echo "❌ Golden test snapshots were modified!"
            git diff tests/golden/
            exit 1
          fi
```

### Performance Validation

```yaml
- name: Check golden test performance
  run: |
    cargo test --release golden_tests test_performance -- --nocapture
    # Ensure tests run within performance targets
```

## Advanced Usage

### Custom Tool Testing

```rust
let scenario = GoldenTestScenario::for_custom_tool(
    "custom_tool_test",
    "my_custom_tool",
    "custom input"
)?.with_description("Test custom tool");

let result = harness.run_golden_test(&scenario.test_id, scenario.tool_call)?;
```

### Batch Operations

```rust
// Update all snapshots
let updated_count = harness.update_all_snapshots()?;
println!("Updated {} snapshots", updated_count);

// Test all standard tools
let inputs = standard_tool_inputs!();
let results = harness.test_all_standard_tools(inputs)?;
```

### Integration with TestRunner

```rust
use skreaver_testing::TestRunner;

let mut runner = TestRunner::new();

// Run both agent and golden tests
runner.run_combined_tests(
    &mut agent_harness,
    &mut golden_harness,
    agent_scenarios,
    golden_scenarios,
)?;

// Get combined summary
let summary = runner.combined_summary();
println!("{}", summary);
```

## Best Practices

### Test Organization

1. **Group Related Tests**: Use test suites for related functionality
2. **Descriptive Names**: Use clear, descriptive test IDs
3. **Document Expected Behavior**: Include descriptions for complex tests
4. **Test Edge Cases**: Include both success and failure scenarios

### Snapshot Management

1. **Review Changes Carefully**: Always review snapshot diffs before committing
2. **Separate Intentional Changes**: Update snapshots deliberately, not accidentally
3. **Keep Snapshots Minimal**: Focus on essential behavior, avoid noise
4. **Version Control**: Always commit snapshots with code changes

### Performance Considerations

1. **Use Release Builds**: Run golden tests in release mode for consistent timing
2. **Mock External Dependencies**: Avoid network calls and file I/O in unit tests
3. **Optimize for CI**: Target <30ms per test for fast CI execution
4. **Parallel Execution**: Use parallel test execution where possible

### Cross-Platform Testing

1. **Enable Normalization**: Always enable output normalization for consistency
2. **Test on Multiple Platforms**: Validate behavior across different OS/architectures
3. **Handle Path Differences**: Ensure file paths are normalized correctly
4. **Abstract Time-Sensitive Data**: Remove or normalize timestamps

## Troubleshooting

### Common Issues

**Snapshot Mismatches**
```
✗ Snapshots differ:
  - Output mismatch:
    Expected: 'old output'
    Actual:   'new output'
```

*Solution*: Review the change and update snapshots if intentional:
```rust
let config = GoldenTestConfig {
    auto_update: true,
    ..Default::default()
};
```

**Tool Not Found**
```
Tool execution failed: Tool 'my_tool' not found in registry
```

*Solution*: Ensure the tool is registered in your tool registry:
```rust
let registry = MyToolRegistry::new()
    .register("my_tool", MyTool::new());
```

**Platform Differences**
```
File path mismatch: expected '/', got '\'
```

*Solution*: Enable output normalization:
```rust
let mut harness = GoldenTestHarnessBuilder::new()
    .normalize_outputs(true)
    .build()?;
```

### Performance Issues

If golden tests are running slowly:

1. Check for network calls or heavy I/O operations
2. Use mocks for external dependencies
3. Optimize test inputs for speed
4. Consider parallel test execution

### Memory Usage

For large test suites:

1. Clear test results periodically: `harness.clear_results()`
2. Use streaming for large outputs
3. Consider snapshot compression for large datasets

## API Reference

### Core Types

- [`GoldenTestHarness`](./src/golden_harness.rs): Main test execution interface
- [`SnapshotManager`](./src/golden.rs): Snapshot storage and retrieval
- [`ToolCapture`](./src/golden.rs): Tool execution capture
- [`GoldenTestConfig`](./src/golden_harness.rs): Configuration options

### Error Types

- [`GoldenTestError`](./src/golden.rs): All golden test related errors
- [`SnapshotComparison`](./src/golden.rs): Snapshot comparison results

### Macros

- [`golden_test!`](./src/macros.rs): Create individual tests
- [`golden_test_suite!`](./src/macros.rs): Create test suites
- [`standard_tool_inputs!`](./src/macros.rs): Generate standard inputs
- [`golden_harness!`](./src/macros.rs): Quick harness creation

## Examples

See [`examples/golden_test_example.rs`](../../examples/golden_test_example.rs) for a comprehensive example demonstrating all features of the golden test framework.

## Contributing

When contributing to the golden test framework:

1. **Maintain Backward Compatibility**: Ensure existing snapshots remain valid
2. **Add Tests**: Include tests for new functionality
3. **Update Documentation**: Keep documentation current with changes
4. **Performance**: Maintain performance targets for CI execution
5. **Cross-Platform**: Test changes across different platforms