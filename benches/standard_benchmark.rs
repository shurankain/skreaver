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
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Semaphore;

/// Metrics collected during benchmark execution
#[derive(Debug, Clone)]
struct BenchmarkMetrics {
    total_operations: usize,
    successful_operations: usize,
    failed_operations: usize,
    error_rate_percent: f64,
    peak_rss_kb: u64,
    avg_cpu_percent: f64,
    latency_p50_ms: f64,
    latency_p95_ms: f64,
    latency_p99_ms: f64,
}

impl BenchmarkMetrics {
    fn new(
        total: usize,
        successful: usize,
        failed: usize,
        peak_rss: u64,
        avg_cpu: f64,
        latencies: &[Duration],
    ) -> Self {
        let error_rate = if total > 0 {
            (failed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Calculate percentiles from collected latencies
        let (p50, p95, p99) = calculate_percentiles(latencies);

        Self {
            total_operations: total,
            successful_operations: successful,
            failed_operations: failed,
            error_rate_percent: error_rate,
            peak_rss_kb: peak_rss,
            avg_cpu_percent: avg_cpu,
            latency_p50_ms: p50,
            latency_p95_ms: p95,
            latency_p99_ms: p99,
        }
    }

    fn print_summary(&self) {
        println!("\n=== Benchmark Metrics Summary ===");
        println!("Total Operations: {}", self.total_operations);
        println!("Successful: {}", self.successful_operations);
        println!("Failed: {}", self.failed_operations);
        println!("Error Rate: {:.2}%", self.error_rate_percent);
        println!(
            "Peak RSS Memory: {} KB ({:.2} MB)",
            self.peak_rss_kb,
            self.peak_rss_kb as f64 / 1024.0
        );
        println!("Avg CPU Usage: {:.2}%", self.avg_cpu_percent);
        println!("Latency p50: {:.2}ms", self.latency_p50_ms);
        println!("Latency p95: {:.2}ms", self.latency_p95_ms);
        println!("Latency p99: {:.2}ms", self.latency_p99_ms);

        // Check against targets
        println!("\n=== Performance Targets ===");
        println!(
            "Error Rate Target: ≤0.5% - {}",
            if self.error_rate_percent <= 0.5 {
                "✓ PASS"
            } else {
                "✗ FAIL"
            }
        );
        println!(
            "RSS Target: ≤128 MB - {}",
            if self.peak_rss_kb <= 128 * 1024 {
                "✓ PASS"
            } else {
                "✗ FAIL"
            }
        );
        println!(
            "Latency p50 Target: <30ms - {}",
            if self.latency_p50_ms < 30.0 {
                "✓ PASS"
            } else {
                "✗ FAIL"
            }
        );
        println!(
            "Latency p95 Target: <200ms - {}",
            if self.latency_p95_ms < 200.0 {
                "✓ PASS"
            } else {
                "✗ FAIL"
            }
        );
        println!("================================\n");
    }
}

/// Calculate percentiles from latency measurements
fn calculate_percentiles(latencies: &[Duration]) -> (f64, f64, f64) {
    if latencies.is_empty() {
        return (0.0, 0.0, 0.0);
    }

    let mut sorted: Vec<f64> = latencies.iter().map(|d| d.as_secs_f64() * 1000.0).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let p50_idx = (sorted.len() as f64 * 0.50) as usize;
    let p95_idx = (sorted.len() as f64 * 0.95) as usize;
    let p99_idx = (sorted.len() as f64 * 0.99) as usize;

    let p50 = sorted
        .get(p50_idx.saturating_sub(1))
        .copied()
        .unwrap_or(0.0);
    let p95 = sorted
        .get(p95_idx.saturating_sub(1))
        .copied()
        .unwrap_or(0.0);
    let p99 = sorted
        .get(p99_idx.saturating_sub(1))
        .copied()
        .unwrap_or(0.0);

    (p50, p95, p99)
}

/// Get current RSS memory usage in KB
fn get_rss_kb() -> u64 {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let Ok(status) = fs::read_to_string("/proc/self/status") else {
            return 0;
        };

        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    return kb_str.parse().unwrap_or(0);
                }
            }
        }
        0
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let Ok(output) = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
        else {
            return 0;
        };

        let Ok(s) = String::from_utf8(output.stdout) else {
            return 0;
        };

        s.trim().parse().unwrap_or(0)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        0 // Not supported on this platform
    }
}

