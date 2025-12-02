//! OpenAPI specification and documentation support
//!
//! This module provides comprehensive OpenAPI 3.0 specification generation
//! for Skreaver HTTP APIs, including automatic schema generation,
//! documentation UI, and validation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

pub mod generator;
pub mod ui;
pub mod validation;

pub use generator::{ApiDocGenerator, OpenApiGenerator};
pub use ui::{
    ApiSpecResponse, ApiUiConfig, HeaderVisibility, RapiDocUi, SwaggerUi, TryItOutMode,
    ValidationMode,
};
pub use validation::{
    RequestValidator, ResponseValidator, ValidatedJson, ValidationConfig, ValidationErrors,
    ValidationLevel, validation_middleware,
};

/// OpenAPI specification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiConfig {
    /// API title
    pub title: String,
    /// API description
    pub description: String,
    /// API version
    pub version: String,
    /// Terms of service URL
    pub terms_of_service: Option<String>,
    /// Contact information
    pub contact: Option<ApiContact>,
    /// License information
    pub license: Option<ApiLicense>,
    /// Server information
    pub servers: Vec<ApiServer>,
    /// External documentation
    pub external_docs: Option<ExternalDocs>,
    /// Enable UI in development
    pub enable_ui: bool,
    /// UI path (default: /docs)
    pub ui_path: String,
    /// JSON spec path (default: /openapi.json)
    pub spec_path: String,
}

impl Default for OpenApiConfig {
    fn default() -> Self {
        Self {
            title: "Skreaver API".to_string(),
            description: "AI Agent Infrastructure API".to_string(),
            version: "0.5.0".to_string(),
            terms_of_service: None,
            contact: Some(ApiContact {
                name: Some("Skreaver Team".to_string()),
                url: Some("https://ohusiev.com".to_string()),
                email: Some("ohusiev@icloud.com".to_string()),
            }),
            license: Some(ApiLicense {
                name: "MIT".to_string(),
                url: Some("https://opensource.org/licenses/MIT".to_string()),
            }),
            servers: vec![ApiServer {
                url: "http://localhost:3000".to_string(),
                description: Some("Development server".to_string()),
                variables: HashMap::new(),
            }],
            external_docs: None,
            enable_ui: true,
            ui_path: "/docs".to_string(),
            spec_path: "/openapi.json".to_string(),
        }
    }
}

/// API contact information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiContact {
    /// Contact name
    pub name: Option<String>,
    /// Contact URL
    pub url: Option<String>,
    /// Contact email
    pub email: Option<String>,
}

/// API license information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiLicense {
    /// License name
    pub name: String,
    /// License URL
    pub url: Option<String>,
}

/// API server information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiServer {
    /// Server URL
    pub url: String,
    /// Server description
    pub description: Option<String>,
    /// Server variables
    pub variables: HashMap<String, ServerVariable>,
}

/// Server variable definition
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServerVariable {
    /// Variable description
    pub description: Option<String>,
    /// Default value
    pub default: String,
    /// Enumerated values
    pub enum_values: Option<Vec<String>>,
}

/// External documentation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ExternalDocs {
    /// Documentation description
    pub description: String,
    /// Documentation URL
    pub url: String,
}

/// Standard API response wrapper
///
/// Type-safe response handling using enum variants to prevent invalid states.
/// This eliminates impossible states like `success=true` with `error=Some(...)`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ApiResponse<T> {
    /// Successful response with data
    Success {
        /// Response data
        data: T,
        /// Request timestamp
        timestamp: chrono::DateTime<chrono::Utc>,
        /// Request ID for tracing
        request_id: String,
    },
    /// Error response
    Error {
        /// Error message
        error: String,
        /// Request timestamp
        timestamp: chrono::DateTime<chrono::Utc>,
        /// Request ID for tracing
        request_id: String,
    },
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self::Success {
            data,
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Create an error response
    pub fn error(message: String) -> Self {
        Self::Error {
            error: message,
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Check if response is successful
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// Check if response is an error
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    /// Convert to Result
    pub fn into_result(self) -> Result<T, String> {
        match self {
            Self::Success { data, .. } => Ok(data),
            Self::Error { error, .. } => Err(error),
        }
    }

    /// Get the request ID
    pub fn request_id(&self) -> &str {
        match self {
            Self::Success { request_id, .. } => request_id,
            Self::Error { request_id, .. } => request_id,
        }
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::Success { timestamp, .. } => *timestamp,
            Self::Error { timestamp, .. } => *timestamp,
        }
    }
}

/// Pagination information for list responses
///
/// Uses computed methods for `has_next` and `has_prev` to avoid redundant state.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PaginationInfo {
    /// Current page number (1-based)
    pub page: u64,
    /// Number of items per page
    pub page_size: u64,
    /// Total number of items
    pub total_items: u64,
    /// Total number of pages
    pub total_pages: u64,
}

impl PaginationInfo {
    /// Check if there are more pages after the current one
    pub fn has_next(&self) -> bool {
        self.page < self.total_pages
    }

    /// Check if there are pages before the current one
    pub fn has_prev(&self) -> bool {
        self.page > 1
    }
}

