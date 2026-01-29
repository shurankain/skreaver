//! MCP Elicitation support (2025-11-25 spec)
//!
//! Elicitation allows servers to request information from users through
//! the client. This is used for interactive workflows where the server
//! needs additional input that can only come from a human.
//!
//! ## Modes
//!
//! - **Form mode**: Server provides a JSON Schema for structured input
//! - **URL mode**: Server directs user to a URL for out-of-band interaction
//!
//! ## Usage
//!
//! ```rust,ignore
//! use skreaver_mcp::elicitation::{ElicitationRequest, ElicitationResponse, ElicitationAction};
//!
//! // Build a form elicitation request
//! let request = ElicitationRequest::form(
//!     "Please provide your API key to continue",
//!     serde_json::json!({
//!         "type": "object",
//!         "properties": {
//!             "api_key": { "type": "string", "description": "Your API key" }
//!         },
//!         "required": ["api_key"]
//!     }),
//! );
//!
//! // Build a URL elicitation request
//! let request = ElicitationRequest::url(
//!     "Please authorize the application",
//!     "https://auth.example.com/authorize?state=abc123",
//! );
//! ```

use serde::{Deserialize, Serialize};

/// Mode of elicitation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElicitationMode {
    /// Structured form input with JSON Schema
    Form,
    /// URL-based out-of-band interaction
    Url,
}

/// An elicitation request from server to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationRequest {
    /// Elicitation mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<ElicitationMode>,
    /// Human-readable message explaining why input is needed
    pub message: String,
    /// JSON Schema for form mode (flat objects with primitive properties)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_schema: Option<serde_json::Value>,
    /// URL for URL mode
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Unique identifier for tracking (URL mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation_id: Option<String>,
}

impl ElicitationRequest {
    /// Create a form-mode elicitation request
    pub fn form(message: impl Into<String>, schema: serde_json::Value) -> Self {
        Self {
            mode: Some(ElicitationMode::Form),
            message: message.into(),
            requested_schema: Some(schema),
            url: None,
            elicitation_id: None,
        }
    }

    /// Create a URL-mode elicitation request
    pub fn url(message: impl Into<String>, url: impl Into<String>) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            mode: Some(ElicitationMode::Url),
            message: message.into(),
            requested_schema: None,
            url: Some(url.into()),
            elicitation_id: Some(id),
        }
    }
}

/// User action in response to an elicitation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ElicitationAction {
    /// User approved and provided data
    Accept,
    /// User explicitly rejected the request
    Decline,
    /// User dismissed without explicit choice
    Cancel,
}

/// Response from client to an elicitation request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationResponse {
    /// User's action
    pub action: ElicitationAction,
    /// Submitted data (when action is Accept, matches requested schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
}

impl ElicitationResponse {
    /// Create an accepted response with content
    pub fn accept(content: serde_json::Value) -> Self {
        Self {
            action: ElicitationAction::Accept,
            content: Some(content),
        }
    }

    /// Create a declined response
    pub fn decline() -> Self {
        Self {
            action: ElicitationAction::Decline,
            content: None,
        }
    }

    /// Create a cancelled response
    pub fn cancel() -> Self {
        Self {
            action: ElicitationAction::Cancel,
            content: None,
        }
    }

    /// Whether the user accepted and provided data
    pub fn is_accepted(&self) -> bool {
        self.action == ElicitationAction::Accept
    }
}

/// Schema builder for elicitation form fields
///
/// Supports primitive types: string, number, integer, boolean, enum.
/// String format support: email, uri, date, date-time.
#[derive(Debug, Clone, Default)]
pub struct ElicitationSchemaBuilder {
    properties: serde_json::Map<String, serde_json::Value>,
    required: Vec<String>,
}

