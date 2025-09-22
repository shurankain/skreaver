//! Integration Tests for End-to-End Scenarios
//!
//! These tests verify that different components work together correctly
//! in realistic usage scenarios, testing the complete execution paths
//! from user input to final output.

use skreaver_core::{
    InMemoryMemory, Tool, ToolCall,
    memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter},
};
use skreaver_testing::mock_tools::MockTool;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test a complete workflow: store context, execute tool, store result, verify
#[tokio::test]
async fn test_complete_memory_tool_workflow() {
    let mut memory = InMemoryMemory::default();

    // Step 1: Store initial context
    let context_key = MemoryKey::new("workflow_context").expect("Valid key");
    let context_value = r#"{"user_id": "123", "task": "data_processing"}"#;

    memory
        .store(MemoryUpdate {
            key: context_key.clone(),
            value: context_value.to_string(),
        })
        .expect("Store context should succeed");

    // Step 2: Create and execute a tool
    let tool = MockTool::new("data_processor").with_response(
        context_value,
        r#"{"status": "processed", "result": "success"}"#,
    );

    let result = tool.call(context_value.to_string());
    assert!(result.is_success());

    // Step 3: Store the result
    let result_key = MemoryKey::new("workflow_result").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: result_key.clone(),
            value: result.output().to_string(),
        })
        .expect("Store result should succeed");

    // Step 4: Verify the complete workflow
    let stored_context = memory
        .load(&context_key)
        .expect("Load context should succeed");
    let stored_result = memory
        .load(&result_key)
        .expect("Load result should succeed");

    assert_eq!(stored_context, Some(context_value.to_string()));
    assert!(stored_result.unwrap().contains("processed"));
}

/// Test concurrent tool execution with shared memory
#[tokio::test]
async fn test_concurrent_tool_execution_with_memory() {
    use tokio::task::JoinSet;

    let memory = Arc::new(Mutex::new(InMemoryMemory::default()));
    let mut join_set = JoinSet::new();

    // Create multiple tools for concurrent execution
    let tools = [
        ("analyzer", "analysis_complete"),
        ("validator", "validation_passed"),
        ("transformer", "transformation_done"),
    ];

    // Execute tools concurrently
    for (i, (tool_name, response)) in tools.iter().enumerate() {
        let memory_clone = Arc::clone(&memory);
        let tool_name = tool_name.to_string();
        let response = response.to_string();

        join_set.spawn(async move {
            let tool = MockTool::new(&tool_name).with_response(format!("input_{}", i), &response);

            // Execute tool
            let result = tool.call(format!("input_{}", i));
            assert!(result.is_success());

            // Store result in shared memory
            let result_key = MemoryKey::new(&format!("result_{}", i)).expect("Valid key");
            let mut mem = memory_clone.lock().await;
            mem.store(MemoryUpdate {
                key: result_key,
                value: result.output().to_string(),
            })
            .expect("Store should succeed");

            i
        });
    }

    // Wait for all tasks to complete
    let mut completed_tasks = Vec::new();
    while let Some(result) = join_set.join_next().await {
        completed_tasks.push(result.expect("Task should complete"));
    }

    // Verify all tasks completed
    assert_eq!(completed_tasks.len(), 3);
    completed_tasks.sort();
    assert_eq!(completed_tasks, vec![0, 1, 2]);

    // Verify all results are stored
    let memory = memory.lock().await;
    for i in 0..3 {
        let key = MemoryKey::new(&format!("result_{}", i)).expect("Valid key");
        let stored = memory.load(&key).expect("Load should succeed");
        assert!(stored.is_some());
    }
}

/// Test error handling and recovery in a multi-step workflow
#[tokio::test]
async fn test_error_handling_workflow() {
    let mut memory = InMemoryMemory::default();

    // Step 1: Store initial data
    let data_key = MemoryKey::new("processing_data").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: data_key.clone(),
            value: "initial_data".to_string(),
        })
        .expect("Store should succeed");

    // Step 2: Create a tool that fails for certain inputs
    let failing_tool = MockTool::new("processor")
        .with_response("initial_data", "success_result")
        .with_failure("bad_input", "Processing failed: invalid input");

    // Test successful execution
    let good_result = failing_tool.call("initial_data".to_string());
    assert!(good_result.is_success());

    // Store success result
    let success_key = MemoryKey::new("success_result").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: success_key.clone(),
            value: good_result.output().to_string(),
        })
        .expect("Store should succeed");

    // Test failed execution
    let bad_result = failing_tool.call("bad_input".to_string());
    assert!(bad_result.is_failure());

    // Store error information
    let error_key = MemoryKey::new("error_log").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: error_key.clone(),
            value: format!("Error: {}", bad_result.output()),
        })
        .expect("Store should succeed");

    // Verify both success and error are properly stored
    let success_data = memory.load(&success_key).expect("Load should succeed");
    let error_data = memory.load(&error_key).expect("Load should succeed");

    assert_eq!(success_data, Some("success_result".to_string()));
    assert!(error_data.unwrap().contains("Processing failed"));
}

