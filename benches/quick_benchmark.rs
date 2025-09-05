//! Ultra-Fast Benchmarks
//!
//! Minimal benchmarks designed to complete in under 30 seconds total.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use skreaver::{InMemoryMemory, MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, Tool};
use skreaver_core::memory::SnapshotableMemory;
use skreaver_tools::standard::data::json::JsonParseTool;
use skreaver_tools::standard::io::file::{FileReadTool, FileWriteTool};
use std::time::Duration;
use tempfile::TempDir;

fn bench_memory_quick(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_quick");

    // Ultra fast settings
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(2));
    group.warm_up_time(Duration::from_secs(1));

    let mut memory = InMemoryMemory::new();
    // Only 100 items for speed
    for i in 0..100 {
        let key = format!("session_{}", i);
        let value = format!("{{\"id\": {}}}", i);
        let update = MemoryUpdate::new(&key, &value).unwrap();
        memory.store(update).unwrap();
    }

    group.bench_function("store", |b| {
        b.iter(|| {
            let key = format!("test_{}", rand::random::<u16>());
            let value = "test_value".to_string();
            let update = MemoryUpdate::new(&key, &value).unwrap();
            let _ = std::hint::black_box(memory.store(std::hint::black_box(update)));
        })
    });

    group.bench_function("load", |b| {
        b.iter(|| {
            let session_id = rand::random::<u32>() % 100;
            let key = MemoryKey::new(&format!("session_{}", session_id)).unwrap();
            std::hint::black_box(memory.load(std::hint::black_box(&key)))
        })
    });

    // Test snapshot functionality
    group.bench_function("snapshot", |b| {
        b.iter(|| {
            let mut snapshot_memory = InMemoryMemory::new();
            // Add some test data
            for i in 0..10 {
                let key = format!("key_{}", i);
                let value = format!("value_{}", i);
                let update = MemoryUpdate::new(&key, &value).unwrap();
                snapshot_memory.store(update).unwrap();
            }
            std::hint::black_box(snapshot_memory.snapshot())
        })
    });

    group.finish();
}

fn bench_file_quick(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_quick");

    group.sample_size(20);
    group.measurement_time(Duration::from_secs(2));
    group.warm_up_time(Duration::from_secs(1));

    let temp_dir = TempDir::new().unwrap();
    let file_write_tool = FileWriteTool::new();
    let file_read_tool = FileReadTool::new();

    // Small data only
    let test_data = "x".repeat(1024); // 1KB

    group.throughput(Throughput::Bytes(1024));

    group.bench_function("write_1kb", |b| {
        b.iter(|| {
            let file_path = temp_dir
                .path()
                .join(format!("test_{}.txt", rand::random::<u16>()));
            let input = serde_json::json!({
                "path": file_path.to_string_lossy(),
                "content": test_data
            })
            .to_string();
            std::hint::black_box(file_write_tool.call(std::hint::black_box(input)))
        })
    });

    // Pre-create one file
    let read_file = temp_dir.path().join("read_test.txt");
    std::fs::write(&read_file, &test_data).unwrap();

    group.bench_function("read_1kb", |b| {
        b.iter(|| {
            let input = serde_json::json!({
                "path": read_file.to_string_lossy()
            })
            .to_string();
            std::hint::black_box(file_read_tool.call(std::hint::black_box(input)))
        })
    });

    group.finish();
}

fn bench_json_quick(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_quick");

    group.sample_size(100);
    group.measurement_time(Duration::from_secs(2));
    group.warm_up_time(Duration::from_secs(1));

    let json_tool = JsonParseTool::new();

    // Simple pre-computed JSON
    let simple_json = r#"{"id": 123, "name": "test", "active": true}"#;
    let complex_json = serde_json::json!({
        "users": (0..5).map(|i| serde_json::json!({
            "id": i,
            "name": format!("User {}", i)
        })).collect::<Vec<_>>()
    })
    .to_string();

    group.throughput(Throughput::Bytes(simple_json.len() as u64));

    group.bench_function("simple", |b| {
        b.iter(|| {
            let input = serde_json::json!({
                "json": simple_json,
                "format": "pretty"
            })
            .to_string();
            std::hint::black_box(json_tool.call(std::hint::black_box(input)))
        })
    });

    group.throughput(Throughput::Bytes(complex_json.len() as u64));

    group.bench_function("complex", |b| {
        b.iter(|| {
            let input = serde_json::json!({
                "json": complex_json,
                "format": "pretty"
            })
            .to_string();
            std::hint::black_box(json_tool.call(std::hint::black_box(input)))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_quick,
    bench_file_quick,
    bench_json_quick
);
criterion_main!(benches);
