//! OpenAPI specification generator

use super::OpenApiConfig;
use serde_json::Value;
use std::collections::HashMap;
use utoipa::ToSchema;

/// OpenAPI specification generator
pub struct OpenApiGenerator {
    config: OpenApiConfig,
    custom_schemas: HashMap<String, Value>,
    security_schemes: HashMap<String, Value>,
}

impl OpenApiGenerator {
    /// Create a new OpenAPI generator
    pub fn new(config: OpenApiConfig) -> Self {
        Self {
            config,
            custom_schemas: HashMap::new(),
            security_schemes: HashMap::new(),
        }
    }
    
    /// Add a custom schema to the specification
    pub fn add_schema<T: ToSchema>(&mut self, name: &str) -> &mut Self {
        // This would use utoipa's schema generation
        // For now, we'll add a placeholder
        self.custom_schemas.insert(
            name.to_string(),
            serde_json::json!({
                "type": "object",
                "description": format!("Schema for {}", name)
            })
        );
        self
    }
    
    /// Add API key security scheme
    pub fn add_api_key_security(&mut self, name: &str, header_name: &str) -> &mut Self {
        self.security_schemes.insert(
            name.to_string(),
            serde_json::json!({
                "type": "apiKey",
                "in": "header",
                "name": header_name,
                "description": "API key for authentication"
            })
        );
        self
    }
    
    /// Add Bearer token security scheme
    pub fn add_bearer_security(&mut self, name: &str) -> &mut Self {
        self.security_schemes.insert(
            name.to_string(),
            serde_json::json!({
                "type": "http",
                "scheme": "bearer",
                "bearerFormat": "JWT",
                "description": "JWT Bearer token"
            })
        );
        self
    }
    
    /// Generate the complete OpenAPI specification
    pub fn generate(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let mut spec = serde_json::json!({
            "openapi": "3.0.3",
            "info": {
                "title": self.config.title,
                "description": self.config.description,
                "version": self.config.version
            },
            "servers": self.config.servers,
            "paths": {},
            "components": {
                "schemas": self.generate_schemas(),
                "securitySchemes": self.security_schemes,
                "responses": self.generate_common_responses()
            }
        });
        
        // Add optional fields
        if let Some(contact) = &self.config.contact {
            spec["info"]["contact"] = serde_json::to_value(contact)?;
        }
        
        if let Some(license) = &self.config.license {
            spec["info"]["license"] = serde_json::to_value(license)?;
        }
        
        if let Some(terms) = &self.config.terms_of_service {
            spec["info"]["termsOfService"] = Value::String(terms.clone());
        }
        
        if let Some(external_docs) = &self.config.external_docs {
            spec["externalDocs"] = serde_json::to_value(external_docs)?;
        }
        
        Ok(spec)
    }
    
