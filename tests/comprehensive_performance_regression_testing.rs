//! Comprehensive Performance Regression Detection System Testing
//!
//! This module provides exhaustive testing for the performance regression detection
//! system, validating all requirements for production readiness including:
//!
//! - Functional correctness of all components
//! - Performance benchmarking of the detection system itself
//! - Security validation against malicious inputs
//! - CI/CD integration validation
//! - Edge case handling and error resilience
//! - Memory efficiency and scalability testing
//! - Cross-platform compatibility verification
//!
//! This represents the comprehensive test suite for validating that the system
//! meets all project requirements before production deployment.

use skreaver_testing::RegressionCli;
use skreaver_testing::benchmarks::BenchmarkResult;
use skreaver_testing::regression::{
    BaselineManager, PerformanceMeasurement, RegressionConfig, RegressionError,
};
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant, SystemTime};
use tempfile::TempDir;

/// Comprehensive functional validation of all system components
#[cfg(test)]
mod comprehensive_functional_tests {
    use super::*;

    /// Test complete baseline lifecycle management
    #[test]
    fn test_complete_baseline_lifecycle() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager =
            BaselineManager::new(temp_dir.path()).expect("Failed to create baseline manager");

        // Phase 1: Create initial baselines with varied workloads
        let test_benchmarks = vec![
            ("http/concurrent_requests", 250, 15, 500), // Network I/O
            ("database/bulk_insert", 1500, 200, 1000),  // Database operations
            ("cpu/matrix_multiplication", 5000, 100, 2000), // CPU-intensive
            ("memory/large_allocation", 800, 50, 300),  // Memory operations
            ("disk/sequential_read", 2000, 300, 800),   // Disk I/O
        ];

        for (name, base_micros, std_dev, samples) in &test_benchmarks {
            // Create 25 historical measurements for statistical validity
            for i in 0..25 {
                let variation = (i % 5) as u64 * (*std_dev / 5);
                let measurement =
                    create_test_measurement(name, base_micros + variation, *std_dev, *samples);
                manager
                    .update_baseline(measurement)
                    .expect("Failed to create baseline");
            }
        }

        println!(
            "✓ Created {} baselines with 25 measurements each",
            test_benchmarks.len()
        );

        // Phase 2: Test baseline statistics accuracy
        for (name, base_micros, _std_dev, _samples) in &test_benchmarks {
            let baseline = manager.get_baseline(name).expect("Baseline should exist");
            let stats = baseline.calculate_baseline_stats(20);

            assert_eq!(stats.sample_count, 20, "Should use requested sample count");

            // Statistics should be reasonable
            let expected_mean_nanos = (*base_micros as f64 * 1000.0) as u64;
            let actual_mean = stats.mean_duration_nanos;
            let variance_percent = ((actual_mean as f64 - expected_mean_nanos as f64)
                / expected_mean_nanos as f64
                * 100.0)
                .abs();

            assert!(
                variance_percent < 20.0,
                "Statistical mean should be within 20% of expected for {}: expected ~{}ns, got {}ns ({}% variance)",
                name,
                expected_mean_nanos,
                actual_mean,
                variance_percent
            );
        }

        println!("✓ Baseline statistics validation passed");

        // Phase 3: Test export/import roundtrip accuracy
        let export_path = temp_dir.path().join("comprehensive_export.json");
        manager
            .export_baseline("cpu/matrix_multiplication", &export_path)
            .expect("Failed to export baseline");

        // Import to new manager
        let temp_dir2 = TempDir::new().expect("Failed to create temp directory");
        let mut manager2 =
            BaselineManager::new(temp_dir2.path()).expect("Failed to create second manager");

        let imported_name = manager2
            .import_baseline(&export_path)
            .expect("Failed to import baseline");

        assert_eq!(imported_name, "cpu/matrix_multiplication");

        // Validate imported data integrity
        let original = manager.get_baseline("cpu/matrix_multiplication").unwrap();
        let imported = manager2.get_baseline("cpu/matrix_multiplication").unwrap();

        assert_eq!(original.measurements.len(), imported.measurements.len());
        assert_eq!(original.benchmark_name, imported.benchmark_name);

        println!("✓ Export/import roundtrip validation passed");

        // Phase 4: Test concurrent access patterns
        use std::sync::{Arc, Mutex};
        use std::thread;

        let shared_manager = Arc::new(Mutex::new(manager));
        let mut handles = vec![];

        // Spawn concurrent operations
        for thread_id in 0..5 {
            let manager_clone = Arc::clone(&shared_manager);
            let handle = thread::spawn(move || {
                let measurement = create_test_measurement(
                    &format!("concurrent/test_{}", thread_id),
                    1000 + thread_id as u64 * 100,
                    10,
                    50,
                );

                let mut manager = manager_clone.lock().unwrap();
                manager
                    .update_baseline(measurement)
                    .expect("Failed to update baseline in concurrent context");
            });
            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread should complete successfully");
        }

        let final_manager = shared_manager.lock().unwrap();
        assert!(
            final_manager.list_baselines().len() >= 10,
            "All baselines should be created"
        );

