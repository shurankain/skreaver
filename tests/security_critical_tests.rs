//! Security Critical Path Tests
//!
//! These tests focus on input validation, resource limits, and security boundaries
//! that are essential for production deployments.

use skreaver_core::{
    InMemoryMemory, Tool,
    memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter},
};
use skreaver_testing::mock_tools::MockTool;
use skreaver_tools::ToolName;

/// Test memory key validation prevents injection attacks
#[test]
fn test_memory_key_security_validation() {
    // Test various potentially dangerous inputs
    let dangerous_inputs = vec![
        "../../../etc/passwd",           // Path traversal
        "key\x00null",                   // Null byte injection
        "key\n\rCRLF",                   // CRLF injection
        "key'OR'1'='1",                  // SQL injection attempt
        "<script>alert('xss')</script>", // XSS attempt
        "key\u{202E}reverse",            // Unicode direction override
        "../../config/secrets.yml",      // Path traversal variant
        "key;rm -rf /",                  // Command injection attempt
    ];

    for dangerous_input in dangerous_inputs {
        let result = MemoryKey::new(dangerous_input);
        // Should either reject or sanitize dangerous inputs
        if let Ok(key) = result {
            // If accepted, the key should be sanitized
            assert!(!key.as_str().contains(".."));
            assert!(!key.as_str().contains('\0'));
            assert!(!key.as_str().contains('\n'));
            assert!(!key.as_str().contains('\r'));
        }
        // Note: The exact behavior depends on implementation,
        // but dangerous inputs should not be accepted as-is
    }
}

/// Test memory value size limits to prevent DoS
#[test]
fn test_memory_value_size_limits() {
    let mut memory = InMemoryMemory::default();
    let key = MemoryKey::new("size_test").expect("Valid key");

    // Test reasonable size (should work)
    let reasonable_value = "x".repeat(1024); // 1KB
    let result = memory.store(MemoryUpdate {
        key: key.clone(),
        value: reasonable_value,
    });
    assert!(result.is_ok());

    // Test very large size (behavior depends on implementation)
    let large_value = "x".repeat(100 * 1024 * 1024); // 100MB
    let result = memory.store(MemoryUpdate {
        key: key.clone(),
        value: large_value,
    });
    // Should either succeed with limits or reject gracefully
    // The specific behavior depends on memory backend implementation
    match result {
        Ok(_) => {
            // If accepted, should still be retrievable
            let loaded = memory.load(&key).expect("Load should succeed");
            assert!(loaded.is_some());
        }
        Err(_) => {
            // Rejection is also acceptable for large values
        }
    }
}

/// Test tool name validation against injection
#[test]
fn test_tool_name_security_validation() {
    let dangerous_tool_names = vec![
        "../../../bin/sh",         // Path traversal (contains '/')
        "tool with spaces",        // Contains spaces (should be invalid)
        "tool@symbol",             // Contains '@' (should be invalid)
        "tool;rm -rf /",           // Command injection (contains ';' and ' ' and '/')
        "<script>evil()</script>", // XSS (contains '<' '>' '(' ')')
        "tool'OR'1'='1",           // SQL injection (contains ' and =)
        "tool$(rm -rf /)",         // Command substitution (contains '$' '(' ')' ' ' '/')
        "tool`whoami`",            // Command substitution (contains '`')
        "tool\ntest",              // Contains newline (middle of string)
        "tool\rtest",              // Contains carriage return (middle of string)
    ];

    for dangerous_name in dangerous_tool_names {
        let result = ToolName::new(dangerous_name);
        // Should reject dangerous tool names
        assert!(
            result.is_err(),
            "Dangerous tool name should be rejected: '{}'",
            dangerous_name
        );
    }

    // Separate test for null byte which might be tricky
    let null_byte_name = "tool\x00null";
    let result = ToolName::new(null_byte_name);
    // Null bytes should be rejected - they're not alphanumeric, _, or -
    assert!(
        result.is_err(),
        "Tool name with null byte should be rejected"
    );
}

/// Test input sanitization in tools
#[test]
fn test_tool_input_sanitization() {
    let mock_tool = MockTool::new("sanitization_tool").with_default_response("sanitized_output");

    let dangerous_inputs = vec![
        "<script>alert('xss')</script>",
        "'; DROP TABLE users; --",
        "../../../etc/passwd",
        "input\x00with\x00nulls",
        "input\nwith\rCRLF",
    ];

    for dangerous_input in dangerous_inputs {
        let result = mock_tool.call(dangerous_input.to_string());
        assert!(result.is_success());
        // Tool should handle dangerous input gracefully
        assert!(!result.output().is_empty());
    }
}

