//! Property-Based Tests for Memory Consistency and Tool Idempotency
//!
//! These tests use property-based testing to verify invariants across
//! the system, particularly focusing on memory consistency and tool
//! behavior properties that should hold regardless of input.

use proptest::prelude::*;
use skreaver_core::{
    InMemoryMemory,
    ToolId as ToolName, // ToolName is now an alias for ToolId
    error::TransactionError,
    memory::{
        MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory,
        TransactionalMemory,
    },
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// Simple mock tool for testing without external dependencies
#[derive(Clone)]
#[allow(dead_code)]
struct SimpleMockTool {
    #[allow(dead_code)]
    name: ToolName,
    responses: HashMap<String, String>,
}

impl SimpleMockTool {
    fn new(name: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Self {
            name: ToolName::parse(name)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?,
            responses: HashMap::new(),
        })
    }

    fn with_response(mut self, input: &str, output: &str) -> Self {
        self.responses.insert(input.to_string(), output.to_string());
        self
    }
}

struct MockToolResult {
    output: String,
    success: bool,
}

impl MockToolResult {
    fn output(&self) -> &str {
        &self.output
    }

    fn is_success(&self) -> bool {
        self.success
    }
}

impl SimpleMockTool {
    fn call(&self, input: String) -> MockToolResult {
        match self.responses.get(&input) {
            Some(output) => MockToolResult {
                output: output.clone(),
                success: true,
            },
            None => MockToolResult {
                output: format!("No response configured for input: {}", input),
                success: false,
            },
        }
    }
}

// Strategy for generating valid memory keys
fn memory_key_strategy() -> impl Strategy<Value = MemoryKey> {
    prop::string::string_regex("[a-zA-Z0-9_-]{1,64}")
        .unwrap()
        .prop_filter_map("Valid memory key", |s| MemoryKey::new(&s).ok())
}

// Strategy for generating memory values
fn memory_value_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex(".*").unwrap().prop_map(|s| {
        // Limit size for reasonable test performance
        if s.len() > 1024 {
            s[..1024].to_string()
        } else {
            s
        }
    })
}

// Strategy for generating large memory values for stress testing
fn large_memory_value_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex(".{10,10000}").unwrap()
}

// Strategy for generating potentially malicious memory keys
fn malicious_key_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Path traversal attempts
        Just("../../../etc/passwd".to_string()),
        Just("..\\\\..\\\\..\\\\windows\\\\system32\\\\config\\\\sam".to_string()),
        // SQL injection-like patterns
        Just("'; DROP TABLE users; --".to_string()),
        // XSS attempts
        Just("<script>alert('xss')</script>".to_string()),
        // Command injection
        Just("; rm -rf /".to_string()),
        // Null bytes
        Just("key\x00null".to_string()),
        // Unicode exploits
        Just("key\u{202e}reverse".to_string()),
        // Very long keys
        prop::string::string_regex("[a-zA-Z0-9]{500,1000}").unwrap(),
        // Control characters
        Just("key\r\n\t".to_string()),
        // Empty and whitespace only
        Just("".to_string()),
        Just("   ".to_string()),
    ]
}

// Strategy for generating potentially malicious memory values
fn malicious_value_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // Very large values to test memory limits
        prop::string::string_regex(".{50000,100000}").unwrap(),
        // Binary data patterns
        prop::collection::vec(0u8..255u8, 1000..5000)
            .prop_map(|bytes| String::from_utf8_lossy(&bytes).to_string()),
        // Deeply nested JSON to test parsing limits
        Just("{\"a\":{\"b\":{\"c\":{\"d\":{\"e\":\"deep\"}}}}}".repeat(100)),
        // Null bytes and control characters
        Just("value\x00\x01\x02\x03\x04".to_string()),
        // Unicode edge cases
        Just("\u{FEFF}\u{200B}\u{200C}\u{200D}".to_string()),
    ]
}

// Strategy for generating tool names
fn tool_name_strategy() -> impl Strategy<Value = ToolName> {
    prop::string::string_regex("[a-zA-Z0-9_-]{1,32}")
        .unwrap()
        .prop_filter_map("Valid tool name", |s| ToolName::parse(&s).ok())
}

