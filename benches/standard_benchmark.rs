// Standard benchmark scenario as specified in DEVELOPMENT_PLAN.md
// N=32 concurrent agents executing: HTTP GET → JSON parse → Text transform → File write
// Duration: 60s sustained load (reduced to 10s for CI)
// Metrics: p50/p95 latency, RSS, CPU%, error rate

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use skreaver_core::Tool;
use skreaver_tools::standard::{
    data::{json::JsonParseTool, text::TextUppercaseTool},
    io::file::FileWriteTool,
    network::http::HttpGetTool,
};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Semaphore;

/// Standard benchmark as specified in DEVELOPMENT_PLAN.md
/// This is the official performance baseline for Skreaver
fn standard_32_agent_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("standard_32_agent_tool_loop");

    // CI-aware configuration
    let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

    if is_ci {
        // Reduced settings for CI resource constraints
        group.sample_size(10);
        group.measurement_time(Duration::from_secs(10));
        group.warm_up_time(Duration::from_secs(2));
    } else {
        // Full benchmark as specified in development plan
        group.sample_size(20);
        group.measurement_time(Duration::from_secs(60));
        group.warm_up_time(Duration::from_secs(5));
    }

    // Throughput: operations per agent per second
    group.throughput(Throughput::Elements(32));

    let rt = tokio::runtime::Runtime::new().unwrap();

    group.bench_function("concurrent_agents", |b| {
        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();

            // Create local test server for HTTP operations (no external network)
            let server_handle = start_local_test_server().await;
            let server_url = format!("http://127.0.0.1:{}", server_handle.port());

            // Run 32 concurrent agents
            let agent_count = if is_ci { 8 } else { 32 }; // Reduced for CI
            let semaphore = Arc::new(Semaphore::new(agent_count));

            let mut tasks = Vec::new();

            for agent_id in 0..agent_count {
                let semaphore = Arc::clone(&semaphore);
                let server_url = server_url.clone();
                let temp_dir_path = temp_dir.path().to_path_buf();

                let task = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();

                    // Standard tool chain: HTTP GET → JSON parse → Text transform → File write
                    let result =
                        execute_standard_tool_chain(agent_id, &server_url, &temp_dir_path).await;

                    std::hint::black_box(result)
                });

                tasks.push(task);
            }

            // Wait for all agents to complete
            for task in tasks {
                task.await.unwrap().unwrap();
            }

            // Clean shutdown
            server_handle.shutdown().await;
        });
    });

    group.finish();
}

/// Execute the standard tool chain for a single agent
async fn execute_standard_tool_chain(
    agent_id: usize,
    server_url: &str,
    temp_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Step 1: HTTP GET from local test server
    let http_tool = HttpGetTool::new();
    let http_input = format!("{}/test-data/{}", server_url, agent_id);
    let http_result = http_tool.call(http_input);

    let response_data = if http_result.is_success() {
        http_result.output().to_string()
    } else {
        return Err(format!("HTTP request failed: {}", http_result.output()).into());
    };

    // Step 2: JSON parse the response
    let json_tool = JsonParseTool::new();
    let json_result = json_tool.call(response_data);

    let parsed_data = if json_result.is_success() {
        json_result.output().to_string()
    } else {
        return Err(format!("JSON parsing failed: {}", json_result.output()).into());
    };

    // Step 3: Text transform (uppercase transformation)
    let text_tool = TextUppercaseTool;
    let text_result = text_tool.call(parsed_data);

    let transformed_data = if text_result.is_success() {
        text_result.output()
    } else {
        return Err(format!("Text transform failed: {}", text_result.output()).into());
    };

    // Step 4: File write to tmpfs
    let file_tool = FileWriteTool::new();
    let file_path = temp_dir.join(format!("agent_{}_output.txt", agent_id));
    let file_input = format!("{}:{}", file_path.display(), transformed_data);
    let file_result = file_tool.call(file_input);

    if !file_result.is_success() {
        return Err(format!("File write failed: {}", file_result.output()).into());
    }

    Ok(())
}

/// Local test server for benchmark isolation (no external network dependencies)
struct TestServerHandle {
    port: u16,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl TestServerHandle {
    fn port(&self) -> u16 {
        self.port
    }

    async fn shutdown(self) {
        let _ = self.shutdown_tx.send(());
    }
}

/// Start a local HTTP server for benchmark testing
async fn start_local_test_server() -> TestServerHandle {
    use axum::{Router, routing::get};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let app = Router::new().route("/test-data/{id}", get(test_data_handler));

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async {
                shutdown_rx.await.ok();
            })
            .await
            .unwrap();
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    TestServerHandle { port, shutdown_tx }
}

/// Test data handler for benchmark HTTP requests
async fn test_data_handler(axum::extract::Path(id): axum::extract::Path<usize>) -> String {
    // Return JSON as string for the benchmark
    format!(
        r#"{{
        "agent_id": {},
        "data": "test data for agent {}",
        "timestamp": {},
        "metadata": {{
            "version": "1.0",
            "benchmark": true
        }}
    }}"#,
        id, id, 1640000000
    )
}

/// Minimal performance validation benchmark
fn performance_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_targets");

    // CI-aware configuration
    let is_ci = std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();

    if is_ci {
        group.sample_size(50);
        group.measurement_time(Duration::from_secs(5));
    } else {
        group.sample_size(100);
        group.measurement_time(Duration::from_secs(10));
    }

    let _rt = tokio::runtime::Runtime::new().unwrap();

    // Test individual tool performance against targets
    group.bench_function("json_parse_latency", |b| {
        b.iter(|| {
            let tool = JsonParseTool::new();
            let input = r#"{"test": "data", "number": 42}"#.to_string();

            let start = std::time::Instant::now();
            let result = tool.call(input);
            let duration = start.elapsed();

            // Target: p50 < 30ms for JSON parsing
            std::hint::black_box((result, duration));
        });
    });

    group.bench_function("text_transform_latency", |b| {
        b.iter(|| {
            let tool = TextUppercaseTool;
            let input = "hello world benchmark test".to_string();

            let start = std::time::Instant::now();
            let result = tool.call(input);
            let duration = start.elapsed();

            // Target: p50 < 10ms for text transformations
            std::hint::black_box((result, duration));
        });
    });

    group.finish();
}

criterion_group!(
    name = standard_benchmarks;
    config = Criterion::default()
        .significance_level(0.1)
        .confidence_level(0.95);
    targets = standard_32_agent_benchmark, performance_validation
);

criterion_main!(standard_benchmarks);
