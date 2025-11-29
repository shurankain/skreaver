//! OpenAPI request and response validation

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

use super::{ApiError, ApiResponse, ValidationError};

/// Validation level for type-safe configuration
///
/// Represents different validation strictness levels with clear behavior.
/// Each level enables specific validation features appropriate for that use case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationLevel {
    /// No validation - all checks disabled (maximum performance)
    /// - Request body validation: disabled
    /// - Parameter validation: disabled
    /// - Response validation: disabled
    /// - Strict mode: disabled
    None,

    /// Basic validation - only request body (production default)
    /// - Request body validation: enabled
    /// - Parameter validation: disabled
    /// - Response validation: disabled
    /// - Strict mode: disabled
    Basic,

    /// Standard validation - request body and parameters
    /// - Request body validation: enabled
    /// - Parameter validation: enabled
    /// - Response validation: disabled
    /// - Strict mode: disabled
    Standard,

    /// Strict validation - all validations with strict mode (development/testing)
    /// - Request body validation: enabled
    /// - Parameter validation: enabled
    /// - Response validation: enabled
    /// - Strict mode: enabled
    Strict,
}

impl ValidationLevel {
    /// Check if request body validation is enabled
    pub fn validate_request_body(&self) -> bool {
        matches!(self, Self::Basic | Self::Standard | Self::Strict)
    }

    /// Check if parameter validation is enabled
    pub fn validate_parameters(&self) -> bool {
        matches!(self, Self::Standard | Self::Strict)
    }

    /// Check if response validation is enabled
    pub fn validate_response(&self) -> bool {
        matches!(self, Self::Strict)
    }

    /// Check if strict mode is enabled
    pub fn strict_mode(&self) -> bool {
        matches!(self, Self::Strict)
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::None => "No validation",
            Self::Basic => "Basic request validation",
            Self::Standard => "Request and parameter validation",
            Self::Strict => "Full validation with strict mode",
        }
    }
}

impl Default for ValidationLevel {
    fn default() -> Self {
        Self::Basic
    }
}

/// Request validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Validation level
    pub level: ValidationLevel,
    /// Maximum request body size (bytes)
    pub max_body_size: usize,
}

impl ValidationConfig {
    /// Create config with specific validation level
    pub fn with_level(level: ValidationLevel) -> Self {
        Self {
            level,
            max_body_size: 1024 * 1024, // 1MB
        }
    }

    /// Check if request body validation is enabled
    pub fn validate_request_body(&self) -> bool {
        self.level.validate_request_body()
    }

    /// Check if parameter validation is enabled
    pub fn validate_parameters(&self) -> bool {
        self.level.validate_parameters()
    }

    /// Check if response validation is enabled
    pub fn validate_response(&self) -> bool {
        self.level.validate_response()
    }

    /// Check if strict mode is enabled
    pub fn strict_mode(&self) -> bool {
        self.level.strict_mode()
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self::with_level(ValidationLevel::default())
    }
}

/// Request validator
#[derive(Debug, Clone)]
pub struct RequestValidator {
    config: ValidationConfig,
    schemas: HashMap<String, Value>,
}

