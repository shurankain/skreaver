//! Integration tests for environment-based configuration

use serial_test::serial;
use skreaver_http::runtime::{ConfigError, HttpRuntimeConfigBuilder};
use std::env;

/// Helper to set environment variable for test
fn set_env(key: &str, value: &str) {
    unsafe {
        env::set_var(key, value);
    }
}

/// Helper to clear environment variable after test
fn clear_env(key: &str) {
    unsafe {
        env::remove_var(key);
    }
}

#[test]
#[serial]
fn test_env_config_default_when_no_vars_set() {
    // Clear all skreaver env vars (in case any are set)
    clear_all_skreaver_env_vars();

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load defaults when no env vars set")
        .build()
        .expect("should build valid config");

    assert_eq!(config.request_timeout_secs, 30);
    assert_eq!(config.max_body_size, 16 * 1024 * 1024);
    assert!(config.enable_cors);
    assert!(config.enable_openapi);
    assert_eq!(config.rate_limit.global_rpm, 1000);
    assert_eq!(config.rate_limit.per_ip_rpm, 60);
}

#[test]
#[serial]
fn test_env_config_request_timeout() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_REQUEST_TIMEOUT_SECS", "60");

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build valid config");

    assert_eq!(config.request_timeout_secs, 60);

    clear_env("SKREAVER_REQUEST_TIMEOUT_SECS");
}

#[test]
#[serial]
fn test_env_config_max_body_size() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_MAX_BODY_SIZE", "33554432"); // 32MB

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build valid config");

    assert_eq!(config.max_body_size, 32 * 1024 * 1024);

    clear_env("SKREAVER_MAX_BODY_SIZE");
}

#[test]
#[serial]
fn test_env_config_cors_disabled() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_ENABLE_CORS", "false");

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build valid config");

    assert!(!config.enable_cors);

    clear_env("SKREAVER_ENABLE_CORS");
}

#[test]
#[serial]
fn test_env_config_rate_limits() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_RATE_LIMIT_GLOBAL_RPM", "2000");
    set_env("SKREAVER_RATE_LIMIT_PER_IP_RPM", "100");
    set_env("SKREAVER_RATE_LIMIT_PER_USER_RPM", "200");

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build valid config");

    assert_eq!(config.rate_limit.global_rpm, 2000);
    assert_eq!(config.rate_limit.per_ip_rpm, 100);
    assert_eq!(config.rate_limit.per_user_rpm, 200);

    clear_env("SKREAVER_RATE_LIMIT_GLOBAL_RPM");
    clear_env("SKREAVER_RATE_LIMIT_PER_IP_RPM");
    clear_env("SKREAVER_RATE_LIMIT_PER_USER_RPM");
}

#[test]
#[serial]
fn test_env_config_backpressure() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE", "200");
    set_env("SKREAVER_BACKPRESSURE_MAX_CONCURRENT", "20");
    set_env("SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT", "1000");

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build valid config");

    assert_eq!(config.backpressure.max_queue_size, 200);
    assert_eq!(config.backpressure.max_concurrent_requests, 20);
    assert_eq!(config.backpressure.global_max_concurrent, 1000);

    clear_env("SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE");
    clear_env("SKREAVER_BACKPRESSURE_MAX_CONCURRENT");
    clear_env("SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT");
}

#[test]
#[serial]
fn test_env_config_observability() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_OBSERVABILITY_ENABLE_METRICS", "false");
    set_env("SKREAVER_OBSERVABILITY_ENABLE_TRACING", "true");
    set_env("SKREAVER_OBSERVABILITY_NAMESPACE", "my-service");

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build valid config");

    assert!(!config.observability.metrics_enabled);
    assert!(config.observability.tracing_enabled);
    assert_eq!(config.observability.namespace, "my-service");

    clear_env("SKREAVER_OBSERVABILITY_ENABLE_METRICS");
    clear_env("SKREAVER_OBSERVABILITY_ENABLE_TRACING");
    clear_env("SKREAVER_OBSERVABILITY_NAMESPACE");
}

#[test]
#[serial]
fn test_env_config_invalid_bool() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_ENABLE_CORS", "invalid");

    let result = HttpRuntimeConfigBuilder::from_env();
    assert!(result.is_err());

    match result {
        Err(ConfigError::InvalidEnvVar { key, message }) => {
            assert_eq!(key, "SKREAVER_ENABLE_CORS");
            assert!(message.contains("invalid boolean value"));
        }
        _ => panic!("Expected InvalidEnvVar error"),
    }

    clear_env("SKREAVER_ENABLE_CORS");
}