    /// Generate common schemas
    fn generate_schemas(&self) -> Value {
        let mut schemas = serde_json::json!({
            "ApiResponse": {
                "type": "object",
                "properties": {
                    "data": {
                        "oneOf": [
                            { "type": "object" },
                            { "type": "array" },
                            { "type": "string" },
                            { "type": "number" },
                            { "type": "boolean" },
                            { "type": "null" }
                        ],
                        "description": "Response data"
                    },
                    "success": {
                        "type": "boolean",
                        "description": "Success indicator"
                    },
                    "error": {
                        "type": "string",
                        "nullable": true,
                        "description": "Error message if any"
                    },
                    "timestamp": {
                        "type": "string",
                        "format": "date-time",
                        "description": "Request timestamp"
                    },
                    "requestId": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Request ID for tracing"
                    }
                },
                "required": ["success", "timestamp", "requestId"]
            },
            "PaginationInfo": {
                "type": "object",
                "properties": {
                    "page": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Current page number"
                    },
                    "pageSize": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 100,
                        "description": "Number of items per page"
                    },
                    "totalItems": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Total number of items"
                    },
                    "totalPages": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Total number of pages"
                    },
                    "hasNext": {
                        "type": "boolean",
                        "description": "Whether there are more pages"
                    },
                    "hasPrev": {
                        "type": "boolean",
                        "description": "Whether there are previous pages"
                    }
                },
                "required": ["page", "pageSize", "totalItems", "totalPages", "hasNext", "hasPrev"]
            },
            "PaginatedResponse": {
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "description": "Response items"
                    },
                    "pagination": {
                        "$ref": "#/components/schemas/PaginationInfo"
                    }
                },
                "allOf": [
                    { "$ref": "#/components/schemas/ApiResponse" }
                ],
                "required": ["items", "pagination"]
            },
            "ValidationError": {
                "type": "object",
                "properties": {
                    "field": {
                        "type": "string",
                        "description": "Field name"
                    },
                    "code": {
                        "type": "string",
                        "description": "Error code"
                    },
                    "message": {
                        "type": "string",
                        "description": "Human-readable message"
                    },
                    "rejectedValue": {
                        "oneOf": [
                            { "type": "object" },
                            { "type": "array" },
                            { "type": "string" },
                            { "type": "number" },
                            { "type": "boolean" },
                            { "type": "null" }
                        ],
                        "description": "Rejected value"
                    }
                },
                "required": ["field", "code", "message"]
            },
            "ApiError": {
                "type": "object",
                "discriminator": {
                    "propertyName": "type"
                },
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["BAD_REQUEST"] },
                            "message": { "type": "string" },
                            "details": { "type": "object", "nullable": true }
                        },
                        "required": ["type", "message"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["UNAUTHORIZED"] },
                            "message": { "type": "string" }
                        },
                        "required": ["type", "message"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["FORBIDDEN"] },
                            "message": { "type": "string" },
                            "requiredPermission": { "type": "string", "nullable": true }
                        },
                        "required": ["type", "message"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["NOT_FOUND"] },
                            "message": { "type": "string" },
                            "resourceType": { "type": "string", "nullable": true },
                            "resourceId": { "type": "string", "nullable": true }
                        },
                        "required": ["type", "message"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["UNPROCESSABLE_ENTITY"] },
                            "message": { "type": "string" },
                            "validationErrors": {
                                "type": "array",
                                "items": { "$ref": "#/components/schemas/ValidationError" }
                            }
                        },
                        "required": ["type", "message", "validationErrors"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "enum": ["RATE_LIMIT_EXCEEDED"] },
                            "message": { "type": "string" },
                            "retryAfter": { "type": "integer", "nullable": true }
                        },
                        "required": ["type", "message"]
                    },
                    {
                        "type": "object", 
                        "properties": {
                            "type": { "type": "string", "enum": ["INTERNAL_SERVER_ERROR"] },
                            "message": { "type": "string" },
                            "errorId": { "type": "string" }
                        },
                        "required": ["type", "message", "errorId"]
                    }
                ]
            }
        });
        
        // Add custom schemas
        if let Some(schemas_obj) = schemas.as_object_mut() {
            for (name, schema) in &self.custom_schemas {
                schemas_obj.insert(name.clone(), schema.clone());
            }
        }
        
        schemas
    }
    
    /// Generate common response definitions
    fn generate_common_responses(&self) -> Value {
        serde_json::json!({
            "BadRequest": {
                "description": "Bad Request",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiResponse" },
                        "examples": {
                            "badRequest": {
                                "summary": "Invalid request parameters",
                                "value": {
                                    "data": null,
                                    "success": false,
                                    "error": "Invalid request parameters",
                                    "timestamp": "2024-01-01T00:00:00Z",
                                    "requestId": "12345678-1234-1234-1234-123456789012"
                                }
                            }
                        }
                    }
                }
            },
            "Unauthorized": {
                "description": "Unauthorized",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiResponse" },
                        "examples": {
                            "unauthorized": {
                                "summary": "Missing or invalid authentication",
                                "value": {
                                    "data": null,
                                    "success": false,
                                    "error": "Authentication required",
                                    "timestamp": "2024-01-01T00:00:00Z",
                                    "requestId": "12345678-1234-1234-1234-123456789012"
                                }
                            }
                        }
                    }
                }
            },
            "Forbidden": {
                "description": "Forbidden",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiResponse" }
                    }
                }
            },
            "NotFound": {
                "description": "Not Found",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiResponse" }
                    }
                }
            },
            "UnprocessableEntity": {
                "description": "Unprocessable Entity",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiResponse" }
                    }
                }
            },
            "RateLimitExceeded": {
                "description": "Rate Limit Exceeded",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiResponse" }
                    }
                },
                "headers": {
                    "Retry-After": {
                        "description": "Number of seconds to wait before retrying",
                        "schema": {
                            "type": "integer"
                        }
                    }
                }
            },
            "InternalServerError": {
                "description": "Internal Server Error",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiResponse" }
                    }
                }
            }
        })
    }
}

/// API documentation generator with automatic endpoint discovery
pub struct ApiDocGenerator {
    generator: OpenApiGenerator,
    endpoints: Vec<EndpointInfo>,
}

/// Endpoint information for documentation
#[derive(Debug, Clone)]
pub struct EndpointInfo {
    pub path: String,
    pub method: String,
    pub summary: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub security: Vec<String>,
    pub parameters: Vec<ParameterInfo>,
    pub request_body: Option<RequestBodyInfo>,
    pub responses: HashMap<u16, ResponseInfo>,
}

