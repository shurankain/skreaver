//! Broadcast example demonstrating one-to-many communication
//!
//! This example shows:
//! - One coordinator agent broadcasting announcements
//! - Multiple worker agents receiving broadcasts via pub/sub
//!
//! Run with: cargo run --example mesh_broadcast --features redis

use futures::StreamExt;
use skreaver_mesh::{AgentId, AgentMesh, Message, RedisMesh, Topic};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting broadcast mesh example");

    // Create mesh connection
    let mesh = RedisMesh::new("redis://localhost:6379").await?;
    info!("Connected to Redis mesh");

    // Agent IDs
    let coordinator_id = AgentId::from("coordinator");
    let worker1_id = AgentId::from("worker-1");
    let worker2_id = AgentId::from("worker-2");
    let worker3_id = AgentId::from("worker-3");

    // Register agents
    mesh.register_presence(&coordinator_id, 60).await?;
    mesh.register_presence(&worker1_id, 60).await?;
    mesh.register_presence(&worker2_id, 60).await?;
    mesh.register_presence(&worker3_id, 60).await?;

    info!("Registered agents");

    // Topic for announcements
    let announcements_topic = Topic::from("announcements");

    // Spawn worker agents
    let workers = vec![worker1_id.clone(), worker2_id.clone(), worker3_id.clone()];

    for worker_id in workers {
        let mesh_clone = RedisMesh::new("redis://localhost:6379").await?;
        let topic = announcements_topic.clone();
        let worker_id_clone = worker_id.clone();

        tokio::spawn(async move {
            info!("{}: Starting worker", worker_id_clone);

            // Subscribe to announcements
            let mut stream = match mesh_clone.subscribe(&topic).await {
                Ok(s) => s,
                Err(e) => {
                    error!("{}: Failed to subscribe: {}", worker_id_clone, e);
                    return;
                }
            };

            info!("{}: Subscribed to announcements", worker_id_clone);

            // Receive 3 messages then exit
            for i in 0..3 {
                match tokio::time::timeout(Duration::from_secs(15), stream.next()).await {
                    Ok(Some(Ok(msg))) => {
                        if let skreaver_mesh::MessagePayload::Text(text) = &msg.payload {
                            info!(
                                "{}: Received announcement {} - '{}'",
                                worker_id_clone,
                                i + 1,
                                text
                            );
                        }
                    }
                    Ok(Some(Err(e))) => {
                        error!("{}: Error receiving message: {}", worker_id_clone, e);
                        break;
                    }
                    Ok(None) => {
                        info!("{}: Stream ended", worker_id_clone);
                        break;
                    }
                    Err(_) => {
                        info!("{}: Timeout waiting for message {}", worker_id_clone, i + 1);
                    }
                }
            }

            info!("{}: Worker finished", worker_id_clone);
        });
    }

    // Give workers time to subscribe
    sleep(Duration::from_secs(1)).await;

    // Coordinator sends announcements
    info!("Coordinator: Starting to send announcements");

    let announcements = [
        "System startup complete",
        "All workers online",
        "Ready for operations",
    ];

    for (i, announcement) in announcements.iter().enumerate() {
        let msg = Message::new(announcement.to_string())
            .from(coordinator_id.clone())
            .with_metadata("sequence", (i + 1).to_string())
            .with_metadata("type", "announcement");

        mesh.publish(&announcements_topic, msg).await?;
        info!(
            "Coordinator: Published announcement {} - '{}'",
            i + 1,
            announcement
        );

        sleep(Duration::from_secs(2)).await;
    }

    // Wait for workers to process messages
    sleep(Duration::from_secs(3)).await;

    // List active agents
    let active_agents = mesh.list_agents().await?;
    info!("Active agents in mesh: {:?}", active_agents);

    // Deregister agents
    mesh.deregister_presence(&coordinator_id).await?;
    mesh.deregister_presence(&worker1_id).await?;
    mesh.deregister_presence(&worker2_id).await?;
    mesh.deregister_presence(&worker3_id).await?;

    info!("Broadcast example completed successfully");

    Ok(())
}