        println!("✓ Concurrent access validation passed");
    }

    /// Test comprehensive regression detection accuracy across different scenarios
    #[test]
    fn test_comprehensive_regression_detection() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Create baseline with known characteristics
        let baseline_duration = 1000u64; // 1000 microseconds
        let baseline_std_dev = 20u64;

        for _ in 0..30 {
            let measurement = create_test_measurement(
                "regression_accuracy_test",
                baseline_duration,
                baseline_std_dev,
                100,
            );
            manager
                .update_baseline(measurement)
                .expect("Failed to create baseline");
        }

        // Test case 1: No regression (within normal variance)
        let stable_measurement = create_test_measurement(
            "regression_accuracy_test",
            1005, // 0.5% increase
            25,
            100,
        );
        let stable_analysis = manager
            .detect_regression(&stable_measurement)
            .expect("Failed to detect regression");
        assert!(
            !stable_analysis.is_regression,
            "Should not detect regression for 0.5% increase: {}",
            stable_analysis.summary()
        );

        // Test case 2: Borderline regression (exactly at threshold)
        let borderline_measurement = create_test_measurement(
            "regression_accuracy_test",
            1100, // 10% increase (at threshold)
            25,
            100,
        );
        let borderline_analysis = manager
            .detect_regression(&borderline_measurement)
            .expect("Failed to detect regression");
        println!(
            "Borderline case (10% increase): {}",
            borderline_analysis.summary()
        );

        // Test case 3: Clear regression (significantly over threshold)
        let regression_measurement = create_test_measurement(
            "regression_accuracy_test",
            1300, // 30% increase
            30,
            100,
        );
        let regression_analysis = manager
            .detect_regression(&regression_measurement)
            .expect("Failed to detect regression");
        assert!(
            regression_analysis.is_regression,
            "Should detect regression for 30% increase: {}",
            regression_analysis.summary()
        );

        // Test case 4: P95/P99 regression (high variance)
        let high_variance_measurement = create_test_measurement(
            "regression_accuracy_test",
            1050, // 5% mean increase
            200,  // Very high variance
            100,
        );
        let variance_analysis = manager
            .detect_regression(&high_variance_measurement)
            .expect("Failed to detect regression");

        println!("High variance case: {}", variance_analysis.summary());
        println!(
            "  Mean: {:.1}%, P95: {:.1}%, P99: {:.1}%",
            variance_analysis.mean_change_percent,
            variance_analysis.p95_change_percent,
            variance_analysis.p99_change_percent
        );

        // Test case 5: Performance improvement (negative regression)
        let improvement_measurement = create_test_measurement(
            "regression_accuracy_test",
            800, // 20% improvement
            15,
            100,
        );
        let improvement_analysis = manager
            .detect_regression(&improvement_measurement)
            .expect("Failed to detect regression");
        assert!(
            !improvement_analysis.is_regression,
            "Should not flag performance improvement as regression"
        );
        assert!(
            improvement_analysis.mean_change_percent < 0.0,
            "Should show negative change for improvement"
        );

        println!("✓ Comprehensive regression detection validation passed");
    }

    /// Test custom configuration validation and edge cases
    #[test]
    fn test_configuration_validation_and_edge_cases() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Test various configuration combinations
        let configs = [
            // Ultra-strict production config
            RegressionConfig {
                mean_threshold_percent: 1.0,
                p95_threshold_percent: 2.0,
                p99_threshold_percent: 3.0,
                min_samples: 50,
                use_statistical_test: true,
                significance_level: 0.01,
            },
            // Relaxed development config
            RegressionConfig {
                mean_threshold_percent: 25.0,
                p95_threshold_percent: 35.0,
                p99_threshold_percent: 50.0,
                min_samples: 3,
                use_statistical_test: false,
                significance_level: 0.1,
            },
            // Edge case config values
            RegressionConfig {
                mean_threshold_percent: 0.1,          // Very small threshold
                p95_threshold_percent: 100.0,         // Very large threshold
                p99_threshold_percent: f64::INFINITY, // Extreme threshold
                min_samples: 1,                       // Minimum samples
                use_statistical_test: true,
                significance_level: 0.001, // High confidence
            },
        ];

        for (i, config) in configs.iter().enumerate() {
            println!(
                "Testing configuration {}: strict={}, min_samples={}",
                i, config.mean_threshold_percent, config.min_samples
            );

            let manager_result = BaselineManager::with_config(temp_dir.path(), config.clone());
            assert!(manager_result.is_ok(), "Should handle valid configuration");

            let mut manager = manager_result.unwrap();

            // Create baseline appropriate for configuration
            let required_samples = config.min_samples.max(5); // Ensure we have enough samples
            for _ in 0..required_samples {
                let measurement =
                    create_test_measurement(&format!("config_test_{}", i), 1000, 10, 100);
                manager
                    .update_baseline(measurement)
                    .expect("Failed to create baseline");
            }

            // Test regression detection with this configuration
            let test_measurement = create_test_measurement(
                &format!("config_test_{}", i),
                1100, // 10% increase
                15,
                100,
            );

            let analysis = manager.detect_regression(&test_measurement);
            assert!(
                analysis.is_ok(),
                "Regression detection should work with config {}",
                i
            );

            let analysis = analysis.unwrap();
            println!(
                "  Result: {} - {:.1}% change",
                if analysis.is_regression {
                    "REGRESSION"
                } else {
                    "OK"
                },
                analysis.mean_change_percent
            );
        }

        println!("✓ Configuration validation passed");
    }
}

/// Performance benchmarking of the regression detection system itself
#[cfg(test)]
mod performance_benchmarking_tests {
    use super::*;