/// Test batch operations with tool execution
#[tokio::test]
async fn test_batch_operations_with_tools() {
    let mut memory = InMemoryMemory::default();

    // Create batch data
    let batch_data = [
        ("item_1", "data_1"),
        ("item_2", "data_2"),
        ("item_3", "data_3"),
    ];

    // Store batch using store_many
    let updates: Vec<MemoryUpdate> = batch_data
        .iter()
        .map(|(key, value)| MemoryUpdate {
            key: MemoryKey::new(key).expect("Valid key"),
            value: value.to_string(),
        })
        .collect();

    memory
        .store_many(updates)
        .expect("Batch store should succeed");

    // Process each item with a tool
    let processor = MockTool::new("batch_processor")
        .with_response("data_1", "processed_1")
        .with_response("data_2", "processed_2")
        .with_response("data_3", "processed_3");

    // Load all items and process them
    let keys: Vec<MemoryKey> = batch_data
        .iter()
        .map(|(key, _)| MemoryKey::new(key).expect("Valid key"))
        .collect();

    let loaded_values = memory.load_many(&keys).expect("Batch load should succeed");

    // Process each loaded value
    let mut processed_results = Vec::new();
    for (i, value_opt) in loaded_values.iter().enumerate() {
        if let Some(value) = value_opt {
            let result = processor.call(value.clone());
            assert!(result.is_success());
            processed_results.push((format!("result_{}", i + 1), result.output().to_string()));
        }
    }

    // Store processed results
    let result_updates: Vec<MemoryUpdate> = processed_results
        .iter()
        .map(|(key, value)| MemoryUpdate {
            key: MemoryKey::new(key).expect("Valid key"),
            value: value.clone(),
        })
        .collect();

    memory
        .store_many(result_updates)
        .expect("Store results should succeed");

    // Verify all processed results are stored
    for i in 1..=3 {
        let result_key = MemoryKey::new(&format!("result_{}", i)).expect("Valid key");
        let stored = memory.load(&result_key).expect("Load should succeed");
        assert!(stored.unwrap().contains("processed"));
    }
}

/// Test complex data flow with multiple tool types
#[tokio::test]
async fn test_complex_data_flow() {
    let mut memory = InMemoryMemory::default();

    // Stage 1: Data ingestion
    let ingestion_tool = MockTool::new("data_ingester")
        .with_response("raw_input", r#"{"ingested": true, "records": 100}"#);

    let ingestion_result = ingestion_tool.call("raw_input".to_string());
    assert!(ingestion_result.is_success());

    let stage1_key = MemoryKey::new("stage1_data").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: stage1_key.clone(),
            value: ingestion_result.output().to_string(),
        })
        .expect("Store stage1 should succeed");

    // Stage 2: Data validation
    let stage1_data = memory
        .load(&stage1_key)
        .expect("Load should succeed")
        .unwrap();

    let validation_tool = MockTool::new("data_validator")
        .with_response(&stage1_data, r#"{"valid": true, "warnings": []}"#);

    let validation_result = validation_tool.call(stage1_data);
    assert!(validation_result.is_success());

    let stage2_key = MemoryKey::new("stage2_validation").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: stage2_key.clone(),
            value: validation_result.output().to_string(),
        })
        .expect("Store stage2 should succeed");

    // Stage 3: Data transformation
    let stage2_data = memory
        .load(&stage2_key)
        .expect("Load should succeed")
        .unwrap();

    let transformation_tool = MockTool::new("data_transformer").with_response(
        &stage2_data,
        r#"{"transformed": true, "output_format": "json"}"#,
    );

    let transformation_result = transformation_tool.call(stage2_data);
    assert!(transformation_result.is_success());

    let final_key = MemoryKey::new("final_output").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: final_key.clone(),
            value: transformation_result.output().to_string(),
        })
        .expect("Store final should succeed");

    // Verify complete pipeline
    let final_data = memory
        .load(&final_key)
        .expect("Load should succeed")
        .unwrap();
    assert!(final_data.contains("transformed"));

    // Verify we can trace the complete data flow
    let stage1_data = memory
        .load(&stage1_key)
        .expect("Load should succeed")
        .unwrap();
    let stage2_data = memory
        .load(&stage2_key)
        .expect("Load should succeed")
        .unwrap();

    assert!(stage1_data.contains("ingested"));
    assert!(stage2_data.contains("valid"));
    assert!(final_data.contains("transformed"));
}