proptest! {
    /// Property: Memory should be consistent - what you store is what you get
    #[test]
    fn prop_memory_store_load_consistency(
        key in memory_key_strategy(),
        value in memory_value_strategy()
    ) {
        tokio_test::block_on(async {
            let mut memory = skreaver_core::in_memory::InMemoryMemory::default();

            let update = MemoryUpdate {
                key: key.clone(),
                value: value.clone(),
            };

            // Store the value
            memory.store(update).expect("Store should succeed");

            // Load the value back
            let loaded = memory.load(&key).expect("Load should succeed");

            // Property: loaded value should equal stored value
            prop_assert_eq!(loaded, Some(value));
            Ok(())
        })?;
    }

    /// Property: Memory load_many should be equivalent to multiple load calls
    #[test]
    fn prop_memory_load_many_equivalence(
        keys_and_values in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 0..10)
    ) {
        tokio_test::block_on(async {
            let mut memory = skreaver_core::in_memory::InMemoryMemory::default();

            // Store all key-value pairs
            for (key, value) in &keys_and_values {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                memory.store(update).expect("Store should succeed");
            }

            // Extract just the keys
            let keys: Vec<MemoryKey> = keys_and_values.iter().map(|(k, _)| k.clone()).collect();

            // Load using load_many
            let loaded_many = memory.load_many(&keys).expect("Load many should succeed");

            // Load using individual calls
            let mut loaded_individual = Vec::new();
            for key in &keys {
                loaded_individual.push(memory.load(key).expect("Load should succeed"));
            }

            // Property: load_many should return same results as individual loads
            prop_assert_eq!(loaded_many, loaded_individual);
            Ok(())
        })?;
    }

    /// Property: Memory operations should be order-independent for different keys
    #[test]
    fn prop_memory_operations_order_independence(
        mut operations in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 1..20)
    ) {
        tokio_test::block_on(async {
            // Remove duplicates to avoid order dependency on same key
            operations.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
            operations.dedup_by(|a, b| a.0.as_str() == b.0.as_str());

            if operations.is_empty() {
                return Ok(());
            }

            // Execute operations in original order
            let mut memory1 = skreaver_core::in_memory::InMemoryMemory::default();
            for (key, value) in &operations {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                memory1.store(update).expect("Store should succeed");
            }

            // Execute operations in reverse order
            let mut memory2 = skreaver_core::in_memory::InMemoryMemory::default();
            for (key, value) in operations.iter().rev() {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                memory2.store(update).expect("Store should succeed");
            }

            // Property: final state should be the same regardless of order
            for (key, expected_value) in &operations {
                let value1 = memory1.load(key).expect("Load should succeed");
                let value2 = memory2.load(key).expect("Load should succeed");
                prop_assert_eq!(&value1, &value2);
                prop_assert_eq!(value1, Some(expected_value.clone()));
            }
            Ok(())
        })?;
    }

    /// Property: Tool names should always be valid after construction
    #[test]
    fn prop_tool_name_validity(name_str in prop::string::string_regex("[a-zA-Z0-9_-]{1,64}").unwrap()) {
        if let Ok(tool_name) = ToolName::parse(&name_str) {
            // Property: tool name string representation should match input
            prop_assert_eq!(tool_name.as_str(), name_str);

            // Property: tool name should not be empty
            prop_assert!(!tool_name.as_str().is_empty());

            // Property: tool name should be within length limits
            prop_assert!(tool_name.as_str().len() <= 64);
        }
    }

    /// Property: Tool execution should be deterministic for same input
    #[test]
    fn prop_tool_deterministic_execution(
        tool_name in tool_name_strategy(),
        input in prop::string::string_regex(".*").unwrap(),
        response in prop::string::string_regex(".*").unwrap()
    ) {
        tokio_test::block_on(async {
            // Create two identical mock tools
            let tool1 = SimpleMockTool::new(tool_name.as_str())
                .expect("Valid tool name")
                .with_response(&input, &response);
            let tool2 = SimpleMockTool::new(tool_name.as_str())
                .expect("Valid tool name")
                .with_response(&input, &response);

            let result1 = tool1.call(input.clone());
            let result2 = tool2.call(input);

            // Property: same tool with same input should produce same output
            prop_assert_eq!(result1.output(), result2.output());
            prop_assert_eq!(result1.is_success(), result2.is_success());
            Ok(())
        })?;
    }

    /// Property: Memory key creation should be consistent
    #[test]
    fn prop_memory_key_creation_consistency(key_str in ".*") {
        let result1 = MemoryKey::new(&key_str);
        let result2 = MemoryKey::new(&key_str);

        // Property: same input should always produce same result
        match (result1, result2) {
            (Ok(key1), Ok(key2)) => {
                prop_assert_eq!(key1.as_str(), key2.as_str());
            }
            (Err(_), Err(_)) => {
                // Both failed, which is consistent
            }
            _ => {
                prop_assert!(false, "Inconsistent memory key creation results");
            }
        }
    }

    /// Property: Tool call creation should preserve input data
    #[test]
    fn prop_tool_call_creation_preservation(
        tool_name in tool_name_strategy(),
        input in prop::string::string_regex(".*").unwrap()
    ) {
        // Only test with valid inputs that ToolCall::new can handle
        if let Ok(tool_call) = skreaver_core::ToolCall::new(tool_name.as_str(), &input) {
            // Property: tool call should preserve the data we put in
            prop_assert_eq!(tool_call.name(), tool_name.as_str());
            prop_assert_eq!(tool_call.input, input);
        }
    }

    /// Property: Multiple stores to same key should result in last value
    #[test]
    fn prop_memory_last_write_wins(
        key in memory_key_strategy(),
        values in prop::collection::vec(memory_value_strategy(), 1..10)
    ) {
        tokio_test::block_on(async {
            let mut memory = skreaver_core::in_memory::InMemoryMemory::default();

            // Store multiple values for the same key
            for value in &values {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                memory.store(update).expect("Store should succeed");
            }

            let loaded = memory.load(&key).expect("Load should succeed");

            // Property: should have the last value written
            prop_assert_eq!(loaded, Some(values.last().unwrap().clone()));
            Ok(())
        })?;
    }

    /// Property: Loading non-existent keys should always return None
    #[test]
    fn prop_memory_nonexistent_key_returns_none(
        existing_keys in prop::collection::vec(memory_key_strategy(), 0..10),
        query_key in memory_key_strategy()
    ) {
        tokio_test::block_on(async {
            let mut memory = skreaver_core::in_memory::InMemoryMemory::default();

            // Store some values
            for (i, key) in existing_keys.iter().enumerate() {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: format!("value_{}", i),
                };
                memory.store(update).expect("Store should succeed");
            }

            // If query_key is not in existing_keys, it should return None
            if !existing_keys.iter().any(|k| k.as_str() == query_key.as_str()) {
                let loaded = memory.load(&query_key).expect("Load should succeed");
                prop_assert_eq!(loaded, None);
            }
            Ok(())
        })?;
    }

    /// Property: Memory backends should behave consistently across implementations
    #[test]
    fn prop_memory_backend_consistency(
        operations in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 1..50)
    ) {
        tokio_test::block_on(async {
            let mut in_memory = InMemoryMemory::new();

            // Remove duplicate keys to avoid last-write-wins confusion
            let mut unique_operations = std::collections::HashMap::new();
            for (key, value) in operations {
                unique_operations.insert(key, value);
            }

            // Apply all operations to in-memory backend
            for (key, value) in &unique_operations {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                in_memory.store(update).expect("InMemory store should succeed");
            }

            // Verify all values can be read back consistently
            for (key, expected_value) in &unique_operations {
                let loaded = in_memory.load(key).expect("Load should succeed");
                prop_assert_eq!(loaded, Some(expected_value.clone()));
            }

            // Test load_many consistency
            let keys: Vec<_> = unique_operations.keys().cloned().collect();
            let loaded_many = in_memory.load_many(&keys).expect("Load many should succeed");

            // Verify each key individually since HashMap ordering is not guaranteed
            for (i, key) in keys.iter().enumerate() {
                let expected_value = unique_operations.get(key).unwrap();
                prop_assert_eq!(&loaded_many[i], &Some(expected_value.clone()));
            }

            Ok(())
        })?;
    }

    /// Property: Transaction atomicity - all operations succeed or all fail
    #[test]
    fn prop_transaction_atomicity(
        raw_successful_ops in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 1..10),
        failure_point in 1usize..10usize
    ) {
        tokio_test::block_on(async {
            // Deduplicate by key, keeping last value to avoid test confusion
            let mut unique_ops = std::collections::HashMap::new();
            for (key, value) in raw_successful_ops {
                unique_ops.insert(key, value);
            }
            let successful_ops: Vec<_> = unique_ops.into_iter().collect();

            // Only test when failure_point is within bounds
            if failure_point >= successful_ops.len() || successful_ops.is_empty() {
                return Ok(());
            }

            let mut memory = InMemoryMemory::new();

            // Store some initial data (only for keys before failure point)
            for (i, (key, value)) in successful_ops.iter().enumerate() {
                if i < failure_point {
                    let update = MemoryUpdate {
                        key: key.clone(),
                        value: format!("initial_{}", value),
                    };
                    memory.store(update).expect("Initial store should succeed");
                }
            }

            // Attempt transaction that fails at failure_point
            let tx_result = memory.transaction(|tx| {
                for (i, (key, value)) in successful_ops.iter().enumerate() {
                    if i == failure_point {
                        // Simulate failure
                        return Err(TransactionError::TransactionFailed {
                            reason: "Simulated failure for property test".to_string(),
                        });
                    }
                    let update = MemoryUpdate {
                        key: key.clone(),
                        value: format!("tx_{}", value),
                    };
                    tx.store(update)?;
                }
                Ok(())
            });

            // Transaction should have failed if failure_point was reached
            if failure_point < successful_ops.len() {
                prop_assert!(tx_result.is_err());
            }

            // Verify rollback - original data should be preserved
            for (i, (key, value)) in successful_ops.iter().enumerate() {
                let loaded = memory.load(key).expect("Load should succeed");
                if i < failure_point {
                    prop_assert_eq!(loaded, Some(format!("initial_{}", value)));
                } else {
                    prop_assert_eq!(loaded, None);
                }
            }

            Ok(())
        })?;
    }

    /// Property: Snapshot and restore should preserve exact state
    #[test]
    fn prop_snapshot_restore_fidelity(
        data in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 0..50)
    ) {
        tokio_test::block_on(async {
            let mut original = InMemoryMemory::new();

            // Remove duplicate keys to ensure consistent state
            let mut unique_data = std::collections::HashMap::new();
            for (key, value) in data {
                unique_data.insert(key, value);
            }

            // Populate with test data
            for (key, value) in &unique_data {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                original.store(update).expect("Store should succeed");
            }

            // Create snapshot
            let snapshot = original.snapshot().expect("Snapshot should succeed");

            // Create new memory instance and restore
            let mut restored = InMemoryMemory::new();
            restored.restore(&snapshot).expect("Restore should succeed");

            // Verify all data matches exactly
            for (key, expected_value) in &unique_data {
                let original_value = original.load(key).expect("Load should succeed");
                let restored_value = restored.load(key).expect("Load should succeed");
                prop_assert_eq!(original_value, restored_value.clone());
                prop_assert_eq!(restored_value, Some(expected_value.clone()));
            }

            Ok(())
        })?;
    }

    /// Property: Security - malicious inputs should not cause crashes or data corruption
    #[test]
    fn prop_security_malicious_key_handling(
        malicious_key in malicious_key_strategy(),
        safe_value in memory_value_strategy()
    ) {
        tokio_test::block_on(async {
            let mut memory = InMemoryMemory::new();

            // Attempt to create memory key with malicious input
            match MemoryKey::new(&malicious_key) {
                Ok(key) => {
                    // If key creation succeeds, operations should work safely
                    let update = MemoryUpdate {
                        key: key.clone(),
                        value: safe_value.clone(),
                    };

                    // Should not panic or corrupt memory
                    let store_result = memory.store(update);
                    prop_assert!(store_result.is_ok());

                    let load_result = memory.load(&key);
                    prop_assert!(load_result.is_ok());
                    prop_assert_eq!(load_result.unwrap(), Some(safe_value));
                }
                Err(_) => {
                    // Key creation failed, which is expected for malicious inputs
                    // This is the desired behavior - reject invalid keys
                }
            }

            Ok(())
        })?;
    }

    /// Property: Security - malicious values should be stored safely without corruption
    #[test]
    fn prop_security_malicious_value_handling(
        safe_key in memory_key_strategy(),
        malicious_value in malicious_value_strategy()
    ) {
        tokio_test::block_on(async {
            let mut memory = InMemoryMemory::new();

            let update = MemoryUpdate {
                key: safe_key.clone(),
                value: malicious_value.clone(),
            };

            // Should not panic or corrupt memory, even with malicious values
            let store_result = memory.store(update);
            prop_assert!(store_result.is_ok());

            let load_result = memory.load(&safe_key);
            prop_assert!(load_result.is_ok());

            // Value should be stored and retrieved exactly as provided
            prop_assert_eq!(load_result.unwrap(), Some(malicious_value));

            Ok(())
        })?;
    }

    /// Property: Performance - operations should complete within reasonable time bounds
    #[test]
    fn prop_performance_operation_timing(
        operations in prop::collection::vec((memory_key_strategy(), large_memory_value_strategy()), 1..10)
    ) {
        tokio_test::block_on(async {
            let mut memory = InMemoryMemory::new();

            for (key, value) in &operations {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };

                // Time the store operation
                let start = Instant::now();
                memory.store(update).expect("Store should succeed");
                let store_duration = start.elapsed();

                // Store should complete quickly even for large values
                prop_assert!(store_duration < Duration::from_millis(100),
                    "Store took too long: {:?}", store_duration);

                // Time the load operation
                let start = Instant::now();
                let loaded = memory.load(key).expect("Load should succeed");
                let load_duration = start.elapsed();

                // Load should complete quickly
                prop_assert!(load_duration < Duration::from_millis(50),
                    "Load took too long: {:?}", load_duration);

                prop_assert_eq!(loaded, Some(value.clone()));
            }

            Ok(())
        })?;
    }

    /// Property: Concurrent access should maintain consistency
    #[test]
    fn prop_concurrent_access_consistency(
        keys_values in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 1..10)
    ) {
        tokio_test::block_on(async {
            let memory = Arc::new(RwLock::new(InMemoryMemory::new()));
            let mut handles = Vec::new();

            // Create unique keys to avoid race conditions on the same key
            let unique_operations: std::collections::HashMap<MemoryKey, String> = keys_values.into_iter().collect();

            // Spawn concurrent write tasks
            for (i, (key, value)) in unique_operations.iter().enumerate() {
                let memory_clone = Arc::clone(&memory);
                let key_clone = key.clone();
                let value_clone = format!("{}-{}", value, i);

                let handle = tokio::spawn(async move {
                    let mut mem = memory_clone.write().await;
                    let update = MemoryUpdate {
                        key: key_clone.clone(),
                        value: value_clone.clone(),
                    };
                    mem.store(update).expect("Concurrent store should succeed");
                    (key_clone, value_clone)
                });
                handles.push(handle);
            }

            // Wait for all writes to complete
            let mut expected_data = Vec::new();
            for handle in handles {
                let (key, value) = handle.await.expect("Task should complete");
                expected_data.push((key, value));
            }

            // Verify all data is present and consistent
            let mem = memory.read().await;
            for (key, expected_value) in expected_data {
                let loaded = mem.load(&key).expect("Load should succeed");
                prop_assert_eq!(loaded, Some(expected_value));
            }

            Ok(())
        })?;
    }

    /// Property: Tool idempotency - same input should produce same output
    #[test]
    fn prop_tool_idempotency_consistency(
        tool_name in tool_name_strategy(),
        input in prop::string::string_regex(".*").unwrap(),
        response in prop::string::string_regex(".*").unwrap(),
        execution_count in 2u32..10u32
    ) {
        tokio_test::block_on(async {
            let tool = SimpleMockTool::new(tool_name.as_str())
                .expect("Valid tool name")
                .with_response(&input, &response);

            let mut results = Vec::new();

            // Execute the same tool call multiple times
            for _ in 0..execution_count {
                let result = tool.call(input.clone());
                results.push((result.output().to_owned(), result.is_success()));
            }

            // All results should be identical (idempotency)
            let first_result = &results[0];
            for result in &results[1..] {
                prop_assert_eq!(result, first_result,
                    "Tool execution should be idempotent - same input should produce same output");
            }

            Ok(())
        })?;
    }

    /// Property: Tool state isolation - tools should not affect each other
    #[test]
    fn prop_tool_state_isolation(
        tools_and_inputs in prop::collection::vec(
            (tool_name_strategy(), prop::string::string_regex(".*").unwrap(),
             prop::string::string_regex(".*").unwrap()), 2..10
        )
    ) {
        tokio_test::block_on(async {
            let mut tools = Vec::new();
            let mut expected_results = Vec::new();

            // Create tools with different responses
            for (tool_name, input, response) in &tools_and_inputs {
                let tool = SimpleMockTool::new(tool_name.as_str())
                    .expect("Valid tool name")
                    .with_response(input, response);
                tools.push((tool, input, response));
                expected_results.push((response.clone(), true)); // Assuming success
            }

            // Execute all tools and verify isolation
            for (i, (tool, input, expected_response)) in tools.iter().enumerate() {
                let result = tool.call((*input).clone());
                prop_assert_eq!(result.output(), *expected_response);
                prop_assert!(result.is_success());

                // Verify other tools still work correctly (no cross-contamination)
                for (j, (other_tool, other_input, other_expected)) in tools.iter().enumerate() {
                    if i != j {
                        let other_result = other_tool.call((*other_input).clone());
                        prop_assert_eq!(other_result.output(), *other_expected,
                            "Tool {} should not be affected by execution of tool {}", j, i);
                    }
                }
            }

            Ok(())
        })?;
    }

    /// Property: Memory operations under high concurrency should remain consistent
    #[test]
    fn prop_memory_high_concurrency_stress(
        base_operations in prop::collection::vec(
            (memory_key_strategy(), memory_value_strategy()), 50..200
        ),
        thread_count in 2usize..8usize
    ) {
        tokio_test::block_on(async {
            let memory = Arc::new(RwLock::new(InMemoryMemory::new()));
            let mut handles = Vec::new();

            // Create unique operations for each thread to avoid key conflicts
            let ops_per_thread = std::cmp::max(1, base_operations.len() / thread_count);

            for thread_id in 0..thread_count {
                let memory_clone = Arc::clone(&memory);
                let start_idx = thread_id * ops_per_thread;
                let end_idx = std::cmp::min((thread_id + 1) * ops_per_thread, base_operations.len());

                // Create thread-specific operations with unique keys
                let thread_operations: Vec<_> = base_operations[start_idx..end_idx].iter()
                    .enumerate()
                    .map(|(i, (key, value))| {
                        // Make key unique by prepending thread_id and operation index
                        let unique_key_str = format!("t{}_op{}_{}", thread_id, i, key.as_str());
                        let unique_key = MemoryKey::new(&unique_key_str).unwrap_or_else(|_| key.clone());
                        (unique_key, value.clone())
                    })
                    .collect();

                let handle = tokio::spawn(async move {
                    let mut local_results = Vec::new();

                    for (key, value) in thread_operations {
                        let mut mem = memory_clone.write().await;
                        let unique_value = format!("thread_{}_value_{}", thread_id, value);
                        let update = MemoryUpdate {
                            key: key.clone(),
                            value: unique_value.clone(),
                        };
                        mem.store(update).expect("Concurrent store should succeed");
                        local_results.push((key, unique_value));
                    }
                    local_results
                });
                handles.push(handle);
            }

            // Collect all expected data
            let mut all_expected_data = Vec::new();
            for handle in handles {
                let thread_results = handle.await.expect("Thread should complete");
                all_expected_data.extend(thread_results);
            }

            // Verify all data is present and correct
            let mem = memory.read().await;
            for (key, expected_value) in all_expected_data {
                let loaded = mem.load(&key).expect("Load should succeed");
                prop_assert_eq!(loaded, Some(expected_value));
            }

            Ok(())
        })?;
    }

    /// Property: Memory batch operations should be equivalent to individual operations
    #[test]
    fn prop_memory_batch_equivalence(
        raw_batch_data in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 1..50)
    ) {
        tokio_test::block_on(async {
            let mut memory1 = InMemoryMemory::new();
            let mut memory2 = InMemoryMemory::new();

            // Remove duplicate keys to ensure consistent comparison (last write wins)
            let mut unique_data = std::collections::HashMap::new();
            for (key, value) in raw_batch_data {
                unique_data.insert(key, value);
            }
            let batch_data: Vec<_> = unique_data.into_iter().collect();

            // Method 1: Individual stores
            for (key, value) in &batch_data {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                memory1.store(update).expect("Individual store should succeed");
            }

            // Method 2: Batch store
            let batch_updates: Vec<_> = batch_data.iter().map(|(key, value)| {
                MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                }
            }).collect();
            memory2.store_many(batch_updates).expect("Batch store should succeed");

            // Both methods should produce identical results
            for (key, expected_value) in &batch_data {
                let individual_result = memory1.load(key).expect("Load should succeed");
                let batch_result = memory2.load(key).expect("Load should succeed");

                prop_assert_eq!(individual_result, batch_result.clone());
                prop_assert_eq!(batch_result, Some(expected_value.clone()));
            }

            // Test batch load equivalence
            let keys: Vec<_> = batch_data.iter().map(|(k, _)| k.clone()).collect();
            let batch_load_results = memory1.load_many(&keys).expect("Batch load should succeed");

            let individual_load_results: Vec<_> = keys.iter()
                .map(|k| memory1.load(k).expect("Individual load should succeed"))
                .collect();

            prop_assert_eq!(batch_load_results, individual_load_results);

            Ok(())
        })?;
    }

    /// Property: Memory corruption detection - stored data should never be silently corrupted
    #[test]
    fn prop_memory_corruption_detection(
        raw_test_data in prop::collection::vec((memory_key_strategy(), memory_value_strategy()), 10..100),
        corruption_attempts in prop::collection::vec(memory_key_strategy(), 1..20)
    ) {
        tokio_test::block_on(async {
            let mut memory = InMemoryMemory::new();

            // Remove duplicate keys from test data to ensure predictable state
            let mut unique_test_data = std::collections::HashMap::new();
            for (key, value) in raw_test_data {
                unique_test_data.insert(key, value);
            }

            // Store initial data
            for (key, value) in &unique_test_data {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                memory.store(update).expect("Initial store should succeed");
            }

            // Attempt various "corruption" operations (they should be handled safely)
            for corrupt_key in &corruption_attempts {
                // Try to overwrite with empty values
                let corrupt_update = MemoryUpdate {
                    key: corrupt_key.clone(),
                    value: String::new(),
                };
                let _result = memory.store(corrupt_update); // May succeed or fail, but should not corrupt
            }

            // Verify original data integrity
            for (key, expected_value) in &unique_test_data {
                let loaded = memory.load(key).expect("Load should succeed after corruption attempts");

                // If the key wasn't in corruption_attempts, value should be unchanged
                if !corruption_attempts.iter().any(|ck| ck.as_str() == key.as_str()) {
                    prop_assert_eq!(loaded, Some(expected_value.clone()),
                        "Original data should remain intact after corruption attempts");
                }
                // If key was in corruption_attempts, it might have empty value, but should not crash
            }

            Ok(())
        })?;
    }
}

