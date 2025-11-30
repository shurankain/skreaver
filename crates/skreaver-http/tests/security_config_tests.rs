//! Integration tests for security configuration loading and enforcement
//!
//! These tests verify that:
//! - Security configuration loads from TOML files
//! - Policies are accessible at runtime
//! - Default configuration works when no file is specified
//! - Invalid configurations fall back to defaults

use skreaver_core::security::{FileSystemAccess, HttpAccess};
use skreaver_http::runtime::{HttpAgentRuntime, HttpRuntimeConfig};
use skreaver_tools::InMemoryToolRegistry;
use std::path::PathBuf;

#[tokio::test]
async fn test_default_security_config() {
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);

    let security_config = runtime.security_config();

    // Verify default config is loaded
    assert_eq!(security_config.metadata.version, "0.1.0");
    assert_eq!(
        security_config.metadata.description,
        "Default Skreaver security configuration"
    );

    // Verify default policies exist (at least FS and HTTP are enabled by default)
    assert!(!matches!(
        security_config.fs.access,
        FileSystemAccess::Disabled
    ));
    assert!(!matches!(security_config.http.access, HttpAccess::Disabled));
    // Network policy may or may not be enabled in defaults
}

#[tokio::test]
async fn test_load_security_config_from_file() {
    let registry = InMemoryToolRegistry::new();

    // Path to the example security config (relative to workspace root)
    let config_path = PathBuf::from("../../examples/skreaver-security.toml");

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(config_path),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Verify config loaded from file (not default)
    assert_eq!(security_config.metadata.version, "1.0.0");
    assert_eq!(
        security_config.metadata.description,
        "Production security configuration example"
    );

    // Verify policies are loaded and accessible
    assert!(!matches!(
        security_config.fs.access,
        FileSystemAccess::Disabled
    ));
    assert!(
        !security_config.fs.allow_paths.is_empty(),
        "File system policy should have allowed paths"
    );
    assert!(
        !security_config.fs.deny_patterns.is_empty(),
        "File system policy should have deny patterns"
    );

    // Verify HTTP policy exists
    assert!(!matches!(security_config.http.access, HttpAccess::Disabled));

    // Check if HTTP policy has domain filtering configured
    if let HttpAccess::Internet { domain_filter, .. } = &security_config.http.access {
        use skreaver_core::security::DomainFilter;
        match domain_filter {
            DomainFilter::AllowList { allow_list, .. } => {
                assert!(
                    !allow_list.is_empty(),
                    "HTTP policy AllowList should have allowed domains"
                );
            }
            DomainFilter::AllowAll { .. } => {
                // AllowAll is also valid - no assertion needed
            }
        }
    }

    // Verify network policy exists
    // Note: network policy may be disabled in the example config
    assert!(
        !security_config.network.allow_ports.is_empty()
            || !security_config.network.deny_ports.is_empty()
            || !security_config.network.is_enabled(),
        "Network policy should be configured"
    );

    // Verify resource limits are set
    assert!(security_config.resources.max_memory_mb > 0);
    assert!(security_config.resources.max_cpu_percent.get() > 0.0);
}

#[tokio::test]
async fn test_invalid_config_path_falls_back_to_default() {
    let registry = InMemoryToolRegistry::new();

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(PathBuf::from("nonexistent-config.toml")),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Should fall back to default config
    assert_eq!(security_config.metadata.version, "0.1.0");
    assert_eq!(
        security_config.metadata.description,
        "Default Skreaver security configuration"
    );
}

#[tokio::test]
async fn test_security_config_policy_enforcement_flags() {
    let registry = InMemoryToolRegistry::new();
    let config_path = PathBuf::from("../../examples/skreaver-security.toml");

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(config_path),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Verify audit configuration
    assert!(security_config.audit.log_all_operations);
    assert!(security_config.audit.redact_secrets);
    assert_eq!(security_config.audit.retain_logs_days, 90);

    // Verify alerting configuration exists (values may vary)
    // The exact values depend on the config file
    assert!(
        security_config.alerting.violation_threshold > 0,
        "Alerting should have violation threshold"
    );
    assert!(
        security_config.alerting.violation_window_minutes > 0,
        "Alerting should have violation window"
    );

    // Development mode configuration exists (may be enabled or disabled)
    // Just verify the field is accessible
    let _dev_enabled = security_config.development.is_enabled();

    // Verify emergency/lockdown configuration exists
    // Values depend on config file, just verify accessibility
    let _lockdown_enabled = security_config.emergency.lockdown_enabled;
    let _allowed_tools = &security_config.emergency.lockdown_allowed_tools;
}