/// Test resource cleanup and memory management
#[tokio::test]
async fn test_resource_cleanup() {
    let mut memory = InMemoryMemory::default();

    // Create a large number of temporary items
    for i in 0..1000 {
        let temp_key = MemoryKey::new(&format!("temp_item_{}", i)).expect("Valid key");
        memory
            .store(MemoryUpdate {
                key: temp_key,
                value: format!("temporary_data_{}", i),
            })
            .expect("Store should succeed");
    }

    // Simulate cleanup by overwriting with small values
    for i in 0..1000 {
        let temp_key = MemoryKey::new(&format!("temp_item_{}", i)).expect("Valid key");
        memory
            .store(MemoryUpdate {
                key: temp_key,
                value: "cleaned".to_string(),
            })
            .expect("Store should succeed");
    }

    // Verify cleanup worked - memory should still be functional
    let test_key = MemoryKey::new("cleanup_test").expect("Valid key");
    memory
        .store(MemoryUpdate {
            key: test_key.clone(),
            value: "cleanup_successful".to_string(),
        })
        .expect("Store should succeed");

    let result = memory.load(&test_key).expect("Load should succeed");
    assert_eq!(result, Some("cleanup_successful".to_string()));
}

/// Test ToolCall creation and execution integration
#[tokio::test]
async fn test_tool_call_integration() {
    let mut memory = InMemoryMemory::default();

    // Create a tool that processes ToolCall information
    let metadata_tool =
        MockTool::new("metadata_processor").with_response("test_input", "metadata_processed");

    // Create a ToolCall to demonstrate the integration
    let tool_call = ToolCall::new("metadata_processor", "test_input").expect("Valid tool call");

    // Verify ToolCall properties
    assert_eq!(tool_call.name(), "metadata_processor");
    assert_eq!(tool_call.input, "test_input");

    // Execute the tool using the input from ToolCall
    let result = metadata_tool.call(tool_call.input.clone());
    assert!(result.is_success());

    // Store the result with metadata
    let result_key = MemoryKey::new("call_result").expect("Valid key");
    let metadata = format!(
        "tool={}, input_len={}, output={}",
        tool_call.name(),
        tool_call.input.len(),
        result.output()
    );

    memory
        .store(MemoryUpdate {
            key: result_key.clone(),
            value: metadata,
        })
        .expect("Store should succeed");

    // Verify the integration worked
    let stored_metadata = memory
        .load(&result_key)
        .expect("Load should succeed")
        .unwrap();
    assert!(stored_metadata.contains("tool=metadata_processor"));
    assert!(stored_metadata.contains("metadata_processed"));
}

/// Performance test for realistic workload
#[tokio::test]
async fn test_realistic_performance() {
    use std::time::Instant;

    let mut memory = InMemoryMemory::default();
    let start = Instant::now();

    // Simulate a realistic workload: 100 operations with mixed tool execution
    for i in 0..100 {
        // Store some data
        let data_key = MemoryKey::new(&format!("data_{}", i)).expect("Valid key");
        memory
            .store(MemoryUpdate {
                key: data_key.clone(),
                value: format!("data_value_{}", i),
            })
            .expect("Store should succeed");

        // Execute a tool every 10 iterations
        if i % 10 == 0 {
            let tool =
                MockTool::new("performance_tool").with_default_response("performance_result");

            let result = tool.call(format!("input_{}", i));
            assert!(result.is_success());

            // Store tool result
            let result_key = MemoryKey::new(&format!("result_{}", i)).expect("Valid key");
            memory
                .store(MemoryUpdate {
                    key: result_key,
                    value: result.output().to_string(),
                })
                .expect("Store should succeed");
        }

        // Load some data
        let loaded = memory.load(&data_key).expect("Load should succeed");
        assert!(loaded.is_some());
    }

    let duration = start.elapsed();

    // Should complete realistic workload in reasonable time (< 50ms)
    assert!(
        duration.as_millis() < 50,
        "Realistic workload too slow: {:?}",
        duration
    );

    // Verify final state
    let final_key = MemoryKey::new("data_99").expect("Valid key");
    let final_data = memory.load(&final_key).expect("Load should succeed");
    assert_eq!(final_data, Some("data_value_99".to_string()));
}