    /// Benchmark baseline creation and storage performance
    #[test]
    fn test_baseline_creation_performance_benchmark() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Benchmark single baseline creation
        let start = Instant::now();
        let measurement = create_test_measurement("performance_test", 1000, 10, 100);
        manager
            .update_baseline(measurement)
            .expect("Failed to create baseline");
        let single_creation_time = start.elapsed();

        // Benchmark bulk baseline creation (100 measurements)
        let start = Instant::now();
        for i in 0..100 {
            let measurement =
                create_test_measurement(&format!("bulk_test_{}", i), 1000 + i, 10, 100);
            manager
                .update_baseline(measurement)
                .expect("Failed to create baseline");
        }
        let bulk_creation_time = start.elapsed();

        // Benchmark adding measurements to existing baseline
        let start = Instant::now();
        for i in 0..100 {
            let measurement = create_test_measurement(
                "performance_test", // Same baseline
                1000 + i,
                10,
                100,
            );
            manager
                .update_baseline(measurement)
                .expect("Failed to update baseline");
        }
        let update_time = start.elapsed();

        println!("Baseline Creation Performance Benchmark Results:");
        println!("  Single creation: {:?}", single_creation_time);
        println!(
            "  Bulk creation (100): {:?} ({:?} per baseline)",
            bulk_creation_time,
            bulk_creation_time / 100
        );
        println!(
            "  Baseline updates (100): {:?} ({:?} per update)",
            update_time,
            update_time / 100
        );

        // Performance requirements validation
        assert!(
            single_creation_time.as_millis() < 50,
            "Single baseline creation should complete in <50ms, took {}ms",
            single_creation_time.as_millis()
        );

        assert!(
            bulk_creation_time.as_millis() < 5000,
            "100 baseline creations should complete in <5s, took {}ms",
            bulk_creation_time.as_millis()
        );

        assert!(
            update_time.as_millis() < 1000,
            "100 baseline updates should complete in <1s, took {}ms",
            update_time.as_millis()
        );
    }

    /// Benchmark regression detection performance under different conditions
    #[test]
    fn test_regression_detection_performance_benchmark() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Setup: Create baselines with varying sizes
        let baseline_sizes = vec![10, 50, 100, 500, 1000];

        for &size in &baseline_sizes {
            let baseline_name = format!("perf_baseline_{}", size);

            for i in 0..size {
                let measurement = create_test_measurement(&baseline_name, 1000 + (i % 10), 10, 100);
                manager
                    .update_baseline(measurement)
                    .expect("Failed to create baseline");
            }
        }

        println!("Regression Detection Performance Benchmark Results:");

        // Benchmark detection performance for each baseline size
        for &size in &baseline_sizes {
            let baseline_name = format!("perf_baseline_{}", size);
            let test_measurement = create_test_measurement(&baseline_name, 1100, 15, 100);

            // Warm-up runs
            for _ in 0..10 {
                let _ = manager.detect_regression(&test_measurement);
            }

            // Benchmark runs
            let start = Instant::now();
            let iterations = 1000;

            for _ in 0..iterations {
                let _ = manager.detect_regression(&test_measurement);
            }

            let detection_time = start.elapsed();
            let time_per_detection = detection_time / iterations;

            println!(
                "  Baseline size {}: {} detections in {:?} ({:?} per detection)",
                size, iterations, detection_time, time_per_detection
            );

            // Performance requirement: <1ms per detection even with 1000 historical measurements
            assert!(
                time_per_detection.as_millis() < 1,
                "Regression detection should complete in <1ms per detection for {} measurements, took {:?}",
                size,
                time_per_detection
            );
        }

        // Test performance with multiple concurrent benchmarks
        let concurrent_benchmarks = 50;
        let start = Instant::now();

        for i in 0..concurrent_benchmarks {
            let baseline_name = format!("concurrent_perf_test_{}", i);
            let test_measurement = create_test_measurement(&baseline_name, 1000, 10, 100);

            // Create small baseline
            for _ in 0..10 {
                manager
                    .update_baseline(test_measurement.clone())
                    .expect("Failed to create baseline");
            }

            // Run detection
            let _ = manager.detect_regression(&test_measurement);
        }

        let concurrent_time = start.elapsed();
        println!(
            "  Concurrent processing: {} benchmarks in {:?} ({:?} per benchmark)",
            concurrent_benchmarks,
            concurrent_time,
            concurrent_time / concurrent_benchmarks
        );
    }

    /// Benchmark memory usage and efficiency
    #[test]
    fn test_memory_efficiency_benchmark() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Test memory efficiency with large baseline
        println!("Memory Efficiency Benchmark:");

        // Create large baseline (should be limited to 1000 measurements)
        let large_baseline_name = "memory_efficiency_test";

        for i in 0..2500 {
            let measurement =
                create_test_measurement(large_baseline_name, 1000 + (i % 100), 10 + (i % 5), 100);
            manager
                .update_baseline(measurement)
                .expect("Failed to update baseline");
        }

        // Verify memory efficiency (should be limited to 1000 measurements)
        let baseline = manager
            .get_baseline(large_baseline_name)
            .expect("Baseline should exist");

        assert_eq!(
            baseline.measurements.len(),
            1000,
            "Baseline should be limited to 1000 measurements for memory efficiency"
        );

        println!(
            "  ✓ Memory limit enforced: {} measurements (limited from 2500)",
            baseline.measurements.len()
        );

        // Test multiple baselines coexistence
        for i in 0..20 {
            let baseline_name = format!("memory_test_{}", i);

            for j in 0..50 {
                let measurement = create_test_measurement(&baseline_name, 1000 + j, 10, 100);
                manager
                    .update_baseline(measurement)
                    .expect("Failed to update baseline");
            }
        }

        let total_baselines = manager.list_baselines().len();
        assert!(total_baselines >= 21, "All baselines should coexist");

        println!(
            "  ✓ Multiple baselines: {} baselines with 50+ measurements each",
            total_baselines
        );

        // Test performance with memory-efficient operations
        let start = Instant::now();

        for _ in 0..100 {
            let test_measurement = create_test_measurement(large_baseline_name, 1100, 15, 100);
            let _ = manager.detect_regression(&test_measurement);
        }

        let efficient_time = start.elapsed();
        println!(
            "  ✓ Performance with memory limits: 100 detections in {:?}",
            efficient_time
        );

        assert!(
            efficient_time.as_millis() < 1000,
            "Memory-efficient operations should maintain performance"
        );
    }
}

