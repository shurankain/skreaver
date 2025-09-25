# Golden Test Snapshots

This directory contains golden test snapshots for validating tool outputs across different versions and platforms.

## Directory Structure

```
tests/golden/
├── README.md                    # This file
├── snapshots.json              # Main snapshot collection
├── standard_tools/             # Snapshots for StandardTool variants
│   ├── http_get.json          # HTTP GET tool snapshots
│   ├── http_post.json         # HTTP POST tool snapshots
│   ├── file_operations.json   # File I/O tool snapshots
│   ├── json_processing.json   # JSON processing tool snapshots
│   └── text_tools.json        # Text manipulation tool snapshots
├── custom_tools/              # Snapshots for custom tools
│   └── example_tool.json      # Example custom tool snapshots
└── integration_tests/         # End-to-end integration snapshots
    └── agent_workflows.json   # Agent workflow snapshots
```

## Usage

Golden tests are automatically managed by the `skreaver-testing` framework. To run golden tests:

### Basic Usage

```rust
use skreaver_testing::{golden_test, run_golden_test};
use skreaver_core::StandardTool;

// Run a single golden test
run_golden_test!(
    harness_config: {
        snapshot_dir: "tests/golden",
        auto_update: false,
    },
    scenarios: [
        golden_test!(
            test_id: "http_get_basic",
            tool: StandardTool::HttpGet,
            input: "https://httpbin.org/get",
            description: "Test basic HTTP GET functionality"
        )
    ]
);
```

### Running All Standard Tool Tests

```rust
use skreaver_testing::{GoldenTestHarness, standard_tool_inputs};

let mut harness = GoldenTestHarness::new_for_testing()?;
let inputs = standard_tool_inputs!();
let results = harness.test_all_standard_tools(inputs)?;
```

## Snapshot Management

### Creating New Snapshots

New snapshots are automatically created when running golden tests for the first time:

```bash
cargo test golden_tests
```

### Updating Snapshots

To update existing snapshots after intentional changes:

```rust
// Set auto_update to true in your test configuration
let config = GoldenTestConfig {
    auto_update: true,
    ..Default::default()
};
```

Or update all snapshots programmatically:

```rust
let updated_count = harness.update_all_snapshots()?;
println!("Updated {} snapshots", updated_count);
```

### Reviewing Snapshots

Snapshots are stored in human-readable JSON format:

```json
{
  "snapshots": {
    "http_get_basic": {
      "tool_name": "http_get",
      "tool_type": "Standard",
      "input": "https://httpbin.org/get",
      "result": {
        "success": true,
        "output": "{\"url\": \"https://httpbin.org/get\"}",
        "error": null
      },
      "timestamp": 1672531200,
      "platform_info": {
        "os": "linux",
        "arch": "x86_64",
        "endianness": "little"
      },
      "duration_ms": 150,
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

## Cross-Platform Considerations

The framework automatically normalizes outputs for cross-platform compatibility:

- **File paths**: Converted to forward slashes
- **JSON objects**: Keys are sorted for consistent ordering
- **Timestamps**: Removed or normalized where possible
- **Platform-specific outputs**: Abstracted to common formats

## CI Integration

Golden tests are designed to run efficiently in CI environments:

- Fast execution (target: <30ms per tool test)
- Minimal I/O operations
- Deterministic outputs
- Comprehensive error reporting

### Example CI Configuration

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
        run: git diff --exit-code tests/golden/
```

## Best Practices

### Test Organization

- Group related tools in test suites
- Use descriptive test IDs
- Include meaningful descriptions
- Test both success and failure cases

### Snapshot Hygiene

- Review snapshot changes carefully
- Don't commit sensitive data in snapshots
- Keep snapshots focused and minimal
- Update snapshots intentionally, not accidentally

### Performance

- Use appropriate timeouts
- Avoid network calls in unit tests
- Mock external dependencies
- Optimize for CI execution speed

## Troubleshooting

### Snapshot Mismatches

When tests fail due to snapshot mismatches:

1. Review the diff output carefully
2. Determine if the change is intentional
3. Update snapshots if the change is correct
4. Investigate if the change indicates a regression

### Platform Differences

If you encounter platform-specific failures:

1. Check if normalization is enabled
2. Review platform_info in snapshots
3. Add platform-specific normalization rules
4. Consider using separate snapshots for different platforms

### Performance Issues

If golden tests are slow:

1. Check for network calls or file I/O
2. Use mocks for external dependencies
3. Optimize test inputs for speed
4. Consider parallel test execution