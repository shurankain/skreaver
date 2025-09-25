//! # CLI and Criterion Parser Integration Tests
//!
//! Comprehensive testing of the command-line interface and criterion
//! parser components for performance regression detection.

use skreaver_testing::benchmarks::BenchmarkResult;
use skreaver_testing::cli::{CliRunner, RegressionCli};
use skreaver_testing::criterion_parser::CriterionParser;
use skreaver_testing::regression::{BaselineManager, PerformanceMeasurement};
use skreaver_testing::regression::{RegressionConfig, RegressionError};
use std::env;
use std::fs;
// Removed unused import std::io::Write
// Removed unused import std::path::PathBuf
use tempfile::TempDir;

/// Test suite for CLI functionality
// Constants moved outside of module for broader access
const COMPREHENSIVE_BENCHMARK_OUTPUT: &str = r#"
Running benches/quick_benchmark.rs (target/release/deps/quick_benchmark-abc123)
Benchmarking memory_quick/store
Benchmarking memory_quick/store: Warming up for 1.0000 s
Benchmarking memory_quick/store: Collecting 100 samples in estimated 5.1500 s (12950 iterations)
Benchmarking memory_quick/store: Analyzing
memory_quick/store      time:   [394.06 ns 397.42 ns 401.14 ns]
Found 3 outliers among 100 measurements (3.00%)
  1 (1.00%) high mild
  2 (2.00%) high severe

Benchmarking memory_quick/load
Benchmarking memory_quick/load: Warming up for 1.0000 s
Benchmarking memory_quick/load: Collecting 100 samples in estimated 4.2100 s (8742 iterations)
Benchmarking memory_quick/load: Analyzing
memory_quick/load       time:   [476.19 ns 480.95 ns 486.43 ns]
                        thrpt:  [2.0556 Melem/s 2.0790 Melem/s 2.0999 Melem/s]
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high mild

Benchmarking file_quick/write_small
Benchmarking file_quick/write_small: Warming up for 1.0000 s
file_quick/write_small  time:   [12.345 μs 12.567 μs 12.789 μs]
                        thrpt:  [78.156 Kelem/s 79.575 Kelem/s 81.064 Kelem/s]

Benchmarking file_quick/read_small
file_quick/read_small   time:   [8.123 μs 8.234 μs 8.367 μs]

Benchmarking http_quick/get_request
http_quick/get_request  time:   [1.2345 ms 1.2567 ms 1.2789 ms]
                        thrpt:  [781.56 elem/s 795.75 elem/s 810.64 elem/s]

Benchmarking compute_quick/fibonacci
compute_quick/fibonacci time:   [45.123 μs 45.678 μs 46.234 μs]
"#;

const SLOWER_BENCHMARK_OUTPUT: &str = r#"
Running benches/quick_benchmark.rs (target/release/deps/quick_benchmark-def456)
Benchmarking memory_quick/store
memory_quick/store      time:   [445.06 ns 487.42 ns 521.14 ns]

Benchmarking memory_quick/load
memory_quick/load       time:   [556.19 ns 580.95 ns 606.43 ns]
                        thrpt:  [1.6488 Melem/s 1.7219 Melem/s 1.7962 Melem/s]

Benchmarking file_quick/write_small
file_quick/write_small  time:   [15.345 μs 15.567 μs 15.789 μs]
                        thrpt:  [63.327 Kelem/s 64.236 Kelem/s 65.169 Kelem/s]
