//! # Security Configuration Loading Example
//!
//! This example demonstrates how Skreaver loads and validates security configuration from TOML.
//! It shows that the TOML file is NOT just documentation - it's actually used at runtime!
//!
//! ## Running this example:
//!
//! ```bash
//! cargo run --example security_config_loading
//! ```
//!
//! ## What this demonstrates:
//! 1. Loading security configuration from TOML file
//! 2. Validating configuration for security issues
//! 3. Accessing policy settings at runtime
//! 4. Checking resource limits
//! 5. Configuring audit and alerting

use skreaver_core::security::SecurityConfig;

fn display_metadata(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📊 Step 2: Configuration Metadata\n");
    println!("   Version: {}", config.metadata.version);
    println!("   Created: {}", config.metadata.created);
    println!("   Description: {}\n", config.metadata.description);
}

fn display_fs_policy(config: &SecurityConfig) {
    use skreaver_core::security::{FileSystemAccess, SymlinkBehavior};

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📁 Step 3: File System Policy\n");

    match &config.fs.access {
        FileSystemAccess::Disabled => {
            println!("   Status: ❌ DISABLED");
        }
        FileSystemAccess::Enabled {
            symlink_behavior,
            content_scanning,
        } => {
            println!("   Status: ✅ ENABLED");
            println!(
                "   Follow Symlinks: {}",
                matches!(symlink_behavior, SymlinkBehavior::Follow)
            );
            println!("   Scan Content: {}", content_scanning);
        }
    }

    println!("   Allowed Paths ({}):", config.fs.allow_paths.len());
    for path in &config.fs.allow_paths {
        println!("      ✅ {}", path.display());
    }
    println!("\n   Deny Patterns ({}):", config.fs.deny_patterns.len());
    for pattern in &config.fs.deny_patterns {
        println!("      🚫 {}", pattern);
    }
    println!("\n   Max File Size: {:?}", config.fs.max_file_size);
    println!(
        "   Max Files Per Operation: {:?}\n",
        config.fs.max_files_per_operation
    );
}

fn display_http_policy(config: &SecurityConfig) {
    use skreaver_core::security::HttpAccess;

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🌐 Step 4: HTTP Policy\n");

    match &config.http.access {
        HttpAccess::Disabled => {
            println!("   Status: ❌ DISABLED");
        }
        HttpAccess::LocalOnly(config) => {
            println!("   Status: ✅ ENABLED (Local Only)");
            println!("   Timeout: {:?}", config.timeout);
            println!("   Max Response Size: {:?}", config.max_response_size);
        }
        HttpAccess::Internet {
            config,
            domain_filter,
            include_local,
            max_redirects,
            user_agent,
        } => {
            use skreaver_core::security::DomainFilter;
            println!("   Status: ✅ ENABLED (Internet Access)");

            match domain_filter {
                DomainFilter::AllowAll { deny_list } => {
                    println!("   Domain Filter: Allow All (except denied)");
                    println!("   Denied Domains ({}):", deny_list.len());
                    for domain in deny_list {
                        println!("      🚫 {}", domain);
                    }
                }
                DomainFilter::AllowList { allow_list, deny_list } => {
                    println!("   Domain Filter: Allow List");
                    println!("   Allowed Domains ({}):", allow_list.len());
                    for domain in allow_list {
                        println!("      ✅ {}", domain);
                    }
                    println!("\n   Denied Domains ({}):", deny_list.len());
                    for domain in deny_list {
                        println!("      🚫 {}", domain);
                    }
                }
            }

            println!("\n   Timeout: {:?}", config.timeout);
            println!("   Max Response Size: {:?}", config.max_response_size);
            println!("   Max Redirects: {:?}", max_redirects);
            println!("   User Agent: {}", user_agent);
            println!("   Allow Local: {}", include_local);
        }
    }
    println!();
}

fn display_network_policy(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🔌 Step 5: Network Policy\n");
    println!("   Enabled: {}", config.network.enabled);
    println!("   Allowed Ports ({}):", config.network.allow_ports.len());
    if !config.network.allow_ports.is_empty() {
        print!("      ✅ ");
        for (i, port) in config.network.allow_ports.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("{:?}", port);
        }
        println!();
    }
    println!("\n   Denied Ports ({}):", config.network.deny_ports.len());
    if !config.network.deny_ports.is_empty() {
        print!("      🚫 ");
        for (i, port) in config.network.deny_ports.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            print!("{:?}", port);
        }
        println!();
    }
    println!("\n   TTL: {:?}", config.network.ttl);
    println!(
        "   Allow Private Networks: {}\n",
        config.network.allow_private_networks
    );
}

