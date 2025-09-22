//! Property-Based Tests for Memory Consistency and Tool Idempotency
//!
//! These tests use property-based testing to verify invariants across
//! the system, particularly focusing on memory consistency and tool
//! behavior properties that should hold regardless of input.

use proptest::prelude::*;
use skreaver_core::{
    memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter},
    tool::{Tool, ToolName},
};
use skreaver_testing::mock_tools::MockTool;

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

// Strategy for generating tool names
fn tool_name_strategy() -> impl Strategy<Value = ToolName> {
    prop::string::string_regex("[a-zA-Z0-9_-]{1,32}")
        .unwrap()
        .prop_filter_map("Valid tool name", |s| ToolName::new(&s).ok())
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
        if let Ok(tool_name) = ToolName::new(&name_str) {
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
            let tool1 = MockTool::new(tool_name.as_str())
                .with_response(&input, &response);
            let tool2 = MockTool::new(tool_name.as_str())
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
        let result = ToolName::new(&name);

        // If creation succeeds, the name should meet our criteria
        match result {
            Ok(tool_name) => TestResult::from_bool(
                !tool_name.as_str().is_empty()
                    && tool_name.as_str().len() <= 64
                    && tool_name
                        .as_str()
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-'),
            ),
            Err(_) => {
                // Creation failed, which is fine for invalid inputs
                TestResult::passed()
            }
        }
    }
}

/// Property test helper functions
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
}