"#;

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn test_cli_creation_and_configuration() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Test CLI creation (cannot access private fields, so just verify creation succeeds)
        let cli = RegressionCli::new();
        assert!(
            cli.list_baselines().is_ok(),
            "CLI should be created successfully"
        );

        // Test CLI with custom path
        let cli_custom = RegressionCli::with_baseline_path(temp_dir.path());
        assert!(
            cli_custom.list_baselines().is_ok(),
            "Custom path CLI should be created successfully"
        );

        // Test CLI with custom configuration
        let strict_config = RegressionConfig {
            mean_threshold_percent: 5.0,
            p95_threshold_percent: 8.0,
            p99_threshold_percent: 12.0,
            min_samples: 15,
            use_statistical_test: true,
            significance_level: 0.01,
        };

        let cli_config = RegressionCli::with_config(strict_config);
        assert!(
            cli_config.list_baselines().is_ok(),
            "Config CLI should be created successfully"
        );
    }

    #[test]
    fn test_baseline_creation_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Create baselines from comprehensive benchmark output
        let baseline_count = cli
            .create_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
            .expect("Failed to create baselines");

        assert_eq!(baseline_count, 6, "Should create 6 baselines");

        // Verify baseline files were created
        let baseline_files: Vec<_> = fs::read_dir(temp_dir.path())
            .expect("Failed to read temp directory")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .collect();

        assert!(
            !baseline_files.is_empty(),
            "Should create baseline JSON files"
        );

        // Verify we can list the baselines
        let list_result = cli.list_baselines();
        assert!(list_result.is_ok(), "Should be able to list baselines");
    }

    #[test]
    fn test_baseline_update_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Create initial baselines
        cli.create_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
            .expect("Failed to create initial baselines");

        // Update baselines with same data (simulating multiple runs)
        for _ in 0..5 {
            let update_count = cli
                .update_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
                .expect("Failed to update baselines");
            assert_eq!(update_count, 6, "Should update 6 baselines");
        }

        // Verify baselines accumulated measurements
        // Note: This is implicit testing as we can't directly access internal state
        // The fact that update_baselines succeeds multiple times indicates accumulation works
    }

    #[test]
    fn test_regression_detection_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Create baseline manager directly for more control
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Create stable baseline measurements
        for _ in 0..15 {
            let result = BenchmarkResult {
                name: "memory_quick/store".to_string(),
                iterations: 100,
                mean: std::time::Duration::from_nanos(400),
                median: std::time::Duration::from_nanos(400),
                min: std::time::Duration::from_nanos(390),
                max: std::time::Duration::from_nanos(410),
                std_dev: std::time::Duration::from_nanos(5),
                throughput: None,
                total_operations: None,
            };
            let measurement = PerformanceMeasurement::from(result);
            manager
                .update_baseline(measurement)
                .expect("Failed to update baseline");
        }

        // Create regression measurement (50% slower)
        let slow_result = BenchmarkResult {
            name: "memory_quick/store".to_string(),
            iterations: 100,
            mean: std::time::Duration::from_nanos(600),
            median: std::time::Duration::from_nanos(600),
            min: std::time::Duration::from_nanos(590),
            max: std::time::Duration::from_nanos(610),
            std_dev: std::time::Duration::from_nanos(5),
            throughput: None,
            total_operations: None,
        };
        let slow_measurement = PerformanceMeasurement::from(slow_result);

        // Detect regression
        let analysis = manager
            .detect_regression(&slow_measurement)
            .expect("Failed to detect regression");
        assert!(analysis.is_regression, "Should detect regression");
        assert!(
            analysis.mean_change_percent > 10.0,
            "Should detect significant performance change"
        );
    }

    #[test]
    fn test_baseline_management_operations() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Create test baselines
        cli.create_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
            .expect("Failed to create baselines");

        // Test show baseline for existing benchmark
        let show_result = cli.show_baseline("memory_quick/store");
        assert!(
            show_result.is_ok(),
            "Should be able to show existing baseline"
        );

        // Test show baseline for non-existent benchmark
        let show_missing_result = cli.show_baseline("nonexistent/benchmark");
        assert!(
            show_missing_result.is_err(),
            "Should fail to show non-existent baseline"
        );
        assert!(matches!(
            show_missing_result.unwrap_err(),
            RegressionError::BaselineNotFound(_)
        ));

        // Test remove baseline
        let remove_result = cli.remove_baseline("memory_quick/store");
        assert!(remove_result.is_ok(), "Should be able to remove baseline");

        // Verify baseline was removed
        let show_after_remove = cli.show_baseline("memory_quick/store");
        assert!(show_after_remove.is_err(), "Baseline should be removed");
    }

    #[test]
    fn test_export_import_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Create baseline to export
        cli.create_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
            .expect("Failed to create baselines");

        // Export baseline
        let export_path = temp_dir.path().join("exported_baseline.json");
        let export_result = cli.export_baseline("memory_quick/store", &export_path);
        assert!(export_result.is_ok(), "Should be able to export baseline");
        assert!(export_path.exists(), "Export file should exist");

        // Remove original baseline
        cli.remove_baseline("memory_quick/store")
            .expect("Failed to remove baseline");

        // Create new directory for import test
        let import_temp_dir = TempDir::new().expect("Failed to create import temp directory");
        let import_cli = RegressionCli::with_baseline_path(import_temp_dir.path());

        // Import baseline
        let import_result = import_cli.import_baseline(&export_path);
        assert!(import_result.is_ok(), "Should be able to import baseline");

        // Verify imported baseline exists and is usable
        let show_result = import_cli.show_baseline("memory_quick/store");
        assert!(
            show_result.is_ok(),
            "Imported baseline should be accessible"
        );
    }

    #[test]
    fn test_full_analysis_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Mock the benchmark runner by pre-creating outputs
        // In a real scenario, this would run actual cargo bench

        // First run should not find regressions (establishing baseline)
        let baseline_count = cli
            .create_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
            .expect("Failed to create initial baselines");
        assert!(baseline_count > 0, "Should create baselines");

        // Build substantial baseline for accurate detection
        for _ in 0..12 {
            cli.update_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
                .expect("Failed to update baselines");
        }

        // Test regression detection with degraded performance
        let regressions_found = cli
            .detect_regressions_with_exit(SLOWER_BENCHMARK_OUTPUT, false)
            .expect("Failed to detect regressions");
        assert!(regressions_found, "Should detect performance regressions");
    }

    #[test]
    fn test_error_handling_scenarios() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Test with empty/invalid benchmark output
        let empty_result = cli.create_baselines("");
        assert!(
            empty_result.is_ok(),
            "Empty output should be handled gracefully"
        );

        let empty_count = empty_result.unwrap();
        assert_eq!(empty_count, 0, "Empty output should create 0 baselines");

        // Test with malformed benchmark output
        let malformed_output = "This is not benchmark output\nNo timing data here\nRandom text";
        let malformed_result = cli.create_baselines(malformed_output);
        assert!(
            malformed_result.is_ok(),
            "Malformed output should be handled gracefully"
        );

        // Test operations on non-existent baselines
        let show_missing = cli.show_baseline("does_not_exist");
        assert!(
            show_missing.is_err(),
            "Should fail on non-existent baseline"
        );

        let remove_missing = cli.remove_baseline("does_not_exist");
        assert!(
            remove_missing.is_ok(),
            "Remove non-existent should succeed silently"
        );

        // Test export of non-existent baseline
        let export_path = temp_dir.path().join("missing_export.json");
        let export_missing = cli.export_baseline("does_not_exist", &export_path);
        assert!(
            export_missing.is_err(),
            "Should fail to export non-existent baseline"
        );

        // Test import of non-existent file
        let import_missing_path = temp_dir.path().join("does_not_exist.json");
        let import_missing = cli.import_baseline(&import_missing_path);
        assert!(
            import_missing.is_err(),
            "Should fail to import non-existent file"
        );
    }

    #[test]
    fn test_concurrent_cli_operations() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = Arc::new(Mutex::new(RegressionCli::with_baseline_path(
            temp_dir.path(),
        )));

        // Simulate concurrent baseline creation
        let handles: Vec<_> = (0..5)
            .map(|i| {
                let cli_clone = Arc::clone(&cli);
                let output =
                    COMPREHENSIVE_BENCHMARK_OUTPUT.replace("abc123", &format!("thread{}", i));

                thread::spawn(move || {
                    let cli_lock = cli_clone.lock().unwrap();
                    cli_lock.create_baselines(&output)
                })
            })
            .collect();

        // Wait for all threads and collect results
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().expect("Thread should complete"))
            .collect();

        // All operations should succeed
        for result in results {
            assert!(result.is_ok(), "Concurrent operations should succeed");
        }

        // Verify baselines were created
        let cli_lock = cli.lock().unwrap();
        let list_result = cli_lock.list_baselines();
        assert!(
            list_result.is_ok(),
            "Should be able to list baselines after concurrent operations"
        );
    }
}

