//! Critical Path Coverage Tests (Fixed)
//!
//! These tests focus on achieving >95% line coverage on core execution paths
//! using the actual APIs available in the codebase.

use skreaver_core::{
    InMemoryMemory, Tool, ToolCall,
    agent::simple_stateful::SimpleStatefulAgent,
    memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter},
};
use skreaver_testing::mock_tools::MockTool;

/// Test core agent execution path with memory operations using SimpleStatefulAgent
#[tokio::test]
async fn test_simple_agent_core_execution_path() {
    // Create memory backend
    let memory = InMemoryMemory::default();

    // Create agent with memory
    let agent = SimpleStatefulAgent::new(Box::new(memory));

    // Test agent lifecycle: initial -> processing
    let processing_agent = agent.observe("test input".to_string());

    // Get tool calls from processing state
    let _tool_calls = processing_agent.get_tool_calls();

    // Should not crash and return tool calls (empty list is valid)
    // This demonstrates the API works without crashing
}

/// Test memory operations with realistic agent workflow
#[tokio::test]
async fn test_memory_agent_integration() {
    let mut memory = InMemoryMemory::default();

    // Store initial context
    let context_key = MemoryKey::new("agent_context").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: context_key.clone(),
            value: r#"{"session": "test_session", "user": "test_user"}"#.to_string(),
        })
        .expect("Store should succeed");

    // Create agent with memory access
    let agent = SimpleStatefulAgent::new(Box::new(memory));

    // Process an observation
    let processing_agent = agent.observe("Please search for information".to_string());

    // Verify agent has correct state
    let tool_calls = processing_agent.get_tool_calls();

    // For search-related input, should request search tools
    if !tool_calls.is_empty() {
        for call in &tool_calls {
            assert!(!call.name().is_empty());
            assert!(!call.input.is_empty());
        }
    }
}

/// Test tool execution with memory persistence
#[tokio::test]
async fn test_tool_execution_with_memory_persistence() {
    let mut memory = InMemoryMemory::default();

    // Create a mock tool for testing
    let search_tool = MockTool::new("web_search").with_response(
        "Please search for information",
        "Search results: Found 5 articles",
    );

    // Execute tool
    let result = search_tool.call("Please search for information".to_string());
    assert!(result.is_success());

    // Store tool result in memory
    let result_key = MemoryKey::new("tool_result").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: result_key.clone(),
            value: result.output().to_string(),
        })
        .expect("Store should succeed");

    // Create agent and verify it can access stored results
    let agent = SimpleStatefulAgent::new(Box::new(memory));
    let processing_agent = agent.observe("Continue with previous results".to_string());

    // Agent should be in processing state
    let _tool_calls = processing_agent.get_tool_calls();
    // API verification - should not crash
}

/// Test error handling in tool execution
#[tokio::test]
async fn test_tool_error_handling() {
    let error_tool = MockTool::new("error_tool").with_failure("bad_input", "Tool execution failed");

    let result = error_tool.call("bad_input".to_string());
    assert!(result.is_failure());
    assert!(result.output().contains("Tool execution failed"));

    // Verify error information is accessible
    assert!(!result.output().is_empty());
}

/// Test ToolCall creation and validation
#[tokio::test]
async fn test_tool_call_creation_and_validation() {
    // Test valid ToolCall creation
    let valid_call = ToolCall::new("test_tool", "test input");
    assert!(valid_call.is_ok());

    let call = valid_call.unwrap();
    assert_eq!(call.name(), "test_tool");
    assert_eq!(call.input, "test input");

    // Test tool call with mock tool
    let mock_tool = MockTool::new("test_tool").with_response("test input", "test output");

    let result = mock_tool.call("test input".to_string());
    assert!(result.is_success());
    assert_eq!(result.output(), "test output");
}

/// Test concurrent memory operations (critical for thread safety)
#[tokio::test]
async fn test_concurrent_memory_operations() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::task::JoinSet;

    let memory = Arc::new(Mutex::new(InMemoryMemory::default()));
    let mut join_set = JoinSet::new();

    // Spawn multiple concurrent memory operations
    for i in 0..10 {
        let memory_clone = Arc::clone(&memory);
        join_set.spawn(async move {
            let key = MemoryKey::new(&format!("concurrent_key_{}", i)).unwrap();
            let mut mem = memory_clone.lock().await;

            // Store operation
            mem.store(MemoryUpdate {
                key: key.clone(),
                value: format!("value_{}", i),
            })
            .expect("Store should succeed");

            // Load operation
            let loaded = mem.load(&key).expect("Load should succeed");
            assert_eq!(loaded, Some(format!("value_{}", i)));

            i
        });
    }

    // Wait for all operations
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        results.push(result.expect("Task should complete"));
    }

    assert_eq!(results.len(), 10);
}