impl ElicitationSchemaBuilder {
    /// Create a new schema builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a string field
    pub fn string_field(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "string",
                "description": description.into()
            }),
        );
        if required {
            self.required.push(name);
        }
        self
    }

    /// Add a string field with format constraint
    pub fn formatted_string_field(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        format: &str,
        required: bool,
    ) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "string",
                "format": format,
                "description": description.into()
            }),
        );
        if required {
            self.required.push(name);
        }
        self
    }

    /// Add a number field
    pub fn number_field(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "number",
                "description": description.into()
            }),
        );
        if required {
            self.required.push(name);
        }
        self
    }

    /// Add an integer field
    pub fn integer_field(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "integer",
                "description": description.into()
            }),
        );
        if required {
            self.required.push(name);
        }
        self
    }

    /// Add a boolean field
    pub fn boolean_field(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "boolean",
                "description": description.into()
            }),
        );
        if required {
            self.required.push(name);
        }
        self
    }

    /// Add an enum field (single select)
    pub fn enum_field(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        options: &[&str],
        required: bool,
    ) -> Self {
        let name = name.into();
        self.properties.insert(
            name.clone(),
            serde_json::json!({
                "type": "string",
                "enum": options,
                "description": description.into()
            }),
        );
        if required {
            self.required.push(name);
        }
        self
    }

    /// Build the JSON Schema
    pub fn build(self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": self.properties,
            "required": self.required
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_form_elicitation() {
        let schema = ElicitationSchemaBuilder::new()
            .string_field("api_key", "Your API key", true)
            .string_field("nickname", "Optional nickname", false)
            .build();

        let request = ElicitationRequest::form("Please provide your API key", schema);
        assert_eq!(request.mode, Some(ElicitationMode::Form));
        assert!(request.requested_schema.is_some());
        assert!(request.url.is_none());

        let schema = request.requested_schema.unwrap();
        assert!(schema["properties"]["api_key"].is_object());
        assert!(schema["properties"]["nickname"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["api_key"]));
    }

    #[test]
    fn test_url_elicitation() {
        let request =
            ElicitationRequest::url("Please authorize", "https://auth.example.com/authorize");
        assert_eq!(request.mode, Some(ElicitationMode::Url));
        assert!(request.url.is_some());
        assert!(request.elicitation_id.is_some());
        assert!(request.requested_schema.is_none());
    }

    #[test]
    fn test_elicitation_response_accept() {
        let response = ElicitationResponse::accept(serde_json::json!({"api_key": "abc123"}));
        assert!(response.is_accepted());
        assert_eq!(response.action, ElicitationAction::Accept);
        assert!(response.content.is_some());
    }

    #[test]
    fn test_elicitation_response_decline() {
        let response = ElicitationResponse::decline();
        assert!(!response.is_accepted());
        assert_eq!(response.action, ElicitationAction::Decline);
        assert!(response.content.is_none());
    }

    #[test]
    fn test_elicitation_response_cancel() {
        let response = ElicitationResponse::cancel();
        assert!(!response.is_accepted());
        assert_eq!(response.action, ElicitationAction::Cancel);
    }

    #[test]
    fn test_schema_builder_all_types() {
        let schema = ElicitationSchemaBuilder::new()
            .string_field("name", "Your name", true)
            .formatted_string_field("email", "Email address", "email", true)
            .number_field("score", "Score value", false)
            .integer_field("count", "Item count", false)
            .boolean_field("agree", "Do you agree?", true)
            .enum_field("tier", "Service tier", &["free", "pro", "enterprise"], true)
            .build();

        assert_eq!(schema["properties"]["name"]["type"], "string");
        assert_eq!(schema["properties"]["email"]["format"], "email");
        assert_eq!(schema["properties"]["score"]["type"], "number");
        assert_eq!(schema["properties"]["count"]["type"], "integer");
        assert_eq!(schema["properties"]["agree"]["type"], "boolean");
        assert!(schema["properties"]["tier"]["enum"].is_array());
        assert_eq!(
            schema["required"],
            serde_json::json!(["name", "email", "agree", "tier"])
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let request = ElicitationRequest::form(
            "Test message",
            serde_json::json!({"type": "object", "properties": {}}),
        );
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ElicitationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.message, "Test message");
        assert_eq!(deserialized.mode, Some(ElicitationMode::Form));

        let response = ElicitationResponse::accept(serde_json::json!({"key": "val"}));
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: ElicitationResponse = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_accepted());
    }
}