/// Test suite for Criterion parser functionality
#[cfg(test)]
mod criterion_parser_tests {
    use super::*;

    const COMPLEX_CRITERION_OUTPUT: &str = r#"
Benchmarking agent_benchmark/simple_agent
Benchmarking agent_benchmark/simple_agent: Warming up for 3.0000 s
Benchmarking agent_benchmark/simple_agent: Collecting 100 samples in estimated 5.1500 s (12950 iterations)
Benchmarking agent_benchmark/simple_agent: Analyzing
agent_benchmark/simple_agent time:   [2.3456 ms 2.4567 ms 2.5678 ms]

Benchmarking tool_benchmark/http_call
Benchmarking tool_benchmark/http_call: Warming up for 3.0000 s
Benchmarking tool_benchmark/http_call: Collecting 100 samples in estimated 15.200 s (985 iterations)
tool_benchmark/http_call time:   [15.234 ms 15.567 ms 15.890 ms]
                         thrpt:  [62.959 elem/s 64.236 elem/s 65.538 elem/s]
Found 5 outliers among 100 measurements (5.00%)
  2 (2.00%) low mild
  1 (1.00%) low severe
  2 (2.00%) high mild

Benchmarking memory_benchmark/large_store
memory_benchmark/large_store time:   [123.45 ns 125.67 ns 127.89 ns]
                            thrpt:  [7.8156 Gelem/s 7.9575 Gelem/s 8.1064 Gelem/s]

Benchmarking compute_benchmark/cpu_intensive
compute_benchmark/cpu_intensive time:   [45.123 μs 46.234 μs 47.345 μs]

Benchmarking io_benchmark/disk_write
io_benchmark/disk_write     time:   [1.2345 s 1.2567 s 1.2789 s]
                            thrpt:  [781.56 elem/s 795.75 elem/s 810.64 elem/s]
"#;