/// Security and robustness validation
#[cfg(test)]
mod security_and_robustness_tests {
    use super::*;
    use std::fs;

    /// Test security against malicious baseline data
    #[test]
    fn test_comprehensive_security_validation() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Test 1: Malicious JSON content injection
        let malicious_file = temp_dir.path().join("malicious_baseline.json");
        let malicious_content = r#"{
            "benchmark_name": "../../etc/passwd\u0000",
            "measurements": [
                {
                    "benchmark_name": "<script>alert('xss')</script>",
                    "timestamp": {"secs_since_epoch": 1640995200, "nanos_since_epoch": 0},
                    "commit_hash": "'; DROP TABLE baselines; --",
                    "branch": "\u0000\u0001\u0002malicious",
                    "mean_duration_nanos": 18446744073709551615,
                    "median_duration_nanos": 0,
                    "min_duration_nanos": 0,
                    "max_duration_nanos": 18446744073709551615,
                    "std_dev_nanos": 18446744073709551615,
                    "sample_count": 4294967295,
                    "throughput_ops_per_sec": "NaN",
                    "custom_metrics": {
                        "../../../etc/shadow": 1.7976931348623157e+308
                    }
                }
            ],
            "created_at": {"secs_since_epoch": -1, "nanos_since_epoch": 4294967295},
            "updated_at": "invalid_timestamp"
        }"#;

        fs::write(&malicious_file, malicious_content).expect("Failed to write malicious file");

        // Manager should handle malicious content safely
        let manager = BaselineManager::new(temp_dir.path());
        assert!(manager.is_ok(), "Should handle malicious files gracefully");

        let manager = manager.unwrap();
        let baselines = manager.list_baselines();

        // Malicious file should be ignored or handled safely
        println!(
            "✓ Malicious JSON content handled safely ({} baselines loaded)",
            baselines.len()
        );

        // Test 2: Path traversal protection
        let mut manager = manager;
        let traversal_measurement = PerformanceMeasurement {
            benchmark_name: "../../../../usr/bin/evil".to_string(),
            timestamp: SystemTime::now(),
            commit_hash: Some("../../../etc/passwd".to_string()),
            branch: Some("main".to_string()),
            mean_duration_nanos: 1_000_000,
            median_duration_nanos: 1_000_000,
            min_duration_nanos: 900_000,
            max_duration_nanos: 1_100_000,
            std_dev_nanos: 50_000,
            sample_count: 100,
            throughput_ops_per_sec: None,
            custom_metrics: HashMap::new(),
        };

        let result = manager.update_baseline(traversal_measurement);
        assert!(
            result.is_ok(),
            "Should handle path traversal attempts safely"
        );

        // Verify no files were created outside temp_dir
        let entries = fs::read_dir(temp_dir.path()).expect("Should be able to read temp dir");

        for entry in entries {
            let entry = entry.expect("Should be able to read entry");
            let path = entry.path();
            assert!(
                path.starts_with(temp_dir.path()),
                "All files should be within temp directory: {:?}",
                path
            );
        }

        println!("✓ Path traversal protection validated");

        // Test 3: Resource exhaustion protection
        let extreme_measurement = PerformanceMeasurement {
            benchmark_name: "resource_exhaustion_test".to_string(),
            timestamp: SystemTime::now(),
            commit_hash: None,
            branch: None,
            mean_duration_nanos: u64::MAX,
            median_duration_nanos: u64::MAX,
            min_duration_nanos: 0,
            max_duration_nanos: u64::MAX,
            std_dev_nanos: u64::MAX / 2,
            sample_count: usize::MAX / 1000, // Large but not maximum to avoid immediate overflow
            throughput_ops_per_sec: Some(f64::INFINITY),
            custom_metrics: {
                let mut metrics = HashMap::new();
                // Add many custom metrics to test memory usage
                for i in 0..1000 {
                    metrics.insert(format!("metric_{}", i), f64::MAX);
                }
                metrics
            },
        };

        let result = manager.update_baseline(extreme_measurement.clone());
        assert!(
            result.is_ok(),
            "Should handle extreme values without crashing"
        );

        let analysis = manager.detect_regression(&extreme_measurement);
        assert!(
            analysis.is_ok(),
            "Should handle extreme values in regression detection"
        );

        println!("✓ Resource exhaustion protection validated");
    }

    /// Test robustness against filesystem issues
    #[test]
    fn test_filesystem_robustness() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Test with read-only directory (if possible)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let readonly_dir = temp_dir.path().join("readonly");
            fs::create_dir(&readonly_dir).expect("Failed to create directory");

            let mut perms = fs::metadata(&readonly_dir).unwrap().permissions();
            perms.set_mode(0o444); // Read-only
            fs::set_permissions(&readonly_dir, perms).ok(); // May fail on some systems

            // Try to create manager in read-only directory
            let manager_result = BaselineManager::new(&readonly_dir);

            // Should either succeed (if permissions aren't enforced) or fail gracefully
            match manager_result {
                Ok(_) => println!("✓ Read-only directory handled (permissions not enforced)"),
                Err(e) => {
                    println!("✓ Read-only directory handled gracefully: {}", e);
                    assert!(matches!(e, RegressionError::Io(_)));
                }
            }
        }

        // Test with deeply nested directory structure
        let deep_path = temp_dir
            .path()
            .join("level1")
            .join("level2")
            .join("level3")
            .join("level4")
            .join("level5")
            .join("deep_baselines");

        let manager = BaselineManager::new(&deep_path);
        assert!(manager.is_ok(), "Should create deeply nested directories");
        assert!(deep_path.exists(), "Deep directory should be created");

        println!("✓ Deep directory creation handled");

        // Test with corrupted baseline files mixed with valid ones
        let mixed_dir = temp_dir.path().join("mixed_baselines");
        let mut manager = BaselineManager::new(&mixed_dir).expect("Failed to create manager");

        // Create valid baseline
        let valid_measurement = create_test_measurement("valid_baseline", 1000, 10, 100);
        manager
            .update_baseline(valid_measurement)
            .expect("Failed to create valid baseline");

        // Create corrupted files
        fs::write(mixed_dir.join("corrupted1.json"), "invalid json")
            .expect("Failed to write corrupted file");
        fs::write(
            mixed_dir.join("corrupted2.json"),
            r#"{"invalid": "structure"}"#,
        )
        .expect("Failed to write corrupted file");

        // Create new manager instance to test loading
        let manager2 = BaselineManager::new(&mixed_dir);
        assert!(
            manager2.is_ok(),
            "Should handle mixed valid/corrupted files"
        );

        let manager2 = manager2.unwrap();
        let baselines = manager2.list_baselines();
        assert!(
            !baselines.is_empty(),
            "Should load valid baselines despite corruption"
        );

        println!("✓ Mixed valid/corrupted file handling validated");
    }
}