/// Parameter information
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: String,
    pub location: ParameterLocation,
    pub description: Option<String>,
    pub required: bool,
    pub schema: Value,
}

/// Parameter location
#[derive(Debug, Clone)]
pub enum ParameterLocation {
    Query,
    Path,
    Header,
    Cookie,
}

/// Request body information
#[derive(Debug, Clone)]
pub struct RequestBodyInfo {
    pub description: Option<String>,
    pub required: bool,
    pub content: HashMap<String, Value>,
}

/// Response information
#[derive(Debug, Clone)]
pub struct ResponseInfo {
    pub description: String,
    pub content: HashMap<String, Value>,
    pub headers: HashMap<String, Value>,
}

impl ApiDocGenerator {
    /// Create a new API documentation generator
    pub fn new(config: OpenApiConfig) -> Self {
        Self {
            generator: OpenApiGenerator::new(config),
            endpoints: Vec::new(),
        }
    }
    
    /// Add an endpoint to the documentation
    pub fn add_endpoint(&mut self, endpoint: EndpointInfo) -> &mut Self {
        self.endpoints.push(endpoint);
        self
    }
    
    /// Generate the complete API documentation
    pub fn generate(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let mut spec = self.generator.generate()?;
        
        // Add paths from endpoints
        let mut paths = serde_json::Map::new();
        
        for endpoint in &self.endpoints {
            let path_item = paths.entry(&endpoint.path)
                .or_insert_with(|| serde_json::json!({}));
            
            let operation = self.generate_operation(endpoint)?;
            path_item[endpoint.method.to_lowercase()] = operation;
        }
        
        spec["paths"] = Value::Object(paths);
        
        Ok(spec)
    }
    
    /// Generate operation documentation for an endpoint
    fn generate_operation(&self, endpoint: &EndpointInfo) -> Result<Value, Box<dyn std::error::Error>> {
        let mut operation = serde_json::json!({
            "summary": endpoint.summary,
            "tags": endpoint.tags,
            "responses": {}
        });
        
        if let Some(description) = &endpoint.description {
            operation["description"] = Value::String(description.clone());
        }
        
        // Add parameters
        if !endpoint.parameters.is_empty() {
            let mut params = Vec::new();
            for param in &endpoint.parameters {
                params.push(serde_json::json!({
                    "name": param.name,
                    "in": match param.location {
                        ParameterLocation::Query => "query",
                        ParameterLocation::Path => "path", 
                        ParameterLocation::Header => "header",
                        ParameterLocation::Cookie => "cookie",
                    },
                    "required": param.required,
                    "schema": param.schema,
                    "description": param.description
                }));
            }
            operation["parameters"] = Value::Array(params);
        }
        
        // Add request body
        if let Some(body) = &endpoint.request_body {
            operation["requestBody"] = serde_json::json!({
                "required": body.required,
                "content": body.content,
                "description": body.description
            });
        }
        
        // Add responses
        let mut responses = serde_json::Map::new();
        for (status, response) in &endpoint.responses {
            responses.insert(
                status.to_string(),
                serde_json::json!({
                    "description": response.description,
                    "content": response.content,
                    "headers": response.headers
                })
            );
        }
        operation["responses"] = Value::Object(responses);
        
        // Add security
        if !endpoint.security.is_empty() {
            let security_reqs: Vec<Value> = endpoint.security.iter()
                .map(|scheme| serde_json::json!({ scheme: [] }))
                .collect();
            operation["security"] = Value::Array(security_reqs);
        }
        
        Ok(operation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_openapi_generator_new() {
        let config = OpenApiConfig::default();
        let generator = OpenApiGenerator::new(config);
        assert!(generator.custom_schemas.is_empty());
        assert!(generator.security_schemes.is_empty());
    }
    
    #[test]
    fn test_add_api_key_security() {
        let config = OpenApiConfig::default();
        let mut generator = OpenApiGenerator::new(config);
        
        generator.add_api_key_security("ApiKey", "X-API-Key");
        assert!(generator.security_schemes.contains_key("ApiKey"));
    }
    
    #[test]
    fn test_add_bearer_security() {
        let config = OpenApiConfig::default();
        let mut generator = OpenApiGenerator::new(config);
        
        generator.add_bearer_security("BearerAuth");
        assert!(generator.security_schemes.contains_key("BearerAuth"));
    }
    
    #[test]
    fn test_generate_basic_spec() {
        let config = OpenApiConfig::default();
        let generator = OpenApiGenerator::new(config);
        
        let spec = generator.generate().unwrap();
        assert_eq!(spec["openapi"], "3.0.3");
        assert_eq!(spec["info"]["title"], "Skreaver API");
        assert_eq!(spec["info"]["version"], "0.3.0");
    }
}