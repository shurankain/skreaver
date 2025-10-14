//! Integration tests for skreaver-mesh
//!
//! These tests require a running Redis instance on localhost:6379

#[cfg(feature = "redis")]
mod redis_tests {
    use skreaver_mesh::{AgentId, AgentMesh, Message, RedisMesh, Topic};
    use std::time::Duration;
    use tokio::time::timeout;

    async fn setup_mesh() -> Result<RedisMesh, Box<dyn std::error::Error>> {
        let mesh = RedisMesh::new("redis://localhost:6379").await?;
        Ok(mesh)
    }

    #[tokio::test]
    async fn test_point_to_point_messaging() {
        let mesh = match setup_mesh().await {
            Ok(m) => m,
            Err(_) => {
                eprintln!("Redis not available, skipping test");
                return;
            }
        };

        let sender = AgentId::from("sender");
        let receiver = AgentId::from("receiver");

        // Register agents
        mesh.register_presence(&sender, 60).await.unwrap();
        mesh.register_presence(&receiver, 60).await.unwrap();

        // Send message
        let msg = Message::unicast(sender.clone(), receiver.clone(), "test message");
        mesh.send(&receiver, msg.clone()).await.unwrap();

        // Receive message
        let received = mesh.receive(&receiver, 5).await.unwrap();
        assert!(received.is_some());

        let received_msg = received.unwrap();
        assert_eq!(received_msg.sender(), Some(&sender));

        // Cleanup
        mesh.deregister_presence(&sender).await.unwrap();
        mesh.deregister_presence(&receiver).await.unwrap();
    }

    #[tokio::test]
    async fn test_pub_sub_messaging() {
        let mesh = match setup_mesh().await {
            Ok(m) => m,
            Err(_) => {
                eprintln!("Redis not available, skipping test");
                return;
            }
        };

        let publisher = AgentId::from("publisher");
        let topic = Topic::from("test-topic");

        mesh.register_presence(&publisher, 60).await.unwrap();

        // Subscribe to topic (in background task)
        let mesh2 = RedisMesh::new("redis://localhost:6379").await.unwrap();
        let topic_clone = topic.clone();
        let receive_task = tokio::spawn(async move {
            let mut stream = mesh2.subscribe(&topic_clone).await.unwrap();
            use futures::StreamExt;

            // Wait for one message with timeout
            timeout(Duration::from_secs(5), stream.next())
                .await
                .ok()
                .flatten()
        });

        // Give subscriber time to connect
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Publish message
        let msg = Message::broadcast(publisher.clone(), "test pub/sub");
        mesh.publish(&topic, msg).await.unwrap();

        // Wait for message to be received
        let result = receive_task.await.unwrap();
        assert!(result.is_some());

        // Cleanup
        mesh.deregister_presence(&publisher).await.unwrap();
    }

    #[tokio::test]
    async fn test_presence_and_listing() {
        let mesh = match setup_mesh().await {
            Ok(m) => m,
            Err(_) => {
                eprintln!("Redis not available, skipping test");
                return;
            }
        };

        let agent1 = AgentId::from("agent-1");
        let agent2 = AgentId::from("agent-2");

        // Register agents
        mesh.register_presence(&agent1, 60).await.unwrap();
        mesh.register_presence(&agent2, 60).await.unwrap();

        // Check reachability
        assert!(mesh.is_reachable(&agent1).await);
        assert!(mesh.is_reachable(&agent2).await);

        // List agents
        let agents = mesh.list_agents().await.unwrap();
        assert!(agents.contains(&agent1));
        assert!(agents.contains(&agent2));

        // Deregister one agent
        mesh.deregister_presence(&agent1).await.unwrap();

        // Check reachability again
        assert!(!mesh.is_reachable(&agent1).await);
        assert!(mesh.is_reachable(&agent2).await);

        // Cleanup
        mesh.deregister_presence(&agent2).await.unwrap();
    }

    #[tokio::test]
    async fn test_queue_depth() {
        let mesh = match setup_mesh().await {
            Ok(m) => m,
            Err(_) => {
                eprintln!("Redis not available, skipping test");
                return;
            }
        };

        let receiver = AgentId::from("queue-receiver");
        mesh.register_presence(&receiver, 60).await.unwrap();

        // Send multiple messages without receiving
        for i in 0..5 {
            let msg = Message::new(format!("message-{}", i));
            mesh.send(&receiver, msg).await.unwrap();
        }

        // Check queue depth
        let depth = mesh.queue_depth().await.unwrap();
        assert!(
            depth >= 5,
            "Expected at least 5 messages in queue, got {}",
            depth
        );

        // Cleanup
        mesh.deregister_presence(&receiver).await.unwrap();
    }

    #[tokio::test]
    async fn test_message_metadata() {
        let mesh = match setup_mesh().await {
            Ok(m) => m,
            Err(_) => {
                eprintln!("Redis not available, skipping test");
                return;
            }
        };

        let sender = AgentId::from("meta-sender");
        let receiver = AgentId::from("meta-receiver");

        mesh.register_presence(&sender, 60).await.unwrap();
        mesh.register_presence(&receiver, 60).await.unwrap();

        // Send message with metadata
        let msg = Message::unicast(sender.clone(), receiver.clone(), "test")
            .with_metadata("priority", "high")
            .with_metadata("type", "command");

        mesh.send(&receiver, msg).await.unwrap();

        // Receive and verify metadata
        let received = mesh.receive(&receiver, 5).await.unwrap().unwrap();
        assert_eq!(received.get_metadata("priority"), Some("high"));
        assert_eq!(received.get_metadata("type"), Some("command"));

        // Cleanup
        mesh.deregister_presence(&sender).await.unwrap();
        mesh.deregister_presence(&receiver).await.unwrap();
    }

    #[tokio::test]
    async fn test_correlation_id() {
        let mesh = match setup_mesh().await {
            Ok(m) => m,
            Err(_) => {
                eprintln!("Redis not available, skipping test");
                return;
            }
        };

        let requester = AgentId::from("requester");
        let responder = AgentId::from("responder");

        mesh.register_presence(&requester, 60).await.unwrap();
        mesh.register_presence(&responder, 60).await.unwrap();

        // Send request with correlation ID
        let correlation_id = "req-123";
        let request = Message::unicast(requester.clone(), responder.clone(), "ping")
            .with_correlation_id(correlation_id);

        mesh.send(&responder, request).await.unwrap();

        // Receive request
        let received_request = mesh.receive(&responder, 5).await.unwrap().unwrap();
        assert_eq!(
            received_request.correlation_id.as_deref(),
            Some(correlation_id)
        );

        // Send response with same correlation ID
        let response = Message::unicast(responder.clone(), requester.clone(), "pong")
            .with_correlation_id(received_request.correlation_id.unwrap());

        mesh.send(&requester, response).await.unwrap();

        // Receive response
        let received_response = mesh.receive(&requester, 5).await.unwrap().unwrap();
        assert_eq!(
            received_response.correlation_id.as_deref(),
            Some(correlation_id)
        );

        // Cleanup
        mesh.deregister_presence(&requester).await.unwrap();
        mesh.deregister_presence(&responder).await.unwrap();
    }
}
