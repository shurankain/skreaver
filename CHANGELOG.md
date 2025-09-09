# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

### Changed

### Fixed

### Security

## [0.3.0] - 2025-09-10

### Added
- **🔒 Enterprise Security Framework**: Comprehensive security system with threat modeling
  - Input validation and sanitization for all tool operations
  - Tool sandboxing with deny-by-default security policies  
  - Resource limits and DoS protection mechanisms
  - Audit logging with structured security events
  - Secret detection and redaction in inputs/outputs
  - Path traversal and SSRF protection
  - Emergency lockdown capabilities
- **📊 Observability Integration**: Full OpenTelemetry support
  - Structured metrics with Prometheus endpoints (`/metrics`, `/health`, `/ready`)
  - Distributed tracing with correlation IDs
  - Health monitoring and status reporting
  - Performance monitoring with configurable sampling
- **🛠️ Security Configuration**: TOML-based security policies
  - File system access control with allowlists
  - HTTP client security with domain filtering  
  - Network access restrictions
  - Resource quotas and limits
- **🔍 Security Testing**: Comprehensive security test suite
  - 21 unit tests + 12 integration tests for security features
  - Path traversal attack prevention testing
  - SSRF protection validation
  - Secret detection accuracy testing
- **📚 Security Documentation**: Complete security model documentation
  - `THREAT_MODEL.md` with attack scenario analysis
  - `SECURITY_IMPLEMENTATION.md` with technical details
  - `skreaver-security.toml` configuration examples

### Changed
- **⚡ Build Performance**: 37% faster cold builds, 98.6% faster incremental builds
  - Lazy regex compilation using `once_cell` for pattern matching
  - Feature gates for optional security components
  - Replaced `chrono` with lighter `time` crate for timestamps
  - Optimized module structure and dependency graph
- **🏗️ Security Architecture**: Modular security system with feature gates
  - `security-basic`: Core validation and input sanitization
  - `security-audit`: Comprehensive audit logging  
  - `security-full`: Complete security feature set
  - `security-content-scanning`: Advanced content analysis
- **🔧 Core Dependencies**: Updated for performance and security
  - Added `once_cell` for lazy static compilation
  - Added `time` crate replacing `chrono` for timestamps
  - Added `sha2` for cryptographic hashing
  - Added `regex` with performance optimizations

### Fixed
- **🐛 Clippy Warnings**: Resolved all clippy warnings in security modules
- **🔧 Feature Gates**: Proper conditional compilation for all security features
- **⚙️ Build Issues**: Fixed compilation errors with feature combinations
- **📝 Documentation**: Updated README with security framework information

### Security
- **🛡️ Threat Model**: Comprehensive threat analysis and mitigation strategies
- **🔒 Input Validation**: All user inputs sanitized and validated
- **🚫 Attack Prevention**: Protection against path traversal, SSRF, and injection attacks
- **📝 Audit Trail**: Complete audit logging for security-relevant operations
- **🔐 Secret Management**: Environment-only secrets with audit-safe logging
- **⚡ Resource Protection**: DoS protection with configurable resource limits

## [0.1.0] - 2024-09-05

### Added
- **Workspace Architecture**: Multi-crate structure with clear separation of concerns
  - `skreaver-core`: Core traits and fundamental types
  - `skreaver-http`: HTTP runtime with Axum integration
  - `skreaver-tools`: Standard tool library with network, I/O, and data processing
  - `skreaver-memory`: Memory backend implementations
  - `skreaver-testing`: Testing framework and utilities
  - `skreaver`: Meta-crate for re-exports and unified API
  - `skreaver-cli`: Command-line interface application

- **Core Framework**
  - Agent trait with lifecycle management and state transitions
  - Memory abstraction with Reader/Writer separation
  - Tool system with type-safe dispatch and zero-copy optimization
  - Coordinator runtime for agent orchestration
  - Transactional memory operations with rollback support
  - Memory snapshots and restore functionality