impl RequestValidator {
    /// Create a new request validator
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            schemas: HashMap::new(),
        }
    }

    /// Add a schema for validation
    pub fn add_schema(&mut self, name: String, schema: Value) {
        self.schemas.insert(name, schema);
    }

    /// Validate request body against schema
    pub fn validate_body(&self, body: &Value, schema_name: &str) -> Result<(), ValidationErrors> {
        if !self.config.validate_request_body() {
            return Ok(());
        }

        let schema = self.schemas.get(schema_name).ok_or_else(|| {
            ValidationErrors::new(vec![ValidationError {
                field: "schema".to_string(),
                code: "SCHEMA_NOT_FOUND".to_string(),
                message: format!("Schema '{}' not found", schema_name),
                rejected_value: Some(Value::String(schema_name.to_string())),
            }])
        })?;

        self.validate_against_schema(body, schema, "")
    }

    /// Validate request parameters
    pub fn validate_parameters(
        &self,
        params: &HashMap<String, String>,
        schema: &Value,
    ) -> Result<(), ValidationErrors> {
        if !self.config.validate_parameters() {
            return Ok(());
        }

        let mut errors = Vec::new();

        // Convert params to JSON for validation
        let params_json: Value = params
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect::<serde_json::Map<_, _>>()
            .into();

        if let Err(validation_errors) =
            self.validate_against_schema(&params_json, schema, "parameters")
        {
            errors.extend(validation_errors.errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors::new(errors))
        }
    }

    /// Validate value against JSON schema
    fn validate_against_schema(
        &self,
        value: &Value,
        schema: &Value,
        path: &str,
    ) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();

        // Basic type validation
        if let Some(expected_type) = schema.get("type").and_then(|t| t.as_str()) {
            let actual_type = match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };

            if expected_type != actual_type {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "TYPE_MISMATCH".to_string(),
                    message: format!("Expected type '{}', got '{}'", expected_type, actual_type),
                    rejected_value: Some(value.clone()),
                });
            }
        }

        // Required fields validation for objects
        if let (Value::Object(obj), Some(Value::Array(required))) = (value, schema.get("required"))
        {
            for required_field in required {
                if let Value::String(field_name) = required_field
                    && !obj.contains_key(field_name)
                {
                    errors.push(ValidationError {
                        field: if path.is_empty() {
                            field_name.clone()
                        } else {
                            format!("{}.{}", path, field_name)
                        },
                        code: "REQUIRED_FIELD_MISSING".to_string(),
                        message: format!("Required field '{}' is missing", field_name),
                        rejected_value: None,
                    });
                }
            }
        }

        // String length validation
        if let Value::String(s) = value {
            if let Some(min_length) = schema.get("minLength").and_then(|v| v.as_u64())
                && (s.len() as u64) < min_length
            {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "STRING_TOO_SHORT".to_string(),
                    message: format!("String must be at least {} characters long", min_length),
                    rejected_value: Some(value.clone()),
                });
            }

            if let Some(max_length) = schema.get("maxLength").and_then(|v| v.as_u64())
                && (s.len() as u64) > max_length
            {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "STRING_TOO_LONG".to_string(),
                    message: format!("String must be at most {} characters long", max_length),
                    rejected_value: Some(value.clone()),
                });
            }

            // Pattern validation
            if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str())
                && let Ok(regex) = regex::Regex::new(pattern)
                && !regex.is_match(s)
            {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "PATTERN_MISMATCH".to_string(),
                    message: format!("String does not match pattern '{}'", pattern),
                    rejected_value: Some(value.clone()),
                });
            }
        }

        // Number range validation
        if let Value::Number(n) = value {
            if let Some(minimum) = schema.get("minimum").and_then(|v| v.as_f64())
                && let Some(num_val) = n.as_f64()
                && num_val < minimum
            {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "NUMBER_TOO_SMALL".to_string(),
                    message: format!("Number must be at least {}", minimum),
                    rejected_value: Some(value.clone()),
                });
            }

            if let Some(maximum) = schema.get("maximum").and_then(|v| v.as_f64())
                && let Some(num_val) = n.as_f64()
                && num_val > maximum
            {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "NUMBER_TOO_LARGE".to_string(),
                    message: format!("Number must be at most {}", maximum),
                    rejected_value: Some(value.clone()),
                });
            }
        }

        // Array validation
        if let Value::Array(arr) = value {
            if let Some(min_items) = schema.get("minItems").and_then(|v| v.as_u64())
                && (arr.len() as u64) < min_items
            {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "ARRAY_TOO_SHORT".to_string(),
                    message: format!("Array must have at least {} items", min_items),
                    rejected_value: Some(value.clone()),
                });
            }

            if let Some(max_items) = schema.get("maxItems").and_then(|v| v.as_u64())
                && (arr.len() as u64) > max_items
            {
                errors.push(ValidationError {
                    field: path.to_string(),
                    code: "ARRAY_TOO_LONG".to_string(),
                    message: format!("Array must have at most {} items", max_items),
                    rejected_value: Some(value.clone()),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors::new(errors))
        }
    }
}

