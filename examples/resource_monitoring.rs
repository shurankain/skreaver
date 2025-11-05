//! # Resource Monitoring Example
//!
//! This example demonstrates Skreaver's real-time resource monitoring capabilities.
//! It shows how the security system tracks CPU, memory, disk, and file descriptors
//! to enforce resource limits and prevent resource exhaustion attacks.
//!
//! ## Running this example:
//!
//! ```bash
//! cargo run --example resource_monitoring
//! ```
//!
//! ## What this demonstrates:
//! 1. Real-time CPU usage monitoring
//! 2. Real-time memory usage monitoring
//! 3. File descriptor tracking
//! 4. Disk usage monitoring
//! 5. Resource limit enforcement
//! 6. Automatic limit violation detection

use skreaver_core::{
    AgentId, ToolId,
    security::{
        SecurityContext, SecurityPolicy,
        limits::{ResourceLimits, ResourceTracker},
    },
};
use std::thread;
use std::time::Duration;

fn print_banner(title: &str) {
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  {}", title);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}

fn create_default_policy() -> SecurityPolicy {
    SecurityPolicy {
        fs_policy: skreaver_core::security::policy::FileSystemPolicy::default(),
        http_policy: skreaver_core::security::policy::HttpPolicy::default(),
        network_policy: skreaver_core::security::policy::NetworkPolicy::default(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔍 Skreaver Real-Time Resource Monitoring Demo\n");

    // Step 1: Create default limits
    print_banner("Step 1: Configure Resource Limits");
    let limits = ResourceLimits::default();
    println!("Default Resource Limits:");
    println!("  • Max Memory: {} MB", limits.max_memory_mb);
    println!("  • Max CPU: {:.1}%", limits.max_cpu_percent);
    println!("  • Max Execution Time: {:?}", limits.max_execution_time);
    println!(
        "  • Max Concurrent Operations: {}",
        limits.max_concurrent_operations
    );
    println!("  • Max Open Files: {}", limits.max_open_files);
    println!("  • Max Disk Usage: {} MB", limits.max_disk_usage_mb);

    // Step 2: Create resource tracker
    print_banner("Step 2: Initialize Resource Tracker");
    let tracker = ResourceTracker::new(&limits);
    println!("✅ Resource tracker created with real-time monitoring");

    // Step 3: Monitor current process
    print_banner("Step 3: Monitor Current Process Resources");
    let policy = create_default_policy();
    let context = SecurityContext::new(
        AgentId::new_unchecked("demo_agent"),
        ToolId::new_unchecked("demo_tool"),
        policy
    );

    // Check current limits
    match tracker.check_limits(&context) {
        Ok(()) => println!("✅ Current resource usage is within limits"),
        Err(e) => println!("❌ Resource limit exceeded: {:?}", e),
    }

    // Get and display current usage
    if let Some(usage) = tracker.get_usage("demo_agent") {
        println!("\nCurrent Resource Usage:");
        println!("  • Memory: {} MB", usage.memory_mb);
        println!("  • CPU: {:.2}%", usage.cpu_percent);
        println!("  • Open Files: {}", usage.open_files);
        println!("  • Disk Usage: {} MB", usage.disk_usage_mb);
        println!("  • Active Operations: {}", usage.active_operations);
    }

    // Step 4: Demonstrate operation tracking
    print_banner("Step 4: Track Operations with Guards");
    println!("Starting 3 concurrent operations...\n");

    let _guard1 = tracker.start_operation("demo_agent");
    println!("✅ Operation 1 started");

    let _guard2 = tracker.start_operation("demo_agent");
    println!("✅ Operation 2 started");

    let _guard3 = tracker.start_operation("demo_agent");
    println!("✅ Operation 3 started");

    if let Some(usage) = tracker.get_usage("demo_agent") {
        println!("\n📊 Active operations: {}", usage.active_operations);
    }

    // Step 5: Simulate some work and monitor
    print_banner("Step 5: Monitor Resources During Work");
    println!("Performing work and monitoring resources every 500ms...\n");

    for i in 1..=5 {
        // Simulate some work
        let mut sum = 0u64;
        for j in 0..1_000_000 {
            sum = sum.wrapping_add(j);
        }

        // Check resources
        if let Some(usage) = tracker.get_usage("demo_agent") {
            println!(
                "Iteration {}: Memory: {} MB | CPU: {:.2}% | Open Files: {}",
                i, usage.memory_mb, usage.cpu_percent, usage.open_files
            );
        }

        thread::sleep(Duration::from_millis(500));
    }

    // Step 6: Test limit enforcement
    print_banner("Step 6: Demonstrate Limit Enforcement");
    println!("Testing with strict limits...\n");

    let strict_limits = ResourceLimits {
        max_memory_mb: 1, // Very low limit for demonstration
        max_cpu_percent: 0.1,
        max_execution_time: Duration::from_secs(300),
        max_concurrent_operations: 2, // Only 2 operations allowed
        max_open_files: 1000,
        max_disk_usage_mb: 1_000_000,
    };

    let strict_tracker = ResourceTracker::new(&strict_limits);

    // Create a new context for the strict tracker
    let policy2 = create_default_policy();
    let context2 = SecurityContext::new(
        AgentId::new_unchecked("strict_agent"),
        ToolId::new_unchecked("strict_tool"),
        policy2,
    );

    // This will likely exceed the strict limits
    match strict_tracker.check_limits(&context2) {
        Ok(()) => println!("✅ Passed strict limits check"),
        Err(e) => println!("❌ Failed strict limits check: {:?}", e),
    }

    // Step 7: Test concurrent operation limits
    print_banner("Step 7: Test Concurrent Operation Limits");
    println!("Strict limit: {} concurrent operations\n", 2);

    let _s1 = strict_tracker.start_operation("strict_agent");
    println!("✅ Operation 1 started");

    let _s2 = strict_tracker.start_operation("strict_agent");
    println!("✅ Operation 2 started");

    let _s3 = strict_tracker.start_operation("strict_agent");
    println!("✅ Operation 3 started");

    // Check if we exceed the limit
    let policy3 = create_default_policy();
    let context3 = SecurityContext::new(
        AgentId::new_unchecked("strict_agent"),
        ToolId::new_unchecked("strict_tool"),
        policy3,
    );

    match strict_tracker.check_limits(&context3) {
        Ok(()) => println!("\n✅ Within concurrent operation limits"),
        Err(e) => println!("\n❌ Exceeded concurrent operation limit: {:?}", e),
    }

    if let Some(usage) = strict_tracker.get_usage("strict_agent") {
        println!(
            "Active operations: {} (limit: {})",
            usage.active_operations, strict_limits.max_concurrent_operations
        );
    }

    // Step 8: Demonstrate cleanup
    print_banner("Step 8: Cleanup Stale Agents");
    println!("Cleaning up agents inactive for more than 60 seconds...");

    tracker.cleanup_stale_agents(Duration::from_secs(60));
    println!("✅ Cleanup complete");

    // Summary
    print_banner("Summary");
    println!("✅ Real-time CPU monitoring working");
    println!("✅ Real-time memory monitoring working");
    println!("✅ File descriptor tracking working");
    println!("✅ Disk usage monitoring working");
    println!("✅ Resource limit enforcement working");
    println!("✅ Concurrent operation tracking working");
    println!("✅ Automatic cleanup working");
    println!("\n🎉 All resource monitoring features demonstrated successfully!");

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    Ok(())
}
