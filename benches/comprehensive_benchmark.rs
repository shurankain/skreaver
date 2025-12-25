//! Comprehensive Real-World Performance Test
//!
//! This benchmark simulates actual production workloads with error handling,
//! complex data processing, and realistic I/O patterns.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use skreaver::{InMemoryMemory, MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, Tool};
use skreaver_tools::standard::data::json::JsonParseTool;
use skreaver_tools::standard::io::file::{FileReadTool, FileWriteTool};
use std::time::Duration;
use tempfile::TempDir;

/// Memory benchmark with realistic concurrent-like access patterns
fn bench_memory_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_realistic");

    // CI-aware configuration
    let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

    if is_ci {
        group.sample_size(100);
        group.measurement_time(Duration::from_secs(3));
    } else {
        group.sample_size(1000);
        group.measurement_time(Duration::from_secs(10));
    }

    // Create pre-populated memory (simulating real app state)
    let mut memory = InMemoryMemory::new();
    let data_count = if is_ci { 500 } else { 5000 }; // Reduced for CI
    for i in 0..data_count {
        let key = format!("user_session_{}", i);
        let value = serde_json::json!({
            "user_id": i,
            "session_data": "x".repeat(500), // 500 chars per session
            "metadata": {
                "created": 1640000000 + i,
                "active": i % 3 == 0,
                "permissions": ["read", "write"]
            },
            "history": (0..10).map(|j| format!("action_{}", j)).collect::<Vec<_>>()
        })
        .to_string();

        let update = MemoryUpdate::new(&key, &value).unwrap();
        memory.store(update).unwrap();
    }

    // Realistic memory operations
    group.bench_function("memory_store_large_session", |b| {
        b.iter_batched(
            || {
                let user_id = rand::random::<u32>();
                let key = format!("new_user_session_{}", user_id);
                let value = serde_json::json!({
                    "user_id": user_id,
                    "session_data": "x".repeat(800), // Larger session
                    "metadata": {
                        "created": chrono::Utc::now().timestamp(),
                        "active": true,
                        "permissions": ["read", "write", "admin"]
                    },
                    "history": (0..20).map(|j| format!("action_{}", j)).collect::<Vec<_>>()
                })
                .to_string();
                (key, value)
            },
            |(key, value)| {
                let update = MemoryUpdate::new(&key, &value).unwrap();
                let _ = std::hint::black_box(memory.store(std::hint::black_box(update)));
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.bench_function("memory_load_with_misses", |b| {
        b.iter(|| {
            // 80% hits, 20% misses (realistic cache pattern)
            let user_id = if rand::random::<f32>() < 0.8 {
                rand::random::<u32>() % 5000 // Hit
            } else {
                rand::random::<u32>() + 10000 // Miss
            };

            let key = MemoryKey::new(&format!("user_session_{}", user_id)).unwrap();
            std::hint::black_box(memory.load(std::hint::black_box(&key)))
        })
    });

    group.finish();
}

/// File I/O with realistic data sizes and error conditions
fn bench_file_io_comprehensive(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_io_comprehensive");

    // CI-aware configuration
    let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

    if is_ci {
        // Reduced settings for CI
        group.sample_size(50);
        group.measurement_time(Duration::from_secs(3));
    } else {
        // Full settings for local development
        group.sample_size(300);
        group.measurement_time(Duration::from_secs(12));
    }

    let temp_dir = TempDir::new().unwrap();
    let file_write_tool = FileWriteTool::new();
    let file_read_tool = FileReadTool::new();

    // Test different file sizes (CI-aware)
    let sizes = if is_ci {
        vec![
            ("small_100b", 100),
            ("medium_10kb", 10 * 1024),
            ("large_50kb", 50 * 1024), // Reduced from 100KB and 1MB
        ]
    } else {
        vec![
            ("small_100b", 100),
            ("medium_10kb", 10 * 1024),
            ("large_100kb", 100 * 1024),
            ("xlarge_1mb", 1024 * 1024),
        ]
    };

    for (size_name, byte_count) in sizes {
        let test_data = "x".repeat(byte_count);

        // HIGH-5: Use try_into() with fallback to prevent overflow on 64-bit systems
        group.throughput(Throughput::Bytes(byte_count.try_into().unwrap_or(u64::MAX)));

        group.bench_with_input(
            BenchmarkId::new("file_write", size_name),
            &test_data,
            |b, data| {
                b.iter_batched(
                    || {
                        let file_name = format!("test_{}_{}.txt", size_name, rand::random::<u32>());
                        let file_path = temp_dir.path().join(file_name);
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
        for i in 0..20 {
            let file_path = temp_dir
                .path()
                .join(format!("read_test_{}_{}.txt", size_name, i));
            std::fs::write(&file_path, &test_data).unwrap();
        }

        group.bench_with_input(
            BenchmarkId::new("file_read", size_name),
            size_name,
            |b, size_name| {
                b.iter(|| {
                    let file_id = rand::random::<u32>() % 20;
                    let file_path = temp_dir
                        .path()
                        .join(format!("read_test_{}_{}.txt", size_name, file_id));
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

/// JSON processing with various complexity levels
fn bench_json_comprehensive(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_comprehensive");

    // CI-aware configuration
    let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

    if is_ci {
        group.sample_size(100);
        group.measurement_time(Duration::from_secs(3));
    } else {
        group.sample_size(800);
        group.measurement_time(Duration::from_secs(10));
    }

    let json_tool = JsonParseTool::new();

    // Different JSON complexity levels
    let json_scenarios = vec![
        (
            "simple_object",
            serde_json::json!({
                "id": 123,
                "name": "Simple Object",
                "active": true,
                "value": 42.5
            }),
        ),
        (
            "nested_structure",
            serde_json::json!({
                "user": {
                    "id": 456,
                    "profile": {
                        "name": "John Doe",
                        "email": "john@example.com",
                        "settings": {
                            "theme": "dark",
                            "notifications": {
                                "email": true,
                                "push": false,
                                "sms": true
                            }
                        }
                    }
                },
                "session": {
                    "created": "2024-01-01T00:00:00Z",
                    "last_active": "2024-01-02T12:34:56Z",
                    "actions": ["login", "view_profile", "update_settings"]
                }
            }),
        ),
        (
            "large_array",
            serde_json::json!({
                "data": (0..500).map(|i| serde_json::json!({
                    "id": i,
                    "timestamp": 1640000000 + i,
                    "value": rand::random::<f64>(),
                    "status": if i % 4 == 0 { "active" } else { "inactive" },
                    "metadata": {
                        "created_by": format!("user_{}", i % 50),
                        "tags": ["tag1", "tag2", "tag3"]
                    }
                })).collect::<Vec<_>>()
            }),
        ),
        ("deeply_nested", {
            let mut nested = serde_json::json!({"level": 0, "value": "root"});
            for i in 1..20 {
                nested = serde_json::json!({
                    "level": i,
                    "nested": nested,
                    "data": format!("level_{}_data", i),
                    "array": (0..5).map(|j| format!("item_{}_{}", i, j)).collect::<Vec<_>>()
                });
            }
            nested
        }),
    ];

    for (scenario_name, json_data) in json_scenarios {
        let json_str = json_data.to_string();
        group.throughput(Throughput::Bytes(json_str.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("json_parse", scenario_name),
            &json_str,
            |b, json_str| {
                b.iter(|| {
                    let input = serde_json::json!({
                        "json": json_str,
                        "format": "pretty"
                    })
                    .to_string();
                    std::hint::black_box(json_tool.call(std::hint::black_box(input)))
                })
            },
        );
    }

    group.finish();
}

/// End-to-end workflow simulation
fn bench_realistic_workflows(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_workflows");

    // CI-aware configuration
    let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

    if is_ci {
        group.sample_size(50);
        group.measurement_time(Duration::from_secs(5));
    } else {
        group.sample_size(200);
        group.measurement_time(Duration::from_secs(15));
    }

    let temp_dir = TempDir::new().unwrap();
    let file_write_tool = FileWriteTool::new();
    let file_read_tool = FileReadTool::new();
    let json_tool = JsonParseTool::new();

    // Simulate realistic agent workflows
    group.bench_function("agent_data_processing_workflow", |b| {
        b.iter_batched(
            || {
                // Generate realistic input data
                let input_data = serde_json::json!({
                    "request_id": rand::random::<u32>(),
                    "timestamp": chrono::Utc::now().timestamp(),
                    "user_data": {
                        "id": rand::random::<u32>() % 10000,
                        "preferences": {
                            "theme": "dark",
                            "language": "en",
                            "notifications": true
                        },
                        "history": (0..100).map(|i| serde_json::json!({
                            "action": format!("action_{}", i),
                            "timestamp": chrono::Utc::now().timestamp() - i,
                            "data": "x".repeat(50)
                        })).collect::<Vec<_>>()
                    },
                    "processing_instructions": {
                        "extract_recent": true,
                        "summarize": true,
                        "store_result": true
                    }
                });

                let file_path = temp_dir
                    .path()
                    .join(format!("workflow_{}.json", rand::random::<u32>()));
                (input_data.to_string(), file_path)
            },
            |(json_data, file_path)| {
                // Step 1: Parse and validate JSON
                let parse_input = serde_json::json!({
                    "json": json_data,
                    "format": "pretty"
                })
                .to_string();
                let parse_result = json_tool.call(parse_input);

                // Step 2: Process and extract data (simulated by re-parsing)
                let processed_data = serde_json::json!({
                    "original_size": json_data.len(),
                    "parsed_successfully": parse_result.is_success(),
                    "processed_at": chrono::Utc::now().timestamp(),
                    "summary": "Data processed successfully",
                    "extracted_items": 100
                })
                .to_string();

                // Step 3: Write processed result to file
                let write_input = serde_json::json!({
                    "path": file_path.to_string_lossy(),
                    "content": processed_data
                })
                .to_string();
                let write_result = file_write_tool.call(write_input);

                // Step 4: Read back to verify
                let read_input = serde_json::json!({
                    "path": file_path.to_string_lossy()
                })
                .to_string();
                let read_result = file_read_tool.call(read_input);

                // Step 5: Final validation (parse the read result)
                let final_parse = json_tool.call(
                    serde_json::json!({
                        "json": read_result.output(),
                        "format": "compact"
                    })
                    .to_string(),
                );

                std::hint::black_box((parse_result, write_result, read_result, final_parse))
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Error handling workflow
    group.bench_function("error_handling_workflow", |b| {
        b.iter(|| {
            // Mix of successful and failed operations
            let operations = vec![
                // Valid JSON
                json_tool.call(r#"{"valid": "json"}"#.to_string()),
                // Invalid JSON (should fail gracefully)
                json_tool.call(r#"{"invalid": json"#.to_string()),
                // Valid file operation
                file_write_tool.call(
                    serde_json::json!({
                        "path": temp_dir.path().join("test.txt").to_string_lossy(),
                        "content": "test content"
                    })
                    .to_string(),
                ),
                // Invalid file operation
                file_read_tool.call(
                    serde_json::json!({
                        "path": "/nonexistent/path/file.txt"
                    })
                    .to_string(),
                ),
            ];

            std::hint::black_box(operations)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_memory_realistic,
    bench_file_io_comprehensive,
    bench_json_comprehensive,
    bench_realistic_workflows
);
criterion_main!(benches);