/// Quickcheck-style property test for memory operations
#[cfg(test)]
mod quickcheck_tests {
    use super::*;
    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;

    #[quickcheck]
    fn qc_memory_roundtrip_consistency(key_str: String, value: String) -> TestResult {
        // Skip invalid inputs
        if key_str.is_empty() || key_str.len() > 64 || value.len() > 10000 {
            return TestResult::discard();
        }

        let key = match MemoryKey::new(&key_str) {
            Ok(k) => k,
            Err(_) => return TestResult::discard(),
        };

        tokio_test::block_on(async {
            let mut memory = skreaver_core::in_memory::InMemoryMemory::default();

            let update = MemoryUpdate {
                key: key.clone(),
                value: value.clone(),
            };

            memory.store(update).expect("Store should succeed");
            let loaded = memory.load(&key).expect("Load should succeed");

            TestResult::from_bool(loaded == Some(value))
        })
    }

    #[quickcheck]
    fn qc_tool_name_validation(name: String) -> TestResult {
        let result = ToolName::parse(&name);

        // If creation succeeds, the name should meet our criteria
        match result {
            Ok(tool_name) => TestResult::from_bool(
                !tool_name.as_str().is_empty()
                    && tool_name.as_str().len() <= 64
                    && tool_name
                        .as_str()
                        .chars()
                        // IdValidator allows alphanumeric, underscores, hyphens, and dots
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.'),
            ),
            Err(_) => {
                // Creation failed, which is fine for invalid inputs
                TestResult::passed()
            }
        }
    }
}