- **Memory Backends**
  - InMemoryMemory: Lock-free concurrent access with DashMap
  - FileMemory: Persistent file-based storage
  - RedisMemory: Redis-backed distributed memory with connection pooling
  - NamespacedMemory: Isolated memory spaces for multi-tenancy
  - PostgreSQL memory backend support with `postgres` feature flag
  - Feature flags: `redis`, `sqlite`, `postgres` for optional backends

- **HTTP Runtime**
  - RESTful API with Axum integration
  - Agent lifecycle endpoints (`/agents/{id}/execute`, `/agents/{id}/status`)
  - Authentication middleware with JWT and API key support
  - OpenAPI documentation generation
  - Rate limiting and request validation
  - Security headers and Content Security Policy
  - Streaming responses for long-running operations
  - WebSocket support through `unstable-websocket` feature flag
  - Feature flags: `auth`, `openapi`, `openapi-ui`, `compression`, `streaming`

- **Tool System**
  - HTTP client tool with configurable requests
  - File I/O operations with path validation
  - JSON processing with path extraction
  - Text manipulation utilities
  - Strongly-typed tool dispatch with compile-time validation
  - Zero-copy tool execution optimization
  - Tool registry for dynamic registration
  - Tools feature granularity: separate `network` and `data` features
  - Feature flags: `io`, `network`, `data` for optional functionality

- **CI/CD Infrastructure**
  - Comprehensive CI pipeline with matrix strategy testing all feature combinations
  - Conditional service startup in CI (Redis/PostgreSQL only when needed)
  - Cargo dependency caching for faster CI builds
  - sccache compilation caching support
  - mold linker optimization for Linux builds
  - CLI-specific testing in CI pipeline
  - Comprehensive HTTP feature combinations testing

- **Security Framework**
  - Input validation and sanitization
  - Path traversal protection
  - Request size limits and timeouts
  - Security headers middleware
  - JWT token validation with HMAC
  - API key authentication
  - Content Security Policy configuration

- **Testing Infrastructure**
  - Comprehensive benchmark suite for performance testing
  - Mock tools for unit testing
  - Integration test harness
  - Property-based testing setup
  - CI/CD pipeline with automated testing
  - Test coverage reporting
  - Performance regression detection

- **Developer Experience**
  - CLI application with agent management
  - Example implementations and tutorials
  - Comprehensive documentation with doctests
  - Type-safe APIs with helpful error messages
  - Hot-reload support for development
  - Structured logging with tracing

### Changed
- **BREAKING**: Redis API updated from deprecated `execute()` to `exec().unwrap()`
- CI build time improved from 13 to 10 minutes (23% faster)
- Matrix strategy now tests 12 parallel jobs instead of monolithic builds
- All dependencies consolidated using `[workspace.dependencies]`
- Improved build profiles for faster CI compilation
- Enhanced error handling in Redis memory backend

### Fixed
- Redis memory backend compilation errors with `InMemoryMemory` imports
- GitHub Actions YAML validation errors with conditional services
- Missing `ToSchema` derive for `AuthError` type (utoipa v5 compatibility)
- Deprecated `rand::thread_rng()` usage replaced with `rand::rng()`
- Circular dependency issues with workspace architecture

### Security
- Added `ToSchema` derive to `AuthError` for secure OpenAPI documentation

### Performance Optimizations
- **Zero-Copy Tool Dispatch**: Eliminated cloning in Coordinator hot paths
- **Lock-Free Memory**: Replaced `Arc<RwLock>` with `DashMap` for concurrent access
- **Minimal Dependencies**: Reduced tokio features and dependency cleanup
- **Type-Safe Memory Keys**: Structured `ExecutionResult` with reduced allocations
- **Compile-Time Validation**: `StandardTool` enum for tool dispatch optimization

### Migration Guide

#### From Monolithic to Workspace Architecture

**Old Import Pattern:**
```rust
use skreaver::{Agent, Memory, Tool, Coordinator};
use skreaver::{InMemoryMemory, FileMemory};
use skreaver::{HttpTool, JsonTool};
```