#[test]
#[serial]
fn test_env_config_invalid_u64() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_REQUEST_TIMEOUT_SECS", "not_a_number");

    let result = HttpRuntimeConfigBuilder::from_env();
    assert!(result.is_err());

    match result {
        Err(ConfigError::InvalidEnvVar { key, message }) => {
            assert_eq!(key, "SKREAVER_REQUEST_TIMEOUT_SECS");
            assert!(message.contains("invalid u64 value"));
        }
        _ => panic!("Expected InvalidEnvVar error"),
    }

    clear_env("SKREAVER_REQUEST_TIMEOUT_SECS");
}

#[test]
#[serial]
fn test_env_config_invalid_f64() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_BACKPRESSURE_LOAD_THRESHOLD", "not_a_float");

    let result = HttpRuntimeConfigBuilder::from_env();
    assert!(result.is_err());

    match result {
        Err(ConfigError::InvalidEnvVar { key, message }) => {
            assert_eq!(key, "SKREAVER_BACKPRESSURE_LOAD_THRESHOLD");
            assert!(message.contains("invalid f64 value"));
        }
        _ => panic!("Expected InvalidEnvVar error"),
    }

    clear_env("SKREAVER_BACKPRESSURE_LOAD_THRESHOLD");
}

#[test]
#[serial]
fn test_env_config_validation_timeout_zero() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_REQUEST_TIMEOUT_SECS", "0");

    let result = HttpRuntimeConfigBuilder::from_env()
        .expect("should parse env")
        .build();

    assert!(result.is_err());
    match result {
        Err(ConfigError::ValidationError(msg)) => {
            assert!(msg.contains("request_timeout_secs must be greater than 0"));
        }
        _ => panic!("Expected ValidationError"),
    }

    clear_env("SKREAVER_REQUEST_TIMEOUT_SECS");
}

#[test]
#[serial]
fn test_env_config_validation_timeout_too_large() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_REQUEST_TIMEOUT_SECS", "301");

    let result = HttpRuntimeConfigBuilder::from_env()
        .expect("should parse env")
        .build();

    assert!(result.is_err());
    match result {
        Err(ConfigError::ValidationError(msg)) => {
            assert!(msg.contains("request_timeout_secs must be <= 300"));
        }
        _ => panic!("Expected ValidationError"),
    }

    clear_env("SKREAVER_REQUEST_TIMEOUT_SECS");
}

#[test]
#[serial]
fn test_env_config_validation_load_threshold_out_of_range() {
    clear_all_skreaver_env_vars();
    set_env("SKREAVER_BACKPRESSURE_LOAD_THRESHOLD", "1.5");

    let result = HttpRuntimeConfigBuilder::from_env()
        .expect("should parse env")
        .build();

    assert!(result.is_err());
    match result {
        Err(ConfigError::ValidationError(msg)) => {
            assert!(msg.contains("load_threshold must be between 0.0 and 1.0"));
        }
        _ => panic!("Expected ValidationError"),
    }

    clear_env("SKREAVER_BACKPRESSURE_LOAD_THRESHOLD");
}

#[test]
#[serial]
fn test_env_config_bool_variants() {
    clear_all_skreaver_env_vars();

    // Test "true" variants
    for true_val in &["true", "TRUE", "1", "yes", "YES", "on", "ON"] {
        set_env("SKREAVER_ENABLE_CORS", true_val);
        let config = HttpRuntimeConfigBuilder::from_env()
            .unwrap()
            .build()
            .unwrap();
        assert!(config.enable_cors, "Failed for value: {}", true_val);
    }

    // Test "false" variants
    for false_val in &["false", "FALSE", "0", "no", "NO", "off", "OFF"] {
        set_env("SKREAVER_ENABLE_CORS", false_val);
        let config = HttpRuntimeConfigBuilder::from_env()
            .unwrap()
            .build()
            .unwrap();
        assert!(!config.enable_cors, "Failed for value: {}", false_val);
    }

    clear_env("SKREAVER_ENABLE_CORS");
}