/// Paginated API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    /// Response items
    pub items: Vec<T>,
    /// Pagination information
    pub pagination: PaginationInfo,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Request ID for tracing
    pub request_id: String,
}

impl<T> PaginatedResponse<T> {
    /// Create a paginated response
    pub fn new(items: Vec<T>, page: u64, page_size: u64, total_items: u64) -> Self {
        let total_pages = total_items.div_ceil(page_size);

        Self {
            items,
            pagination: PaginationInfo {
                page,
                page_size,
                total_items,
                total_pages,
            },
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Common API error types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiError {
    /// Bad request (400)
    BadRequest {
        message: String,
        details: Option<serde_json::Value>,
    },
    /// Unauthorized (401)
    Unauthorized { message: String },
    /// Forbidden (403)
    Forbidden {
        message: String,
        required_permission: Option<String>,
    },
    /// Not found (404)
    NotFound {
        message: String,
        resource_type: Option<String>,
        resource_id: Option<String>,
    },
    /// Conflict (409)
    Conflict { message: String },
    /// Unprocessable entity (422)
    UnprocessableEntity {
        message: String,
        validation_errors: Vec<ValidationError>,
    },
    /// Rate limit exceeded (429)
    RateLimitExceeded {
        message: String,
        retry_after: Option<u64>,
    },
    /// Internal server error (500)
    InternalServerError { message: String, error_id: String },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::BadRequest { message, .. } => write!(f, "Bad Request: {}", message),
            ApiError::Unauthorized { message } => write!(f, "Unauthorized: {}", message),
            ApiError::Forbidden { message, .. } => write!(f, "Forbidden: {}", message),
            ApiError::NotFound { message, .. } => write!(f, "Not Found: {}", message),
            ApiError::Conflict { message } => write!(f, "Conflict: {}", message),
            ApiError::UnprocessableEntity { message, .. } => {
                write!(f, "Unprocessable Entity: {}", message)
            }
            ApiError::RateLimitExceeded { message, .. } => {
                write!(f, "Rate Limit Exceeded: {}", message)
            }
            ApiError::InternalServerError { message, .. } => {
                write!(f, "Internal Server Error: {}", message)
            }
        }
    }
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ValidationError {
    /// Field name
    pub field: String,
    /// Error code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Rejected value
    pub rejected_value: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.is_success());
        assert!(!response.is_error());

        match response {
            ApiResponse::Success { data, .. } => assert_eq!(data, "test data"),
            _ => panic!("Expected Success variant"),
        }
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error("test error".to_string());
        assert!(!response.is_success());
        assert!(response.is_error());

        match response {
            ApiResponse::Error { error, .. } => assert_eq!(error, "test error"),
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn test_api_response_into_result() {
        let success_response = ApiResponse::success("data");
        assert_eq!(success_response.into_result(), Ok("data"));

        let error_response: ApiResponse<String> = ApiResponse::error("error".to_string());
        assert_eq!(error_response.into_result(), Err("error".to_string()));
    }

    #[test]
    fn test_api_response_accessors() {
        let response = ApiResponse::success("test");
        assert!(!response.request_id().is_empty());
        assert!(response.timestamp() <= chrono::Utc::now());
    }

    #[test]
    fn test_paginated_response() {
        let items = vec!["item1", "item2", "item3"];
        let response = PaginatedResponse::new(items.clone(), 1, 2, 5);

        assert_eq!(response.items, items);
        assert_eq!(response.pagination.page, 1);
        assert_eq!(response.pagination.page_size, 2);
        assert_eq!(response.pagination.total_items, 5);
        assert_eq!(response.pagination.total_pages, 3);
        assert!(response.pagination.has_next());
        assert!(!response.pagination.has_prev());
    }

    #[test]
    fn test_pagination_info_computed_methods() {
        // First page
        let first_page = PaginationInfo {
            page: 1,
            page_size: 10,
            total_items: 50,
            total_pages: 5,
        };
        assert!(first_page.has_next());
        assert!(!first_page.has_prev());

        // Middle page
        let middle_page = PaginationInfo {
            page: 3,
            page_size: 10,
            total_items: 50,
            total_pages: 5,
        };
        assert!(middle_page.has_next());
        assert!(middle_page.has_prev());

        // Last page
        let last_page = PaginationInfo {
            page: 5,
            page_size: 10,
            total_items: 50,
            total_pages: 5,
        };
        assert!(!last_page.has_next());
        assert!(last_page.has_prev());

        // Only page
        let only_page = PaginationInfo {
            page: 1,
            page_size: 10,
            total_items: 5,
            total_pages: 1,
        };
        assert!(!only_page.has_next());
        assert!(!only_page.has_prev());
    }

    #[test]
    fn test_openapi_config_default() {
        let config = OpenApiConfig::default();
        assert_eq!(config.title, "Skreaver API");
        assert_eq!(config.version, "0.5.0");
        assert!(config.enable_ui);
        assert_eq!(config.ui_path, "/docs");
        assert_eq!(config.spec_path, "/openapi.json");
    }
}