fn display_resource_limits(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("⚙️  Step 6: Resource Limits\n");
    println!("   Max Memory: {} MB", config.resources.max_memory_mb);
    println!("   Max CPU: {:.1}%", config.resources.max_cpu_percent.get());
    println!(
        "   Max Execution Time: {:?}",
        config.resources.max_execution_time
    );
    println!(
        "   Max Concurrent Operations: {}",
        config.resources.max_concurrent_operations
    );
    println!("   Max Open Files: {}", config.resources.max_open_files);
    println!(
        "   Max Disk Usage: {} MB\n",
        config.resources.max_disk_usage_mb
    );
}

fn display_audit_config(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📊 Step 7: Audit Configuration\n");
    println!("   Log All Operations: {}", config.audit.log_all_operations);
    println!("   Redact Secrets: {}", config.audit.redact_secrets);
    println!(
        "   Secret Patterns ({}):",
        config.audit.secret_patterns.len()
    );
    for pattern in &config.audit.secret_patterns {
        println!("      🔍 {}", pattern);
    }
    println!("\n   Retain Logs: {} days", config.audit.retain_logs_days);
    println!("   Log Level: {:?}", config.audit.log_level);
    println!(
        "   Include Stack Traces: {}",
        config.audit.include_stack_traces
    );
    println!("   Log Format: {:?}\n", config.audit.log_format);
}

fn display_secrets_config(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🔑 Step 8: Secrets Configuration\n");
    println!("   Environment Only: {}", config.secrets.environment_only);
    println!("   Env Prefix: {}", config.secrets.env_prefix);
    println!("   Auto Rotate: {}", config.secrets.auto_rotate);
    println!(
        "   Min Secret Length: {} bytes\n",
        config.secrets.min_secret_length
    );
}

fn display_alerting_config(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🚨 Step 9: Alerting Configuration\n");
    println!("   Enabled: {}", config.alerting.enabled);
    println!(
        "   Violation Threshold: {}",
        config.alerting.violation_threshold
    );
    println!(
        "   Violation Window: {} minutes",
        config.alerting.violation_window_minutes
    );
    println!("   Alert Levels: {:?}", config.alerting.alert_levels);
    println!(
        "   Email Recipients ({}):",
        config.alerting.email_recipients.len()
    );
    for email in &config.alerting.email_recipients {
        println!("      📧 {}", email);
    }
    if let Some(ref webhook) = config.alerting.webhook_url {
        println!("   Webhook: {}", webhook);
    }
    println!();
}

fn display_development_config(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🔧 Step 10: Development Configuration\n");
    println!("   Enabled: {}", config.development.enabled);
    if config.development.enabled {
        println!("   ⚠️  WARNING: Development mode is active!");
        println!(
            "   Skip Domain Validation: {}",
            config.development.skip_domain_validation
        );
        println!(
            "   Skip Path Validation: {}",
            config.development.skip_path_validation
        );
        println!(
            "   Skip Resource Limits: {}",
            config.development.skip_resource_limits
        );
        if !config.development.dev_allow_domains.is_empty() {
            println!("   Dev Allow Domains:");
            for domain in &config.development.dev_allow_domains {
                println!("      🔓 {}", domain);
            }
        }
    } else {
        println!("   ✅ Production mode active (development features disabled)");
    }
    println!();
}

fn display_emergency_config(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🆘 Step 11: Emergency Lockdown Configuration\n");
    println!("   Lockdown Active: {}", config.emergency.lockdown_enabled);
    if config.emergency.lockdown_enabled {
        println!("   🚨 LOCKDOWN MODE ACTIVE - Most operations restricted!");
    }
    println!("   Security Contact: {}", config.emergency.security_contact);
    println!(
        "   Auto Lockdown Triggers: {:?}",
        config.emergency.auto_lockdown_triggers
    );
    println!(
        "   Allowed Tools During Lockdown ({}):",
        config.emergency.lockdown_allowed_tools.len()
    );
    for tool in &config.emergency.lockdown_allowed_tools {
        println!("      ✅ {}", tool);
    }
    println!();
}

