//! Critical Performance Benchmarks
//!
//! These benchmarks measure the performance characteristics of core functionality
//! to ensure we meet the performance targets defined in the development plan:
//! - p50 < 30ms, p95 < 200ms, p99 < 400ms, RSS ≤ 128MB, error budget ≤ 0.5%

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use skreaver_core::{
    InMemoryMemory, Tool,
    memory::{MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter},
};
use skreaver_testing::mock_tools::MockTool;
use std::time::Duration;

/// Benchmark memory store operations
fn benchmark_memory_store(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_store");

    // Benchmark different value sizes
    for size in [1, 10, 100, 1000, 10000].iter() {
        let _value = "x".repeat(*size);

        // HIGH-5: Use try_into() with fallback to prevent overflow
        group.throughput(Throughput::Bytes((*size).try_into().unwrap_or(u64::MAX)));
        group.bench_with_input(BenchmarkId::new("store_by_size", size), size, |b, &size| {
            let mut memory = InMemoryMemory::default();
            let value = "x".repeat(size);
            let key = MemoryKey::new("benchmark_key").unwrap();

            b.iter(|| {
                let update = MemoryUpdate {
                    key: key.clone(),
                    value: value.clone(),
                };
                memory.store(update).unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark memory load operations
fn benchmark_memory_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_load");

    // Pre-populate memory with different sized values
    for size in [1, 10, 100, 1000, 10000].iter() {
        let mut memory = InMemoryMemory::default();
        let value = "x".repeat(*size);
        let key = MemoryKey::new("benchmark_key").unwrap();

        memory
            .store(MemoryUpdate {
                key: key.clone(),
                value: value.clone(),
            })
            .unwrap();

        // HIGH-5: Use try_into() with fallback to prevent overflow
        group.throughput(Throughput::Bytes((*size).try_into().unwrap_or(u64::MAX)));
        group.bench_with_input(BenchmarkId::new("load_by_size", size), size, |b, _| {
            b.iter(|| {
                memory.load(&key).unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark memory batch operations
fn benchmark_memory_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_batch");

    // Benchmark batch sizes
    for batch_size in [1, 10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));

        // Benchmark store_many
        group.bench_with_input(
            BenchmarkId::new("store_many", batch_size),
            batch_size,
            |b, &batch_size| {
                let mut memory = InMemoryMemory::default();

                b.iter(|| {
                    let updates: Vec<MemoryUpdate> = (0..batch_size)
                        .map(|i| MemoryUpdate {
                            key: MemoryKey::new(&format!("batch_key_{}", i)).unwrap(),
                            value: format!("value_{}", i),
                        })
                        .collect();

                    memory.store_many(updates).unwrap();
                });
            },
        );

        // Benchmark load_many
        group.bench_with_input(
            BenchmarkId::new("load_many", batch_size),
            batch_size,
            |b, &batch_size| {
                let mut memory = InMemoryMemory::default();

                // Pre-populate
                let keys: Vec<MemoryKey> = (0..batch_size)
                    .map(|i| {
                        let key = MemoryKey::new(&format!("batch_key_{}", i)).unwrap();
                        memory
                            .store(MemoryUpdate {
                                key: key.clone(),
                                value: format!("value_{}", i),
                            })
                            .unwrap();
                        key
                    })
                    .collect();

                b.iter(|| {
                    memory.load_many(&keys).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark tool execution
fn benchmark_tool_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_execution");

    // Benchmark different input sizes
    for size in [1, 10, 100, 1000].iter() {
        let _input = "x".repeat(*size);

        // HIGH-5: Use try_into() with fallback to prevent overflow
        group.throughput(Throughput::Bytes((*size).try_into().unwrap_or(u64::MAX)));
        group.bench_with_input(
            BenchmarkId::new("tool_call_by_input_size", size),
            size,
            |b, &size| {
                let mock_tool =
                    MockTool::new("benchmark_tool").with_default_response("benchmark_response");
                let input = "x".repeat(size);

                b.iter(|| {
                    mock_tool.call(input.clone());
                });
            },
        );
    }

    group.finish();
}

/// Benchmark memory key validation
fn benchmark_memory_key_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_key_validation");

    // Benchmark different key lengths
    for length in [1, 10, 32, 64].iter() {
        let _key_string = "a".repeat(*length);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("key_validation", length),
            length,
            |b, &length| {
                let key_string = "a".repeat(length);

                b.iter(|| {
                    MemoryKey::new(&key_string).unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent memory access
fn benchmark_concurrent_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_memory");

    // Benchmark different concurrency levels
    for threads in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_access", threads),
            threads,
            |b, &threads| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async move {
                        use std::sync::Arc;
                        use tokio::sync::Mutex;
                        use tokio::task::JoinSet;

                        let memory = Arc::new(Mutex::new(InMemoryMemory::default()));
                        let mut join_set = JoinSet::new();

                        for i in 0..threads {
                            let memory_clone = Arc::clone(&memory);
                            join_set.spawn(async move {
                                let key = MemoryKey::new(&format!("concurrent_key_{}", i)).unwrap();
                                let mut mem = memory_clone.lock().await;

                                // Perform 10 operations per thread
                                for j in 0..10 {
                                    let update = MemoryUpdate {
                                        key: key.clone(),
                                        value: format!("value_{}_{}", i, j),
                                    };
                                    mem.store(update).unwrap();
                                    mem.load(&key).unwrap();
                                }
                            });
                        }

                        while let Some(result) = join_set.join_next().await {
                            result.unwrap();
                        }
                    });
            },
        );
    }

    group.finish();
}

/// Benchmark end-to-end agent scenario
fn benchmark_agent_scenario(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_scenario");

    group.bench_function("simple_agent_workflow", |b| {
        b.iter(|| {
            // Simulate a basic agent workflow:
            // 1. Store context in memory
            // 2. Execute tool
            // 3. Store result

            let mut memory = InMemoryMemory::default();
            let mock_tool =
                MockTool::new("workflow_tool").with_default_response("workflow_response");

            // Step 1: Store context
            let context_key = MemoryKey::new("agent_context").unwrap();
            memory
                .store(MemoryUpdate {
                    key: context_key.clone(),
                    value: "agent context data".to_string(),
                })
                .unwrap();

            // Step 2: Execute tool
            let result = mock_tool.call("workflow_input".to_string());

            // Step 3: Store result
            let result_key = MemoryKey::new("agent_result").unwrap();
            memory
                .store(MemoryUpdate {
                    key: result_key,
                    value: result.output().to_string(),
                })
                .unwrap();

            // Step 4: Retrieve context for next iteration
            memory.load(&context_key).unwrap();
        });
    });

    group.finish();
}

/// Benchmark resource usage patterns
fn benchmark_resource_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_usage");

    // Test memory usage growth patterns
    group.bench_function("memory_growth_1000_keys", |b| {
        b.iter(|| {
            let mut memory = InMemoryMemory::default();

            // Store 1000 key-value pairs
            for i in 0..1000 {
                let key = MemoryKey::new(&format!("growth_key_{}", i)).unwrap();
                let update = MemoryUpdate {
                    key,
                    value: format!("growth_value_{}", i),
                };
                memory.store(update).unwrap();
            }

            // Verify we can still access data efficiently
            let test_key = MemoryKey::new("growth_key_500").unwrap();
            memory.load(&test_key).unwrap();
        });
    });

    // Test cleanup patterns
    group.bench_function("memory_cleanup_pattern", |b| {
        b.iter(|| {
            let mut memory = InMemoryMemory::default();

            // Fill with data
            for i in 0..100 {
                let key = MemoryKey::new(&format!("cleanup_key_{}", i)).unwrap();
                memory
                    .store(MemoryUpdate {
                        key,
                        value: "cleanup_value".to_string(),
                    })
                    .unwrap();
            }

            // Simulate cleanup by overwriting with empty values
            for i in 0..100 {
                let key = MemoryKey::new(&format!("cleanup_key_{}", i)).unwrap();
                memory
                    .store(MemoryUpdate {
                        key,
                        value: String::new(),
                    })
                    .unwrap();
            }
        });
    });

    group.finish();
}

// Configure benchmarks
fn configure_benchmarks() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::from_secs(10)) // 10 second measurement time
        .sample_size(100) // 100 samples per benchmark
        .warm_up_time(Duration::from_secs(3)) // 3 second warmup
        .with_plots() // Generate plots
}

criterion_group!(
    name = benches;
    config = configure_benchmarks();
    targets =
        benchmark_memory_store,
        benchmark_memory_load,
        benchmark_memory_batch,
        benchmark_tool_execution,
        benchmark_memory_key_validation,
        benchmark_concurrent_memory,
        benchmark_agent_scenario,
        benchmark_resource_usage
);

criterion_main!(benches);
