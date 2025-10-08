# Skreaver API Stability Guarantee

> **Version**: 1.0
> **Effective Date**: 2025-10-08
> **Current Release**: v0.3.0
> **Status**: Pre-1.0 Development

---

## Table of Contents

- [Stability Promise](#stability-promise)
- [Stable Public API](#stable-public-api)
- [Unstable APIs](#unstable-apis)
- [Versioning Policy](#versioning-policy)
- [Breaking Changes](#breaking-changes)
- [Deprecation Policy](#deprecation-policy)
- [Feature Flags](#feature-flags)

---

## Stability Promise

### What We Guarantee

**For versions 0.x.x (Pre-1.0)**:
- **Patch versions** (0.3.x): No breaking changes, only bug fixes and performance improvements
- **Minor versions** (0.x.0): May include breaking changes, but will be documented with migration guides
- **Features marked `unstable-*`**: May break at any time, even in patch releases

**For versions 1.x.x and beyond**:
- Strict adherence to [Semantic Versioning 2.0.0](https://semver.org/)
- Breaking changes only in major versions
- Deprecation warnings provided at least one minor version before removal
- Comprehensive migration guides for all breaking changes

### What You Can Rely On

✅ **Stable APIs** (exported from `skreaver` meta-crate):
- Core traits and types will remain backwards compatible
- Public API surface will be clearly documented
- Breaking changes will follow deprecation policy

⚠️ **Internal APIs** (direct crate imports like `skreaver_core::*`):
- May change without notice
- Use at your own risk
- **Recommendation**: Always import from `skreaver`, not individual crates

❌ **Unstable Features** (prefixed with `unstable-`):
- Explicitly marked as experimental
- May change or be removed without warning
- Not recommended for production use

---

## Stable Public API

### How to Use Stable APIs

**✅ Correct - Import from meta-crate:**
```rust
use skreaver::{Agent, Memory, Tool};
use skreaver::InMemoryMemory;
use skreaver::runtime::Coordinator;
```

**❌ Incorrect - Direct crate imports:**
```rust
use skreaver_core::Agent;  // Unstable path
use skreaver_memory::InMemoryMemory;  // Unstable path
```

### Stable Exports

The following items are **stable** when imported from the `skreaver` crate:

#### Core Traits

```rust
// Agent system
pub use skreaver::Agent;
pub use skreaver::MemoryReader;
pub use skreaver::MemoryWriter;
pub use skreaver::MemoryUpdate;
pub use skreaver::MemoryKey;
pub use skreaver::Tool;
pub use skreaver::ToolCall;
pub use skreaver::ExecutionResult;

// Transactions and snapshots
pub use skreaver::TransactionalMemory;
pub use skreaver::SnapshotableMemory;
```

#### Memory Backends

```rust
// Always available
pub use skreaver::InMemoryMemory;
pub use skreaver::FileMemory;
pub use skreaver::NamespacedMemory;

// Feature-gated (stable when feature is enabled)
#[cfg(feature = "redis")]
pub use skreaver::RedisMemory;

#[cfg(feature = "sqlite")]
pub use skreaver::SqliteMemory;

#[cfg(feature = "postgres")]
pub use skreaver::PostgresMemory;
```

#### Tool System

```rust
pub use skreaver::StandardTool;
pub use skreaver::ToolDispatch;
pub use skreaver::InMemoryToolRegistry;

// Standard tools (feature-gated)
#[cfg(feature = "network")]
pub use skreaver::tools::HttpGetTool;

#[cfg(feature = "data")]
pub use skreaver::tools::JsonParseTool;
```

#### Security & Validation

```rust
pub use skreaver::SecurityConfig;
pub use skreaver::SecurityPolicy;
pub use skreaver::SecurityManager;
pub use skreaver::InputValidator;
pub use skreaver::PathValidator;
pub use skreaver::DomainValidator;
pub use skreaver::ResourceLimits;
```

#### Authentication (Feature: `auth`)

```rust
#[cfg(feature = "auth")]
pub use skreaver::AuthManager;
pub use skreaver::ApiKeyManager;
pub use skreaver::JwtManager;
pub use skreaver::RoleManager;
pub use skreaver::Permission;
pub use skreaver::Role;
```

#### Error Types

```rust
pub use skreaver::SkreverError;
pub use skreaver::SkreverResult;
pub use skreaver::SecurityError;
pub use skreaver::AuthError;
```

#### Type Safety Collections

```rust
pub use skreaver::NonEmptyVec;
pub use skreaver::NonEmptyQueue;
```

---

## Unstable APIs

### Internal Crate Paths

**Status**: ❌ **UNSTABLE** - May change without notice

Direct imports from sub-crates are considered unstable:

```rust
// ❌ These may break at any time
use skreaver_core::agent::Agent;
use skreaver_memory::file_memory::FileMemory;
use skreaver_tools::registry::ToolRegistry;
use skreaver_http::runtime::HttpRuntime;
```

**Migration Path**: Use the meta-crate re-exports instead:
```rust
// ✅ Stable
use skreaver::Agent;
use skreaver::FileMemory;
use skreaver::InMemoryToolRegistry;
use skreaver::http::HttpRuntime;
```

### Experimental Features

**Status**: ❌ **UNSTABLE** - Explicitly marked as experimental

Features prefixed with `unstable-` are experimental and may:
- Change API in minor or patch releases
- Be removed without prior deprecation
- Have incomplete documentation or testing

#### Current Unstable Features:

```toml
[features]
unstable-websocket = ["streaming", "skreaver-http/unstable-websocket"]
```

**Usage Warning**:
```rust
// ❌ May break in minor releases
#[cfg(feature = "unstable-websocket")]
use skreaver::http::WebSocketManager;
```

#### Stabilization Path:

Unstable features will be stabilized when:
1. API design is finalized
2. Comprehensive tests are in place
3. Documentation is complete
4. At least one minor version has passed with no breaking changes

---

## Versioning Policy

### Pre-1.0 (Current)

**Format**: `0.MINOR.PATCH`

- **0.MINOR.0**: May include breaking changes
  - All breaking changes documented in CHANGELOG.md
  - Migration guide provided
  - Notice given in previous minor release when possible

- **0.MINOR.PATCH**: No breaking changes
  - Bug fixes only
  - Performance improvements
  - Documentation updates
  - Internal refactoring (not affecting public API)

### Post-1.0 (Future)

**Format**: `MAJOR.MINOR.PATCH`

Strict [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR**: Breaking changes to stable public API
  - Deprecation warnings in previous MINOR release
  - Migration guide provided
  - At least 3 months notice for major changes

- **MINOR**: Backwards-compatible additions
  - New features
  - New APIs
  - Deprecations (with warnings)

- **PATCH**: Backwards-compatible fixes
  - Bug fixes
  - Security patches
  - Performance improvements

---

## Breaking Changes

### Definition

A **breaking change** is any modification that:
1. Changes the signature of a public function or method
2. Removes a public type, trait, or function
3. Changes the behavior of existing functionality in a non-backwards-compatible way
4. Changes the meaning of configuration options
5. Requires users to modify their code to upgrade

### Examples

**Breaking changes:**
```rust
// Before (v0.3.0)
fn execute_tool(&self, tool: &str) -> Result<String, Error>;

// After (v0.4.0) - BREAKING: parameter type changed
fn execute_tool(&self, tool: ToolDispatch) -> Result<String, Error>;
```

**Non-breaking changes:**
```rust
// Before (v0.3.0)
fn store(&mut self, key: &str, value: &str) -> Result<(), Error>;

// After (v0.3.1) - NOT BREAKING: added optional parameter with default
fn store(&mut self, key: &str, value: &str) -> Result<(), Error>;
fn store_with_ttl(&mut self, key: &str, value: &str, ttl: Duration) -> Result<(), Error>;
```

### Breaking Change Process (Post-1.0)

1. **Proposal**: Document proposed breaking change with rationale
2. **Deprecation** (N.x.y): Add deprecation warning with migration path
3. **Notice Period**: Minimum 1 minor version (3+ months)
4. **Implementation** (N+1.0.0): Remove deprecated API in next major version
5. **Migration Guide**: Provide comprehensive migration documentation

---

## Deprecation Policy

### Deprecation Process

1. **Mark as Deprecated**:
   ```rust
   #[deprecated(since = "0.4.0", note = "Use `new_api()` instead")]
   pub fn old_api() -> Result<(), Error> {
       // ...
   }
   ```

2. **Update Documentation**:
   - Add deprecation notice to rustdoc
   - Explain why it's deprecated
   - Provide migration instructions
   - Link to replacement API

3. **CHANGELOG Entry**:
   ```markdown
   ### Deprecated
   - `old_api()`: Deprecated in favor of `new_api()`. Will be removed in v0.5.0.
   ```

4. **Removal Timeline**:
   - **Pre-1.0**: Minimum 1 minor version before removal
   - **Post-1.0**: Minimum 1 major version before removal

### Current Deprecations

**None** - v0.3.0 has no deprecated APIs

---

## Feature Flags

### Stable Features

Features without the `unstable-` prefix are considered stable:

```toml
[features]
# Core features (stable)
default = []
auth = ["skreaver-http/auth"]
openapi = ["skreaver-http/openapi"]
compression = ["skreaver-http/compression"]
streaming = ["skreaver-http/streaming"]

# Backend features (stable)
redis = ["skreaver-memory/redis"]
sqlite = ["skreaver-memory/sqlite"]
postgres = ["skreaver-memory/postgres"]

# Observability features (stable)
metrics = ["skreaver-observability/metrics"]
tracing = ["skreaver-observability/tracing"]
observability = ["metrics", "tracing"]
opentelemetry = ["observability", "skreaver-observability/opentelemetry"]

# Testing features (stable)
testing = ["skreaver-testing"]
```

### Unstable Features

Features prefixed with `unstable-` are experimental:

```toml
[features]
# Experimental features (may break at any time)
unstable-websocket = ["streaming", "skreaver-http/unstable-websocket"]
```

**Usage Warning**: When using unstable features, your code may break in minor or patch releases.

### Feature Stability Promise

- **Stable features**: Follow normal versioning policy
- **Unstable features**: May break at any time, even in patch releases
- **Feature promotion**: Unstable features will be stabilized when ready

---

## API Stability Levels

### Level 1: Stable ✅

**Exported from `skreaver` meta-crate, no `unstable-` prefix**

- Follows semantic versioning
- Breaking changes only in major versions (post-1.0)
- Deprecation warnings provided
- Full documentation and tests
- **Examples**: `Agent`, `Memory`, `Tool`, `InMemoryMemory`

### Level 2: Unstable ⚠️

**Features marked with `unstable-` prefix**

- May change in minor releases
- Limited or no deprecation warnings
- Partial documentation
- Use with caution in production
- **Examples**: `unstable-websocket`

### Level 3: Internal ❌

**Direct crate imports (e.g., `skreaver_core::*`)**

- No stability guarantee
- May change at any time
- Not recommended for external use
- Use meta-crate re-exports instead
- **Examples**: `skreaver_core::agent::Agent`

---

## Migration Support

### Migration Guides

For all breaking changes, we provide:
1. **CHANGELOG.md**: Detailed list of changes
2. **MIGRATION.md**: Step-by-step migration guide
3. **Code examples**: Before/after code snippets
4. **Deprecation warnings**: Compile-time guidance

### Example Migration Guide Structure

```markdown
## Migrating from v0.3.x to v0.4.x

### Breaking Change: Tool Dispatch API

**Before (v0.3.x):**
```rust
coordinator.execute_tool_by_name("http_get", "http://example.com")?;
```

**After (v0.4.x):**
```rust
let tool = ToolDispatch::from_name("http_get")?;
coordinator.execute_tool(tool, "http://example.com")?;
```

**Rationale**: Type-safe tool dispatch prevents runtime errors
```

---

## Enforcement

### Automated Checks

- **cargo-semver-checks**: Detect API breaking changes in CI
- **Clippy lints**: Warn about deprecated API usage
- **Documentation tests**: Ensure examples remain valid
- **CI failures**: Block releases with breaking changes in stable APIs

### Review Process

All PRs that touch public APIs must:
1. Update this document if API stability changes
2. Add CHANGELOG entry for breaking changes
3. Provide deprecation warnings where appropriate
4. Include migration instructions
5. Pass `cargo-semver-checks` validation

---

## Feedback & Questions

### Reporting Issues

If you encounter:
- Undocumented breaking changes
- Missing deprecation warnings
- Unclear migration paths
- API instability concerns

Please file an issue at: https://github.com/shurankain/skreaver/issues

### API Stability Requests

To request API stabilization for:
- Unstable features
- Internal APIs
- Experimental functionality

Open a discussion with:
1. Use case description
2. Current workarounds
3. Proposed API design
4. Testing plan

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-10-08 | Initial API stability guarantee |

---

## Appendix: Full Stable API Reference

See the [API documentation](https://docs.rs/skreaver) for complete API reference.

### Quick Reference

**Core System**:
- `Agent` - Core agent trait
- `Memory{Reader,Writer}` - Memory abstractions
- `Tool` - Tool trait
- `ToolCall` - Tool invocation
- `ExecutionResult` - Tool execution result

**Memory Backends**:
- `InMemoryMemory` - In-memory storage
- `FileMemory` - File-based persistence
- `RedisMemory` (feature: `redis`)
- `SqliteMemory` (feature: `sqlite`)
- `PostgresMemory` (feature: `postgres`)

**Security**:
- `SecurityConfig` - Security configuration
- `SecurityPolicy` - Security policies
- `InputValidator` - Input validation
- `ResourceLimits` - Resource limits

**Authentication** (feature: `auth`):
- `AuthManager` - Authentication manager
- `ApiKeyManager` - API key auth
- `JwtManager` - JWT auth
- `RoleManager` - RBAC manager

**Error Types**:
- `SkreverError` - Main error type
- `SecurityError` - Security errors
- `AuthError` - Authentication errors

---

**Last Updated**: 2025-10-08
**Next Review**: Before v0.4.0 release
