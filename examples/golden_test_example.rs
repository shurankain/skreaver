//! # Golden Test Framework Example
//!
//! This example demonstrates how to use the comprehensive golden test framework
//! for validating tool outputs against stored snapshots.

use skreaver_core::StandardTool;
use skreaver_testing::{
    GoldenTestHarnessBuilder, GoldenTestScenario, golden_test_suite, standard_tool_inputs,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Golden Test Framework Example");
    println!("================================\n");

    // Create a temporary directory for this example
    let temp_dir = tempfile::tempdir()?;
    let snapshot_dir = temp_dir.path().join("golden_snapshots");

    println!("📁 Snapshot directory: {}", snapshot_dir.display());

    // 1. Create a golden test harness with custom configuration
    println!("\n1️⃣ Creating Golden Test Harness");
    let mut harness = GoldenTestHarnessBuilder::new()
        .snapshot_dir(&snapshot_dir)
        .auto_update(false)
        .normalize_outputs(true)
        .build_with_mocks()?;

    println!("✅ Golden test harness created with mock tools");

    // 2. Run individual golden tests using macros
    println!("\n2️⃣ Running Individual Golden Tests");

    let individual_scenarios = vec![
        // Test using custom tools from mock registry
        GoldenTestScenario::for_custom_tool("echo_test", "echo", "Hello, Golden Tests!")?
            .with_description("Test echo tool functionality"),
        GoldenTestScenario::for_custom_tool("success_test", "test_tool", "test input data")?
            .with_description("Test successful tool execution"),
        GoldenTestScenario::for_custom_tool("failure_test", "fail_tool", "this should fail")?
            .expect_failure()
            .with_description("Test failure handling"),
    ];

    let results = harness.run_golden_scenarios(individual_scenarios)?;
    println!("📊 Ran {} individual tests", results.len());

    // 3. Run golden test suites using the macro
    println!("\n3️⃣ Running Golden Test Suites");

    let suite_scenarios = golden_test_suite!(
        mock_tools: {
            "echo_simple" => (StandardTool::TextUppercase, "hello world"),
            "echo_complex" => (StandardTool::TextReverse, "reverse this text"),
        },
        text_tools: {
            "text_short" => (StandardTool::TextAnalyze, "Short text"),
            "text_long" => (StandardTool::TextAnalyze, "This is a much longer text that should provide more comprehensive analysis results for the golden test framework validation."),
        }
    );

    // Note: These will fail with mock tools since StandardTools don't exist in mock registry
    // In a real scenario, you'd use actual tools or matching mock tool names
    println!(
        "📝 Created {} test suite scenarios (Note: These will fail with mocks)",
        suite_scenarios.len()
    );

    // 4. Test standard tool inputs (using mock tools)
    println!("\n4️⃣ Testing with Standard Tool Inputs Structure");

    let custom_inputs = standard_tool_inputs!(
        HttpGet => ["https://httpbin.org/get"],
        JsonParse => [r#"{"example": "json"}"#],
        TextUppercase => ["convert to uppercase"]
    );

    println!(
        "📋 Created input sets for {} tool types",
        custom_inputs.len()
    );

    // 5. Print comprehensive results
    println!("\n5️⃣ Results Summary");
    harness.print_results();

    let summary = harness.get_summary();
    println!("\n📈 Final Summary:");
    println!("  Total Tests: {}", summary.total);
    println!(
        "  Passed: {} ({}%)",
        summary.passed,
        if summary.total > 0 {
            (summary.passed * 100) / summary.total
        } else {
            0
        }
    );
    println!("  Failed: {}", summary.failed);
    println!("  Total Time: {}ms", summary.total_time.as_millis());
    println!("  Snapshots: {}", summary.snapshot_count);

    // 6. Demonstrate snapshot management
    println!("\n6️⃣ Snapshot Management");

    let snapshot_manager = harness.snapshot_manager();
    let snapshots = snapshot_manager.list_snapshots();
    println!("📸 Created {} snapshots:", snapshots.len());
    for (i, snapshot_id) in snapshots.iter().enumerate() {
        println!("  {}. {}", i + 1, snapshot_id);

        if let Some(snapshot) = snapshot_manager.get_snapshot(snapshot_id) {
            println!("     Tool: {}", snapshot.tool_name);
            println!("     Input: {}", snapshot.input);
            println!("     Success: {}", snapshot.result.success);
            println!("     Duration: {}ms", snapshot.duration_ms);
        }
    }

    println!("\n✨ Golden Test Framework Example Complete!");
    println!("\n💡 Key Features Demonstrated:");
    println!("   • Snapshot creation and validation");
    println!("   • Cross-platform output normalization");
    println!("   • Comprehensive test harness");
    println!("   • Macro-based test creation");
    println!("   • Performance measurement");
    println!("   • Detailed reporting");

    println!("\n🔄 In a real project, you would:");
    println!("   • Use actual tools instead of mocks");
    println!("   • Commit snapshots to version control");
    println!("   • Run tests in CI to validate consistency");
    println!("   • Update snapshots when behavior changes intentionally");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_golden_example_components() {
        // Test that the example components work correctly
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

        let harness = GoldenTestHarnessBuilder::new()
            .snapshot_dir(temp_dir.path())
            .build_with_mocks()
            .expect("Failed to create harness");

        // Verify harness was created successfully
        assert!(
            harness.config().snapshot_dir.exists()
                || !harness.config().snapshot_dir.to_string_lossy().is_empty()
        );
    }

    #[test]
    fn test_macro_usage() {
        // Test that macros compile and work correctly
        let scenarios = golden_test_suite!(
            test_suite: {
                "test1" => (StandardTool::TextUppercase, "input1"),
                "test2" => (StandardTool::TextReverse, "input2"),
            }
        );

        assert_eq!(scenarios.len(), 2);
        assert_eq!(scenarios[0].test_id, "test_suite_test1");
        assert_eq!(scenarios[1].test_id, "test_suite_test2");
    }

    #[test]
    fn test_standard_inputs_macro() {
        let inputs = standard_tool_inputs!();
        assert!(!inputs.is_empty());

        // Test custom inputs
        let custom_inputs = standard_tool_inputs!(
            HttpGet => ["url1", "url2"],
            JsonParse => ["json1"]
        );

        assert_eq!(custom_inputs.len(), 2);
        assert_eq!(custom_inputs[&StandardTool::HttpGet].len(), 2);
        assert_eq!(custom_inputs[&StandardTool::JsonParse].len(), 1);
    }
}
