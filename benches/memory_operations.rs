//! Memory Operations Performance Benchmarks
//!
//! Benchmarks different memory backend operations including:
//! - InMemory storage and retrieval
//! - File-based persistence
//! - Concurrent access patterns
//! - Memory usage under load

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use skreaver_core::InMemoryMemory;
use skreaver_core::memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory,
};
use skreaver_memory::FileMemory;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Benchmark basic memory operations (store/load)
fn bench_memory_basic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_basic_operations");

    group.throughput(Throughput::Elements(1));
    group.sample_size(2000);
    group.measurement_time(Duration::from_secs(10));

    // InMemory backend
    group.bench_function("inmemory_store", |b| {
        b.iter(|| {
            let mut memory = InMemoryMemory::new();
            let update = MemoryUpdate::new("benchmark_key", "benchmark_value").unwrap();
            black_box(memory.store(black_box(update)))
        })
    });

    group.bench_function("inmemory_load", |b| {
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

    // File-based memory
    group.bench_function("file_memory_store", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                FileMemory::new(temp_dir.path().join("bench.db"))
            },
            |mut memory| {
                let update = MemoryUpdate::new("benchmark_key", "benchmark_value").unwrap();
                black_box(memory.store(black_box(update)))
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("file_memory_load", |b| {
        b.iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let mut memory = FileMemory::new(temp_dir.path().join("bench.db"));
                let update = MemoryUpdate::new("benchmark_key", "benchmark_value").unwrap();
                memory.store(update).unwrap();
                (memory, MemoryKey::new("benchmark_key").unwrap())
            },
            |(memory, key)| black_box(memory.load(black_box(&key))),
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// Benchmark bulk memory operations
fn bench_memory_bulk_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_bulk_operations");

    group.sample_size(500);
    group.measurement_time(Duration::from_secs(15));

    for count in [10, 100, 1000].iter() {
        group.throughput(Throughput::Elements(*count as u64));

        group.bench_with_input(
            BenchmarkId::new("inmemory_bulk_store", count),
            count,
            |b, &count| {
                b.iter(|| {
                    let mut memory = InMemoryMemory::new();
                    for i in 0..count {
                        let key = format!("key_{}", i);
                        let value = format!("value_{}", i);
                        let update = MemoryUpdate::new(&key, &value).unwrap();
                        let _ = black_box(memory.store(update));
                    }
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("inmemory_bulk_load", count),
            count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let mut memory = InMemoryMemory::new();
                        for i in 0..count {
                            let key = format!("key_{}", i);
                            let value = format!("value_{}", i);
                            let update = MemoryUpdate::new(&key, &value).unwrap();
                            memory.store(update).unwrap();
                        }
                        memory
                    },
                    |memory| {
                        for i in 0..count {
                            let key = MemoryKey::new(&format!("key_{}", i)).unwrap();
                            let _ = black_box(memory.load(&key));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("file_memory_bulk_store", count),
            count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        FileMemory::new(temp_dir.path().join("bench.db"))
                    },
                    |mut memory| {
                        for i in 0..count {
                            let key = format!("key_{}", i);
                            let value = format!("value_{}", i);
                            let update = MemoryUpdate::new(&key, &value).unwrap();
                            let _ = black_box(memory.store(update));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

/// Benchmark memory operations under concurrent load
fn bench_memory_concurrent_access(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_concurrent_access");

    group.sample_size(200);
    group.measurement_time(Duration::from_secs(20));

    for concurrent_tasks in [2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*concurrent_tasks as u64));

        group.bench_with_input(
            BenchmarkId::new("concurrent_store", concurrent_tasks),
            concurrent_tasks,
            |b, &task_count| {
                b.to_async(&rt).iter(|| async move {
                    let mut handles = Vec::with_capacity(task_count);

                    for task_id in 0..task_count {
                        let handle = tokio::spawn(async move {
                            let mut memory = InMemoryMemory::new();
                            for i in 0..10 {
                                let key = format!("task_{}_key_{}", task_id, i);
                                let value = format!("task_{}_value_{}", task_id, i);
                                let update = MemoryUpdate::new(&key, &value).unwrap();
                                let _ = black_box(memory.store(update));
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.await.unwrap();
                        black_box(());
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark memory snapshot and restore operations
fn bench_memory_snapshots(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_snapshots");

    group.sample_size(200);
    group.measurement_time(Duration::from_secs(10));

    // Test snapshots with InMemoryMemory
    for data_size in [10, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("inmemory_snapshot_create", data_size),
            data_size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut memory = InMemoryMemory::new();
                        // Populate with test data
                        for i in 0..size {
                            let key = format!("key_{}", i);
                            let value = format!("value_{}", i);
                            let update = MemoryUpdate::new(&key, &value).unwrap();
                            memory.store(update).unwrap();
                        }
                        memory
                    },
                    |mut memory| black_box(memory.snapshot()),
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("inmemory_snapshot_restore", data_size),
            data_size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut memory = InMemoryMemory::new();
                        // Populate with test data
                        for i in 0..size {
                            let key = format!("key_{}", i);
                            let value = format!("value_{}", i);
                            let update = MemoryUpdate::new(&key, &value).unwrap();
                            memory.store(update).unwrap();
                        }

                        let snapshot = memory.snapshot();
                        let empty_memory = InMemoryMemory::new();
                        (empty_memory, snapshot)
                    },
                    |(mut memory, snapshot)| {
                        if let Some(snap) = snapshot {
                            black_box(memory.restore(&snap))
                        } else {
                            black_box(Ok(()))
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        // Test snapshots with FileMemory
        group.bench_with_input(
            BenchmarkId::new("file_memory_snapshot_create", data_size),
            data_size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let mut memory = FileMemory::new(temp_dir.path().join("bench.db"));
                        // Populate with test data
                        for i in 0..size {
                            let key = format!("key_{}", i);
                            let value = format!("value_{}", i);
                            let update = MemoryUpdate::new(&key, &value).unwrap();
                            memory.store(update).unwrap();
                        }
                        memory
                    },
                    |mut memory| black_box(memory.snapshot()),
                    criterion::BatchSize::SmallInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("file_memory_snapshot_restore", data_size),
            data_size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let mut memory = FileMemory::new(temp_dir.path().join("bench.db"));
                        // Populate with test data
                        for i in 0..size {
                            let key = format!("key_{}", i);
                            let value = format!("value_{}", i);
                            let update = MemoryUpdate::new(&key, &value).unwrap();
                            memory.store(update).unwrap();
                        }

                        let snapshot = memory.snapshot();
                        let empty_memory = FileMemory::new(temp_dir.path().join("restore.db"));
                        (empty_memory, snapshot)
                    },
                    |(mut memory, snapshot)| {
                        if let Some(snap) = snapshot {
                            black_box(memory.restore(&snap))
                        } else {
                            black_box(Ok(()))
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

/// Benchmark memory operations with different value sizes
fn bench_memory_value_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_value_sizes");

    group.throughput(Throughput::Elements(1));
    group.sample_size(1000);
    group.measurement_time(Duration::from_secs(10));

    let value_sizes = vec![
        ("small_10b", "x".repeat(10)),
        ("medium_1kb", "x".repeat(1024)),
        ("large_10kb", "x".repeat(10 * 1024)),
        ("xlarge_100kb", "x".repeat(100 * 1024)),
    ];

    for (size_name, value) in value_sizes {
        group.bench_with_input(
            BenchmarkId::new("inmemory_store", size_name),
            &value,
            |b, value| {
                b.iter(|| {
                    let mut memory = InMemoryMemory::new();
                    let update = MemoryUpdate::new("benchmark_key", value).unwrap();
                    black_box(memory.store(black_box(update)))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("file_memory_store", size_name),
            &value,
            |b, value| {
                b.iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        FileMemory::new(temp_dir.path().join("bench.db"))
                    },
                    |mut memory| {
                        let update = MemoryUpdate::new("benchmark_key", value).unwrap();
                        black_box(memory.store(black_box(update)))
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_basic_operations,
    bench_memory_bulk_operations,
    bench_memory_concurrent_access,
    bench_memory_snapshots,
    bench_memory_value_sizes
);
criterion_main!(benches);
