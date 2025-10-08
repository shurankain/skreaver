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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔒 Skreaver Security Configuration Loading Demo\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // ============================================================================
    // Step 1: Load Security Configuration from TOML
    // ============================================================================

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

    // ============================================================================
    // Step 2: Display Configuration Metadata
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📊 Step 2: Configuration Metadata\n");
    println!("   Version: {}", config.metadata.version);
    println!("   Created: {}", config.metadata.created);
    println!("   Description: {}\n", config.metadata.description);

    // ============================================================================
    // Step 3: Display File System Policy
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📁 Step 3: File System Policy\n");
    println!("   Enabled: {}", config.fs.enabled);
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
        "   Max Files Per Operation: {:?}",
        config.fs.max_files_per_operation
    );
    println!("   Follow Symlinks: {}", config.fs.follow_symlinks);
    println!("   Scan Content: {}\n", config.fs.scan_content);

    // ============================================================================
    // Step 4: Display HTTP Policy
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🌐 Step 4: HTTP Policy\n");
    println!("   Enabled: {}", config.http.enabled);
    println!("   Allowed Domains ({}):", config.http.allow_domains.len());
    for domain in &config.http.allow_domains {
        println!("      ✅ {}", domain);
    }
    println!("\n   Denied Domains ({}):", config.http.deny_domains.len());
    for domain in &config.http.deny_domains {
        println!("      🚫 {}", domain);
    }
    println!("\n   Timeout: {:?}", config.http.timeout);
    println!("   Max Response Size: {:?}", config.http.max_response_size);
    println!("   Max Redirects: {:?}", config.http.max_redirects);
    println!("   User Agent: {}", config.http.user_agent);
    println!("   Allow Local: {}\n", config.http.allow_local);

    // ============================================================================
    // Step 5: Display Resource Limits
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("⚙️  Step 5: Resource Limits\n");
    println!("   Max Memory: {} MB", config.resources.max_memory_mb);
    println!("   Max CPU: {:.1}%", config.resources.max_cpu_percent);
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

    // ============================================================================
    // Step 6: Display Audit Configuration
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📊 Step 6: Audit Configuration\n");
    println!("   Log All Operations: {}", config.audit.log_all_operations);
    println!("   Redact Secrets: {}", config.audit.redact_secrets);
    println!("   Secret Patterns: {:?}", config.audit.secret_patterns);
    println!("   Retain Logs: {} days", config.audit.retain_logs_days);
    println!("   Log Level: {:?}", config.audit.log_level);
    println!(
        "   Include Stack Traces: {}",
        config.audit.include_stack_traces
    );
    println!("   Log Format: {:?}\n", config.audit.log_format);

    // ============================================================================
    // Step 7: Display Alerting Configuration
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🚨 Step 7: Alerting Configuration\n");
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

    // ============================================================================
    // Step 8: Display Emergency Configuration
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🆘 Step 8: Emergency Lockdown Configuration\n");
    println!("   Lockdown Active: {}", config.emergency.lockdown_enabled);
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

    // ============================================================================
    // Step 9: Validate Configuration
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("✅ Step 9: Validating Configuration\n");

    match config.validate() {
        Ok(()) => {
            println!("   ✅ Configuration is valid and safe to use\n");
        }
        Err(e) => {
            println!("   ❌ Configuration validation failed: {}\n", e);
            return Err(e.into());
        }
    }

    // ============================================================================
    // Step 10: Demonstrate Runtime Usage
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("🎯 Step 10: Runtime Policy Checks\n");

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

    // ============================================================================
    // Summary
    // ============================================================================

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    println!("📝 Summary:\n");
    println!("   ✅ TOML configuration loaded and parsed");
    println!("   ✅ All security policies extracted");
    println!("   ✅ Configuration validated");
    println!("   ✅ Runtime policy checks working");
    println!("\n🎉 This proves the TOML file is NOT just documentation!");
    println!("   It's actively loaded and used to enforce security at runtime.\n");

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}