/// Response validator
#[derive(Debug, Clone)]
pub struct ResponseValidator {
    config: ValidationConfig,
    schemas: HashMap<String, Value>,
}

impl ResponseValidator {
    /// Create a new response validator
    pub fn new(config: ValidationConfig) -> Self {
        Self {
            config,
            schemas: HashMap::new(),
        }
    }

    /// Add a schema for validation
    pub fn add_schema(&mut self, name: String, schema: Value) {
        self.schemas.insert(name, schema);
    }

    /// Validate response body
    pub fn validate_response(
        &self,
        body: &Value,
        schema_name: &str,
    ) -> Result<(), ValidationErrors> {
        if !self.config.validate_response() {
            return Ok(());
        }

        let schema = self.schemas.get(schema_name).ok_or_else(|| {
            ValidationErrors::new(vec![ValidationError {
                field: "schema".to_string(),
                code: "SCHEMA_NOT_FOUND".to_string(),
                message: format!("Response schema '{}' not found", schema_name),
                rejected_value: Some(Value::String(schema_name.to_string())),
            }])
        })?;

        // Use the same validation logic as request validator
        let request_validator = RequestValidator::new(self.config.clone());
        request_validator.validate_against_schema(body, schema, "response")
    }
}

/// Validation error collection
#[derive(Debug, Clone, Error)]
#[error("Validation failed with {} errors", errors.len())]
pub struct ValidationErrors {
    pub errors: Vec<ValidationError>,
}