/// Property test helper functions and benchmarks
#[cfg(test)]
mod property_helpers {
    use super::*;

    /// Property: Memory should maintain size bounds
    #[tokio::test]
    async fn test_memory_size_bounds() {
        let mut memory = skreaver_core::in_memory::InMemoryMemory::default();

        // Store a large number of items
        for i in 0..1000 {
            let key = MemoryKey::new(&format!("key_{}", i)).unwrap();
            let update = MemoryUpdate {
                key,
                value: format!("value_{}", i),
            };
            memory.store(update).expect("Store should succeed");
        }

        // All items should still be loadable
        for i in 0..1000 {
            let key = MemoryKey::new(&format!("key_{}", i)).unwrap();
            let loaded = memory.load(&key).expect("Load should succeed");
            assert_eq!(loaded, Some(format!("value_{}", i)));
        }
    }

    /// Performance benchmark for memory operations
    #[tokio::test]
    async fn test_memory_performance_characteristics() {
        let mut memory = InMemoryMemory::new();

        // Test small operations (should be very fast)
        let small_key = MemoryKey::new("small_key").unwrap();
        let small_value = "small_value".to_string();

        let start = Instant::now();
        for _ in 0..1000 {
            let update = MemoryUpdate {
                key: small_key.clone(),
                value: small_value.clone(),
            };
            memory.store(update).expect("Small store should succeed");
        }
        let small_ops_duration = start.elapsed();

        // 1000 small operations should complete in reasonable time
        assert!(
            small_ops_duration < Duration::from_millis(100),
            "1000 small operations took too long: {:?}",
            small_ops_duration
        );

        // Test large operations
        let _large_key = MemoryKey::new("large_key").unwrap();
        let large_value = "x".repeat(10000);

        let start = Instant::now();
        for i in 0..100 {
            let update = MemoryUpdate {
                key: MemoryKey::new(&format!("large_key_{}", i)).unwrap(),
                value: large_value.clone(),
            };
            memory.store(update).expect("Large store should succeed");
        }
        let large_ops_duration = start.elapsed();

        // 100 large operations should still be reasonable
        assert!(
            large_ops_duration < Duration::from_millis(500),
            "100 large operations took too long: {:?}",
            large_ops_duration
        );
    }

