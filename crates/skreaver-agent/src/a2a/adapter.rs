//! A2A protocol adapter implementation.

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tracing::info;

use crate::error::{AgentError, AgentResult};
use crate::traits::UnifiedAgent;
use crate::types::{AgentInfo, Protocol, StreamEvent, UnifiedMessage, UnifiedTask};

use super::conversions::{
    a2a_card_to_agent_info, a2a_to_unified_stream_event, a2a_to_unified_task,
    unified_to_a2a_message,
};

use skreaver_a2a::{A2aClient, AgentCard};

/// Adapter that wraps an A2A client to provide the unified agent interface.
pub struct A2aAgentAdapter {
    info: AgentInfo,
    client: A2aClient,
    agent_card: Option<Arc<AgentCard>>,
}

impl std::fmt::Debug for A2aAgentAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("A2aAgentAdapter")
            .field("info", &self.info)
            .finish()
    }
}

impl A2aAgentAdapter {
    /// Create a new A2A adapter from a client.
    pub fn new(client: A2aClient) -> Self {
        let base_url = client.base_url().to_string();
        let info = AgentInfo::new(&base_url, &base_url).with_protocol(Protocol::A2a);

        Self {
            info,
            client,
            agent_card: None,
        }
    }

    /// Connect to an A2A agent and create an adapter.
    pub async fn connect(url: &str) -> AgentResult<Self> {
        info!(url = %url, "Connecting to A2A agent");
        let client = A2aClient::new(url).map_err(|e| AgentError::ConnectionError(e.to_string()))?;
        let mut adapter = Self::new(client);
        adapter.discover().await?;
        Ok(adapter)
    }

    /// Connect with bearer token authentication.
    pub async fn connect_with_bearer(url: &str, token: impl Into<String>) -> AgentResult<Self> {
        let client = A2aClient::new(url)
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?
            .with_bearer_token(token);
        let mut adapter = Self::new(client);
        adapter.discover().await?;
        Ok(adapter)
    }

    /// Connect with API key authentication.
    pub async fn connect_with_api_key(
        url: &str,
        header: impl Into<String>,
        key: impl Into<String>,
    ) -> AgentResult<Self> {
        let client = A2aClient::new(url)
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?
            .with_api_key(header, key);
        let mut adapter = Self::new(client);
        adapter.discover().await?;
        Ok(adapter)
    }

    /// Discover the agent's capabilities.
    pub async fn discover(&mut self) -> AgentResult<()> {
        let card = self
            .client
            .get_agent_card()
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        // Update info from agent card
        self.info = a2a_card_to_agent_info(&card);
        self.agent_card = Some(Arc::new(card));

        Ok(())
    }

    /// Get the cached agent card.
    pub fn agent_card(&self) -> Option<&AgentCard> {
        self.agent_card.as_deref()
    }

    /// Get the underlying client.
    pub fn client(&self) -> &A2aClient {
        &self.client
    }
}

#[async_trait]
impl UnifiedAgent for A2aAgentAdapter {
    fn info(&self) -> &AgentInfo {
        &self.info
    }

    async fn send_message(&self, message: UnifiedMessage) -> AgentResult<UnifiedTask> {
        let a2a_message = unified_to_a2a_message(&message);

        let task = self
            .client
            .send(a2a_message, None, None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        Ok(a2a_to_unified_task(&task))
    }

    async fn send_message_to_task(
        &self,
        task_id: &str,
        message: UnifiedMessage,
    ) -> AgentResult<UnifiedTask> {
        let a2a_message = unified_to_a2a_message(&message);

        let task = self
            .client
            .send(a2a_message, Some(task_id.to_string()), None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        Ok(a2a_to_unified_task(&task))
    }

    async fn send_message_streaming(
        &self,
        message: UnifiedMessage,
    ) -> AgentResult<Pin<Box<dyn Stream<Item = AgentResult<StreamEvent>> + Send>>> {
        let a2a_message = unified_to_a2a_message(&message);

        let stream = self
            .client
            .send_streaming(a2a_message, None, None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        // Map the stream
        use futures::StreamExt;
        let mapped = stream.map(|result| {
            result
                .map(|event| a2a_to_unified_stream_event(&event))
                .map_err(|e| AgentError::ConnectionError(e.to_string()))
        });

        Ok(Box::pin(mapped))
    }

    async fn get_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let task = self.client.get_task(task_id).await.map_err(|e| match e {
            skreaver_a2a::A2aError::TaskNotFound { .. } => {
                AgentError::TaskNotFound(task_id.to_string())
            }
            _ => AgentError::ConnectionError(e.to_string()),
        })?;

        Ok(a2a_to_unified_task(&task))
    }

    async fn cancel_task(&self, task_id: &str) -> AgentResult<UnifiedTask> {
        let task = self
            .client
            .cancel_task(task_id, None)
            .await
            .map_err(|e| AgentError::ConnectionError(e.to_string()))?;

        Ok(a2a_to_unified_task(&task))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skreaver_a2a::AgentCard;

    #[test]
    fn test_a2a_card_to_agent_info() {
        let card = AgentCard::new("test-agent", "Test Agent", "https://test.example.com")
            .with_description("A test agent")
            .with_streaming();

        let info = a2a_card_to_agent_info(&card);

        assert_eq!(info.id, "test-agent");
        assert_eq!(info.name, "Test Agent");
        assert!(info.supports_streaming);
        assert!(info.protocols.contains(&Protocol::A2a));
    }
}