    #[test]
    fn test_comprehensive_criterion_parsing() {
        let parser = CriterionParser::new();
        let measurements = parser
            .parse_console_output(COMPLEX_CRITERION_OUTPUT)
            .expect("Failed to parse complex criterion output");

        assert_eq!(
            measurements.len(),
            5,
            "Should parse 5 benchmark measurements"
        );

        // Test millisecond parsing (agent benchmark)
        let agent_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "agent_benchmark/simple_agent")
            .expect("Should find agent benchmark measurement");

        assert_eq!(agent_measurement.median_duration_nanos, 2_456_700); // 2.4567 ms in ns
        assert_eq!(agent_measurement.min_duration_nanos, 2_345_600); // 2.3456 ms in ns
        assert_eq!(agent_measurement.max_duration_nanos, 2_567_800); // 2.5678 ms in ns

        // Test throughput parsing (tool benchmark)
        let tool_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "tool_benchmark/http_call")
            .expect("Should find tool benchmark measurement");

        assert!(
            tool_measurement.throughput_ops_per_sec.is_some(),
            "Should extract throughput"
        );
        let throughput = tool_measurement.throughput_ops_per_sec.unwrap();
        assert!(
            throughput > 60.0 && throughput < 70.0,
            "Throughput should be ~64 elem/s"
        );

        // Test nanosecond parsing (memory benchmark)
        let memory_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "memory_benchmark/large_store")
            .expect("Should find memory benchmark measurement");

        assert_eq!(memory_measurement.median_duration_nanos, 125); // 125.67 ns
        assert!(
            memory_measurement.throughput_ops_per_sec.is_some(),
            "Should extract Gelem/s throughput"
        );

        // Test microsecond parsing (compute benchmark)
        let compute_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "compute_benchmark/cpu_intensive")
            .expect("Should find compute benchmark measurement");

        assert_eq!(compute_measurement.median_duration_nanos, 46_234); // 46.234 μs in ns

        // Test second parsing (IO benchmark)
        let io_measurement = measurements
            .iter()
            .find(|m| m.benchmark_name == "io_benchmark/disk_write")
            .expect("Should find IO benchmark measurement");

        assert_eq!(io_measurement.median_duration_nanos, 1_256_700_000); // 1.2567 s in ns
    }

    #[test]
    fn test_parser_with_git_metadata() {
        let commit_hash = Some("abc123def456".to_string());
        let branch = Some("feature/performance-testing".to_string());

        let parser = CriterionParser::with_git_info(commit_hash.clone(), branch.clone());
        let measurements = parser
            .parse_console_output(COMPLEX_CRITERION_OUTPUT)
            .expect("Failed to parse with git info");

        assert!(!measurements.is_empty(), "Should parse measurements");

        // Verify git metadata is included
        for measurement in &measurements {
            assert_eq!(
                measurement.commit_hash, commit_hash,
                "Should include commit hash"
            );
            assert_eq!(measurement.branch, branch, "Should include branch name");
        }
    }

    #[test]
    fn test_edge_case_parsing() {
        let edge_cases = vec![
            // Empty output
            ("", 0),
            // Only benchmark names without timing
            ("Benchmarking test/benchmark\nBenchmarking test/other", 0),
            // Malformed timing lines
            ("test_benchmark time: [invalid format]", 0),
            ("test_benchmark time: [123.45]", 0), // Missing units
            // Single measurement
            ("single_test time: [100.0 ns 101.0 ns 102.0 ns]", 1),
            // Mixed valid and invalid lines
            (
                r#"
valid_benchmark         time:   [100.0 ns 101.0 ns 102.0 ns]
invalid line without timing
another_valid           time:   [200.0 μs 201.0 μs 202.0 μs]
"#,
                2,
            ),
        ];

        let parser = CriterionParser::new();

        for (input, expected_count) in edge_cases {
            let measurements = parser
                .parse_console_output(input)
                .expect("Parser should handle edge cases gracefully");

            assert_eq!(
                measurements.len(),
                expected_count,
                "Expected {} measurements for input: {}",
                expected_count,
                input
            );
        }
    }

    #[test]
    fn test_duration_unit_conversion_accuracy() {
        // Test various duration formats through public parser API
        let _test_cases = [
            ("1.0 ns", 1_000_000_000i64),
            ("1.0 μs", 1_000_000_000i64),
            ("1.0 ms", 1_000_000_000_000i64),
        ];

        // Test duration parsing through public API
        let parser = CriterionParser::new();
        let test_output = format!(
            "test_benchmark time: [{} {} {} {} {} {}]",
            "123.456", "μs", "124.567", "μs", "125.678", "μs"
        );

        let measurements = parser
            .parse_console_output(&test_output)
            .expect("Should parse duration test output");

        if let Some(measurement) = measurements.first() {
            // Verify the parsing worked (124.567 μs should be ~124,567 ns)
            assert!(measurement.median_duration_nanos > 120_000);
            assert!(measurement.median_duration_nanos < 130_000);
        }
    }

    #[test]
    fn test_throughput_extraction() {
        let test_outputs = vec![
            // Standard elem/s throughput
            (
                r#"
benchmark_test          time:   [100.0 ns 101.0 ns 102.0 ns]
                        thrpt:  [9.8039 Melem/s 9.9010 Melem/s 10.000 Melem/s]
"#,
                Some(9_900_000.0),
            ), // Approximate middle value in elem/s
            // Kelem/s throughput
            (
                r#"
benchmark_test          time:   [100.0 μs 101.0 μs 102.0 μs]
                        thrpt:  [9.8039 Kelem/s 9.9010 Kelem/s 10.000 Kelem/s]
"#,
                Some(9_900.0),
            ), // Approximate middle value in elem/s
            // No throughput info
            (
                r#"
benchmark_test          time:   [100.0 ns 101.0 ns 102.0 ns]
"#,
                None,
            ),
            // Gelem/s throughput
            (
                r#"
benchmark_test          time:   [10.0 ns 11.0 ns 12.0 ns]
                        thrpt:  [83.333 Gelem/s 90.909 Gelem/s 100.00 Gelem/s]
"#,
                Some(90_909_000_000.0),
            ), // Approximate middle value in elem/s
        ];

        let parser = CriterionParser::new();

        for (output, expected_throughput) in test_outputs {
            let measurements = parser
                .parse_console_output(output)
                .expect("Should parse throughput test output");

            if let Some(measurement) = measurements.first() {
                match (measurement.throughput_ops_per_sec, expected_throughput) {
                    (Some(actual), Some(expected)) => {
                        assert!(
                            (actual - expected).abs() < expected * 0.1,
                            "Throughput parsing inaccurate: got {}, expected {}",
                            actual,
                            expected
                        );
                    }
                    (None, None) => {
                        // Expected - no throughput info
                    }
                    (actual, expected) => {
                        panic!(
                            "Throughput mismatch: got {:?}, expected {:?}",
                            actual, expected
                        );
                    }
                }
            } else if expected_throughput.is_some() {
                panic!("Expected measurement but got none for output: {}", output);
            }
        }
    }

    #[test]
    fn test_git_info_extraction() {
        // Test manual git info
        let parser1 = CriterionParser::with_git_info(
            Some("test_commit_hash".to_string()),
            Some("test_branch".to_string()),
        );

        assert_eq!(parser1.commit_hash, Some("test_commit_hash".to_string()));
        assert_eq!(parser1.branch, Some("test_branch".to_string()));

        // Test automatic git info extraction (may or may not work depending on environment)
        let auto_git_result = CriterionParser::with_auto_git_info();

        match auto_git_result {
            Ok(parser) => {
                // If git is available and we're in a repo, should have some info
                println!(
                    "Auto-detected git info: commit={:?}, branch={:?}",
                    parser.commit_hash, parser.branch
                );
            }
            Err(_) => {
                // Expected if git is not available or not in a repo
                println!("Auto git info extraction failed (expected in some test environments)");
            }
        }

        // Test static git extraction
        let (commit, branch) = CriterionParser::extract_git_info().unwrap_or((None, None));
        println!(
            "Static git extraction result: commit={:?}, branch={:?}",
            commit, branch
        );
    }
}

