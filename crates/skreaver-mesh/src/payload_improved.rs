//! Improved message payload with explicit discrimination

use serde::{Deserialize, Serialize};

/// Message payload with explicit tagging for unambiguous deserialization
///
/// The `#[serde(tag = "type", content = "data")]` attribute ensures that
/// deserialization is deterministic and unambiguous.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum MessagePayload {
    /// String payload
    #[serde(rename = "text")]
    Text(String),

    /// JSON payload (arbitrary structured data)
    #[serde(rename = "json")]
    Json(serde_json::Value),

    /// Binary payload (base64 encoded in JSON)
    #[serde(rename = "binary")]
    Binary(#[serde(with = "base64_serde")] Vec<u8>),
}

mod base64_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
            .map_err(serde::de::Error::custom)
    }
}

impl From<String> for MessagePayload {
    fn from(s: String) -> Self {
        MessagePayload::Text(s)
    }
}

impl From<&str> for MessagePayload {
    fn from(s: &str) -> Self {
        MessagePayload::Text(s.to_string())
    }
}

impl From<serde_json::Value> for MessagePayload {
    fn from(v: serde_json::Value) -> Self {
        MessagePayload::Json(v)
    }
}

impl From<Vec<u8>> for MessagePayload {
    fn from(v: Vec<u8>) -> Self {
        MessagePayload::Binary(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_text_serde() {
        let payload = MessagePayload::Text("hello".to_string());
        let json = serde_json::to_string(&payload).unwrap();

        // Should serialize with explicit tag
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains(r#""data":"hello""#));

        // Should deserialize correctly
        let deserialized: MessagePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, deserialized);
    }

    #[test]
    fn test_payload_json_serde() {
        let payload = MessagePayload::Json(serde_json::json!({"key": "value"}));
        let json = serde_json::to_string(&payload).unwrap();

        // Should serialize with explicit tag
        assert!(json.contains(r#""type":"json""#));

        // Should deserialize correctly
        let deserialized: MessagePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, deserialized);
    }

    #[test]
    fn test_payload_binary_serde() {
        let payload = MessagePayload::Binary(vec![1, 2, 3, 4]);
        let json = serde_json::to_string(&payload).unwrap();

        // Should serialize with explicit tag
        assert!(json.contains(r#""type":"binary""#));

        // Should deserialize correctly
        let deserialized: MessagePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(payload, deserialized);
    }

    #[test]
    fn test_payload_disambiguation() {
        // With untagged, this might be ambiguous
        // With tagged, it's always clear what type we're deserializing

        let text_json = r#"{"type":"text","data":"hello"}"#;
        let text: MessagePayload = serde_json::from_str(text_json).unwrap();
        assert!(matches!(text, MessagePayload::Text(_)));

        let json_json = r#"{"type":"json","data":{"key":"value"}}"#;
        let json: MessagePayload = serde_json::from_str(json_json).unwrap();
        assert!(matches!(json, MessagePayload::Json(_)));
    }
}
