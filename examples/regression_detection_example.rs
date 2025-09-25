//! # Performance Regression Detection Example
//!
//! This example demonstrates how to use the skreaver-testing framework's
//! performance regression detection capabilities.

use skreaver_testing::benchmarks::BenchmarkResult;
use skreaver_testing::{
    BaselineManager, CriterionCli, CriterionParser, PerformanceMeasurement, RegressionCli,
    RegressionConfig,
};
use std::time::Duration;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Skreaver Performance Regression Detection Example");
    println!("===================================================\n");

    // Create a temporary directory for baselines
    let temp_dir = TempDir::new()?;
    let baseline_path = temp_dir.path().join("baselines");

    println!("📁 Using baseline storage: {}\n", baseline_path.display());

    // Example 1: Direct API usage
    println!("🔧 Example 1: Direct Baseline Manager API");
    println!("-----------------------------------------");

    example_direct_api(&baseline_path)?;

    println!("\n🔧 Example 2: Criterion Output Parsing");
    println!("--------------------------------------");

    example_criterion_parsing(&baseline_path)?;

    println!("\n🔧 Example 3: CLI Usage Simulation");
    println!("----------------------------------");

    example_cli_simulation(&baseline_path)?;

    println!("\n🔧 Example 4: Custom Configuration");
    println!("----------------------------------");

    example_custom_config(&baseline_path)?;

    println!("\n✅ All examples completed successfully!");
    println!("💡 Check the implementation in crates/skreaver-testing/ for more details.");

    Ok(())
}

/// Example 1: Direct usage of BaselineManager API
fn example_direct_api(baseline_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = BaselineManager::new(baseline_path)?;

    // Create some baseline measurements
    println!("📊 Creating baseline measurements...");

    for i in 0..15 {
        let result = BenchmarkResult {
            name: "example/memory_operation".to_string(),
            iterations: 100,
            mean: Duration::from_micros(100 + i), // Stable performance around 100μs
            median: Duration::from_micros(100 + i),
            min: Duration::from_micros(95 + i),
            max: Duration::from_micros(105 + i),
            std_dev: Duration::from_micros(2),
            throughput: Some(10000.0), // 10k ops/sec
            total_operations: Some(100),
        };

        let measurement = PerformanceMeasurement::from(result);
        manager.update_baseline(measurement)?;
    }

    println!("✓ Created baseline with 15 measurements");

    // Test with good performance (no regression)
    println!("🧪 Testing with good performance...");
    let good_result = BenchmarkResult {
        name: "example/memory_operation".to_string(),
        iterations: 100,
        mean: Duration::from_micros(102), // Only 2% increase
        median: Duration::from_micros(102),
        min: Duration::from_micros(98),
        max: Duration::from_micros(106),
        std_dev: Duration::from_micros(2),
        throughput: Some(9800.0),
        total_operations: Some(100),
    };

    let good_measurement = PerformanceMeasurement::from(good_result);
    let analysis = manager.detect_regression(&good_measurement)?;

    println!("  📈 Analysis: {}", analysis.summary());
    assert!(!analysis.is_regression);

    // Test with poor performance (regression detected)
    println!("🚨 Testing with poor performance (regression)...");
    let bad_result = BenchmarkResult {
        name: "example/memory_operation".to_string(),
        iterations: 100,
        mean: Duration::from_micros(150), // 50% increase - clear regression
        median: Duration::from_micros(150),
        min: Duration::from_micros(145),
        max: Duration::from_micros(155),
        std_dev: Duration::from_micros(3),
        throughput: Some(6600.0),
        total_operations: Some(100),
    };

    let bad_measurement = PerformanceMeasurement::from(bad_result);
    let analysis = manager.detect_regression(&bad_measurement)?;

    println!("  📈 Analysis: {}", analysis.summary());
    println!("  📝 Details: {}", analysis.details);
    assert!(analysis.is_regression);

    Ok(())
}

