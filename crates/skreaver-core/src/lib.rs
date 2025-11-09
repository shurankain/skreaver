//! # Skreaver Core
//!
//! Core traits and types for the Skreaver agent framework.
//! This crate provides the fundamental building blocks for creating AI agents.

pub mod agent;
pub mod auth;
pub mod collections;
pub mod error;
pub mod identifiers;
pub mod in_memory;
pub mod memory;
pub mod metadata;
pub mod sanitization;
pub mod security;
pub mod tool;
pub mod validation;

pub use agent::Agent;
pub use error::{SkreverError, SkreverResult};
pub use in_memory::InMemoryMemory;
pub use memory::{
    MemoryKey, MemoryReader, MemoryUpdate, MemoryWriter, SnapshotableMemory, TransactionalMemory,
};
pub use metadata::{Metadata, MetadataBuilder, MetadataKey, MetadataValue};
pub use sanitization::{
    ContentSanitizer, DatabaseErrorSanitizer, SanitizeError, SanitizeIdentifier, SecretRedactor,
};
pub use security::{
    DomainValidator, InputValidator, PathValidator, ResourceLimits, ResourceTracker,
    SecureFileSystem, SecurityConfig, SecurityContext, SecurityError, SecurityManager,
    SecurityPolicy, ValidatedPath,
};
pub use tool::{ExecutionResult, StandardTool, Tool, ToolCall, ToolDispatch};

// Re-export collections types
pub use collections::{
    NonEmptyQueue, NonEmptyVec, non_empty_queue::EmptyQueueError, non_empty_vec::EmptyVecError,
};

// Re-export identifier types
pub use identifiers::{AgentId, IdValidationError, PrincipalId, RequestId, SessionId, ToolId};

// Re-export auth types
pub use auth::{
    AuthContext, AuthError, AuthManager, AuthMethod, AuthResult, Principal,
    api_key::{ApiKey, ApiKeyConfig, ApiKeyManager},
    jwt::{JwtClaims, JwtConfig, JwtManager, JwtToken},
    middleware::{AuthMiddleware, AuthenticatedRequest},
    rbac::{Permission, Role, RoleManager, ToolPolicy},
    storage::{CredentialStorage, InMemoryStorage, SecureStorage},
};

// Re-export agent extensions
pub use agent::{
    CompleteState, InitialState, ProcessingState, SimpleComplete, SimpleInitial, SimpleProcessing,
    SimpleStatefulAgent, SimpleToolExecution, StatefulAgent, StatefulAgentAdapter,
    ToolExecutionState,
};
