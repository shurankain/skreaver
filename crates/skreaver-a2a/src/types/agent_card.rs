//! Agent Card types for capability discovery in the A2A protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent Card for capability discovery
///
/// The Agent Card is a JSON document that describes an agent's capabilities,
/// skills, and how to interact with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCard {
    /// Unique identifier for the agent
    pub agent_id: String,

    /// Human-readable name of the agent
    pub name: String,

    /// Description of the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Provider information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<AgentProvider>,

    /// Agent capabilities
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Skills the agent can perform
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<AgentSkill>,

    /// Security schemes for authentication
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub security_schemes: Vec<SecurityScheme>,

    /// Interfaces for interacting with the agent
    pub interfaces: Vec<AgentInterface>,

    /// Supported protocol versions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_versions: Vec<String>,

    /// Extensions supported by the agent
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<AgentExtension>,

    /// Optional signature for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<AgentCardSignature>,
}

impl AgentCard {
    /// Create a new agent card with required fields
    pub fn new(
        agent_id: impl Into<String>,
        name: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            agent_id: agent_id.into(),
            name: name.into(),
            description: None,
            provider: None,
            capabilities: AgentCapabilities::default(),
            skills: Vec::new(),
            security_schemes: Vec::new(),
            interfaces: vec![AgentInterface::http(base_url)],
            protocol_versions: vec!["0.3".to_string()],
            extensions: Vec::new(),
            signature: None,
        }
    }

    /// Add a skill to the agent card
    pub fn with_skill(mut self, skill: AgentSkill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Enable streaming capability
    pub fn with_streaming(mut self) -> Self {
        self.capabilities.streaming = true;
        self
    }

    /// Enable push notifications
    pub fn with_push_notifications(mut self) -> Self {
        self.capabilities.push_notifications = true;
        self
    }
}

/// Information about the agent provider
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentProvider {
    /// Provider name
    pub name: String,

    /// Provider URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Agent capabilities
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    /// Whether the agent supports streaming responses
    #[serde(default)]
    pub streaming: bool,

    /// Whether the agent supports push notifications
    #[serde(default)]
    pub push_notifications: bool,

    /// Whether the agent provides an extended agent card
    #[serde(default)]
    pub extended_agent_card: bool,
}

/// A skill that the agent can perform
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    /// Unique identifier for the skill
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of what the skill does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Input schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,

    /// Output schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,

    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl AgentSkill {
    /// Create a new skill
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            input_schema: None,
            output_schema: None,
            tags: Vec::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the input schema
    pub fn with_input_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    /// Set the output schema
    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }
}

/// Security scheme for authentication
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SecurityScheme {
    /// API key authentication
    #[serde(rename = "apiKey")]
    ApiKey {
        /// Name of the header or query parameter
        name: String,
        /// Where the key is sent
        #[serde(rename = "in")]
        location: ApiKeyLocation,
    },

    /// HTTP authentication (Bearer, Basic, etc.)
    #[serde(rename = "http")]
    Http {
        /// Authentication scheme (bearer, basic, etc.)
        scheme: String,
    },

    /// OAuth2 authentication
    #[serde(rename = "oauth2")]
    OAuth2 {
        /// OAuth2 flows
        flows: OAuth2Flows,
    },
}

/// Location of API key
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyLocation {
    Header,
    Query,
}

/// OAuth2 flows configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Flows {
    /// Authorization code flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_code: Option<OAuth2Flow>,

    /// Client credentials flow
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_credentials: Option<OAuth2Flow>,
}

/// OAuth2 flow configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Flow {
    /// Authorization URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_url: Option<String>,

    /// Token URL
    pub token_url: String,

    /// Available scopes
    #[serde(default)]
    pub scopes: HashMap<String, String>,
}

/// Interface for interacting with the agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentInterface {
    /// HTTP/REST interface
    #[serde(rename = "http")]
    Http {
        /// Base URL for the agent
        base_url: String,
    },

    /// gRPC interface
    #[serde(rename = "grpc")]
    Grpc {
        /// Host and port for gRPC
        host: String,
    },
}

impl AgentInterface {
    /// Create an HTTP interface
    pub fn http(base_url: impl Into<String>) -> Self {
        AgentInterface::Http {
            base_url: base_url.into(),
        }
    }

    /// Create a gRPC interface
    pub fn grpc(host: impl Into<String>) -> Self {
        AgentInterface::Grpc { host: host.into() }
    }
}

/// Extension supported by the agent
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentExtension {
    /// Extension identifier
    pub id: String,

    /// Extension version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Extension configuration
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub config: HashMap<String, serde_json::Value>,
}

/// Signature for agent card verification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCardSignature {
    /// Algorithm used for signing
    pub algorithm: String,

    /// The signature value
    pub value: String,

    /// Key ID for verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
}
