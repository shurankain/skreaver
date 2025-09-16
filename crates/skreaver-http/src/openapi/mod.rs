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
pub use ui::{SwaggerUi, RapiDocUi, ApiUiConfig, ApiSpecResponse};
pub use validation::{RequestValidator, ResponseValidator, ValidationConfig, ValidatedJson, ValidationErrors, validation_middleware};

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
            version: "0.3.0".to_string(),
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
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApiResponse<T> {
    /// Response data
    pub data: Option<T>,
    /// Success indicator
    pub success: bool,
    /// Error message if any
    pub error: Option<String>,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Request ID for tracing
    pub request_id: String,
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            data: Some(data),
            success: true,
            error: None,
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }
    
    /// Create an error response
    pub fn error(message: String) -> Self {
        Self {
            data: None,
            success: false,
            error: Some(message),
            timestamp: chrono::Utc::now(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Pagination information for list responses
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
    /// Whether there are more pages
    pub has_next: bool,
    /// Whether there are previous pages
    pub has_prev: bool,
}

/// Paginated API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedResponse<T> {
    /// Response items
    pub items: Vec<T>,
    /// Pagination information
    pub pagination: PaginationInfo,
    /// Request metadata
    #[serde(flatten)]
    pub meta: ApiResponse<()>,
}

impl<T> PaginatedResponse<T> {
    /// Create a paginated response
    pub fn new(
        items: Vec<T>,
        page: u64,
        page_size: u64,
        total_items: u64,
    ) -> Self {
        let total_pages = total_items.div_ceil(page_size);
        
        Self {
            items,
            pagination: PaginationInfo {
                page,
                page_size,
                total_items,
                total_pages,
                has_next: page < total_pages,
                has_prev: page > 1,
            },
            meta: ApiResponse {
                data: None,
                success: true,
                error: None,
                timestamp: chrono::Utc::now(),
                request_id: uuid::Uuid::new_v4().to_string(),
            },
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
    Unauthorized {
        message: String,
    },
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
    Conflict {
        message: String,
    },
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
    InternalServerError {
        message: String,
        error_id: String,
    },
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::BadRequest { message, .. } => write!(f, "Bad Request: {}", message),
            ApiError::Unauthorized { message } => write!(f, "Unauthorized: {}", message),
            ApiError::Forbidden { message, .. } => write!(f, "Forbidden: {}", message),
            ApiError::NotFound { message, .. } => write!(f, "Not Found: {}", message),
            ApiError::Conflict { message } => write!(f, "Conflict: {}", message),
            ApiError::UnprocessableEntity { message, .. } => write!(f, "Unprocessable Entity: {}", message),
            ApiError::RateLimitExceeded { message, .. } => write!(f, "Rate Limit Exceeded: {}", message),
            ApiError::InternalServerError { message, .. } => write!(f, "Internal Server Error: {}", message),
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
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }
    
    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error("test error".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
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
        assert!(response.pagination.has_next);
        assert!(!response.pagination.has_prev);
    }
    
    #[test]
    fn test_openapi_config_default() {
        let config = OpenApiConfig::default();
        assert_eq!(config.title, "Skreaver API");
        assert_eq!(config.version, "0.3.0");
        assert!(config.enable_ui);
        assert_eq!(config.ui_path, "/docs");
        assert_eq!(config.spec_path, "/openapi.json");
    }
}