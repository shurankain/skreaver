//! Realistic Performance Benchmarks
//!
//! Tests real-world scenarios with actual I/O, networking, and data processing.
//! No mocking - only real operations that would happen in production.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use skreaver::{InMemoryMemory, MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, Tool};
use skreaver_tools::standard::data::json::JsonParseTool;
use skreaver_tools::standard::io::file::{FileReadTool, FileWriteTool};
use std::time::Duration;
use tempfile::TempDir;

/// Benchmark REAL memory operations with existing data
fn bench_realistic_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_memory");

    group.sample_size(500);
    group.measurement_time(Duration::from_secs(8));

    // Pre-populate memory with realistic data
    let mut memory = InMemoryMemory::new();
    for i in 0..1000 {
        let key = format!("session_{}", i);
        let value = format!(
            "{{\"user_id\": {}, \"session_data\": \"{}\"}}",
            i,
            "x".repeat(100)
        );
        let update = MemoryUpdate::new(&key, &value).unwrap();
        memory.store(update).unwrap();
    }

    group.bench_function("store_to_populated_memory", |b| {
        b.iter_batched(
            || {
                let key = format!("new_session_{}", rand::random::<u32>());
                let value = format!(
                    "{{\"user_id\": {}, \"data\": \"{}\"}}",
                    rand::random::<u32>(),
                    "x".repeat(200)
                );
                (key, value)
            },
            |(key, value)| {
                let update = MemoryUpdate::new(&key, &value).unwrap();
                let _ = std::hint::black_box(memory.store(std::hint::black_box(update)));
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("load_from_populated_memory", |b| {
        b.iter(|| {
            let session_id = rand::random::<u32>() % 1000;
            let key = MemoryKey::new(&format!("session_{}", session_id)).unwrap();
            std::hint::black_box(memory.load(std::hint::black_box(&key)))
        })
    });

    group.finish();
}

/// Benchmark REAL file I/O operations
fn bench_realistic_file_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_file_io");

    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));

    let temp_dir = TempDir::new().unwrap();
    let file_write_tool = FileWriteTool::new();
    let file_read_tool = FileReadTool::new();

    // Create test data files
    let test_data = "x".repeat(1024); // 1KB
    for i in 0..10 {
        let file_path = temp_dir.path().join(format!("test_{}.txt", i));
        std::fs::write(&file_path, &test_data).unwrap();
    }

    group.bench_function("real_file_write_1kb", |b| {
        b.iter_batched(
            || {
                let file_name = format!("bench_{}.txt", rand::random::<u32>());
                let file_path = temp_dir.path().join(file_name);
                serde_json::json!({
                    "path": file_path.to_string_lossy(),
                    "content": "x".repeat(1024)
                })
                .to_string()
            },
            |input| std::hint::black_box(file_write_tool.call(std::hint::black_box(input))),
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("real_file_read_1kb", |b| {
        b.iter(|| {
            let file_id = rand::random::<u32>() % 10;
            let file_path = temp_dir.path().join(format!("test_{}.txt", file_id));
            let input = serde_json::json!({
                "path": file_path.to_string_lossy()
            })
            .to_string();
            std::hint::black_box(file_read_tool.call(std::hint::black_box(input)))
        })
    });

    group.finish();
}

/// Benchmark REAL JSON processing
fn bench_realistic_json_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_json");

    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(8));

    let json_tool = JsonParseTool::new();

    // Real JSON data that would be processed
    let complex_json = serde_json::json!({
        "users": (0..100).map(|i| serde_json::json!({
            "id": i,
            "name": format!("User {}", i),
            "email": format!("user{}@example.com", i),
            "profile": {
                "age": 20 + (i % 50),
                "settings": {
                    "theme": "dark",
                    "notifications": true,
                    "features": ["feature_a", "feature_b", "feature_c"]
                }
            },
            "history": (0..10).map(|j| serde_json::json!({
                "action": format!("action_{}", j),
                "timestamp": 1000000000 + i * 1000 + j
            })).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    })
    .to_string();

    // HIGH-5: Use saturating conversion to prevent overflow in throughput calculation
    group.throughput(Throughput::Bytes(
        complex_json.len().try_into().unwrap_or(u64::MAX),
    ));

    group.bench_function("parse_complex_json", |b| {
        b.iter(|| {
            let input = serde_json::json!({
                "json": complex_json,
                "path": "$.users[*].profile.age"
            })
            .to_string();
            std::hint::black_box(json_tool.call(std::hint::black_box(input)))
        })
    });

    // Test with different JSON sizes
    for size in [1, 10, 100].iter() {
        let json_data = serde_json::json!({
            "items": (0..*size).map(|i| serde_json::json!({
                "id": i,
                "data": "x".repeat(100),
                "nested": {
                    "field1": format!("value_{}", i),
                    "field2": i * 2,
                    "array": (0..5).collect::<Vec<_>>()
                }
            })).collect::<Vec<_>>()
        })
        .to_string();

        // HIGH-5: Use saturating conversion to prevent overflow in throughput calculation
        group.throughput(Throughput::Bytes(
            json_data.len().try_into().unwrap_or(u64::MAX),
        ));
        group.bench_with_input(
            BenchmarkId::new("json_parse_size", size),
            &json_data,
            |b, json_data| {
                b.iter(|| {
                    let input = serde_json::json!({
                        "json": json_data,
                        "path": "$.items[*].id"
                    })
                    .to_string();
                    std::hint::black_box(json_tool.call(std::hint::black_box(input)))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark realistic tool chain execution
fn bench_realistic_tool_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_tool_chain");

    group.sample_size(100);
    group.measurement_time(Duration::from_secs(15));

    let temp_dir = TempDir::new().unwrap();
    let file_write_tool = FileWriteTool::new();
    let file_read_tool = FileReadTool::new();
    let json_tool = JsonParseTool::new();

    // Real-world scenario: Process JSON data, extract fields, write to file, read back
    group.bench_function("json_process_file_write_read_chain", |b| {
        b.iter_batched(
            || {
                let json_data = serde_json::json!({
                    "timestamp": chrono::Utc::now().timestamp(),
                    "data": (0..50).map(|i| serde_json::json!({
                        "id": i,
                        "value": rand::random::<f64>(),
                        "status": if i % 3 == 0 { "active" } else { "inactive" }
                    })).collect::<Vec<_>>()
                })
                .to_string();

                let file_path = temp_dir
                    .path()
                    .join(format!("chain_{}.json", rand::random::<u32>()));
                (json_data, file_path)
            },
            |(json_data, file_path)| {
                // Step 1: Parse and extract JSON data
                let parse_input = serde_json::json!({
                    "json": json_data,
                    "path": "$.data[?(@.status == 'active')].id"
                })
                .to_string();
                let parse_result = json_tool.call(parse_input);

                // Step 2: Write processed data to file
                let write_input = serde_json::json!({
                    "path": file_path.to_string_lossy(),
                    "content": parse_result.output()
                })
                .to_string();
                let write_result = file_write_tool.call(write_input);

                // Step 3: Read back the file
                let read_input = serde_json::json!({
                    "path": file_path.to_string_lossy()
                })
                .to_string();
                let read_result = file_read_tool.call(read_input);

                std::hint::black_box((parse_result, write_result, read_result))
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_realistic_memory,
    bench_realistic_file_operations,
    bench_realistic_json_processing,
    bench_realistic_tool_chain
);
criterion_main!(benches);