/// Get current CPU usage percentage
fn get_cpu_percent() -> f64 {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        let Ok(stat) = fs::read_to_string("/proc/self/stat") else {
            return 0.0;
        };

        let fields: Vec<&str> = stat.split_whitespace().collect();
        if fields.len() < 14 {
            return 0.0;
        }

        // utime (field 13) + stime (field 14) in clock ticks
        let utime: u64 = fields[13].parse().unwrap_or(0);
        let stime: u64 = fields[14].parse().unwrap_or(0);
        let total_ticks = utime + stime;

        // Get clock ticks per second
        let ticks_per_sec = 100; // Standard for Linux

        // Simple approximation: convert to percentage
        (total_ticks as f64 / ticks_per_sec as f64) * 100.0
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let Ok(output) = Command::new("ps")
            .args(["-o", "%cpu=", "-p", &std::process::id().to_string()])
            .output()
        else {
            return 0.0;
        };

        let Ok(s) = String::from_utf8(output.stdout) else {
            return 0.0;
        };

        s.trim().parse().unwrap_or(0.0)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        0.0 // Not supported on this platform
    }
}

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

    // Metrics tracking
    let success_count = Arc::new(AtomicUsize::new(0));
    let error_count = Arc::new(AtomicUsize::new(0));
    let peak_rss = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(std::sync::Mutex::new(Vec::new()));
    let cpu_samples = Arc::new(std::sync::Mutex::new(Vec::new()));

    group.bench_function("concurrent_agents", |b| {
        let success_count = Arc::clone(&success_count);
        let error_count = Arc::clone(&error_count);
        let peak_rss = Arc::clone(&peak_rss);
        let latencies = Arc::clone(&latencies);
        let cpu_samples = Arc::clone(&cpu_samples);

        b.to_async(&rt).iter(|| async {
            let temp_dir = TempDir::new().unwrap();

            // Create local test server for HTTP operations (no external network)
            let server_handle = start_local_test_server().await;
            let server_url = format!("http://127.0.0.1:{}", server_handle.port());

            // Run 32 concurrent agents
            let agent_count = if is_ci { 8 } else { 32 }; // Reduced for CI
            let semaphore = Arc::new(Semaphore::new(agent_count));

            let mut tasks = Vec::new();

            // Track RSS memory and CPU during execution
            let peak_rss_clone = Arc::clone(&peak_rss);
            let cpu_samples_clone = Arc::clone(&cpu_samples);
            let monitor = tokio::spawn(async move {
                let mut max_rss = 0u64;
                let mut samples = Vec::new();
                for _ in 0..100 {
                    let current_rss = get_rss_kb();
                    max_rss = max_rss.max(current_rss);
                    samples.push(get_cpu_percent());
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                peak_rss_clone.store(max_rss, Ordering::Relaxed);
                if let Ok(mut cpu) = cpu_samples_clone.lock() {
                    *cpu = samples;
                }
            });

            for agent_id in 0..agent_count {
                let semaphore = Arc::clone(&semaphore);
                let server_url = server_url.clone();
                let temp_dir_path = temp_dir.path().to_path_buf();
                let success_count = Arc::clone(&success_count);
                let error_count = Arc::clone(&error_count);
                let latencies = Arc::clone(&latencies);

                let task = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();

                    // Measure latency for the entire tool chain
                    let start = std::time::Instant::now();

                    // Standard tool chain: HTTP GET → JSON parse → Text transform → File write
                    let result =
                        execute_standard_tool_chain(agent_id, &server_url, &temp_dir_path).await;

                    let latency = start.elapsed();

                    // Track success/error metrics and latency
                    match result {
                        Ok(_) => {
                            success_count.fetch_add(1, Ordering::Relaxed);
                            if let Ok(mut lat) = latencies.lock() {
                                lat.push(latency);
                            }
                        }
                        Err(_) => {
                            error_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }

                    std::hint::black_box(result)
                });

                tasks.push(task);
            }

            // Wait for all agents to complete
            for task in tasks {
                let _ = task.await.unwrap();
            }

            // Wait for monitor
            let _ = monitor.await;

            // Clean shutdown
            server_handle.shutdown().await;
        });
    });

    group.finish();

    // Print metrics summary after benchmark completes
    let total = success_count.load(Ordering::Relaxed) + error_count.load(Ordering::Relaxed);
    if total > 0 {
        let latency_vec = latencies.lock().unwrap().clone();
        let cpu_vec = cpu_samples.lock().unwrap().clone();
        let avg_cpu = if !cpu_vec.is_empty() {
            cpu_vec.iter().sum::<f64>() / cpu_vec.len() as f64
        } else {
            0.0
        };

        let metrics = BenchmarkMetrics::new(
            total,
            success_count.load(Ordering::Relaxed),
            error_count.load(Ordering::Relaxed),
            peak_rss.load(Ordering::Relaxed),
            avg_cpu,
            &latency_vec,
        );
        metrics.print_summary();
    }
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