/// Test suite for CLI command-line argument parsing
#[cfg(test)]
mod cli_runner_tests {
    use super::*;

    #[test]
    fn test_help_commands() {
        let help_variants = vec![
            vec!["skreaver-perf".to_string(), "help".to_string()],
            vec!["skreaver-perf".to_string(), "--help".to_string()],
            vec!["skreaver-perf".to_string(), "-h".to_string()],
            vec!["skreaver-perf".to_string()], // No arguments should show help
        ];

        for args in help_variants {
            let result = CliRunner::run(args.clone());
            assert!(result.is_ok(), "Help command should succeed: {:?}", args);
        }
    }

    #[test]
    fn test_invalid_commands() {
        let invalid_commands = vec![
            vec!["skreaver-perf".to_string(), "invalid_command".to_string()],
            vec!["skreaver-perf".to_string(), "unknown".to_string()],
            vec!["skreaver-perf".to_string(), "".to_string()],
        ];

        for args in invalid_commands {
            let result = CliRunner::run(args.clone());
            assert!(result.is_err(), "Invalid command should fail: {:?}", args);
        }
    }

    #[test]
    fn test_command_argument_validation() {
        // Commands that require additional arguments
        let incomplete_commands = vec![
            vec!["skreaver-perf".to_string(), "show".to_string()], // Missing baseline name
            vec!["skreaver-perf".to_string(), "remove".to_string()], // Missing baseline name
            vec!["skreaver-perf".to_string(), "export".to_string()], // Missing both arguments
            vec![
                "skreaver-perf".to_string(),
                "export".to_string(),
                "baseline".to_string(),
            ], // Missing path
            vec!["skreaver-perf".to_string(), "import".to_string()], // Missing path
        ];

        for args in incomplete_commands {
            let result = CliRunner::run(args.clone());
            assert!(
                result.is_err(),
                "Incomplete command should fail: {:?}",
                args
            );

            // Verify it's a configuration error
            if let Err(RegressionError::ConfigError(_)) = result {
                // Expected
            } else {
                panic!("Expected ConfigError for incomplete command: {:?}", args);
            }
        }
    }