/// Example 2: Parsing Criterion benchmark output
fn example_criterion_parsing(
    baseline_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Simulate criterion benchmark output
    let criterion_output = r#"
Benchmarking example/fast_operation
Benchmarking example/fast_operation: Warming up for 1.0000 s
example/fast_operation     time:   [98.123 ns 100.45 ns 102.78 ns]
                           thrpt:  [9.729 Melem/s 9.955 Melem/s 10.193 Melem/s]

Benchmarking example/slow_operation
example/slow_operation      time:   [1.234 μs 1.456 μs 1.678 μs]
                           thrpt:  [595.95 Kelem/s 686.81 Kelem/s 810.37 Kelem/s]

Benchmarking example/file_operation
example/file_operation      time:   [12.34 ms 13.56 ms 14.78 ms]
"#;

    println!("🔍 Parsing criterion benchmark output...");

    let parser = CriterionParser::with_git_info(
        Some("abc123def456".to_string()),
        Some("feature/performance".to_string()),
    );

    let measurements = parser.parse_console_output(criterion_output)?;
    println!("  ✓ Parsed {} benchmark results", measurements.len());

    for measurement in &measurements {
        println!(
            "    📊 {}: {}μs (commit: {:?})",
            measurement.benchmark_name,
            measurement.mean_duration_nanos / 1000,
            measurement.commit_hash.as_ref().map(|h| &h[..7])
        );
    }

    // Update baselines with parsed measurements
    println!("💾 Updating baselines with new measurements...");
    let measurements = CriterionCli::parse_and_update_baselines(criterion_output, baseline_path)?;
    println!("  ✓ Updated {} baselines", measurements.len());

    Ok(())
}

/// Example 3: CLI usage simulation
fn example_cli_simulation(
    baseline_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let cli = RegressionCli::with_baseline_path(baseline_path);

    println!("📋 Listing available baselines:");
    cli.list_baselines()?;

    // Show details of a specific baseline if it exists
    let manager = BaselineManager::new(baseline_path)?;
    let baselines = manager.list_baselines();

    if let Some(first_baseline) = baselines.first() {
        println!("\n📊 Showing details for '{}':", first_baseline);
        cli.show_baseline(first_baseline)?;
    }

    Ok(())
}

/// Example 4: Custom regression detection configuration
fn example_custom_config(
    baseline_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create a very strict configuration
    let strict_config = RegressionConfig {
        mean_threshold_percent: 5.0, // Only 5% increase allowed
        p95_threshold_percent: 8.0,  // 8% for P95
        p99_threshold_percent: 12.0, // 12% for P99
        min_samples: 5,              // Fewer samples needed for testing
        use_statistical_test: true,
        significance_level: 0.01, // 99% confidence
    };

    println!("🎯 Testing with strict regression configuration:");
    println!(
        "   📊 Mean threshold: {:.1}%",
        strict_config.mean_threshold_percent
    );
    println!(
        "   📊 P95 threshold: {:.1}%",
        strict_config.p95_threshold_percent
    );
    println!("   📊 Minimum samples: {}", strict_config.min_samples);

    let strict_path = baseline_path.join("strict");
    let mut strict_manager = BaselineManager::with_config(&strict_path, strict_config)?;

    // Create a tight baseline (very consistent performance)
    for _ in 0..10 {
        let result = BenchmarkResult {
            name: "strict/consistent_operation".to_string(),
            iterations: 100,
            mean: Duration::from_micros(100), // Exactly 100μs every time
            median: Duration::from_micros(100),
            min: Duration::from_micros(99),
            max: Duration::from_micros(101),
            std_dev: Duration::from_micros(1),
            throughput: Some(10000.0),
            total_operations: Some(100),
        };

        strict_manager.update_baseline(PerformanceMeasurement::from(result))?;
    }

    // Test with a small performance degradation that would pass normal config
    let slightly_slower = BenchmarkResult {
        name: "strict/consistent_operation".to_string(),
        iterations: 100,
        mean: Duration::from_micros(107), // 7% slower - would pass default (10%) but fail strict (5%)
        median: Duration::from_micros(107),
        min: Duration::from_micros(106),
        max: Duration::from_micros(108),
        std_dev: Duration::from_micros(1),
        throughput: Some(9340.0),
        total_operations: Some(100),
    };

    let measurement = PerformanceMeasurement::from(slightly_slower);
    let analysis = strict_manager.detect_regression(&measurement)?;

    println!("  🔍 Testing 7% performance decrease:");
    println!("     📈 Result: {}", analysis.summary());

    if analysis.is_regression {
        println!("     ⚠️  Detected as regression with strict config!");
    } else {
        println!("     ✅ Would pass with normal config");
    }

    Ok(())
}
