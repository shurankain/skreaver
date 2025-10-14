//! Multi-agent task coordination example
//!
//! This example demonstrates a practical multi-agent system where:
//! - A Supervisor coordinates a pool of worker agents
//! - Workers process tasks and report results via Request/Reply
//! - Demonstrates load balancing, fault tolerance, and health monitoring
//!
//! Architecture:
//! ```text
//!   Supervisor (assigns tasks)
//!        |
//!   +----|----+----+
//!   |    |    |    |
//! Worker1 Worker2 Worker3 (process tasks in parallel)
//! ```

use skreaver_mesh::{
    AgentId, AgentMesh, Message, RedisMesh, Topic,
    patterns::supervisor::{Supervisor, SupervisorConfig},
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("🚀 Starting Multi-Agent Task Coordination Example");

    // Connect to Redis
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let mesh = Arc::new(RedisMesh::new(&redis_url).await?);

    info!("✅ Connected to Redis mesh");

    // Create supervisor
    let supervisor_config = SupervisorConfig {
        max_tasks_per_worker: 5,
        heartbeat_timeout: Duration::from_secs(10),
        max_retries: 2,
        task_timeout: Duration::from_secs(30),
    };
    let supervisor = Arc::new(Supervisor::new(mesh.clone(), supervisor_config));

    // Spawn 3 worker agents
    let worker_ids = vec![
        AgentId::from("worker-1"),
        AgentId::from("worker-2"),
        AgentId::from("worker-3"),
    ];

    for worker_id in worker_ids.clone() {
        let mesh_clone = mesh.clone();
        let worker_id_clone = worker_id.clone();
        let worker_id_for_log = worker_id.clone();

        tokio::spawn(async move {
            if let Err(e) = run_worker(mesh_clone, worker_id_clone).await {
                warn!("Worker {} error: {}", worker_id_for_log, e);
            }
        });

        // Register worker with supervisor
        supervisor.register_worker(worker_id.clone()).await;
        info!("✅ Registered worker: {}", worker_id);
    }

    // Give workers time to start
    sleep(Duration::from_millis(100)).await;

    // Submit tasks to supervisor
    info!("\n📋 Submitting 10 tasks to supervisor...");
    for i in 1..=10 {
        let message = Message::new(format!("Process data chunk {}", i));
        let task_id = supervisor.submit_task(message).await;
        info!("📤 Submitted {}", task_id);
    }

    // Start supervisor (distributes tasks to workers)
    let supervisor_clone = supervisor.clone();
    let supervisor_handle = tokio::spawn(async move {
        loop {
            let _ = supervisor_clone.assign_tasks().await;
            sleep(Duration::from_millis(500)).await;
        }
    });

    // Monitor progress
    info!("\n📊 Monitoring task progress...\n");
    for _ in 0..20 {
        sleep(Duration::from_secs(1)).await;

        let stats = supervisor.worker_stats().await;
        let active_workers = stats.len();
        let total_completed: u64 = stats.values().map(|(_, completed, _)| completed).sum();
        let total_active: usize = stats.values().map(|(active, _, _)| active).sum();

        info!(
            "📊 Workers: {} active | Tasks: {} active, {} completed",
            active_workers, total_active, total_completed
        );

        if total_completed >= 10 && total_active == 0 {
            info!("\n✅ All tasks completed successfully!");
            break;
        }
    }

    // Show final statistics
    info!("\n📈 Final Statistics:");
    let stats = supervisor.worker_stats().await;
    for (worker_id, (active, completed, failed)) in stats {
        info!(
            "  {} - Active: {}, Completed: {}, Failed: {}",
            worker_id, active, completed, failed
        );
    }

    supervisor_handle.abort();
    info!("\n🏁 Example completed");

    Ok(())
}

/// Worker agent that processes tasks
async fn run_worker(
    mesh: Arc<RedisMesh>,
    worker_id: AgentId,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("👷 Worker {} started", worker_id);

    // Subscribe to worker's mailbox
    let topic = Topic::from(format!("agent:{}", worker_id));
    let mut stream = mesh.subscribe(&topic).await?;

    use futures::StreamExt;
    while let Some(result) = stream.next().await {
        let message = match result {
            Ok(msg) => msg,
            Err(e) => {
                warn!("Worker {} stream error: {}", worker_id, e);
                continue;
            }
        };
        info!("👷 Worker {} received task: {:?}", worker_id, message.id);

        // Simulate task processing (random duration)
        let processing_time = Duration::from_millis(500 + (rand::random::<u64>() % 1000));
        sleep(processing_time).await;

        // Simulate occasional failures (10% chance)
        let success = rand::random::<f32>() > 0.1;

        if success {
            info!("✅ Worker {} completed task successfully", worker_id);

            // Send success response if there's a correlation ID (request/reply pattern)
            if let Some(correlation_id) = &message.correlation_id
                && let Some(reply_to) = message.sender()
            {
                let response = Message::unicast(
                    worker_id.clone(),
                    reply_to.clone(),
                    "Task completed successfully",
                )
                .with_correlation_id(correlation_id);
                let _ = mesh.send(reply_to, response).await;
            }
        } else {
            warn!("❌ Worker {} failed to process task", worker_id);

            // Send failure response
            if let Some(correlation_id) = &message.correlation_id
                && let Some(reply_to) = message.sender()
            {
                let response = Message::unicast(worker_id.clone(), reply_to.clone(), "Task failed")
                    .with_correlation_id(correlation_id);
                let _ = mesh.send(reply_to, response).await;
            }
        }
    }

    Ok(())
}