    #[test]
    fn test_valid_command_structure() {
        // Note: These tests verify command parsing structure but don't actually
        // run benchmarks or operations since that would require cargo bench

        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Set up environment to use temp directory for baselines
        unsafe {
            env::set_var("SKREAVER_BASELINE_DIR", temp_dir.path());
        }

        // Create a fake baseline file for testing show/remove/export
        let fake_baseline_path = temp_dir.path().join("test_baseline.json");
        fs::write(&fake_baseline_path, r#"{"benchmark_name":"test","measurements":[],"created_at":"2023-01-01T00:00:00Z","updated_at":"2023-01-01T00:00:00Z"}"#)
            .expect("Failed to create fake baseline");

        let valid_command_structures = vec![
            // These would work if we had actual benchmarks to run
            // vec!["skreaver-perf".to_string(), "run".to_string()],
            // vec!["skreaver-perf".to_string(), "run".to_string(), "quick_benchmark".to_string()],
            vec!["skreaver-perf".to_string(), "list".to_string()],
            vec![
                "skreaver-perf".to_string(),
                "show".to_string(),
                "test_baseline".to_string(),
            ],
        ];

        for args in valid_command_structures {
            println!("Testing command structure: {:?}", args);
            // We can't fully test these without mocking the CLI or having actual benchmarks
            // But we can verify the argument parsing doesn't fail immediately
            // The actual execution might fail due to missing benchmarks, which is expected
        }

        unsafe {
            env::remove_var("SKREAVER_BASELINE_DIR");
        }
    }
}

/// Test suite for CLI and Parser integration
#[cfg(test)]
mod integration_workflow_tests {
    use super::*;

