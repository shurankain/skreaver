//! Performance Regression CI Integration Tests
//!
//! These tests demonstrate how to integrate performance regression detection
//! into CI/CD pipelines with proper baseline management and thresholds.

use skreaver_testing::benchmarks::BenchmarkResult;
use skreaver_testing::{
    BaselineManager, CriterionCli, PerformanceMeasurement, RegressionCli, RegressionConfig,
};
use std::process::Command;
use std::time::Duration;
use tempfile::TempDir;

/// Test CI workflow: Create baselines, detect regressions, and handle edge cases
#[test]
fn test_ci_workflow_baseline_creation_and_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let baseline_path = temp_dir.path().join("ci_baselines");

    // Step 1: Create initial baselines (typically done once)
    println!("Setting up CI baselines...");

    let mut manager = BaselineManager::new(&baseline_path).expect("Failed to create manager");

    // Simulate stable performance over multiple runs
    let stable_benchmarks = vec![
        ("http/request_handling", 500),  // 500μs mean
        ("memory/allocation_test", 100), // 100μs mean
        ("json/parsing_operation", 250), // 250μs mean
    ];

    for (benchmark_name, base_micros) in &stable_benchmarks {
        // Create 20 baseline measurements with small variations
        for i in 0..20 {
            let variation = (i % 3) as u64; // Small variation: 0, 1, 2 μs
            let result = BenchmarkResult {
                name: benchmark_name.to_string(),
                iterations: 100,
                mean: Duration::from_micros(base_micros + variation),
                median: Duration::from_micros(base_micros + variation),
                min: Duration::from_micros(base_micros.saturating_sub(5) + variation),
                max: Duration::from_micros(base_micros + 5 + variation),
                std_dev: Duration::from_micros(3),
                throughput: Some(1_000_000.0 / (*base_micros as f64 + variation as f64)),
                total_operations: Some(100),
            };

            manager
                .update_baseline(PerformanceMeasurement::from(result))
                .expect("Failed to update baseline");
        }
    }

    println!(
        "✓ Created baselines for {} benchmarks",
        stable_benchmarks.len()
    );

    // Step 2: Test acceptable performance (no regression)
    println!("Testing acceptable performance changes...");

    for (benchmark_name, base_micros) in &stable_benchmarks {
        // Test with 5% increase (should pass default 10% threshold)
        let acceptable_increase = (*base_micros as f64 * 1.05) as u64;
        let result = BenchmarkResult {
            name: benchmark_name.to_string(),
            iterations: 100,
            mean: Duration::from_micros(acceptable_increase),
            median: Duration::from_micros(acceptable_increase),
            min: Duration::from_micros(acceptable_increase.saturating_sub(5)),
            max: Duration::from_micros(acceptable_increase + 5),
            std_dev: Duration::from_micros(3),
            throughput: Some(1_000_000.0 / acceptable_increase as f64),
            total_operations: Some(100),
        };

        let measurement = PerformanceMeasurement::from(result);
        let analysis = manager
            .detect_regression(&measurement)
            .expect("Failed to detect regression");

        assert!(
            !analysis.is_regression,
            "5% increase should not trigger regression for {}",
            benchmark_name
        );
        println!("  ✓ {}: {}", benchmark_name, analysis.summary());
    }

    // Step 3: Test regression detection (should fail CI)
    println!("Testing regression detection...");

    let regression_test = &stable_benchmarks[0]; // Test first benchmark
    let regression_increase = (regression_test.1 as f64 * 1.25) as u64; // 25% increase

    let bad_result = BenchmarkResult {
        name: regression_test.0.to_string(),
        iterations: 100,
        mean: Duration::from_micros(regression_increase),
        median: Duration::from_micros(regression_increase),
        min: Duration::from_micros(regression_increase.saturating_sub(5)),
        max: Duration::from_micros(regression_increase + 5),
        std_dev: Duration::from_micros(4),
        throughput: Some(1_000_000.0 / regression_increase as f64),
        total_operations: Some(100),
    };

    let bad_measurement = PerformanceMeasurement::from(bad_result);
    let regression_analysis = manager
        .detect_regression(&bad_measurement)
        .expect("Failed to detect regression");

    assert!(
        regression_analysis.is_regression,
        "25% increase should trigger regression"
    );
    println!(
        "  🚨 Regression detected: {}",
        regression_analysis.summary()
    );
    println!("     Details: {}", regression_analysis.details);
}

/// Test criterion output parsing workflow for CI integration
#[test]
fn test_criterion_parsing_ci_workflow() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Simulate realistic criterion output from a CI benchmark run
    let sample_criterion_output = r#"