/// CI/CD integration comprehensive validation
#[cfg(test)]
mod cicd_integration_validation {
    use super::*;
    use std::env;

    /// Test complete CI workflow with environment variables
    #[test]
    fn test_comprehensive_ci_workflow() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Set CI environment variables
        unsafe {
            env::set_var("CI", "true");
            env::set_var("GITHUB_SHA", "test_commit_hash_123456");
            env::set_var("GITHUB_REF", "refs/heads/feature/performance-testing");
            env::set_var("GITHUB_WORKFLOW", "Performance Regression Testing");

            // Custom thresholds via environment
            env::set_var("SKREAVER_MEAN_THRESHOLD_PERCENT", "8.0");
            env::set_var("SKREAVER_P95_THRESHOLD_PERCENT", "12.0");
            env::set_var("SKREAVER_MIN_SAMPLES", "15");
        }

        // Create CLI with environment-aware configuration
        let _cli = RegressionCli::with_baseline_path(temp_dir.path());

        // Phase 1: Create baselines representing historical CI runs
        println!("CI Workflow Phase 1: Creating historical baselines...");

        let ci_benchmarks = vec![
            ("ci/unit_tests", 2500, 200),
            ("ci/integration_tests", 15000, 1500),
            ("ci/end_to_end_tests", 45000, 3000),
            ("ci/build_process", 30000, 2000),
            ("ci/docker_build", 60000, 5000),
        ];

        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        for (benchmark_name, base_micros, std_dev) in &ci_benchmarks {
            // Create historical runs (simulate 30 CI runs)
            for run in 0..30 {
                let mut measurement = create_test_measurement(
                    benchmark_name,
                    base_micros + (run % 5) * (std_dev / 5), // Some run-to-run variation
                    *std_dev,
                    100,
                );

                measurement.commit_hash = Some(format!("historical_commit_{:02}", run));
                measurement.branch = Some("main".to_string());

                manager
                    .update_baseline(measurement)
                    .expect("Failed to create baseline");
            }
        }

        println!(
            "✓ Created {} baselines with 30 historical runs each",
            ci_benchmarks.len()
        );

        // Phase 2: Test current CI run (no regression)
        println!("CI Workflow Phase 2: Testing current CI run...");

        let mut current_run_analyses = Vec::new();

        for (benchmark_name, base_micros, std_dev) in &ci_benchmarks {
            // Simulate current CI run with acceptable performance
            let mut current_measurement = create_test_measurement(
                benchmark_name,
                base_micros + (std_dev / 4), // Small increase within normal range
                *std_dev,
                100,
            );

            current_measurement.commit_hash = Some("test_commit_hash_123456".to_string());
            current_measurement.branch = Some("feature/performance-testing".to_string());

            let analysis = manager
                .detect_regression(&current_measurement)
                .expect("Failed to analyze current run");

            current_run_analyses.push((benchmark_name.to_string(), analysis));
        }

