//! Ping-pong example demonstrating basic multi-agent communication
//!
//! This example shows two agents communicating through the mesh:
//! - Agent 1 sends "ping" messages
//! - Agent 2 receives pings and responds with "pong"
//!
//! Run with: cargo run --example mesh_ping_pong --features redis

use skreaver_mesh::{AgentId, AgentMesh, Message, RedisMesh};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting ping-pong mesh example");

    // Create Redis mesh connection
    let mesh = RedisMesh::new("redis://localhost:6379").await?;
    info!("Connected to Redis mesh");

    // Agent IDs
    let agent1_id = AgentId::from("ping-agent");
    let agent2_id = AgentId::from("pong-agent");

    // Register agents
    mesh.register_presence(&agent1_id, 60).await?;
    mesh.register_presence(&agent2_id, 60).await?;
    info!("Registered agents: {} and {}", agent1_id, agent2_id);

    // Clone mesh for agent2 task
    let mesh2 = RedisMesh::new("redis://localhost:6379").await?;
    let agent2_id_clone = agent2_id.clone();

    // Spawn agent 2 (responder)
    let agent2_task = tokio::spawn(async move {
        info!("Agent 2 starting (responder)");

        for i in 0..5 {
            // Receive message with 10 second timeout
            match mesh2.receive(&agent2_id_clone, 10).await {
                Ok(Some(msg)) => {
                    if let skreaver_mesh::MessagePayload::Text(text) = &msg.payload {
                        info!("Agent 2 received: '{}' (message {})", text, msg.id);

                        // Send pong response
                        if let Some(from) = &msg.from {
                            let response = Message::new("pong")
                                .from(agent2_id_clone.clone())
                                .with_correlation_id(msg.id.as_str());

                            mesh2.send(from, response).await.unwrap();
                            info!("Agent 2 sent: 'pong' (response to {})", msg.id);
                        }
                    }
                }
                Ok(None) => {
                    info!("Agent 2: receive timeout (round {})", i + 1);
                }
                Err(e) => {
                    error!("Agent 2 receive error: {}", e);
                    break;
                }
            }
        }

        info!("Agent 2 finished");
    });

    // Give agent 2 time to start
    sleep(Duration::from_millis(500)).await;

    // Agent 1 (sender)
    info!("Agent 1 starting (sender)");

    for i in 0..5 {
        // Send ping message
        let msg = Message::new("ping")
            .from(agent1_id.clone())
            .with_metadata("round", i.to_string());

        mesh.send(&agent2_id, msg.clone()).await?;
        info!("Agent 1 sent: 'ping' (round {})", i + 1);

        // Wait for pong response
        sleep(Duration::from_millis(100)).await;

        match mesh.receive(&agent1_id, 5).await {
            Ok(Some(response)) => {
                if let skreaver_mesh::MessagePayload::Text(text) = &response.payload {
                    info!(
                        "Agent 1 received: '{}' (correlation: {:?})",
                        text, response.correlation_id
                    );
                }
            }
            Ok(None) => {
                info!("Agent 1: no response (timeout)");
            }
            Err(e) => {
                error!("Agent 1 receive error: {}", e);
            }
        }

        sleep(Duration::from_millis(500)).await;
    }

    info!("Agent 1 finished");

    // Wait for agent 2 to complete
    agent2_task.await?;

    // Deregister agents
    mesh.deregister_presence(&agent1_id).await?;
    mesh.deregister_presence(&agent2_id).await?;
    info!("Deregistered agents");

    info!("Ping-pong example completed successfully");

    Ok(())
}