#[test]
#[serial]
fn test_env_config_comprehensive() {
    clear_all_skreaver_env_vars();

    // Set all configuration via environment
    set_env("SKREAVER_REQUEST_TIMEOUT_SECS", "45");
    set_env("SKREAVER_MAX_BODY_SIZE", "10485760"); // 10MB
    set_env("SKREAVER_ENABLE_CORS", "false");
    set_env("SKREAVER_ENABLE_OPENAPI", "false");
    set_env("SKREAVER_RATE_LIMIT_GLOBAL_RPM", "500");
    set_env("SKREAVER_RATE_LIMIT_PER_IP_RPM", "30");
    set_env("SKREAVER_RATE_LIMIT_PER_USER_RPM", "50");
    set_env("SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE", "50");
    set_env("SKREAVER_BACKPRESSURE_MAX_CONCURRENT", "5");
    set_env("SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT", "250");
    set_env("SKREAVER_BACKPRESSURE_QUEUE_TIMEOUT_SECS", "15");
    set_env("SKREAVER_BACKPRESSURE_PROCESSING_TIMEOUT_SECS", "30");
    set_env("SKREAVER_BACKPRESSURE_ENABLE_ADAPTIVE", "false");
    set_env("SKREAVER_BACKPRESSURE_TARGET_PROCESSING_MS", "500");
    set_env("SKREAVER_BACKPRESSURE_LOAD_THRESHOLD", "0.7");
    set_env("SKREAVER_OBSERVABILITY_ENABLE_METRICS", "true");
    set_env("SKREAVER_OBSERVABILITY_ENABLE_TRACING", "false");
    set_env("SKREAVER_OBSERVABILITY_NAMESPACE", "test-service");

    let config = HttpRuntimeConfigBuilder::from_env()
        .expect("should load config")
        .build()
        .expect("should build valid config");

    // Verify all values
    assert_eq!(config.request_timeout_secs, 45);
    assert_eq!(config.max_body_size, 10 * 1024 * 1024);
    assert!(!config.enable_cors);
    assert!(!config.enable_openapi);
    assert_eq!(config.rate_limit.global_rpm, 500);
    assert_eq!(config.rate_limit.per_ip_rpm, 30);
    assert_eq!(config.rate_limit.per_user_rpm, 50);
    assert_eq!(config.backpressure.max_queue_size, 50);
    assert_eq!(config.backpressure.max_concurrent_requests, 5);
    assert_eq!(config.backpressure.global_max_concurrent, 250);
    assert_eq!(config.backpressure.queue_timeout.as_secs(), 15);
    assert_eq!(config.backpressure.processing_timeout.as_secs(), 30);
    assert_eq!(
        config.backpressure.mode,
        skreaver_http::runtime::backpressure::BackpressureMode::Static
    );
    assert_eq!(config.backpressure.target_processing_time_ms, 500);
    assert_eq!(config.backpressure.load_threshold, 0.7);
    assert!(config.observability.metrics_enabled);
    assert!(!config.observability.tracing_enabled);
    assert_eq!(config.observability.namespace, "test-service");

    // Clean up
    clear_all_skreaver_env_vars();
}

/// Helper to clear all SKREAVER_* environment variables
fn clear_all_skreaver_env_vars() {
    let vars_to_clear = vec![
        "SKREAVER_REQUEST_TIMEOUT_SECS",
        "SKREAVER_MAX_BODY_SIZE",
        "SKREAVER_ENABLE_CORS",
        "SKREAVER_ENABLE_OPENAPI",
        "SKREAVER_SECURITY_CONFIG_PATH",
        "SKREAVER_RATE_LIMIT_GLOBAL_RPM",
        "SKREAVER_RATE_LIMIT_PER_IP_RPM",
        "SKREAVER_RATE_LIMIT_PER_USER_RPM",
        "SKREAVER_BACKPRESSURE_MAX_QUEUE_SIZE",
        "SKREAVER_BACKPRESSURE_MAX_CONCURRENT",
        "SKREAVER_BACKPRESSURE_GLOBAL_MAX_CONCURRENT",
        "SKREAVER_BACKPRESSURE_QUEUE_TIMEOUT_SECS",
        "SKREAVER_BACKPRESSURE_PROCESSING_TIMEOUT_SECS",
        "SKREAVER_BACKPRESSURE_ENABLE_ADAPTIVE",
        "SKREAVER_BACKPRESSURE_TARGET_PROCESSING_MS",
        "SKREAVER_BACKPRESSURE_LOAD_THRESHOLD",
        "SKREAVER_OBSERVABILITY_ENABLE_METRICS",
        "SKREAVER_OBSERVABILITY_ENABLE_TRACING",
        "SKREAVER_OBSERVABILITY_ENABLE_HEALTH",
        "SKREAVER_OBSERVABILITY_OTEL_ENDPOINT",
        "SKREAVER_OBSERVABILITY_NAMESPACE",
    ];

    for var in vars_to_clear {
        unsafe {
            env::remove_var(var);
        }
    }
}