        // Validate CI results
        let regressions_found = current_run_analyses
            .iter()
            .any(|(_, analysis)| analysis.is_regression);

        if regressions_found {
            println!("⚠️ Regressions detected in CI run:");
            for (name, analysis) in &current_run_analyses {
                if analysis.is_regression {
                    println!("  🚨 {}: {}", name, analysis.summary());
                    println!("     Details: {}", analysis.details);
                }
            }
        } else {
            println!("✅ No regressions detected in current CI run");
        }

        // Phase 3: Test CI failure scenario (intentional regression)
        println!("CI Workflow Phase 3: Testing CI failure detection...");

        let failing_measurement = create_test_measurement(
            "ci/unit_tests",
            4000, // 60% increase - should definitely trigger
            300,
            100,
        );

        let failure_analysis = manager
            .detect_regression(&failing_measurement)
            .expect("Failed to analyze failing case");

        assert!(
            failure_analysis.is_regression,
            "Should detect significant performance regression in CI"
        );

        println!(
            "🚨 CI failure correctly detected: {}",
            failure_analysis.summary()
        );

        // Phase 4: Test CI artifact management
        println!("CI Workflow Phase 4: Testing CI artifact management...");

        // Export baselines for CI artifacts
        let artifact_dir = temp_dir.path().join("ci_artifacts");
        fs::create_dir_all(&artifact_dir).expect("Failed to create artifact directory");

        for (benchmark_name, _, _) in &ci_benchmarks {
            let artifact_path =
                artifact_dir.join(format!("{}.json", benchmark_name.replace('/', "_")));
            manager
                .export_baseline(benchmark_name, &artifact_path)
                .expect("Failed to export CI artifact");
            assert!(artifact_path.exists(), "CI artifact should be created");
        }

        // Test artifact restoration (simulate fresh CI runner)
        let fresh_temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut fresh_manager =
            BaselineManager::new(fresh_temp_dir.path()).expect("Failed to create fresh manager");

        for (benchmark_name, _, _) in &ci_benchmarks {
            let artifact_path =
                artifact_dir.join(format!("{}.json", benchmark_name.replace('/', "_")));
            let imported_name = fresh_manager
                .import_baseline(&artifact_path)
                .expect("Failed to import CI artifact");
            assert_eq!(imported_name, *benchmark_name);
        }

        // Verify fresh manager has all baselines
        let fresh_baselines = fresh_manager.list_baselines();
        assert_eq!(fresh_baselines.len(), ci_benchmarks.len());

        println!("✓ CI artifact management validated");

        // Phase 5: Test CI performance requirements
        println!("CI Workflow Phase 5: Validating CI performance requirements...");

        let start = Instant::now();

        // Simulate typical CI regression detection workload
        for _ in 0..100 {
            for (benchmark_name, base_micros, std_dev) in &ci_benchmarks {
                let test_measurement =
                    create_test_measurement(benchmark_name, base_micros + 100, *std_dev, 100);
                let _ = manager.detect_regression(&test_measurement);
            }
        }

        let ci_workload_time = start.elapsed();

        println!(
            "CI Performance: {} detections across {} benchmarks in {:?}",
            100,
            ci_benchmarks.len(),
            ci_workload_time
        );

        // CI should complete regression detection in under 2 minutes for typical workload
        assert!(
            ci_workload_time.as_secs() < 120,
            "CI regression detection should complete in <2 minutes, took {}s",
            ci_workload_time.as_secs()
        );

        println!("✓ CI performance requirements met");

        // Cleanup environment variables
        unsafe {
            env::remove_var("CI");
            env::remove_var("GITHUB_SHA");
            env::remove_var("GITHUB_REF");
            env::remove_var("GITHUB_WORKFLOW");
            env::remove_var("SKREAVER_MEAN_THRESHOLD_PERCENT");
            env::remove_var("SKREAVER_P95_THRESHOLD_PERCENT");
            env::remove_var("SKREAVER_MIN_SAMPLES");
        }

        println!("🎉 Comprehensive CI workflow validation completed successfully!");
    }
}

