//! Critical Path Tests for Memory Operations
//!
//! These tests focus on the most important memory functionality that users
//! rely on for production workloads, aiming for >95% line coverage.

use skreaver_core::{
    InMemoryMemory,
    memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter},
};

/// Test basic memory store and load operations
#[tokio::test]
async fn test_memory_store_load_critical_path() {
    let mut memory = InMemoryMemory::default();

    // Test storing and loading a simple value
    let key = MemoryKey::new("test_key").expect("Valid key");
    let update = MemoryUpdate {
        key: key.clone(),
        value: "test_value".to_string(),
    };

    memory.store(update).expect("Store should succeed");
    let loaded = memory.load(&key).expect("Load should succeed");
    assert_eq!(loaded, Some("test_value".to_string()));
}

/// Test memory load_many operation
#[tokio::test]
async fn test_memory_load_many_critical_path() {
    let mut memory = InMemoryMemory::default();

    // Store multiple values
    let key1 = MemoryKey::new("key1").expect("Valid key");
    let key2 = MemoryKey::new("key2").expect("Valid key");
    let key3 = MemoryKey::new("key3").expect("Valid key");

    memory
        .store(MemoryUpdate {
            key: key1.clone(),
            value: "value1".to_string(),
        })
        .expect("Store should succeed");
    memory
        .store(MemoryUpdate {
            key: key2.clone(),
            value: "value2".to_string(),
        })
        .expect("Store should succeed");
    memory
        .store(MemoryUpdate {
            key: key3.clone(),
            value: "value3".to_string(),
        })
        .expect("Store should succeed");

    // Test load_many
    let keys = vec![key1, key2, key3];
    let values = memory.load_many(&keys).expect("Load many should succeed");

    assert_eq!(values.len(), 3);
    assert_eq!(values[0], Some("value1".to_string()));
    assert_eq!(values[1], Some("value2".to_string()));
    assert_eq!(values[2], Some("value3".to_string()));
}

/// Test memory store_many operation
#[tokio::test]
async fn test_memory_store_many_critical_path() {
    let mut memory = InMemoryMemory::default();

    // Prepare multiple updates
    let updates = vec![
        MemoryUpdate {
            key: MemoryKey::new("batch_key1").expect("Valid key"),
            value: "batch_value1".to_string(),
        },
        MemoryUpdate {
            key: MemoryKey::new("batch_key2").expect("Valid key"),
            value: "batch_value2".to_string(),
        },
        MemoryUpdate {
            key: MemoryKey::new("batch_key3").expect("Valid key"),
            value: "batch_value3".to_string(),
        },
    ];

    // Store all at once
    memory
        .store_many(updates.clone())
        .expect("Store many should succeed");

    // Verify all were stored
    for update in updates {
        let loaded = memory.load(&update.key).expect("Load should succeed");
        assert_eq!(loaded, Some(update.value));
    }
}

/// Test loading non-existent keys
#[tokio::test]
async fn test_memory_nonexistent_key() {
    let memory = InMemoryMemory::default();

    let nonexistent_key = MemoryKey::new("does_not_exist").expect("Valid key");
    let result = memory.load(&nonexistent_key).expect("Load should succeed");
    assert_eq!(result, None);
}

/// Test overwriting existing keys
#[tokio::test]
async fn test_memory_overwrite_key() {
    let mut memory = InMemoryMemory::default();

    let key = MemoryKey::new("overwrite_key").expect("Valid key");

    // Store initial value
    memory
        .store(MemoryUpdate {
            key: key.clone(),
            value: "initial_value".to_string(),
        })
        .expect("Store should succeed");

    // Overwrite with new value
    memory
        .store(MemoryUpdate {
            key: key.clone(),
            value: "new_value".to_string(),
        })
        .expect("Store should succeed");

    // Should have the new value
    let loaded = memory.load(&key).expect("Load should succeed");
    assert_eq!(loaded, Some("new_value".to_string()));
}

/// Test memory key validation
#[test]
fn test_memory_key_validation() {
    // Valid keys should work
    assert!(MemoryKey::new("valid_key").is_ok());
    assert!(MemoryKey::new("valid-key-123").is_ok());
    assert!(MemoryKey::new("a").is_ok());

    // Invalid keys should fail
    assert!(MemoryKey::new("").is_err()); // Empty string
    assert!(MemoryKey::new("key with spaces").is_err()); // Contains spaces
}

/// Test concurrent memory access
#[tokio::test]
async fn test_memory_concurrent_access() {
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::task::JoinSet;

    let memory = Arc::new(Mutex::new(InMemoryMemory::default()));
    let mut join_set = JoinSet::new();

    // Spawn multiple concurrent operations
    for i in 0..10 {
        let memory_clone = Arc::clone(&memory);
        join_set.spawn(async move {
            let key = MemoryKey::new(&format!("concurrent_key_{}", i)).unwrap();
            let update = MemoryUpdate {
                key: key.clone(),
                value: format!("value_{}", i),
            };

            // Store and immediately load
            {
                let mut mem = memory_clone.lock().await;
                mem.store(update).expect("Store should succeed");
                let loaded = mem.load(&key).expect("Load should succeed");
                assert_eq!(loaded, Some(format!("value_{}", i)));
            }

            i
        });
    }

    // Wait for all operations to complete
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        results.push(result.expect("Task should complete successfully"));
    }

    // Verify all operations completed
    assert_eq!(results.len(), 10);
    results.sort();
    assert_eq!(results, (0..10).collect::<Vec<_>>());
}

/// Test large value storage and retrieval
#[tokio::test]
async fn test_memory_large_values() {
    let mut memory = InMemoryMemory::default();

    // Create a large value (1MB)
    let large_value = "x".repeat(1024 * 1024);
    let key = MemoryKey::new("large_key").expect("Valid key");

    memory
        .store(MemoryUpdate {
            key: key.clone(),
            value: large_value.clone(),
        })
        .expect("Store should succeed");

    let loaded = memory.load(&key).expect("Load should succeed");
    assert_eq!(loaded, Some(large_value));
}

/// Test memory with special characters
#[tokio::test]
async fn test_memory_special_characters() {
    let mut memory = InMemoryMemory::default();

    let special_value = "Test with special chars: 🦀 \n\t\r中文 العربية";
    let key = MemoryKey::new("special_key").expect("Valid key");

    memory
        .store(MemoryUpdate {
            key: key.clone(),
            value: special_value.to_string(),
        })
        .expect("Store should succeed");

    let loaded = memory.load(&key).expect("Load should succeed");
    assert_eq!(loaded, Some(special_value.to_string()));
}

/// Test memory performance characteristics
#[tokio::test]
async fn test_memory_performance() {
    use std::time::Instant;

    let mut memory = InMemoryMemory::default();
    let start = Instant::now();

    // Perform 1000 operations
    for i in 0..1000 {
        let key = MemoryKey::new(&format!("perf_key_{}", i)).unwrap();
        let update = MemoryUpdate {
            key: key.clone(),
            value: format!("value_{}", i),
        };
        memory.store(update).expect("Store should succeed");
        memory.load(&key).expect("Load should succeed");
    }

    let duration = start.elapsed();

    // Should complete 1000 operations in reasonable time (< 100ms on modern hardware)
    assert!(
        duration.as_millis() < 100,
        "Memory operations too slow: {:?}",
        duration
    );
}