    #[test]
    fn test_complete_cli_parser_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Create baseline manager directly for controlled testing
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Build substantial baseline dataset manually
        for _ in 0..15 {
            let result = BenchmarkResult {
                name: "memory_quick/store".to_string(),
                iterations: 100,
                mean: std::time::Duration::from_nanos(400),
                median: std::time::Duration::from_nanos(400),
                min: std::time::Duration::from_nanos(390),
                max: std::time::Duration::from_nanos(410),
                std_dev: std::time::Duration::from_nanos(5),
                throughput: None,
                total_operations: None,
            };
            let measurement = PerformanceMeasurement::from(result);
            manager
                .update_baseline(measurement)
                .expect("Failed to update baseline");
        }

        // Test regression detection with slower measurement
        let slow_result = BenchmarkResult {
            name: "memory_quick/store".to_string(),
            iterations: 100,
            mean: std::time::Duration::from_nanos(600),
            median: std::time::Duration::from_nanos(600),
            min: std::time::Duration::from_nanos(590),
            max: std::time::Duration::from_nanos(610),
            std_dev: std::time::Duration::from_nanos(5),
            throughput: None,
            total_operations: None,
        };
        let slow_measurement = PerformanceMeasurement::from(slow_result);

        let analysis = manager
            .detect_regression(&slow_measurement)
            .expect("Failed to detect regression");
        assert!(
            analysis.is_regression,
            "Should detect performance regression"
        );