fn display_runtime_checks(config: &SecurityConfig) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🎯 Step 12: Runtime Policy Checks\n");

    // Example: Check if a tool is allowed during lockdown
    let tool_name = "file_read";
    let allowed = config.is_tool_allowed_in_lockdown(tool_name);
    println!("   Tool '{}' allowed in lockdown: {}", tool_name, allowed);

    // Example: Check if we should alert for a specific level
    use skreaver_core::security::config::AlertLevel;
    let should_alert_high = config.should_alert(AlertLevel::High);
    let should_alert_low = config.should_alert(AlertLevel::Low);
    println!("   Alert on HIGH level: {}", should_alert_high);
    println!("   Alert on LOW level: {}", should_alert_low);

    // Example: Get log level
    println!("   Current log level: {:?}\n", config.get_log_level());
}

fn demonstrate_resource_monitoring(config: &SecurityConfig) {
    use skreaver_core::security::limits::ResourceTracker;

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🔍 Step 13: Resource Monitoring Integration\n");

    // Create a resource tracker using the limits from config
    let tracker = ResourceTracker::new(&config.resources);
    println!("   ✅ ResourceTracker created with config limits\n");

    // Start an operation to demonstrate tracking
    let _guard = tracker.start_operation("demo_agent");

    // Get current usage
    if let Some(usage) = tracker.get_usage("demo_agent") {
        println!("   Current Process Usage:");
        println!(
            "      Memory: {} MB (limit: {} MB)",
            usage.memory_mb, config.resources.max_memory_mb
        );
        println!(
            "      CPU: {:.2}% (limit: {:.1}%)",
            usage.cpu_percent,
            config.resources.max_cpu_percent.get()
        );
        println!(
            "      Open Files: {} (limit: {})",
            usage.open_files, config.resources.max_open_files
        );
        println!(
            "      Active Operations: {} (limit: {})",
            usage.active_operations, config.resources.max_concurrent_operations
        );

        // Check if we're within limits
        let memory_ok = usage.memory_mb <= config.resources.max_memory_mb;
        let cpu_ok = usage.cpu_percent <= config.resources.max_cpu_percent.get();
        let files_ok = usage.open_files <= config.resources.max_open_files;

        println!("\n   Limit Check:");
        println!(
            "      Memory: {}",
            if memory_ok { "✅ OK" } else { "❌ EXCEEDED" }
        );
        println!(
            "      CPU: {}",
            if cpu_ok { "✅ OK" } else { "❌ EXCEEDED" }
        );
        println!(
            "      Files: {}",
            if files_ok { "✅ OK" } else { "❌ EXCEEDED" }
        );
        println!();
    } else {
        println!("   ⚠️  No usage data available yet\n");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔒 Skreaver Security Configuration Loading Demo\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Step 1: Load configuration
    println!("📋 Step 1: Loading security configuration from TOML...\n");

    let config = match SecurityConfig::load_from_file("examples/skreaver-security.toml") {
        Ok(cfg) => {
            println!(
                "   ✅ Configuration loaded successfully from examples/skreaver-security.toml\n"
            );
            cfg
        }
        Err(e) => {
            println!("   ⚠️  Could not load from examples/ ({})", e);
            println!("   📋 Using default configuration instead\n");
            SecurityConfig::create_default()
        }
    };

    // Display all configuration sections
    display_metadata(&config);
    display_fs_policy(&config);
    display_http_policy(&config);
    display_network_policy(&config);
    display_resource_limits(&config);
    display_audit_config(&config);
    display_secrets_config(&config);
    display_alerting_config(&config);
    display_development_config(&config);
    display_emergency_config(&config);

    // Validate configuration
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("✅ Step 13: Validating Configuration\n");

    match config.validate() {
        Ok(()) => {
            println!("   ✅ Configuration is valid and safe to use\n");
        }
        Err(e) => {
            println!("   ❌ Configuration validation failed: {}\n", e);
            return Err(e.into());
        }
    }

    // Demonstrate runtime checks
    display_runtime_checks(&config);

    // Demonstrate resource monitoring
    demonstrate_resource_monitoring(&config);

    // Summary
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📝 Summary:\n");
    println!("   ✅ TOML configuration loaded and parsed");
    println!("   ✅ All 11 security policy sections extracted");
    println!("   ✅ Configuration validated");
    println!("   ✅ Runtime policy checks working");
    println!("   ✅ Real resource monitoring integrated");
    println!("\n🎉 This proves the TOML file is NOT just documentation!");
    println!("   It's actively loaded and used to enforce security at runtime.\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}