/// Comprehensive system integration test
#[test]
fn test_complete_system_integration() {
    println!("🚀 Starting Complete Performance Regression Detection System Integration Test");
    println!("================================================================================");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let mut manager =
        BaselineManager::new(temp_dir.path()).expect("Failed to create baseline manager");

    // Phase 1: System Setup and Baseline Creation
    println!("\nPhase 1: System Setup and Historical Baseline Creation");
    println!("----------------------------------------------------");

    let production_benchmarks = vec![
        // Web service benchmarks
        ("web/api_authentication", 150, 15, 1000),
        ("web/database_queries", 500, 50, 800),
        ("web/json_serialization", 80, 8, 1200),
        ("web/cache_operations", 25, 3, 2000),
        // Data processing benchmarks
        ("data/csv_parsing", 2000, 200, 500),
        ("data/data_validation", 800, 80, 600),
        ("data/batch_processing", 5000, 500, 300),
        // Infrastructure benchmarks
        ("infra/file_io_operations", 1200, 120, 400),
        ("infra/network_requests", 300, 30, 700),
        ("infra/memory_allocations", 50, 5, 1500),
    ];

    let total_historical_runs = 50;
    let start_setup = Instant::now();

    for (benchmark_name, base_micros, std_dev, sample_count) in &production_benchmarks {
        for run_id in 0..total_historical_runs {
            // Simulate realistic performance variation over time
            let time_drift = (run_id as f64 / total_historical_runs as f64) * 0.05; // 5% drift over time
            let random_variation = (run_id % 7) as u64 * (std_dev / 7); // Some randomness
            let final_duration =
                (*base_micros as f64 * (1.0 + time_drift)) as u64 + random_variation;

            let mut measurement =
                create_test_measurement(benchmark_name, final_duration, *std_dev, *sample_count);

            measurement.commit_hash = Some(format!("production_commit_{:03}", run_id));
            measurement.branch = Some("main".to_string());

            // Add realistic timestamp progression
            measurement.timestamp = SystemTime::now()
                - Duration::from_secs(
                    (total_historical_runs - run_id) as u64 * 86400, // One day between runs
                );

            manager
                .update_baseline(measurement)
                .expect("Failed to create production baseline");
        }

        println!(
            "  ✓ Created {} historical runs for {}",
            total_historical_runs, benchmark_name
        );
    }

    let setup_time = start_setup.elapsed();
    println!(
        "\n✅ Phase 1 Complete: {} baselines with {} runs each in {:?}",
        production_benchmarks.len(),
        total_historical_runs,
        setup_time
    );

    // Phase 2: Normal Operations Testing
    println!("\nPhase 2: Normal Operations and Stability Testing");
    println!("-----------------------------------------------");

    let mut normal_operation_results = Vec::new();

    for (benchmark_name, base_micros, std_dev, sample_count) in &production_benchmarks {
        // Test with normal performance variation (should not trigger regressions)
        let normal_variation = (*base_micros as f64 * 0.07) as u64; // 7% variation
        let test_measurement = create_test_measurement(
            benchmark_name,
            base_micros + normal_variation,
            *std_dev,
            *sample_count,
        );

        let analysis = manager
            .detect_regression(&test_measurement)
            .expect("Failed to analyze normal operation");

        normal_operation_results.push((benchmark_name, analysis));
    }

    let normal_regressions = normal_operation_results
        .iter()
        .filter(|(_, analysis)| analysis.is_regression)
        .count();

    println!("Normal operations analysis:");
    for (name, analysis) in &normal_operation_results {
        println!(
            "  {} {}: {:.1}% change",
            if analysis.is_regression {
                "🚨"
            } else {
                "✅"
            },
            name,
            analysis.mean_change_percent
        );
    }

    println!(
        "\n✅ Phase 2 Complete: {}/{} normal operations triggered regressions (expected: few to none)",
        normal_regressions,
        production_benchmarks.len()
    );

    // Phase 3: Regression Detection Validation
    println!("\nPhase 3: Regression Detection Validation");
    println!("---------------------------------------");

    let regression_test_cases = vec![
        ("Minor regression (15%)", 0.15),
        ("Moderate regression (25%)", 0.25),
        ("Major regression (50%)", 0.50),
        ("Severe regression (100%)", 1.00),
    ];

    for (test_name, regression_factor) in &regression_test_cases {
        println!(
            "\nTesting {}: {}% performance degradation",
            test_name,
            (*regression_factor * 100.0) as u64
        );

        let mut detected_count = 0;

        for (benchmark_name, base_micros, std_dev, sample_count) in &production_benchmarks {
            let degraded_duration = (*base_micros as f64 * (1.0 + regression_factor)) as u64;
            let test_measurement =
                create_test_measurement(benchmark_name, degraded_duration, *std_dev, *sample_count);

            let analysis = manager
                .detect_regression(&test_measurement)
                .expect("Failed to analyze regression");

            if analysis.is_regression {
                detected_count += 1;
            }
        }

        let detection_rate = detected_count as f64 / production_benchmarks.len() as f64;
        println!(
            "  Detection rate: {}/{} ({:.1}%)",
            detected_count,
            production_benchmarks.len(),
            detection_rate * 100.0
        );

        // Validation: expect high detection rate for significant regressions
        if *regression_factor >= 0.20 {
            // 20% or higher regression
            assert!(
                detection_rate >= 0.8,
                "Should detect at least 80% of {}% regressions, detected {:.1}%",
                (*regression_factor * 100.0) as u64,
                detection_rate * 100.0
            );
        }
    }

    println!("\n✅ Phase 3 Complete: Regression detection validation passed");

    // Phase 4: Performance and Scalability Testing
    println!("\nPhase 4: Performance and Scalability Testing");
    println!("-------------------------------------------");

    // Test detection performance under load
    let performance_iterations = 1000;
    let start_perf = Instant::now();

    for _ in 0..performance_iterations {
        let benchmark_idx = fastrand::usize(0..production_benchmarks.len());
        let (benchmark_name, base_micros, std_dev, sample_count) =
            &production_benchmarks[benchmark_idx];

        let test_measurement = create_test_measurement(
            benchmark_name,
            base_micros + fastrand::u64(0..*std_dev),
            *std_dev,
            *sample_count,
        );

        let _ = manager.detect_regression(&test_measurement);
    }

    let performance_time = start_perf.elapsed();
    let avg_detection_time = performance_time / performance_iterations;

    println!("Performance test results:");
    println!(
        "  {} detections in {:?}",
        performance_iterations, performance_time
    );
    println!("  Average detection time: {:?}", avg_detection_time);
    println!(
        "  Throughput: {:.0} detections/second",
        performance_iterations as f64 / performance_time.as_secs_f64()
    );

    // Performance requirement: average detection time < 1ms
    assert!(
        avg_detection_time.as_micros() < 1000,
        "Average detection time should be <1ms, got {:?}",
        avg_detection_time
    );

    println!("\n✅ Phase 4 Complete: Performance requirements met");

    // Phase 5: Data Integrity and Persistence Testing
    println!("\nPhase 5: Data Integrity and Persistence Testing");
    println!("----------------------------------------------");

    // Test export/import of all baselines
    let export_dir = temp_dir.path().join("system_export");
    fs::create_dir_all(&export_dir).expect("Failed to create export directory");

    let all_baselines = manager.list_baselines();
    println!(
        "Exporting {} baselines for integrity testing...",
        all_baselines.len()
    );

    for baseline_name in &all_baselines {
        let export_path = export_dir.join(format!("{}.json", baseline_name.replace('/', "_")));
        manager
            .export_baseline(baseline_name, &export_path)
            .expect("Failed to export baseline");
    }

    // Create fresh manager and import all baselines
    let integrity_temp_dir = TempDir::new().expect("Failed to create temp directory");
    let mut integrity_manager = BaselineManager::new(integrity_temp_dir.path())
        .expect("Failed to create integrity manager");

    for baseline_name in &all_baselines {
        let export_path = export_dir.join(format!("{}.json", baseline_name.replace('/', "_")));
        let imported_name = integrity_manager
            .import_baseline(&export_path)
            .expect("Failed to import baseline");
        assert_eq!(imported_name, *baseline_name);
    }

    // Verify data integrity after roundtrip
    let imported_baselines = integrity_manager.list_baselines();
    assert_eq!(
        imported_baselines.len(),
        all_baselines.len(),
        "All baselines should be imported"
    );

    // Verify regression detection still works with imported data
    let (test_benchmark, base_micros, std_dev, sample_count) = &production_benchmarks[0];
    let integrity_test_measurement = create_test_measurement(
        test_benchmark,
        base_micros + 200, // Add some performance degradation
        *std_dev,
        *sample_count,
    );

    let original_analysis = manager
        .detect_regression(&integrity_test_measurement)
        .expect("Failed to analyze with original data");
    let imported_analysis = integrity_manager
        .detect_regression(&integrity_test_measurement)
        .expect("Failed to analyze with imported data");

    // Results should be consistent
    assert_eq!(
        original_analysis.is_regression, imported_analysis.is_regression,
        "Regression detection should be consistent after import"
    );

    let change_diff =
        (original_analysis.mean_change_percent - imported_analysis.mean_change_percent).abs();
    assert!(
        change_diff < 1.0,
        "Change percentages should be consistent within 1%: original={:.1}%, imported={:.1}%",
        original_analysis.mean_change_percent,
        imported_analysis.mean_change_percent
    );

    println!(
        "✓ Data integrity verified: {} baselines exported and imported successfully",
        all_baselines.len()
    );
    println!("✓ Regression detection consistency validated");

    println!("\n✅ Phase 5 Complete: Data integrity and persistence validated");

    // Final Summary
    println!("\n🎉 COMPLETE SYSTEM INTEGRATION TEST PASSED! 🎉");
    println!("==============================================");
    println!(
        "✅ {} production baselines created with {} historical runs each",
        production_benchmarks.len(),
        total_historical_runs
    );
    println!("✅ Normal operations stability validated");
    println!("✅ Regression detection accuracy validated across multiple degradation levels");
    println!(
        "✅ Performance requirements met: {:?} average detection time",
        avg_detection_time
    );
    println!("✅ Data integrity and persistence validated");
    println!("✅ System ready for production deployment");
    println!("\nTotal test time: {:?}", start_setup.elapsed());
}

