# Error Handling Style Guide

This guide establishes consistent error handling patterns across the Skreaver codebase.

## Table of Contents

1. [Overview](#overview)
2. [Error Pattern Selection](#error-pattern-selection)
3. [Flat Variants Pattern](#flat-variants-pattern)
4. [Nested Enums Pattern](#nested-enums-pattern)
5. [Builder Methods](#builder-methods)
6. [Best Practices](#best-practices)
7. [Examples](#examples)

## Overview

Skreaver uses **three complementary error patterns**, each suitable for different use cases:

1. **Flat Variants**: For domain-specific errors with rich context
2. **Nested Enums**: For top-level aggregation across domains
3. **Builder Methods**: For ergonomic error construction

## Error Pattern Selection

### When to Use Flat Variants ✅

Use flat variants for **domain-specific error types** that need rich contextual information:

```rust
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("Agent not found: {agent_id}")]
    AgentNotFound {
        agent_id: String,
        request_id: RequestId,
    },

    #[error("Agent creation failed: {reason}")]
    AgentCreationFailed {
        reason: String,
        agent_type: Option<String>,
        request_id: RequestId,
    },
}
```

**Benefits:**
- Rich contextual information directly in variant
- Better error messages with interpolation
- Easy to pattern match on specific errors
- All context available without unwrapping

**Use for:**
- HTTP runtime errors (`RuntimeError`)
- Tool operation errors (`ToolError`)
- Memory operation errors (`MemoryError`)
- WebSocket errors (`WsError`)

### When to Use Nested Enums ✅

Use nested enums for **top-level aggregation** across different error domains:

```rust
#[derive(Debug, Clone)]
pub enum SkreverError {
    Tool(ToolError),
    Memory(MemoryError),
    Agent(AgentError),
    Coordinator(CoordinatorError),
}
```

**Benefits:**
- Clean separation of concerns
- Domain errors remain independent
- Easy to convert from domain errors (`impl From<ToolError>`)
- Supports error propagation with `?` operator

**Use for:**
- Top-level application errors
- Library public APIs
- Cross-domain error aggregation

**Don't use for:**
- Domain-specific errors (use flat variants instead)
- Errors that need rich context (nested reduces visibility)

### When to Use Builder Methods ✅

Add builder methods to **frequently constructed errors** or errors with **complex initialization**:

```rust
impl ToolError {
    /// Create a NotFound error (builder method)
    pub fn not_found(tool: ToolDispatch) -> Self {
        Self::NotFound { tool }
    }

    /// Create an ExecutionFailed error with formatted message
    pub fn execution_failed(tool: ToolDispatch, message: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            tool,
            message: message.into(),
        }
    }

    /// Create a Timeout error
    pub fn timeout(tool: ToolDispatch, duration: Duration) -> Self {
        Self::Timeout {
            tool,
            duration_ms: duration.as_millis() as u64,
        }
    }
}
```

**Benefits:**
- More ergonomic API
- Type conversions handled internally
- Consistent error construction
- Better discoverability in IDE

**Use for:**
- Errors constructed in multiple places
- Errors requiring type conversions
- Complex initialization logic

## Flat Variants Pattern

### Structure

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Entity not found: {entity_id}")]
    NotFound {
        entity_id: String,
        context: String,
    },

    #[error("Operation {operation} failed: {reason}")]
    OperationFailed {
        operation: String,
        reason: String,
        retry_count: usize,
    },
}
```

### Key Requirements

1. **Use `thiserror::Error`** for automatic `Display` implementation
2. **Include error context** in variant fields
3. **Add `#[error]` attributes** with formatted messages
4. **Use meaningful variant names** (e.g., `AgentNotFound` not `Error1`)
5. **Include identifiers** for tracing (e.g., `request_id`, `agent_id`)

### Example: RuntimeError

✅ **Good** - Flat variants with rich context:

```rust
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("Agent not found: {agent_id}")]
    AgentNotFound {
        agent_id: String,
        request_id: RequestId,
    },

    #[error("Rate limit exceeded: {limit_type}")]
    RateLimitExceeded {
        limit_type: String,
        retry_after: u64,
        current_usage: u32,
        limit: u32,
        request_id: RequestId,
    },
}

impl RuntimeError {
    pub fn request_id(&self) -> &RequestId {
        match self {
            RuntimeError::AgentNotFound { request_id, .. } => request_id,
            RuntimeError::RateLimitExceeded { request_id, .. } => request_id,
        }
    }
}
```

❌ **Avoid** - Nested enums for domain errors:

```rust
// DON'T DO THIS for domain-specific errors
pub enum RuntimeError {
    Agent(AgentError),  // Loses context, harder to match
    RateLimit(RateLimitError),
}
```

## Nested Enums Pattern

### Structure

```rust
#[derive(Debug, Clone)]
pub enum TopLevelError {
    Domain1(Domain1Error),
    Domain2(Domain2Error),
    Domain3(Domain3Error),
}

impl From<Domain1Error> for TopLevelError {
    fn from(err: Domain1Error) -> Self {
        TopLevelError::Domain1(err)
    }
}
```

### Key Requirements

1. **Only for top-level aggregation** across independent domains
2. **Implement `From` traits** for each domain error
3. **Keep domain errors flat** (don't nest inside nested)
4. **Provide helper methods** to access common data

### Example: SkreverError

✅ **Good** - Top-level aggregation:

```rust
#[derive(Debug, Clone)]
pub enum SkreverError {
    Tool(ToolError),
    Memory(MemoryError),
    Agent(AgentError),
}

// Each domain error is flat internally
#[derive(Debug, Clone)]
pub enum ToolError {
    NotFound { tool: ToolDispatch },
    ExecutionFailed { tool: ToolDispatch, message: String },
}
```

## Builder Methods

### When to Add Builders

Add builder methods when:
- Error is constructed in 3+ places
- Construction involves type conversions
- Simplifies API usage

### Builder Method Patterns

```rust
impl DomainError {
    // Simple builder - just wraps construction
    pub fn not_found(id: impl Into<String>) -> Self {
        Self::NotFound {
            entity_id: id.into(),
            context: String::new(),
        }
    }

    // Builder with context
    pub fn not_found_with_context(
        id: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        Self::NotFound {
            entity_id: id.into(),
            context: context.into(),
        }
    }

    // Builder with conversions
    pub fn timeout(operation: &str, duration: Duration) -> Self {
        Self::Timeout {
            operation: operation.to_string(),
            duration_ms: duration.as_millis() as u64,
        }
    }
}
```

### Usage Example

```rust
// Without builder - verbose
return Err(ToolError::NotFound {
    tool: ToolDispatch::Custom(tool_id),
});

// With builder - concise
return Err(ToolError::not_found(tool_dispatch));
```

## Best Practices

### 1. Include Request/Trace IDs

Always include tracing identifiers for debugging:

```rust
#[error("Operation failed")]
OperationFailed {
    operation: String,
    request_id: RequestId,  // ✅ Include for tracing
}
```

### 2. Provide Contextual Information

Include enough context to diagnose the error:

```rust
// ❌ Bad - no context
#[error("Failed")]
Failed,

// ✅ Good - rich context
#[error("Agent {agent_id} failed to execute {operation}: {reason}")]
OperationFailed {
    agent_id: String,
    operation: String,
    reason: String,
}
```

### 3. Use Semantic Variant Names

```rust
// ❌ Bad - generic names
Error1, Error2, GenericError

// ✅ Good - semantic names
AgentNotFound, RateLimitExceeded, InvalidConfiguration
```

### 4. Implement Helper Methods

Provide methods to access common data:

```rust
impl RuntimeError {
    pub fn request_id(&self) -> &RequestId {
        match self { /* ... */ }
    }

    pub fn status_code(&self) -> StatusCode {
        match self { /* ... */ }
    }
}
```

### 5. Use `thiserror` for Display

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MyError {
    #[error("Description: {field}")]
    Variant { field: String },
}
```

### 6. Document Error Conditions

```rust
/// Errors that can occur during agent operations
#[derive(Debug, Error)]
pub enum AgentError {
    /// Agent was not found in the registry
    #[error("Agent not found: {agent_id}")]
    NotFound { agent_id: String },

    /// Agent initialization failed due to invalid configuration
    #[error("Agent initialization failed: {reason}")]
    InitializationFailed { reason: String },
}
```

## Examples

### Complete Error Type Example

```rust
use thiserror::Error;
use skreaver_core::RequestId;
use std::time::Duration;

/// Errors that can occur during tool operations
#[derive(Debug, Clone, Error)]
pub enum ToolError {
    /// Tool was not found in the registry
    #[error("Tool '{tool_name}' not found")]
    NotFound { tool_name: String },

    /// Tool execution failed
    #[error("Tool '{tool_name}' execution failed: {message}")]
    ExecutionFailed {
        tool_name: String,
        message: String,
    },

    /// Tool timed out during execution
    #[error("Tool '{tool_name}' timed out after {duration_ms}ms")]
    Timeout {
        tool_name: String,
        duration_ms: u64,
    },
}

impl ToolError {
    // Builder methods
    pub fn not_found(tool_name: impl Into<String>) -> Self {
        Self::NotFound {
            tool_name: tool_name.into(),
        }
    }

    pub fn execution_failed(
        tool_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::ExecutionFailed {
            tool_name: tool_name.into(),
            message: message.into(),
        }
    }

    pub fn timeout(tool_name: impl Into<String>, duration: Duration) -> Self {
        Self::Timeout {
            tool_name: tool_name.into(),
            duration_ms: duration.as_millis() as u64,
        }
    }

    // Helper methods
    pub fn tool_name(&self) -> &str {
        match self {
            ToolError::NotFound { tool_name } => tool_name,
            ToolError::ExecutionFailed { tool_name, .. } => tool_name,
            ToolError::Timeout { tool_name, .. } => tool_name,
        }
    }
}
```

### Top-Level Aggregation Example

```rust
/// Top-level error for the Skreaver framework
#[derive(Debug, Clone)]
pub enum SkreverError {
    Tool(ToolError),
    Memory(MemoryError),
    Agent(AgentError),
}

// Enable ? operator for each domain error
impl From<ToolError> for SkreverError {
    fn from(err: ToolError) -> Self {
        SkreverError::Tool(err)
    }
}

impl From<MemoryError> for SkreverError {
    fn from(err: MemoryError) -> Self {
        SkreverError::Memory(err)
    }
}

impl std::fmt::Display for SkreverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkreverError::Tool(e) => write!(f, "Tool error: {}", e),
            SkreverError::Memory(e) => write!(f, "Memory error: {}", e),
            SkreverError::Agent(e) => write!(f, "Agent error: {}", e),
        }
    }
}
```

## Summary

### Decision Tree

```
Do you need to aggregate errors from multiple domains?
├─ Yes → Use **Nested Enums** (SkreverError pattern)
└─ No
   └─ Is this a domain-specific error?
      ├─ Yes → Use **Flat Variants** (RuntimeError pattern)
      └─ No → Reconsider your error boundaries

Is the error constructed in multiple places?
├─ Yes → Add **Builder Methods**
└─ No → Direct construction is fine
```

### Quick Reference

| Pattern | Use Case | Example |
|---------|----------|---------|
| **Flat Variants** | Domain errors with context | `RuntimeError`, `ToolError` |
| **Nested Enums** | Top-level aggregation | `SkreverError` |
| **Builder Methods** | Ergonomic construction | `ToolError::not_found()` |

### Anti-Patterns to Avoid

❌ Nesting domain errors inside domain errors
❌ Generic error names (`Error1`, `GenericError`)
❌ Missing context in error variants
❌ Not including trace IDs for debugging
❌ Using nested enums for domain-specific errors

---

**Last Updated**: 2025-01-09
**Version**: 1.0.0