impl ValidationErrors {
    pub fn new(errors: Vec<ValidationError>) -> Self {
        Self { errors }
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl IntoResponse for ValidationErrors {
    fn into_response(self) -> Response {
        let api_error = ApiError::UnprocessableEntity {
            message: "Request validation failed".to_string(),
            validation_errors: self.errors,
        };

        let response = ApiResponse::<()>::error(format!("{}", api_error));
        (StatusCode::UNPROCESSABLE_ENTITY, Json(response)).into_response()
    }
}

/// Validated JSON extractor
#[derive(Debug)]
pub struct ValidatedJson<T> {
    pub data: T,
}

impl<T> ValidatedJson<T> {
    /// Create a new validated JSON wrapper
    pub fn new(data: T) -> Self {
        Self { data }
    }

    /// Extract the inner data
    pub fn into_inner(self) -> T {
        self.data
    }
}

/// Validation middleware for OpenAPI compliance
///
/// This middleware validates requests against OpenAPI schemas. To use it:
///
/// 1. Create a `RequestValidator` with schemas loaded from your OpenAPI spec
/// 2. Store it in Axum state
/// 3. Apply this middleware to routes that need validation
///
/// # Example
///
/// ```rust,ignore
/// use axum::{Router, routing::post, middleware};
/// use skreaver_http::openapi::validation::{RequestValidator, ValidationConfig, validation_middleware};
/// use serde_json::json;
///
/// let mut validator = RequestValidator::new(ValidationConfig::default());
/// validator.add_schema("CreateUser".to_string(), json!({
///     "type": "object",
///     "required": ["name", "email"],
///     "properties": {
///         "name": {"type": "string", "minLength": 1},
///         "email": {"type": "string", "pattern": "^.+@.+\\..+$"}
///     }
/// }));
///
/// let app = Router::new()
///     .route("/users", post(create_user))
///     .layer(middleware::from_fn(validation_middleware))
///     .with_state(validator);
/// ```
///
/// # Note
///
/// This is a basic implementation. For production use, consider:
/// - Using a proper JSON Schema validator library (e.g., `jsonschema`)
/// - Extracting schema name from route metadata
/// - Caching validation results
/// - Adding content-type validation
pub async fn validation_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<Response, Response> {
    // Basic implementation: pass through all requests
    //
    // For full implementation, you would:
    // 1. Extract the request path and method
    // 2. Look up the corresponding OpenAPI operation
    // 3. Extract and parse the request body
    // 4. Validate against the operation's requestBody schema
    // 5. If validation fails, return 422 Unprocessable Entity
    // 6. If validation succeeds, reconstruct the request and pass to next
    //
    // This requires:
    // - Access to the OpenAPI spec (via state or extension)
    // - Buffering the request body (which consumes it)
    // - Re-creating the request with the buffered body
    //
    // Example implementation sketch:
    //
    // let (parts, body) = req.into_parts();
    // let bytes = axum::body::to_bytes(body, usize::MAX).await.map_err(|_| {
    //     (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read body").into_response()
    // })?;
    //
    // if let Ok(json_value) = serde_json::from_slice::<Value>(&bytes) {
    //     // Validate json_value against schema
    //     // If validation fails, return error response
    // }
    //
    // let new_body = axum::body::Body::from(bytes);
    // let new_req = axum::http::Request::from_parts(parts, new_body);

    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_validation_config_default() {
        let config = ValidationConfig::default();
        assert_eq!(config.level, ValidationLevel::Basic);
        assert!(config.validate_request_body());
        assert!(!config.validate_parameters());
        assert!(!config.validate_response());
        assert!(!config.strict_mode());
        assert_eq!(config.max_body_size, 1024 * 1024);
    }

    #[test]
    fn test_validation_level_none() {
        let level = ValidationLevel::None;
        assert!(!level.validate_request_body());
        assert!(!level.validate_parameters());
        assert!(!level.validate_response());
        assert!(!level.strict_mode());
        assert_eq!(level.description(), "No validation");
    }

    #[test]
    fn test_validation_level_basic() {
        let level = ValidationLevel::Basic;
        assert!(level.validate_request_body());
        assert!(!level.validate_parameters());
        assert!(!level.validate_response());
        assert!(!level.strict_mode());
        assert_eq!(level.description(), "Basic request validation");
    }

    #[test]
    fn test_validation_level_standard() {
        let level = ValidationLevel::Standard;
        assert!(level.validate_request_body());
        assert!(level.validate_parameters());
        assert!(!level.validate_response());
        assert!(!level.strict_mode());
        assert_eq!(level.description(), "Request and parameter validation");
    }

    #[test]
    fn test_validation_level_strict() {
        let level = ValidationLevel::Strict;
        assert!(level.validate_request_body());
        assert!(level.validate_parameters());
        assert!(level.validate_response());
        assert!(level.strict_mode());
        assert_eq!(level.description(), "Full validation with strict mode");
    }

    #[test]
    fn test_validation_config_with_level() {
        let config = ValidationConfig::with_level(ValidationLevel::Strict);
        assert_eq!(config.level, ValidationLevel::Strict);
        assert!(config.validate_request_body());
        assert!(config.validate_parameters());
        assert!(config.validate_response());
        assert!(config.strict_mode());
    }

    #[test]
    fn test_request_validator_type_validation() {
        let config = ValidationConfig::default();
        let validator = RequestValidator::new(config);

        let schema = json!({
            "type": "string"
        });

        // Valid string
        let valid_value = json!("hello");
        assert!(
            validator
                .validate_against_schema(&valid_value, &schema, "test")
                .is_ok()
        );

        // Invalid type
        let invalid_value = json!(42);
        let result = validator.validate_against_schema(&invalid_value, &schema, "test");
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.errors.len(), 1);
        assert_eq!(errors.errors[0].code, "TYPE_MISMATCH");
    }

    #[test]
    fn test_request_validator_required_fields() {
        let config = ValidationConfig::default();
        let validator = RequestValidator::new(config);

        let schema = json!({
            "type": "object",
            "required": ["name", "email"]
        });

        // Valid object
        let valid_value = json!({
            "name": "John",
            "email": "john@example.com"
        });
        assert!(
            validator
                .validate_against_schema(&valid_value, &schema, "")
                .is_ok()
        );

        // Missing required field
        let invalid_value = json!({
            "name": "John"
        });
        let result = validator.validate_against_schema(&invalid_value, &schema, "");
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert_eq!(errors.errors.len(), 1);
        assert_eq!(errors.errors[0].code, "REQUIRED_FIELD_MISSING");
        assert_eq!(errors.errors[0].field, "email");
    }

    #[test]
    fn test_request_validator_string_length() {
        let config = ValidationConfig::default();
        let validator = RequestValidator::new(config);

        let schema = json!({
            "type": "string",
            "minLength": 3,
            "maxLength": 10
        });

        // Valid length
        let valid_value = json!("hello");
        assert!(
            validator
                .validate_against_schema(&valid_value, &schema, "test")
                .is_ok()
        );

        // Too short
        let short_value = json!("hi");
        let result = validator.validate_against_schema(&short_value, &schema, "test");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.errors[0].code, "STRING_TOO_SHORT");

        // Too long
        let long_value = json!("this is too long");
        let result = validator.validate_against_schema(&long_value, &schema, "test");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.errors[0].code, "STRING_TOO_LONG");
    }

    #[test]
    fn test_request_validator_number_range() {
        let config = ValidationConfig::default();
        let validator = RequestValidator::new(config);

        let schema = json!({
            "type": "number",
            "minimum": 0,
            "maximum": 100
        });

        // Valid number
        let valid_value = json!(50);
        assert!(
            validator
                .validate_against_schema(&valid_value, &schema, "test")
                .is_ok()
        );

        // Too small
        let small_value = json!(-1);
        let result = validator.validate_against_schema(&small_value, &schema, "test");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.errors[0].code, "NUMBER_TOO_SMALL");

        // Too large
        let large_value = json!(101);
        let result = validator.validate_against_schema(&large_value, &schema, "test");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.errors[0].code, "NUMBER_TOO_LARGE");
    }

    #[test]
    fn test_request_validator_array_items() {
        let config = ValidationConfig::default();
        let validator = RequestValidator::new(config);

        let schema = json!({
            "type": "array",
            "minItems": 1,
            "maxItems": 3
        });

        // Valid array
        let valid_value = json!(["a", "b"]);
        assert!(
            validator
                .validate_against_schema(&valid_value, &schema, "test")
                .is_ok()
        );

        // Too few items
        let empty_value = json!([]);
        let result = validator.validate_against_schema(&empty_value, &schema, "test");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.errors[0].code, "ARRAY_TOO_SHORT");

        // Too many items
        let long_value = json!(["a", "b", "c", "d"]);
        let result = validator.validate_against_schema(&long_value, &schema, "test");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.errors[0].code, "ARRAY_TOO_LONG");
    }

    #[test]
    fn test_response_validator() {
        let config = ValidationConfig::with_level(ValidationLevel::Strict);
        let mut validator = ResponseValidator::new(config);

        let schema = json!({
            "type": "object",
            "required": ["success"]
        });
        validator.add_schema("response".to_string(), schema);

        // Valid response
        let valid_response = json!({
            "success": true,
            "data": "test"
        });
        assert!(
            validator
                .validate_response(&valid_response, "response")
                .is_ok()
        );

        // Invalid response
        let invalid_response = json!({
            "data": "test"
        });
        let result = validator.validate_response(&invalid_response, "response");
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_errors() {
        let errors = vec![ValidationError {
            field: "test".to_string(),
            code: "TEST_ERROR".to_string(),
            message: "Test error".to_string(),
            rejected_value: None,
        }];

        let validation_errors = ValidationErrors::new(errors);
        assert_eq!(validation_errors.errors.len(), 1);
        assert!(!validation_errors.is_empty());

        // Test error display
        let error_msg = format!("{}", validation_errors);
        assert!(error_msg.contains("Validation failed with 1 errors"));
    }
}