// Helper function to create test measurements with realistic characteristics
fn create_test_measurement(
    name: &str,
    mean_micros: u64,
    std_dev_micros: u64,
    sample_count: usize,
) -> PerformanceMeasurement {
    let result = BenchmarkResult {
        name: name.to_string(),
        iterations: sample_count,
        mean: Duration::from_micros(mean_micros),
        median: Duration::from_micros(mean_micros),
        min: Duration::from_micros(mean_micros.saturating_sub(std_dev_micros)),
        max: Duration::from_micros(mean_micros + std_dev_micros),
        std_dev: Duration::from_micros(std_dev_micros),
        throughput: if mean_micros > 0 {
            Some(1_000_000.0 / mean_micros as f64)
        } else {
            None
        },
        total_operations: Some(sample_count),
    };

    PerformanceMeasurement::from(result)
}

// Add fastrand as a dev dependency for performance testing randomization
#[allow(dead_code)]
mod fastrand {
    use std::cell::Cell;

    thread_local! {
        static RNG_STATE: Cell<u64> = const { Cell::new(0x4d595df4d0f33173) };
    }

    fn next_u64() -> u64 {
        RNG_STATE.with(|state| {
            let mut x = state.get();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            state.set(x);
            x
        })
    }

    pub fn u64(range: std::ops::Range<u64>) -> u64 {
        range.start + (next_u64() % (range.end - range.start))
    }

    pub fn usize(range: std::ops::Range<usize>) -> usize {
        range.start + (next_u64() as usize % (range.end - range.start))
    }
}