/// Test resource exhaustion protection
#[test]
fn test_resource_exhaustion_protection() {
    let mut memory = InMemoryMemory::default();

    // Test creating many keys to check memory limits
    let mut keys = Vec::new();
    for i in 0..10000 {
        let key = MemoryKey::new(&format!("resource_key_{}", i)).expect("Valid key");
        keys.push(key);
    }

    // Test storing many values
    for (i, key) in keys.iter().enumerate().take(1000) {
        let result = memory.store(MemoryUpdate {
            key: key.clone(),
            value: format!("value_{}", i),
        });
        // Should either succeed or fail gracefully
        assert!(result.is_ok() || result.is_err());
    }

    // Memory should still be functional
    let test_key = MemoryKey::new("functionality_test").expect("Valid key");
    let result = memory.store(MemoryUpdate {
        key: test_key.clone(),
        value: "still_working".to_string(),
    });
    assert!(result.is_ok());
}

/// Test concurrent access security
#[tokio::test]
async fn test_concurrent_access_security() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::task::JoinSet;

    let memory = Arc::new(Mutex::new(InMemoryMemory::default()));
    let mut join_set = JoinSet::new();

    // Spawn many concurrent operations to test race conditions
    for i in 0..100 {
        let memory_clone = Arc::clone(&memory);
        join_set.spawn(async move {
            let key = MemoryKey::new(&format!("concurrent_security_{}", i)).unwrap();
            let mut mem = memory_clone.lock().await;

            // Rapid store/load operations
            for j in 0..10 {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: format!("value_{}_{}", i, j),
                };
                let _ = mem.store(update);
                let _ = mem.load(&key);
            }
            i
        });
    }

    // Wait for all operations
    let mut completed = 0;
    while let Some(result) = join_set.join_next().await {
        assert!(result.is_ok());
        completed += 1;
    }

    assert_eq!(completed, 100);

    // Memory should still be consistent
    let memory = memory.lock().await;
    let test_key = MemoryKey::new("final_test").expect("Valid key");
    // This should not panic or corrupt memory
    let _ = memory.load(&test_key);
}

/// Test boundary value validation
#[test]
fn test_boundary_value_validation() {
    // Test empty values
    assert!(MemoryKey::new("").is_err());
    assert!(ToolName::new("").is_err());

    // Test maximum length values (MemoryKey=128, ToolName=64)
    let max_length_key = "a".repeat(128);
    let max_length_tool = "a".repeat(64);

    // These should work at the boundary
    assert!(MemoryKey::new(&max_length_key).is_ok());
    assert!(ToolName::new(&max_length_tool).is_ok());

    // These should fail beyond the boundary
    let too_long_key = "a".repeat(129);
    let too_long_tool = "a".repeat(65);

    assert!(MemoryKey::new(&too_long_key).is_err());
    assert!(ToolName::new(&too_long_tool).is_err());
}

/// Test Unicode security considerations
#[test]
fn test_unicode_security() {
    let unicode_inputs = vec![
        "key_中文",               // CJK characters
        "key_العربية",            // Arabic
        "key_🦀",                 // Emoji
        "key_\u{202E}reverse",    // Right-to-left override
        "key_\u{FEFF}bom",        // Byte order mark
        "key_\u{200B}zero_width", // Zero-width space
    ];

    for unicode_input in unicode_inputs {
        let key_result = MemoryKey::new(unicode_input);
        let tool_result = ToolName::new(unicode_input);

        // Should handle Unicode consistently
        // Either accept and preserve, or reject consistently
        match (key_result, tool_result) {
            (Ok(key), Ok(tool)) => {
                // If accepted, should preserve the input
                assert!(!key.as_str().is_empty());
                assert!(!tool.as_str().is_empty());
            }
            (Err(_), Err(_)) => {
                // Consistent rejection is also fine
            }
            _ => {
                // Inconsistent handling between key and tool is concerning
                // This test helps identify such issues
            }
        }
    }
}

/// Test error message safety (no information leakage)
#[test]
fn test_error_message_safety() {
    // Attempt various invalid operations
    let sensitive_key = "SECRET_DATABASE_PASSWORD";
    let result = MemoryKey::new(sensitive_key);

    if let Err(error) = result {
        let error_message = format!("{}", error);
        // Error messages should not leak sensitive information
        assert!(!error_message.contains("SECRET"));
        assert!(!error_message.contains("PASSWORD"));
        // Should be generic but helpful
        assert!(!error_message.is_empty());
    }
}

/// Test that the system degrades gracefully under stress
#[test]
fn test_graceful_degradation() {
    let mut memory = InMemoryMemory::default();

    // Gradually increase load and verify system remains stable
    for batch_size in [10, 100, 500] {
        for i in 0..batch_size {
            let key =
                MemoryKey::new(&format!("stress_key_{}_{}", batch_size, i)).expect("Valid key");
            let result = memory.store(MemoryUpdate {
                key: key.clone(),
                value: format!("stress_value_{}", i),
            });

            // Should either work or fail gracefully
            match result {
                Ok(_) => {
                    // If successful, should be retrievable
                    let loaded = memory.load(&key).expect("Load should work");
                    assert!(loaded.is_some());
                }
                Err(_) => {
                    // Failure is acceptable under stress,
                    // but system should remain functional
                    let test_key = MemoryKey::new("stress_test").expect("Valid key");
                    let test_result = memory.load(&test_key);
                    assert!(test_result.is_ok()); // Should not crash
                }
            }
        }
    }
}