**New Import Pattern:**
```rust
// For application development (recommended)
use skreaver::{Agent, Memory, Tool, Coordinator};
use skreaver::{InMemoryMemory, FileMemory, RedisMemory};
use skreaver::{HttpTool, JsonTool};

// For advanced usage (direct crate access)
use skreaver_core::{Agent, Memory, Tool, Coordinator, InMemoryMemory};
use skreaver_memory::{FileMemory, RedisMemory, NamespacedMemory};
use skreaver_tools::{HttpTool, JsonTool, FileReadTool};
use skreaver_http::{HttpRuntime, SecurityConfig};
```

#### Memory Backend Changes

**InMemoryMemory Location:**
- **Before**: `skreaver_memory::InMemoryMemory` 
- **After**: `skreaver_core::InMemoryMemory`

**Feature Flag Requirements:**
```toml
[dependencies]
skreaver = { version = "0.1", features = ["redis", "sqlite"] }
# Or for specific backends:
skreaver-memory = { version = "0.1", features = ["redis"] }
```

#### Tool System Breaking Changes

**Tool Dispatch:**
```rust
// Old API (removed)
coordinator.execute_tool_by_name("http_get", params).await?;

// New API (type-safe)
let tool = HttpTool::new(url);
coordinator.execute_tool(tool).await?;

// Or with StandardTool enum
let standard_tool = StandardTool::Http(HttpTool::new(url));
coordinator.dispatch(standard_tool).await?;
```

#### Redis API Updates

**Pipeline Execution:**
```rust
// Old API (deprecated)
pipe.execute(&mut conn);

// New API (required)
pipe.exec(&mut conn).unwrap();
```

#### Feature Flag Changes

**HTTP Features:**
```toml
# Basic HTTP runtime
skreaver-http = { version = "0.1", features = ["auth"] }

# Full HTTP with UI (development only)
skreaver-http = { version = "0.1", features = ["auth", "openapi", "openapi-ui"] }

# Production HTTP
skreaver-http = { version = "0.1", features = ["auth", "openapi", "compression"] }
```

**Tools Features:**
```toml
# All tools (old default)
skreaver-tools = { version = "0.1", features = ["io", "network", "data"] }

# Selective tools (new approach)
skreaver-tools = { version = "0.1", features = ["network"] }  # HTTP client only
skreaver-tools = { version = "0.1", features = ["data"] }     # JSON processing only
```

### Known Issues
- WebSocket support marked as `unstable-websocket` - API may change
- OpenAPI UI should only be enabled in development builds
- Redis connection pooling requires explicit configuration
- Large file operations may require increased timeout limits

### Compatibility
- **Minimum Rust Version**: 1.80.0
- **Edition**: 2024
- **Platform Support**: Linux, macOS, Windows
- **Architecture**: x86_64, ARM64

---

## Development Guidelines

### Versioning Strategy
- **Patch versions** (0.1.x): Bug fixes, performance improvements, documentation
- **Minor versions** (0.x.0): New features, non-breaking API additions
- **Major versions** (x.0.0): Breaking changes, architectural updates

### Breaking Change Policy
- All breaking changes are documented with migration guides
- Deprecation warnings provided at least one minor version before removal
- `unstable-*` prefixed features may break in minor releases
- Internal crate APIs (`skreaver-*::*`) may break without notice - use meta-crate `skreaver`

### Release Process
1. Update version numbers in all `Cargo.toml` files
2. Update `CHANGELOG.md` with release notes
3. Create git tag: `git tag -a v0.1.0 -m "Release v0.1.0"`
4. Push tag: `git push origin v0.1.0`
5. GitHub Actions will automatically create release with binaries

### Contributing
- Follow [Conventional Commits](https://www.conventionalcommits.org/) format
- All changes must include appropriate tests
- Performance-sensitive changes require benchmark validation
- Security-related changes require threat model review

---

**Note**: This project is in active development. APIs may change rapidly before v1.0.0. 
For production use, pin to specific versions and review changelog before upgrading.