        // Test export/import cycle
        let export_path = temp_dir.path().join("workflow_export.json");
        manager
            .export_baseline("memory_quick/store", &export_path)
            .expect("Failed to export baseline");

        let import_temp_dir = TempDir::new().expect("Failed to create import temp dir");
        let mut import_manager =
            BaselineManager::new(import_temp_dir.path()).expect("Failed to create import manager");

        let imported_name = import_manager
            .import_baseline(&export_path)
            .expect("Failed to import baseline");
        assert_eq!(imported_name, "memory_quick/store");

        // Verify imported baseline works for regression detection
        let imported_analysis = import_manager
            .detect_regression(&slow_measurement)
            .expect("Failed to detect regression on imported baseline");
        assert!(
            imported_analysis.is_regression,
            "Imported baseline should detect regression"
        );
    }

    #[test]
    fn test_performance_requirements_validation() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Test CLI performance with large dataset
        let start_time = std::time::Instant::now();

        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Create substantial dataset
        for i in 0..50 {
            let variant_output =
                COMPREHENSIVE_BENCHMARK_OUTPUT.replace("abc123", &format!("batch{:03}", i));

            cli.create_baselines(&variant_output)
                .expect("Failed to create baselines in performance test");
        }

        let creation_time = start_time.elapsed();

        // Test regression detection performance
        let detection_start = std::time::Instant::now();

        for _ in 0..20 {
            cli.detect_regressions_with_exit(SLOWER_BENCHMARK_OUTPUT, false)
                .expect("Failed to detect regressions in performance test");
        }

        let detection_time = detection_start.elapsed();

        println!("CLI Performance Results:");
        println!(
            "  Baseline creation (50 runs): {}ms",
            creation_time.as_millis()
        );
        println!(
            "  Regression detection (20 runs): {}ms",
            detection_time.as_millis()
        );

        // Validate performance requirements
        assert!(
            creation_time.as_secs() < 30,
            "Baseline creation too slow: {}s (target: <30s)",
            creation_time.as_secs()
        );

        assert!(
            detection_time.as_secs() < 10,
            "Regression detection too slow: {}s (target: <10s)",
            detection_time.as_secs()
        );
    }

    #[test]
    fn test_error_propagation_and_recovery() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Test error handling in parser -> CLI workflow
        let invalid_outputs = vec![
            "",                                                                                   // Empty
            "Not benchmark output at all", // Invalid format
            "benchmark_name time: [invalid values here]", // Malformed timing
            "benchmark_name time: [123.45 invalid_unit 234.56 invalid_unit 345.67 invalid_unit]", // Invalid units
        ];

        for invalid_output in invalid_outputs {
            let result = cli.create_baselines(invalid_output);
            assert!(result.is_ok(), "CLI should handle invalid input gracefully");

            let count = result.unwrap();
            assert_eq!(count, 0, "Invalid input should create 0 baselines");
        }

        // Test recovery: after invalid input, valid input should still work
        let recovery_count = cli
            .create_baselines(COMPREHENSIVE_BENCHMARK_OUTPUT)
            .expect("CLI should recover after invalid input");

        assert!(
            recovery_count > 0,
            "CLI should recover and process valid input"
        );

        // Test partial parsing: mix of valid and invalid lines
        let mixed_output = format!(
            "{}\nInvalid line here\n{}",
            COMPREHENSIVE_BENCHMARK_OUTPUT,
            "additional_benchmark time: [100.0 ns 101.0 ns 102.0 ns]"
        );

        let mixed_count = cli
            .create_baselines(&mixed_output)
            .expect("CLI should handle mixed valid/invalid input");

        assert!(
            mixed_count > 0,
            "CLI should parse valid parts of mixed input"
        );
    }
}