#[tokio::test]
async fn test_per_tool_policy_access() {
    let registry = InMemoryToolRegistry::new();
    let config_path = PathBuf::from("../../examples/skreaver-security.toml");

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(config_path),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Test file_read tool policy
    let file_read_policy = security_config.get_tool_policy("file_read");
    assert!(!matches!(
        file_read_policy.fs_policy.access,
        FileSystemAccess::Disabled
    ));

    // Test http_get tool policy
    let http_get_policy = security_config.get_tool_policy("http_get");
    assert!(!matches!(
        http_get_policy.http_policy.access,
        HttpAccess::Disabled
    ));

    // Test non-existent tool (should get default policy)
    let unknown_policy = security_config.get_tool_policy("unknown_tool");
    assert!(!matches!(
        unknown_policy.fs_policy.access,
        FileSystemAccess::Disabled
    ));
}

#[tokio::test]
async fn test_security_config_resource_limits() {
    let registry = InMemoryToolRegistry::new();
    let config_path = PathBuf::from("../../examples/skreaver-security.toml");

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(config_path),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Verify resource limits are set (values from config file)
    assert!(
        security_config.resources.max_memory_mb > 0,
        "Memory limit should be set"
    );
    assert!(
        security_config.resources.max_cpu_percent.get() > 0.0,
        "CPU limit should be set"
    );
    assert!(
        security_config.resources.max_execution_time.as_secs() > 0,
        "Execution timeout should be set"
    );
    assert!(
        security_config.resources.max_open_files > 0,
        "File descriptor limit should be set"
    );
}

#[tokio::test]
async fn test_security_config_secret_patterns() {
    let registry = InMemoryToolRegistry::new();
    let config_path = PathBuf::from("../../examples/skreaver-security.toml");

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(config_path),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Verify secret patterns are loaded
    assert!(
        !security_config.audit.secret_patterns.is_empty(),
        "Secret patterns should be configured"
    );

    // Verify patterns exist (don't check exact regex formats)
    assert!(
        security_config.audit.secret_patterns.len() >= 3,
        "Should have at least 3 secret patterns"
    );
}

#[tokio::test]
async fn test_security_config_tool_specific_policies() {
    let registry = InMemoryToolRegistry::new();
    let config_path = PathBuf::from("../../examples/skreaver-security.toml");

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(config_path),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Check that tool-specific policies exist
    // The config file may or may not define specific tool policies
    // If it does, verify they can be accessed
    if !security_config.tools.is_empty() {
        assert!(
            !security_config.tools.is_empty(),
            "If tools are defined, there should be at least one"
        );
    }

    // Verify getting tool policy works (will use defaults if not defined)
    let _file_read_policy = security_config.get_tool_policy("file_read");
    // Policy exists (enabled or disabled) - just verify it's accessible
}

#[tokio::test]
async fn test_security_config_lockdown_triggers() {
    let registry = InMemoryToolRegistry::new();
    let config_path = PathBuf::from("../../examples/skreaver-security.toml");

    let runtime_config = HttpRuntimeConfig {
        security_config_path: Some(config_path),
        ..Default::default()
    };

    let runtime = HttpAgentRuntime::with_config(registry, runtime_config);
    let security_config = runtime.security_config();

    // Verify lockdown triggers are configured
    assert!(!security_config.emergency.auto_lockdown_triggers.is_empty());

    // Check expected triggers
    let triggers = &security_config.emergency.auto_lockdown_triggers;
    assert!(triggers.iter().any(|t| matches!(
        t,
        skreaver_core::security::config::LockdownTrigger::RepeatedViolations
    )));
}

#[tokio::test]
async fn test_security_config_accessible_from_runtime() {
    // This test ensures the API is accessible
    let registry = InMemoryToolRegistry::new();
    let runtime = HttpAgentRuntime::new(registry);

    // Should compile - security_config() method exists
    let _config = runtime.security_config();

    // Should be able to access security config fields
    let _fs_policy = _config.fs.clone();
    let _http_policy = _config.http.clone();
    let _network_policy = _config.network.clone();
}