Running benches/http_benchmarks.rs (target/release/deps/http_benchmarks-abc123)
Benchmarking http/simple_request
Benchmarking http/simple_request: Warming up for 3.0000 s
Benchmarking http/simple_request: Collecting 100 samples in estimated 5.0200 s (10040 iterations)
Benchmarking http/simple_request: Analyzing
http/simple_request        time:   [498.20 μs 502.45 μs 506.91 μs]
                           thrpt:  [1.9725 Kelem/s 1.9902 Kelem/s 2.0072 Kelem/s]

Benchmarking memory/large_allocation
Benchmarking memory/large_allocation: Warming up for 3.0000 s
memory/large_allocation     time:   [1.234 ms 1.267 ms 1.301 ms]
                           thrpt:  [768.64 elem/s 789.26 elem/s 810.37 elem/s]

Benchmarking json/complex_parsing
json/complex_parsing        time:   [89.12 μs 91.34 μs 93.78 μs]
"#;

    // Parse the output and create baselines
    let measurements =
        CriterionCli::parse_and_update_baselines(sample_criterion_output, temp_dir.path())
            .expect("Failed to parse and update baselines");

    assert_eq!(measurements.len(), 3);
    println!(
        "✓ Parsed and stored {} measurements from criterion output",
        measurements.len()
    );

    // Verify that baselines were created correctly
    let manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");
    let baselines = manager.list_baselines();
    assert_eq!(baselines.len(), 3);

    for baseline_name in &baselines {
        let baseline = manager
            .get_baseline(baseline_name)
            .expect("Baseline should exist");
        assert_eq!(baseline.measurements.len(), 1);
        println!(
            "  ✓ Baseline '{}': {} measurements",
            baseline_name,
            baseline.measurements.len()
        );
    }

    // Test regression analysis with the parsed measurements
    let analyses = CriterionCli::analyze_regressions(&measurements, temp_dir.path())
        .expect("Failed to analyze regressions");

    // First run should have no regressions (only one measurement per baseline)
    for analysis in &analyses {
        // Should have insufficient data for regression detection
        assert!(!analysis.is_regression);
        println!("  ✓ {}: {}", analysis.benchmark_name, analysis.summary());
    }
}

/// Test custom configuration for strict CI environments
#[test]
fn test_strict_ci_configuration() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create strict configuration for production CI
    let strict_config = RegressionConfig {
        mean_threshold_percent: 3.0, // Very strict: only 3% increase allowed
        p95_threshold_percent: 5.0,  // 5% for P95
        p99_threshold_percent: 8.0,  // 8% for P99
        min_samples: 5,              // Need at least 5 samples
        use_statistical_test: true,
        significance_level: 0.01, // 99% confidence
    };

    let mut manager = BaselineManager::with_config(temp_dir.path(), strict_config)
        .expect("Failed to create strict manager");

    // Create a tight baseline (very consistent performance)
    let base_performance = 1000; // 1ms
    for _ in 0..10 {
        let result = BenchmarkResult {
            name: "critical/payment_processing".to_string(),
            iterations: 100,
            mean: Duration::from_micros(base_performance),
            median: Duration::from_micros(base_performance),
            min: Duration::from_micros(base_performance - 2),
            max: Duration::from_micros(base_performance + 2),
            std_dev: Duration::from_micros(1),
            throughput: Some(1000.0), // 1k ops/sec
            total_operations: Some(100),
        };

        manager
            .update_baseline(PerformanceMeasurement::from(result))
            .expect("Failed to update baseline");
    }

    println!("✓ Created strict baseline for critical operations");

    // Test that small degradation is caught with strict config
    let small_degradation = BenchmarkResult {
        name: "critical/payment_processing".to_string(),
        iterations: 100,
        mean: Duration::from_micros(1040), // 4% increase - fails strict 3% threshold
        median: Duration::from_micros(1040),
        min: Duration::from_micros(1038),
        max: Duration::from_micros(1042),
        std_dev: Duration::from_micros(1),
        throughput: Some(961.5), // Corresponding throughput decrease
        total_operations: Some(100),
    };

    let measurement = PerformanceMeasurement::from(small_degradation);
    let analysis = manager
        .detect_regression(&measurement)
        .expect("Failed to analyze regression");

    assert!(
        analysis.is_regression,
        "4% increase should fail strict 3% threshold"
    );
    println!(
        "🚨 Strict config correctly detected regression: {}",
        analysis.summary()
    );
}