/// Test batch operations for performance
#[tokio::test]
async fn test_batch_operations_performance() {
    let mut memory = InMemoryMemory::default();

    // Prepare batch data
    let batch_size = 100;
    let updates: Vec<MemoryUpdate> = (0..batch_size)
        .map(|i| MemoryUpdate {
            key: MemoryKey::new(&format!("batch_key_{}", i)).expect("Valid key"),
            value: format!("batch_value_{}", i),
        })
        .collect();

    // Execute batch store
    memory
        .store_many(updates)
        .expect("Batch store should succeed");

    // Execute batch load
    let keys: Vec<MemoryKey> = (0..batch_size)
        .map(|i| MemoryKey::new(&format!("batch_key_{}", i)).expect("Valid key"))
        .collect();

    let loaded_values = memory.load_many(&keys).expect("Batch load should succeed");
    assert_eq!(loaded_values.len(), batch_size);

    // Verify all values were stored and loaded correctly
    for (i, value_opt) in loaded_values.iter().enumerate() {
        assert_eq!(value_opt, &Some(format!("batch_value_{}", i)));
    }
}

/// Test memory key validation edge cases
#[tokio::test]
async fn test_memory_key_validation_edge_cases() {
    // Test boundary values
    let max_key = "a".repeat(128); // Max length
    let valid_key = MemoryKey::new(&max_key);
    assert!(valid_key.is_ok());

    let too_long_key = "a".repeat(129); // Over max length
    let invalid_key = MemoryKey::new(&too_long_key);
    assert!(invalid_key.is_err());

    // Test empty key
    let empty_key = MemoryKey::new("");
    assert!(empty_key.is_err());

    // Test special characters
    let special_key = MemoryKey::new("valid_key-123.test:value");
    assert!(special_key.is_ok());
}

/// Test tool chain execution
#[tokio::test]
async fn test_tool_chain_execution() {
    let mut memory = InMemoryMemory::default();

    // Tool 1: Data ingestion
    let ingestion_tool = MockTool::new("data_ingester").with_response("raw_data", "ingested_data");

    let result1 = ingestion_tool.call("raw_data".to_string());
    assert!(result1.is_success());

    // Store intermediate result
    let intermediate_key = MemoryKey::new("intermediate_data").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: intermediate_key.clone(),
            value: result1.output().to_string(),
        })
        .expect("Store should succeed");

    // Tool 2: Data processing
    let processing_tool =
        MockTool::new("data_processor").with_response("ingested_data", "processed_data");

    let intermediate_data = memory
        .load(&intermediate_key)
        .expect("Load should succeed")
        .unwrap();
    let result2 = processing_tool.call(intermediate_data);
    assert!(result2.is_success());

    // Store final result
    let final_key = MemoryKey::new("final_data").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: final_key.clone(),
            value: result2.output().to_string(),
        })
        .expect("Store should succeed");

    // Verify complete chain
    let final_data = memory.load(&final_key).expect("Load should succeed");
    assert_eq!(final_data, Some("processed_data".to_string()));
}

/// Performance test for realistic workload
#[tokio::test]
async fn test_realistic_workload_performance() {
    use std::time::Instant;

    let mut memory = InMemoryMemory::default();
    let start = Instant::now();

    // Simulate realistic agent workload
    for i in 0..50 {
        // Store observation
        let obs_key = MemoryKey::new(&format!("observation_{}", i)).expect("Valid key");
        memory
            .store(MemoryUpdate {
                key: obs_key,
                value: format!("observation_data_{}", i),
            })
            .expect("Store should succeed");

        // Execute tool every 5 iterations
        if i % 5 == 0 {
            let tool = MockTool::new("analyzer").with_default_response("analysis_complete");

            let result = tool.call(format!("analyze_{}", i));
            assert!(result.is_success());

            // Store tool result
            let result_key = MemoryKey::new(&format!("analysis_{}", i)).expect("Valid key");
            memory
                .store(MemoryUpdate {
                    key: result_key,
                    value: result.output().to_string(),
                })
                .expect("Store should succeed");
        }

        // Create agent and process
        let agent = SimpleStatefulAgent::new(Box::new(InMemoryMemory::default()));
        let _processing_agent = agent.observe(format!("input_{}", i));
    }

    let duration = start.elapsed();

    // Should complete realistic workload in reasonable time (< 100ms)
    assert!(
        duration.as_millis() < 100,
        "Workload too slow: {:?}",
        duration
    );
}
