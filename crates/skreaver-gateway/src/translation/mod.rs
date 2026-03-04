//! Protocol Translation
//!
//! This module provides bidirectional translation between MCP and A2A protocols.

mod a2a_to_mcp;
mod mcp_to_a2a;

pub use a2a_to_mcp::A2aToMcpTranslator;
pub use mcp_to_a2a::McpToA2aTranslator;

use crate::detection::Protocol;
use crate::error::{GatewayError, GatewayResult};
use serde_json::Value;

/// Bidirectional protocol translator
///
/// Provides translation between MCP and A2A protocols in both directions.
#[derive(Debug, Clone, Default)]
pub struct ProtocolTranslator {
    mcp_to_a2a: McpToA2aTranslator,
    a2a_to_mcp: A2aToMcpTranslator,
}

impl ProtocolTranslator {
    /// Create a new bidirectional translator
    pub fn new() -> Self {
        Self::default()
    }

    /// Translate a message from source to target protocol
    pub fn translate(&self, message: Value, from: Protocol, to: Protocol) -> GatewayResult<Value> {
        if from == to {
            return Ok(message);
        }

        match (from, to) {
            (Protocol::Mcp, Protocol::A2a) => self.mcp_to_a2a.translate(message),
            (Protocol::A2a, Protocol::Mcp) => self.a2a_to_mcp.translate(message),
            _ => Err(GatewayError::TranslationError(format!(
                "Unsupported translation: {} -> {}",
                from, to
            ))),
        }
    }

    /// Get the MCP to A2A translator
    pub fn mcp_to_a2a(&self) -> &McpToA2aTranslator {
        &self.mcp_to_a2a
    }

    /// Get the A2A to MCP translator
    pub fn a2a_to_mcp(&self) -> &A2aToMcpTranslator {
        &self.a2a_to_mcp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_same_protocol_passthrough() {
        let translator = ProtocolTranslator::new();

        let msg = json!({"test": "data"});
        let result = translator
            .translate(msg.clone(), Protocol::Mcp, Protocol::Mcp)
            .unwrap();
        assert_eq!(result, msg);
    }
}