/// Test baseline persistence and recovery for CI environments
#[test]
fn test_baseline_persistence_and_recovery() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create initial baselines
    {
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        let result = BenchmarkResult {
            name: "persistence/test".to_string(),
            iterations: 100,
            mean: Duration::from_micros(500),
            median: Duration::from_micros(500),
            min: Duration::from_micros(495),
            max: Duration::from_micros(505),
            std_dev: Duration::from_micros(2),
            throughput: Some(2000.0),
            total_operations: Some(100),
        };

        manager
            .update_baseline(PerformanceMeasurement::from(result))
            .expect("Failed to update baseline");
    } // manager goes out of scope, files should be persisted

    // Load baselines in a new manager instance (simulating CI restart)
    {
        let manager = BaselineManager::new(temp_dir.path()).expect("Failed to recreate manager");
        let baselines = manager.list_baselines();
        assert_eq!(baselines.len(), 1);
        assert!(baselines.contains(&"persistence/test".to_string()));

        let baseline = manager
            .get_baseline("persistence/test")
            .expect("Baseline should be loaded");
        assert_eq!(baseline.measurements.len(), 1);

        println!("✓ Successfully recovered baseline after restart");
    }

    // Test export/import for baseline sharing between CI environments
    let export_path = temp_dir.path().join("exported_baseline.json");
    {
        let manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");
        manager
            .export_baseline("persistence/test", &export_path)
            .expect("Failed to export baseline");
    }

    assert!(export_path.exists());

    // Import to a different location
    let import_dir = temp_dir.path().join("import_location");
    std::fs::create_dir_all(&import_dir).expect("Failed to create import directory");

    {
        let mut manager =
            BaselineManager::new(&import_dir).expect("Failed to create import manager");
        let imported_name = manager
            .import_baseline(&export_path)
            .expect("Failed to import baseline");
        assert_eq!(imported_name, "persistence/test");

        let imported_baselines = manager.list_baselines();
        assert_eq!(imported_baselines.len(), 1);

        println!("✓ Successfully exported and imported baseline");
    }
}

/// Test CI regression cli workflow simulation
#[test]
fn test_ci_cli_workflow_simulation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let cli = RegressionCli::with_baseline_path(temp_dir.path());

    // In a real CI environment, you would run:
    // 1. cargo bench > benchmark_output.txt
    // 2. Parse and analyze the output

    // Simulate benchmark output
    let mock_benchmark_output = r#"
Running benches/ci_benchmark.rs
Benchmarking ci/critical_path
ci/critical_path            time:   [245.67 μs 248.12 μs 250.89 μs]
                           thrpt:  [3.9856 Kelem/s 4.0305 Kelem/s 4.0702 Kelem/s]
"#;

    // Create initial baseline
    let baseline_count = cli
        .create_baselines(mock_benchmark_output)
        .expect("Failed to create baselines");
    assert_eq!(baseline_count, 1);
    println!("✓ Created {} baseline(s) via CLI", baseline_count);

    // Test listing baselines
    cli.list_baselines().expect("Failed to list baselines");

    // Add more baseline measurements first (need minimum samples for regression detection)
    for i in 1..15 {
        let slight_variation = i % 3;
        let mock_output = format!(
            r#"
Running benches/ci_benchmark.rs
Benchmarking ci/critical_path
ci/critical_path            time:   [{}.{:02} μs {}.{:02} μs {}.{:02} μs]
"#,
            245 + slight_variation,
            67 + (i % 10),
            248 + slight_variation,
            12 + (i % 10),
            250 + slight_variation,
            89 + (i % 10)
        );

        cli.update_baselines(&mock_output)
            .expect("Failed to update baselines");
    }

    // Now simulate performance regression in CI run
    let regression_output = r#"
Running benches/ci_benchmark.rs
Benchmarking ci/critical_path
ci/critical_path            time:   [298.45 μs 301.23 μs 304.78 μs]
                           thrpt:  [3.2810 Kelem/s 3.3201 Kelem/s 3.3513 Kelem/s]
"#;

    // This should detect regression (20%+ increase)
    let regressions_found = cli
        .detect_regressions_with_exit(regression_output, false)
        .expect("Failed to detect regressions");
    assert!(
        regressions_found,
        "Should detect regression with 20%+ increase"
    );

    println!("✓ CI workflow simulation completed successfully");
}

/// Integration test with actual cargo bench command (if available)
#[test]
#[ignore] // Only run in integration test environments where benchmarks are available
fn test_real_benchmark_integration() {
    // This test would run in a real CI environment with actual benchmarks
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let _cli = RegressionCli::with_baseline_path(temp_dir.path());

    // Check if cargo bench is available and we have benchmarks
    let output = Command::new("cargo")
        .args(["bench", "--bench", "quick_benchmark", "--", "--help"])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            println!("✓ Cargo bench is available, running real integration test");

            // This would run the actual benchmarks and create baselines
            // let benchmark_output = cli.run_benchmarks(Some("quick_benchmark"))
            //     .expect("Failed to run benchmarks");
            //
            // let baseline_count = cli.create_baselines(&benchmark_output)
            //     .expect("Failed to create baselines");
            //
            // println!("✓ Created {} real baselines", baseline_count);
        } else {
            println!("⚠️ Cargo bench not available or no benchmarks found");
        }
    } else {
        println!("⚠️ Could not execute cargo bench command");
    }
}