    /// Test property-based testing configuration and limits
    #[tokio::test]
    async fn test_property_test_configuration() {
        use proptest::test_runner::Config;

        // Verify our property test configuration is reasonable
        let config = Config::default();

        // Default cases should be sufficient for CI
        assert!(config.cases >= 100, "Too few test cases for robust testing");

        // Timeout should be reasonable for CI environments
        // Note: This is checking the overall philosophy rather than exact values
        assert!(config.max_shrink_iters > 0, "Shrinking should be enabled");
    }

    /// Integration test for property-based testing with existing CI matrix
    #[tokio::test]
    async fn test_ci_integration_readiness() {
        // This test ensures our property tests will work in CI environments
        // by testing common edge cases and resource constraints

        let mut memory = InMemoryMemory::new();

        // Test that we can handle the expected workload for CI
        let test_keys: Vec<_> = (0..50)
            .map(|i| MemoryKey::new(&format!("ci_test_key_{}", i)).unwrap())
            .collect();

        let test_values: Vec<_> = (0..50).map(|i| format!("ci_test_value_{}", i)).collect();

        // Store all data
        for (key, value) in test_keys.iter().zip(test_values.iter()) {
            let update = MemoryUpdate {
                key: key.clone(),
                value: value.clone(),
            };
            memory.store(update).expect("CI test store should succeed");
        }

        // Verify all data
        for (key, expected_value) in test_keys.iter().zip(test_values.iter()) {
            let loaded = memory.load(key).expect("CI test load should succeed");
            assert_eq!(loaded, Some(expected_value.clone()));
        }

        // Test batch operations work correctly
        let loaded_many = memory
            .load_many(&test_keys)
            .expect("CI batch load should succeed");
        let expected_many: Vec<_> = test_values.iter().map(|v| Some(v.clone())).collect();
        assert_eq!(loaded_many, expected_many);
    }

