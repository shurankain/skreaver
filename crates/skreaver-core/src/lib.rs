//! # Skreaver Core
//!
//! Core traits and types for the Skreaver agent framework.
//! This crate provides the fundamental building blocks for creating AI agents.

pub mod agent;
pub mod auth;
pub mod collections;
pub mod database;
pub mod error;
pub mod identifiers;
pub mod in_memory;
pub mod memory;
pub mod metadata;
pub mod sanitization;
pub mod security;
pub mod structured_tool_result;
pub mod tool;
pub mod validation;

pub use agent::Agent;
pub use database::{
    DatabaseName, HostAddress, PoolSize,
    health::{HealthCheck, HealthReport, HealthStatus, PerformanceMetrics, PoolStatistics},
};
pub use error::{SkreverError, SkreverResult};
pub use in_memory::InMemoryMemory;
pub use memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};
pub use metadata::{Metadata, MetadataBuilder, MetadataError, MetadataKey, MetadataValue};
pub use sanitization::{
    ContentSanitizer, DatabaseErrorSanitizer, SanitizeError, SanitizeIdentifier, SecretRedactor,
};
pub use security::{
    DomainValidator, InputValidator, PathValidator, ResourceLimits, ResourceTracker, SecretBytes,
    SecretString, SecretValue, SecureFileSystem, SecurityConfig, SecurityContext, SecurityError,
    SecurityManager, SecurityPolicy, ValidatedPath, ValidatedUrl,
};
pub use structured_tool_result::{StructuredToolResult, ToolExecutionMetadata, ToolResultBuilder};
pub use tool::{
    ExecutionResult, FailureReason, StandardTool, StructuredTool, StructuredToolAdapter, Tool,
    ToolCall, ToolDispatch, ToolInput,
};

// Re-export collections types
pub use collections::{
    NonEmptyQueue, NonEmptyVec, non_empty_queue::EmptyQueueError, non_empty_vec::EmptyVecError,
};

// Re-export identifier types
pub use identifiers::{AgentId, PrincipalId, RequestId, SessionId, ToolId};

// Re-export validation types
pub use validation::ValidationError;

// LOW-4: Deprecated type - will be REMOVED in v0.6.0
// Use `validation::ValidationError` instead
#[allow(deprecated)]
#[deprecated(
    since = "0.5.0",
    note = "Use `validation::ValidationError` instead. This type will be REMOVED in 0.6.0."
)]
pub use identifiers::IdValidationError;

// Re-export auth types
pub use auth::{
    AuthContext, AuthError, AuthManager, AuthMethod, AuthResult, Principal,
    api_key::{ApiKey, ApiKeyConfig, ApiKeyManager},
    jwt::{JwtClaims, JwtConfig, JwtManager, JwtToken},
    middleware::{AuthMiddleware, AuthenticatedRequest, AuthenticationPolicy},
    rbac::{Permission, Role, RoleManager, ToolPolicy},
    storage::{CredentialStorage, InMemoryStorage, SecureStorage},
};

// Re-export agent extensions
pub use agent::{
    CompleteState, InitialState, ProcessingState, SimpleComplete, SimpleInitial, SimpleProcessing,
    SimpleStatefulAgent, SimpleToolExecution, StatefulAgent, StatefulAgentAdapter,
    ToolExecutionState,
};
