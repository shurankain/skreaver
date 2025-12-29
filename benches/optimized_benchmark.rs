//! Optimized Performance Benchmarks
//!
//! Fast, focused benchmarks that complete in reasonable time while
//! maintaining statistical significance.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use skreaver::{InMemoryMemory, MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, Tool};
use skreaver_tools::standard::data::json::JsonParseTool;
use skreaver_tools::standard::io::file::{FileReadTool, FileWriteTool};
use std::time::Duration;
use tempfile::TempDir;

/// Fast memory benchmarks with realistic but manageable data
fn bench_memory_optimized(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_optimized");

    // Reduced but statistically significant
    group.sample_size(500);
    group.measurement_time(Duration::from_secs(3));

    // Pre-populate with reasonable amount (was 5000, now 1000)
    let mut memory = InMemoryMemory::new();
    for i in 0..1000 {
        let key = format!("session_{}", i);
        let value = format!("{{\"user_id\": {}, \"data\": \"{}\"}}", i, "x".repeat(100));
        let update = MemoryUpdate::new(&key, &value).unwrap();
        memory.store(update).unwrap();
    }

    group.bench_function("memory_store_realistic", |b| {
        b.iter_batched(
            || {
                let key = format!("session_{}", rand::random::<u32>() % 10000);
                let value = format!("{{\"data\": \"{}\"}}", "x".repeat(200));
                (key, value)
            },
            |(key, value)| {
                let update = MemoryUpdate::new(&key, &value).unwrap();
                let _ = std::hint::black_box(memory.store(std::hint::black_box(update)));
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("memory_load_realistic", |b| {
        b.iter(|| {
            let session_id = rand::random::<u32>() % 1000;
            let key = MemoryKey::new(&format!("session_{}", session_id)).unwrap();
            std::hint::black_box(memory.load(std::hint::black_box(&key)))
        })
    });

    group.finish();
}

/// File I/O benchmarks focusing on most common sizes
fn bench_file_io_focused(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_io_focused");

    group.sample_size(200);
    group.measurement_time(Duration::from_secs(4));

    let temp_dir = TempDir::new().unwrap();
    let file_write_tool = FileWriteTool::new();
    let file_read_tool = FileReadTool::new();

    // Focus on most common sizes in production
    let test_cases = vec![
        ("small_1kb", "x".repeat(1024)),
        ("medium_50kb", "x".repeat(50 * 1024)),
        ("large_500kb", "x".repeat(500 * 1024)),
    ];

    for (size_name, test_data) in test_cases {
        // HIGH-5: Use saturating conversion to prevent overflow in throughput calculation
        group.throughput(Throughput::Bytes(
            test_data.len().try_into().unwrap_or(u64::MAX),
        ));

        group.bench_with_input(
            BenchmarkId::new("write", size_name),
            &test_data,
            |b, data| {
                b.iter_batched(
                    || {
                        let file_path = temp_dir.path().join(format!(
                            "{}_{}.txt",
                            size_name,
                            rand::random::<u16>()
                        ));
                        serde_json::json!({
                            "path": file_path.to_string_lossy(),
                            "content": data
                        })
                        .to_string()
                    },
                    |input| std::hint::black_box(file_write_tool.call(std::hint::black_box(input))),
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        // Pre-create files for read benchmark
        for i in 0..10 {
            let file_path = temp_dir
                .path()
                .join(format!("read_{}_{}.txt", size_name, i));
            std::fs::write(&file_path, &test_data).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::new("read", size_name),
            size_name,
            |b, size_name| {
                b.iter(|| {
                    let file_id = rand::random::<u32>() % 10;
                    let file_path = temp_dir
                        .path()
                        .join(format!("read_{}_{}.txt", size_name, file_id));
                    let input = serde_json::json!({
                        "path": file_path.to_string_lossy()
                    })
                    .to_string();
                    std::hint::black_box(file_read_tool.call(std::hint::black_box(input)))
                })
            },
        );
    }

    group.finish();
}

/// JSON benchmarks with pre-computed data to avoid setup overhead
fn bench_json_focused(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_focused");

    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(3));

    let json_tool = JsonParseTool::new();

    // Pre-compute JSON strings to avoid expensive generation in benchmark
    let simple_json = serde_json::json!({
        "id": 123,
        "name": "Test Object",
        "active": true,
        "metadata": {"type": "test", "version": "1.0"}
    })
    .to_string();

    let complex_json = serde_json::json!({
        "users": (0..20).map(|i| serde_json::json!({  // Reduced from 100 to 20
            "id": i,
            "name": format!("User {}", i),
            "profile": {
                "settings": {"theme": "dark", "lang": "en"},
                "history": ["action1", "action2", "action3"]
            }
        })).collect::<Vec<_>>()
    })
    .to_string();

    let nested_json = {
        let mut nested = serde_json::json!({"level": 0, "value": "root"});
        for i in 1..8 {
            // Reduced from 20 to 8 levels
            nested = serde_json::json!({
                "level": i,
                "nested": nested,
                "data": format!("level_{}", i)
            });
        }
        nested.to_string()
    };

    // Test scenarios with pre-computed data
    let scenarios = vec![
        ("simple", simple_json),
        ("complex_array", complex_json),
        ("nested_structure", nested_json),
    ];

    for (name, json_str) in scenarios {
        // HIGH-5: Use saturating conversion to prevent overflow in throughput calculation
        group.throughput(Throughput::Bytes(
            json_str.len().try_into().unwrap_or(u64::MAX),
        ));

        group.bench_with_input(BenchmarkId::new("parse", name), &json_str, |b, json_str| {
            b.iter(|| {
                let input = serde_json::json!({
                    "json": json_str,
                    "format": "pretty"
                })
                .to_string();
                std::hint::black_box(json_tool.call(std::hint::black_box(input)))
            })
        });
    }

    group.finish();
}

/// Single end-to-end workflow test
fn bench_workflow_focused(c: &mut Criterion) {
    let mut group = c.benchmark_group("workflow_focused");

    group.sample_size(100);
    group.measurement_time(Duration::from_secs(5));

    let temp_dir = TempDir::new().unwrap();
    let file_write_tool = FileWriteTool::new();
    let file_read_tool = FileReadTool::new();
    let json_tool = JsonParseTool::new();

    // Pre-compute workflow data
    let workflow_json = serde_json::json!({
        "request_id": 12345,
        "user_data": {
            "id": 789,
            "preferences": {"theme": "dark", "lang": "en"},
            "recent_actions": (0..10).map(|i| format!("action_{}", i)).collect::<Vec<_>>()
        },
        "processing": {"extract": true, "summarize": true}
    })
    .to_string();

    group.bench_function("complete_workflow", |b| {
        b.iter_batched(
            || {
                let file_path = temp_dir
                    .path()
                    .join(format!("workflow_{}.json", rand::random::<u16>()));
                (workflow_json.clone(), file_path)
            },
            |(json_data, file_path)| {
                // Step 1: Parse JSON
                let parse_result = json_tool.call(
                    serde_json::json!({
                        "json": json_data,
                        "format": "compact"
                    })
                    .to_string(),
                );

                // Step 2: Write to file
                let write_result = file_write_tool.call(
                    serde_json::json!({
                        "path": file_path.to_string_lossy(),
                        "content": parse_result.output()
                    })
                    .to_string(),
                );

                // Step 3: Read back
                let read_result = file_read_tool.call(
                    serde_json::json!({
                        "path": file_path.to_string_lossy()
                    })
                    .to_string(),
                );

                std::hint::black_box((parse_result, write_result, read_result))
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_optimized,
    bench_file_io_focused,
    bench_json_focused,
    bench_workflow_focused
);
criterion_main!(benches);
