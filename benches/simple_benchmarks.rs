//! Simple Synchronous Performance Benchmarks
//!
//! Basic benchmarks for core operations without async complexity.
//! This gives us immediate baseline performance measurements.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use skreaver::{
    InMemoryMemory, MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, Tool, ToolCall, ToolName,
};
use skreaver_testing::{MockTool, MockToolRegistry};
use skreaver_tools::ToolRegistry;
use std::time::Duration;

/// Benchmark basic memory operations
fn bench_memory_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_operations");

    group.throughput(Throughput::Elements(1));
    group.sample_size(2000);
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("store_operation", |b| {
        b.iter(|| {
            let mut memory = InMemoryMemory::new();
            let update = MemoryUpdate::new("benchmark_key", "benchmark_value").unwrap();
            black_box(memory.store(black_box(update)))
        })
    });

    group.bench_function("load_operation", |b| {
        b.iter_batched(
            || {
                let mut memory = InMemoryMemory::new();
                let update = MemoryUpdate::new("benchmark_key", "benchmark_value").unwrap();
                memory.store(update).unwrap();
                (memory, MemoryKey::new("benchmark_key").unwrap())
            },
            |(memory, key)| black_box(memory.load(black_box(&key))),
            criterion::BatchSize::SmallInput,
        )
    });

    // Bulk operations
    for count in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("bulk_store", count), count, |b, &count| {
            b.iter(|| {
                let mut memory = InMemoryMemory::new();
                for i in 0..count {
                    let key = format!("key_{}", i);
                    let value = format!("value_{}", i);
                    let update = MemoryUpdate::new(&key, &value).unwrap();
                    let _ = memory.store(update);
                }
            })
        });
    }

    group.finish();
}

/// Benchmark tool operations
fn bench_tool_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_operations");

    group.throughput(Throughput::Elements(1));
    group.sample_size(3000);
    group.measurement_time(Duration::from_secs(5));

    // Direct tool execution
    let tool = MockTool::new("benchmark_tool").with_default_response("response");

    group.bench_function("direct_tool_call", |b| {
        b.iter(|| black_box(tool.call(black_box("benchmark input".to_string()))))
    });

    // Registry dispatch
    let registry = MockToolRegistry::new()
        .with_tool(MockTool::new("tool1").with_default_response("response1"))
        .with_tool(MockTool::new("tool2").with_default_response("response2"))
        .with_tool(MockTool::new("tool3").with_default_response("response3"));

    group.bench_function("registry_dispatch", |b| {
        b.iter(|| {
            let tool_call = ToolCall {
                name: ToolName::new("tool1").unwrap(),
                input: "benchmark input".to_string(),
            };
            black_box(registry.dispatch(black_box(tool_call)))
        })
    });

    // Tool name validation
    group.bench_function("tool_name_validation", |b| {
        b.iter(|| black_box(ToolName::new(black_box("valid_tool_name"))))
    });

    group.finish();
}

/// Benchmark different payload sizes
fn bench_payload_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("payload_sizes");

    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(8));

    let payloads = vec![
        ("small_10b", "x".repeat(10)),
        ("medium_1kb", "x".repeat(1024)),
        ("large_10kb", "x".repeat(10 * 1024)),
    ];

    for (size_name, payload) in payloads {
        group.throughput(Throughput::Bytes(payload.len() as u64));

        // Memory storage with different sizes
        group.bench_with_input(
            BenchmarkId::new("memory_store", size_name),
            &payload,
            |b, payload| {
                b.iter(|| {
                    let mut memory = InMemoryMemory::new();
                    let update = MemoryUpdate::new("key", payload).unwrap();
                    black_box(memory.store(black_box(update)))
                })
            },
        );

        // Tool processing with different sizes
        group.bench_with_input(
            BenchmarkId::new("tool_process", size_name),
            &payload,
            |b, payload| {
                b.iter(|| {
                    let tool = MockTool::new("test").with_default_response("processed");
                    black_box(tool.call(black_box(payload.clone())))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark registry with different tool counts
fn bench_registry_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_scalability");

    group.sample_size(1500);
    group.measurement_time(Duration::from_secs(8));

    let tool_counts = vec![5, 20, 100];

    for count in tool_counts {
        // Build registry with specified number of tools
        let registry = (0..count).fold(MockToolRegistry::new(), |reg, i| {
            reg.with_tool(
                MockTool::new(format!("tool_{}", i))
                    .with_default_response(format!("response_{}", i)),
            )
        });

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("registry_lookup", count),
            &registry,
            |b, registry| {
                b.iter(|| {
                    // Test lookup of first tool (best case) and middle tool
                    let tool_call = ToolCall {
                        name: ToolName::new("tool_0").unwrap(),
                        input: "test".to_string(),
                    };
                    black_box(registry.dispatch(black_box(tool_call)))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark error handling paths
fn bench_error_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_handling");

    group.throughput(Throughput::Elements(1));
    group.sample_size(2000);
    group.measurement_time(Duration::from_secs(5));

    let registry = MockToolRegistry::new()
        .with_tool(MockTool::new("success_tool").with_default_response("success"))
        .with_tool(MockTool::new("failure_tool").with_failure("test", "failure"));

    group.bench_function("success_path", |b| {
        b.iter(|| {
            let tool_call = ToolCall {
                name: ToolName::new("success_tool").unwrap(),
                input: "test".to_string(),
            };
            black_box(registry.dispatch(black_box(tool_call)))
        })
    });

    group.bench_function("failure_path", |b| {
        b.iter(|| {
            let tool_call = ToolCall {
                name: ToolName::new("failure_tool").unwrap(),
                input: "test".to_string(),
            };
            black_box(registry.dispatch(black_box(tool_call)))
        })
    });

    group.bench_function("nonexistent_tool", |b| {
        b.iter(|| {
            let tool_call = ToolCall {
                name: ToolName::new("nonexistent").unwrap(),
                input: "test".to_string(),
            };
            black_box(registry.dispatch(black_box(tool_call)))
        })
    });

    // Invalid tool names
    group.bench_function("invalid_tool_name", |b| {
        b.iter(|| black_box(ToolName::new(black_box(""))))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_operations,
    bench_tool_operations,
    bench_payload_sizes,
    bench_registry_scalability,
    bench_error_handling
);
criterion_main!(benches);