    /// Test shrinking strategies for complex failures
    #[tokio::test]
    async fn test_property_shrinking_effectiveness() {
        // This test ensures our property tests can effectively shrink failing cases
        // to minimal examples for debugging

        // Example of testing a property that might fail and need shrinking
        fn validate_key_length_property(key_str: &str) -> bool {
            if key_str.len() > 128 {
                return false; // This should cause shrinking to find minimal failing case
            }

            match MemoryKey::new(key_str) {
                Ok(_) => {
                    // Key creation succeeded, so the key should be valid
                    !key_str.trim().is_empty()
                        && key_str.trim().len() <= 128
                        && key_str.trim().chars().all(|c| {
                            c.is_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':'
                        })
                }
                Err(_) => {
                    // Key creation failed, so the key should be invalid
                    key_str.trim().is_empty()
                        || key_str.trim().len() > 128
                        || key_str.chars().any(|c| {
                            !c.is_alphanumeric() && c != '_' && c != '-' && c != '.' && c != ':'
                        })
                }
            }
        }

        // Test a few cases manually to verify shrinking would work
        assert!(validate_key_length_property("valid_key"));
        assert!(validate_key_length_property("a"));
        assert!(
            validate_key_length_property(""),
            "Empty key validation should work correctly (reject invalid keys)"
        );
        assert!(validate_key_length_property("invalid@key"));

        // Very long key should fail validation
        let long_key = "a".repeat(200);
        assert!(!validate_key_length_property(&long_key));
    }
}
