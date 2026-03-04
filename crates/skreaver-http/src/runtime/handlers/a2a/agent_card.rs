//! Agent Card Endpoint
//!
//! This module implements the agent card discovery endpoint for the A2A protocol.
//! The agent card describes the agent's capabilities and how to interact with it.

use axum::{extract::State, response::Json};
use skreaver_a2a::{AgentCapabilities, AgentCard, AgentInterface, AgentSkill};
use skreaver_tools::ToolRegistry;

use super::A2aState;
use super::errors::A2aApiResult;

/// GET /a2a/agent-card - Retrieve the agent's capability card
///
/// Returns an AgentCard describing this agent's capabilities, supported
/// operations, and communication preferences.
///
/// # A2A Protocol
///
/// The agent card is the primary discovery mechanism in the A2A protocol.
/// Other agents use this endpoint to understand what this agent can do
/// and how to communicate with it.
pub async fn get_agent_card<T: ToolRegistry + Clone + Send + Sync + 'static>(
    State(state): State<A2aState<T>>,
) -> A2aApiResult<Json<AgentCard>> {
    // Build capabilities
    let capabilities = AgentCapabilities {
        streaming: state.agent_card_config.supports_streaming,
        push_notifications: state.agent_card_config.supports_push_notifications,
        extended_agent_card: false,
    };

    // Create the agent card
    let agent_id = format!(
        "{}-{}",
        state
            .agent_card_config
            .name
            .to_lowercase()
            .replace(' ', "-"),
        uuid::Uuid::new_v4().simple()
    );

    let mut agent_card = AgentCard::new(
        &agent_id,
        &state.agent_card_config.name,
        &state.agent_card_config.base_url,
    )
    .with_description(&state.agent_card_config.description);

    // Set capabilities
    agent_card.capabilities = capabilities;

    // Add a default skill representing the agent's general capabilities
    agent_card = agent_card.with_skill(
        AgentSkill::new("general", "General Assistant")
            .with_description("General purpose assistant capabilities"),
    );

    // Set the default interface
    agent_card.interfaces = vec![AgentInterface::http(format!(
        "{}/a2a",
        state.agent_card_config.base_url
    ))];

    Ok(Json(agent_card))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_creation() {
        let skill =
            AgentSkill::new("calculator", "Calculator").with_description("Performs calculations");

        assert_eq!(skill.id, "calculator");
        assert_eq!(skill.name, "Calculator");
        assert_eq!(skill.description.as_deref(), Some("Performs calculations"));
    }

    #[test]
    fn test_agent_card_builder() {
        let card = AgentCard::new("test-agent", "Test Agent", "http://localhost:3000")
            .with_description("A test agent")
            .with_skill(AgentSkill::new("skill1", "Skill One"));

        assert_eq!(card.name, "Test Agent");
        assert_eq!(card.agent_id, "test-agent");
        assert_eq!(card.skills.len(), 1);
    }
}
