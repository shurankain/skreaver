//! Agent Factory Pattern Demo
//!
//! This example demonstrates the new agent factory pattern for dynamic
//! agent creation in the Skreaver HTTP runtime.

use serde_json::json;
use skreaver_http::runtime::{
    AdvancedAgentBuilder, AgentFactory, AnalyticsAgentBuilder, EchoAgentBuilder,
    api_types::{AgentLimits, AgentSpec, AgentType},
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🦀 Skreaver Agent Factory Demo");
    println!("=================================");

    // Create and configure the agent factory
    let mut factory = AgentFactory::new();

    // Register all available agent builders
    factory.register_builder(Box::new(EchoAgentBuilder));
    factory.register_builder(Box::new(AdvancedAgentBuilder));
    factory.register_builder(Box::new(AnalyticsAgentBuilder));

    println!("\n📋 Supported Agent Types:");
    for agent_type in factory.supported_types() {
        println!("  - {}", agent_type);
    }

    // Example 1: Create a simple echo agent
    println!("\n🔵 Creating Echo Agent...");
    let echo_spec = AgentSpec {
        agent_type: AgentType::Echo,
        name: Some("demo-echo".to_string()),
        config: HashMap::new(),
        limits: AgentLimits::default(),
    };

    let echo_response = factory.create_agent(echo_spec, None).await?;
    println!("✅ Created agent: {}", echo_response.agent_id);
    println!("   Type: {}", echo_response.spec.agent_type);
    println!("   Status: {}", echo_response.status.simple_name());

    // Example 2: Create an advanced agent with configuration
    println!("\n🟡 Creating Advanced Agent with analytical mode...");
    let mut config = HashMap::new();
    config.insert("mode".to_string(), json!("analytical"));
    config.insert("use_tools".to_string(), json!(true));

    let advanced_spec = AgentSpec {
        agent_type: AgentType::Advanced,
        name: Some("demo-advanced".to_string()),
        config,
        limits: AgentLimits {
            max_memory_mb: 128,
            max_observation_size_kb: 512,
            max_concurrent_tools: 3,
            execution_timeout_secs: 45,
        },
    };

    let advanced_response = factory.create_agent(advanced_spec, None).await?;
    println!("✅ Created agent: {}", advanced_response.agent_id);
    println!("   Type: {}", advanced_response.spec.agent_type);
    println!(
        "   Endpoints available: {:?}",
        advanced_response.endpoints.stream.is_some()
    );

    // Example 3: Create an analytics agent with comprehensive analysis
    println!("\n🟢 Creating Analytics Agent with comprehensive analysis...");
    let mut analytics_config = HashMap::new();
    analytics_config.insert("depth".to_string(), json!("comprehensive"));

    let analytics_spec = AgentSpec {
        agent_type: AgentType::Analytics,
        name: Some("demo-analytics".to_string()),
        config: analytics_config,
        limits: AgentLimits::default(),
    };

    let analytics_response = factory.create_agent(analytics_spec, None).await?;
    println!("✅ Created agent: {}", analytics_response.agent_id);
    println!("   Type: {}", analytics_response.spec.agent_type);

    // Show factory statistics
    println!("\n📊 Factory Statistics:");
    println!("  Total agents created: {}", factory.agent_count().await);
    println!("  Active agent IDs: {:?}", factory.list_agent_ids().await);

    // Example 4: Demonstrate error handling
    println!("\n❌ Demonstrating Error Handling...");

    // Try to create an agent with invalid configuration
    let mut invalid_config = HashMap::new();
    invalid_config.insert("mode".to_string(), json!("invalid_mode"));

    let invalid_spec = AgentSpec {
        agent_type: AgentType::Advanced,
        name: Some("invalid-agent".to_string()),
        config: invalid_config,
        limits: AgentLimits::default(),
    };

    match factory.create_agent(invalid_spec, None).await {
        Ok(_) => println!("❗ Unexpected success with invalid config"),
        Err(e) => println!("✅ Correctly caught error: {}", e),
    }

    // Try to create an agent with invalid ID
    let echo_spec_invalid_id = AgentSpec {
        agent_type: AgentType::Echo,
        name: Some("invalid-id-test".to_string()),
        config: HashMap::new(),
        limits: AgentLimits::default(),
    };

    match factory
        .create_agent(echo_spec_invalid_id, Some("invalid@id!".to_string()))
        .await
    {
        Ok(_) => println!("❗ Unexpected success with invalid ID"),
        Err(e) => println!("✅ Correctly caught ID error: {}", e),
    }

    // Clean up by removing one agent
    println!("\n🧹 Cleanup: Removing advanced agent...");
    factory.remove_agent(&advanced_response.agent_id).await?;
    println!("✅ Agent removed successfully");
    println!("  Remaining agents: {}", factory.agent_count().await);

    println!("\n🎉 Agent Factory Demo Complete!");
    println!("\nKey Features Demonstrated:");
    println!("  ✅ Dynamic agent creation from specifications");
    println!("  ✅ Type-safe agent configuration validation");
    println!("  ✅ Multiple agent types (Echo, Advanced, Analytics)");
    println!("  ✅ Comprehensive error handling");
    println!("  ✅ Agent lifecycle management (create/remove)");
    println!("  ✅ Factory statistics and monitoring");

    Ok(())
}
