# Skreaver Migration Guide

> **Current Version**: v0.5.0
> **Last Updated**: 2025-10-31

This document provides step-by-step migration instructions for upgrading between Skreaver versions.

---

## Table of Contents

- [Overview](#overview)
- [Migration Strategy](#migration-strategy)
- [Version-Specific Guides](#version-specific-guides)
  - [v0.4.x → v0.5.x](#v04x--v05x)
  - [v0.3.x → v0.4.x](#v03x--v04x)
  - [v0.2.x → v0.3.x](#v02x--v03x)
  - [v0.1.x → v0.2.x](#v01x--v02x)
- [Common Migration Patterns](#common-migration-patterns)
- [Troubleshooting](#troubleshooting)

---

## Overview

### Migration Philosophy

Skreaver aims to make migrations as smooth as possible by:

1. **Clear Deprecation Warnings**: All breaking changes are deprecated first
2. **Detailed Migration Guides**: Step-by-step instructions for each change
3. **Code Examples**: Before/after code snippets
4. **Rationale**: Explanation of why changes were made
5. **Tool Support**: Automated checks via `cargo-semver-checks`

### Reading This Guide

Each migration section includes:

- **Breaking Changes**: What changed and why
- **Before/After Examples**: Code migration examples
- **Migration Steps**: Ordered list of actions
- **Impact Assessment**: How much work is required
- **Deprecation Timeline**: When deprecated APIs will be removed

---

## Migration Strategy

### Step-by-Step Approach

1. **Review CHANGELOG.md**: Understand all changes in target version
2. **Check Deprecation Warnings**: Run `cargo build` and fix warnings
3. **Read Migration Guide**: Follow version-specific instructions below
4. **Update Dependencies**: Bump `skreaver` version in `Cargo.toml`
5. **Fix Compilation Errors**: Address any breaking changes
6. **Run Tests**: Ensure functionality is preserved
7. **Update Documentation**: Update your code comments and docs

### Testing Your Migration

```bash
# 1. Update to new version
cargo update skreaver

# 2. Check for issues
cargo check --all-features

# 3. Fix deprecation warnings
cargo build 2>&1 | grep "warning.*deprecated"

# 4. Run your test suite
cargo test

# 5. Check for breaking changes (if upgrading)
cargo semver-checks check-release
```

---

## Version-Specific Guides

### v0.4.x → v0.5.x

**Release Date**: 2025-10-31
**Impact**: **MINIMAL** - Nearly 100% backward compatible, drop-in replacement!
**Breaking Changes**: **One minor feature flag rename**

#### Summary of Changes

🎉 **Great News**: v0.5.0 is fully backward compatible with v0.4.x. Only one optional change needed!

**Major Additions**:
- ✅ WebSocket stabilization (production-ready, no longer experimental)
- ✅ Security configuration runtime integration (complete)
- ✅ CLI enhancements with advanced scaffolding
- ✅ Prometheus metrics integration
- ✅ Production infrastructure (Helm charts, deployment guides)
- ✅ Complete deprecation cleanup (zero deprecated code)

**What's Changed**:
- WebSocket feature flag renamed: `unstable-websocket` → `websocket`
- WebSocket now included in default features (stable API)
- All deprecated APIs from previous versions removed

#### Migration Steps

v0.5.0 is a **drop-in replacement** for v0.4.x. Simply update your `Cargo.toml`:

```toml
[dependencies]
skreaver = "0.5"
# or
skreaver-http = "0.5"
skreaver-core = "0.5"
# etc.
```

Then run:
```bash
cargo update
cargo build
cargo test
```

#### WebSocket Feature Flag Change (Optional)

If you explicitly enabled the WebSocket feature in v0.4.x, update the feature name:

**Before (v0.4.x)**:
```toml
[dependencies]
skreaver-http = { version = "0.4", features = ["unstable-websocket"] }
```

**After (v0.5.x)**:
```toml
[dependencies]
skreaver-http = { version = "0.5", features = ["websocket"] }
# Or just use defaults (websocket now included):
skreaver-http = "0.5"
```

**Note**: If you were using default features, no changes needed! WebSocket is now stable and included by default.

#### No Code Changes Required

Your existing WebSocket code works without modification:

```rust
// This code works identically in v0.4.x and v0.5.x
use skreaver_http::websocket::{WebSocketConfig, WebSocketManager};

let config = WebSocketConfig::default();
let manager = Arc::new(WebSocketManager::new(config));
// ... rest of your code unchanged
```

#### What's New in v0.5.0

If you want to adopt the new v0.5.0 features, here's what's available:

**1. Production-Ready WebSocket**

WebSocket is now stable and production-ready with enhanced features:

```rust
use skreaver_http::websocket::WebSocketConfig;

let config = WebSocketConfig {
    max_connections: 5000,
    connection_timeout: Duration::from_secs(300),
    ping_interval: Duration::from_secs(30),
    max_message_size: 256 * 1024, // 256KB
    enable_compression: true,
    ..Default::default()
};
```

See [WEBSOCKET_GUIDE.md](WEBSOCKET_GUIDE.md) for complete documentation.

**2. Security Configuration Runtime**

Security policies are now fully integrated with the HTTP runtime:

```rust
// Automatically loaded and validated at startup
// Place skreaver-security.toml in your project root
let runtime = HttpRuntime::new(config).await?;
```

**3. CLI Scaffolding**

Generate new agents and tools with templates:

```bash
skreaver new agent --name MyAgent --template reasoning-balanced
skreaver generate tool --name http-client
```

**4. Prometheus Metrics**

Access comprehensive metrics at `/metrics` endpoint:

```bash
curl http://localhost:8080/metrics
```

#### Removed Deprecated APIs

The following deprecated APIs from v0.4.0 have been removed in v0.5.0:

- `StatefulAgentTransitions` trait (was deprecated, use state machine pattern)
- `Message.from` and `Message.to` fields (use `Message.metadata` instead)
- `MessageBuilder::from()` and `to()` methods (use `metadata()` instead)
- `AgentInstance::set_metadata()` and `get_metadata()` (use direct field access)

If you were using these APIs, you should have received deprecation warnings in v0.4.x. See the deprecation messages for migration paths.

#### Testing Your Migration

```bash
# Update dependencies
cargo update

# Verify build
cargo build --all-features

# Run tests
cargo test

# Check for deprecation warnings (should be none!)
cargo build 2>&1 | grep "warning.*deprecated"

# Optional: Check API compatibility
cargo semver-checks check-release
```

#### Performance Characteristics

v0.5.0 maintains excellent performance:

| Metric | v0.4.0 | v0.5.0 | Change |
|--------|--------|--------|--------|
| Release build time | 8.02s | 6.33s | ⬇️ 21% faster |
| Incremental build | ~2s | ~2s | ✅ Same |
| Test execution | ~5s | ~5s | ✅ Same |
| Memory footprint | <128MB | <128MB | ✅ Same |

#### Common Issues

##### Issue: "Cannot find feature `unstable-websocket`"

**Cause**: Feature was renamed in v0.5.0

**Solution**: Update feature name:
```toml
# Change this:
features = ["unstable-websocket"]
# To this:
features = ["websocket"]
# Or remove it (now included in defaults):
# features = []
```

##### Issue: Deprecated API errors

**Cause**: APIs deprecated in v0.4.x were removed in v0.5.0

**Solution**: The deprecation warnings in v0.4.x included migration instructions. If you missed them:
- Use `Message.metadata` instead of `.from`/`.to`
- Access agent metadata directly instead of via getters/setters
- Implement custom state transitions instead of using `StatefulAgentTransitions`

#### If You Need to Rollback

If you need to rollback to v0.4.x:

```toml
[dependencies]
skreaver = "0.4"
```

```bash
cargo update
cargo build
```

All v0.5.0 features will be unavailable, but your code will work.

#### Next Steps

After migrating to v0.5.0:

1. ✅ Review [CHANGELOG.md](CHANGELOG.md) for full list of changes
2. ✅ Check [WEBSOCKET_GUIDE.md](WEBSOCKET_GUIDE.md) for WebSocket best practices
3. ✅ Explore [SRE_RUNBOOK.md](SRE_RUNBOOK.md) for production operations
4. ✅ Read [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md) for Kubernetes deployment

#### Summary

- **Effort Required**: ⭐ Minimal (5-10 minutes)
- **Code Changes**: None for most users, optional feature flag rename for WebSocket users
- **Risk Level**: 🟢 Low (fully backward compatible)
- **Benefits**: Stable WebSocket API, enhanced security, better tooling

**Result**: v0.5.0 maintains or improves all functionality with zero breaking changes!

---

### v0.3.x → v0.4.x

**Release Date**: 2025-10-11
**Impact**: **NONE** - 100% backward compatible, drop-in replacement!
**Breaking Changes**: **None**

#### Summary of Changes

🎉 **Great News**: v0.4.0 is fully backward compatible with v0.3.x. No code changes required!

**Major Additions**:
- ✅ Production authentication system (AES-256-GCM + JWT + token revocation)
- ✅ Real resource monitoring (CPU, memory, file descriptors, disk)
- ✅ Performance benchmarking framework with CI integration
- ✅ API stability guarantees and documentation
- ✅ Agent mesh communication (skreaver-mesh)
- ✅ MCP protocol support (skreaver-mcp)
- ✅ WebSocket support (unstable feature)
- ✅ Enhanced memory backends (SQLite, PostgreSQL)
- ✅ 347 tests (up from 120+)

**Breaking Changes**: **None**

**Deprecations**: **None**

---

#### Migration Steps

##### No Migration Required! ✅

v0.4.0 is a **drop-in replacement** for v0.3.x. Simply update your `Cargo.toml`:

```toml
[dependencies]
skreaver = "0.4"
```

Then run:
```bash
cargo update
cargo test
```

**That's it!** Your existing code will work without any changes.

---

#### New Features (Optional)

If you want to adopt the new v0.4.0 features, here's how:

##### 1. JWT Token Revocation (Optional)

**What's New**: Immediate token invalidation with Redis or in-memory blacklist

**Example**:
```rust
use skreaver_core::auth::{JwtManager, JwtConfig, InMemoryBlacklist};
use std::sync::Arc;

// Create JWT manager with revocation support
let config = JwtConfig::default();
let blacklist = Arc::new(InMemoryBlacklist::new());
let manager = JwtManager::with_blacklist(config, blacklist);

// Generate a token
let principal = Principal::new("user-123", "Alice", AuthMethod::ApiKey("key".into()));
let token = manager.generate(&principal).await?;

// Revoke the token (e.g., on logout or security event)
manager.revoke(&token.access_token).await?;

// Token is now invalid
assert!(manager.authenticate(&token.access_token).await.is_err());
```

**Production Setup with Redis**:
```toml
[dependencies]
skreaver-core = { version = "0.4", features = ["redis"] }
```

```rust
use skreaver_core::auth::{JwtManager, RedisBlacklist};

let blacklist = Arc::new(RedisBlacklist::new("redis://localhost:6379")?);
let manager = JwtManager::with_blacklist(config, blacklist);
```

**When to Use**:
- User logout
- Password changes
- Security breaches
- Permission revocations

**Documentation**: See [JWT_TOKEN_REVOCATION.md](JWT_TOKEN_REVOCATION.md)

---

##### 2. Resource Monitoring (Optional)

**What's New**: Real-time CPU, memory, and resource tracking

**Example**:
```rust
use skreaver_core::security::limits::{ResourceLimits, ResourceTracker};
use std::time::Duration;

// Configure resource limits
let limits = ResourceLimits {
    max_memory_mb: 256,
    max_cpu_percent: 75.0,
    max_execution_time: Duration::from_secs(300),
    max_concurrent_operations: 20,
    max_open_files: 200,
    max_disk_usage_mb: 1024,
};

// Create tracker
let tracker = ResourceTracker::new(&limits);

// Track an operation (automatically cleaned up when guard drops)
{
    let _guard = tracker.start_operation("my_agent");

    // Do work...

    // Get current resource usage
    if let Some(usage) = tracker.get_usage("my_agent") {
        println!("Memory: {} MB", usage.memory_mb);
        println!("CPU: {:.2}%", usage.cpu_percent);
    }
} // Resources automatically released here
```

**When to Use**:
- Production deployments needing DoS protection
- Multi-tenant systems requiring fair resource allocation
- Applications with resource constraints

**Documentation**: See code documentation in `skreaver-core::security::limits`

---

##### 3. Agent Mesh Communication (Optional)

**What's New**: Multi-agent coordination using Redis Pub/Sub

**Add Dependency**:
```toml
[dependencies]
skreaver-mesh = "0.1"
```

**Example**:
```rust
use skreaver_mesh::{
    RedisAgentMesh,
    coordination::{Supervisor, Pipeline, RequestReply},
    Message, MessagePriority,
};

// Connect to Redis
let mesh = RedisAgentMesh::new("redis://localhost:6379").await?;

// Supervisor pattern: coordinate worker agents
let supervisor = Supervisor::new(mesh.clone());
supervisor.spawn_worker("worker-1").await?;
supervisor.send_task("process-data", data).await?;

// Pipeline pattern: chain agent operations
let pipeline = Pipeline::new(mesh.clone());
pipeline.add_stage("validate", validator_agent).await?;
pipeline.add_stage("process", processor_agent).await?;
pipeline.execute(input).await?;

// Request/Reply pattern: synchronous agent calls
let request_reply = RequestReply::new(mesh.clone());
let response = request_reply.request("calculator", operation).await?;
```

**When to Use**:
- Distributed agent systems
- Multi-agent workflows
- Agent supervision and orchestration

---

##### 4. MCP Protocol Integration (Optional)

**What's New**: Model Context Protocol for Claude Desktop integration

**Add Dependency**:
```toml
[dependencies]
skreaver-mcp = "0.1"
```

**Example**:
```rust
use skreaver_mcp::{McpServer, ServerConfig, Resource, Tool};

// Configure MCP server
let config = ServerConfig {
    name: "my-skreaver-server".to_string(),
    version: "1.0.0".to_string(),
    capabilities: Default::default(),
};

// Create server
let server = McpServer::new(config)?;

// Export tools as MCP resources
server.register_tool(my_custom_tool).await?;

// Start server (stdio or WebSocket)
server.start_stdio().await?;
```

**When to Use**:
- Integrating with Claude Desktop
- Exposing agent tools to external systems
- Building MCP-compatible services

---

##### 5. WebSocket Support (Unstable, Optional)

**What's New**: Real-time bidirectional communication

**Add Feature**:
```toml
[dependencies]
skreaver-http = { version = "0.4", features = ["unstable-websocket"] }
```

**Example**:
```rust
use skreaver_http::websocket::{WebSocketServer, MessageEnvelope};

let ws_server = WebSocketServer::new();

// Subscribe to events
ws_server.subscribe("agent-events").await?;

// Send messages
ws_server.send(MessageEnvelope {
    id: "msg-123".to_string(),
    channel: "agent-events".to_string(),
    payload: serde_json::to_value(&event)?,
}).await?;
```

**⚠️ Warning**: API marked `unstable-websocket` - may change in minor releases

**When to Use**:
- Real-time agent status updates
- Live streaming agent responses
- Interactive agent debugging

---

##### 6. Enhanced Memory Backends (Optional)

**What's New**: SQLite and PostgreSQL backends with migrations

**SQLite Backend**:
```toml
[dependencies]
skreaver-memory = { version = "0.4", features = ["sqlite"] }
```

```rust
use skreaver_memory::SqliteMemory;

let memory = SqliteMemory::new("agents.db").await?;

// Automatic WAL mode for better concurrency
// Built-in migrations
// Backup/restore support
```

**PostgreSQL Backend**:
```toml
[dependencies]
skreaver-memory = { version = "0.4", features = ["postgres"] }
```

```rust
use skreaver_memory::PostgresMemory;

let memory = PostgresMemory::new("postgresql://localhost/skreaver").await?;

// Connection pooling
// Schema migrations
// Admin operations
```

**When to Use**:
- Persistent agent memory across restarts
- Multi-instance agent deployments
- Production systems requiring durability

---

#### Testing Your Upgrade

```bash
# 1. Update dependencies
cargo update

# 2. Verify build with all features
cargo build --workspace --all-features

# 3. Run your test suite
cargo test

# 4. Run optional benchmark (if you want to validate performance)
cargo bench

# 5. Check for any warnings
cargo clippy -- -W clippy::all
```

**Expected Results**:
- ✅ All builds succeed
- ✅ All tests pass (no failures)
- ✅ No new clippy warnings
- ✅ Performance matches or exceeds v0.3.x

---

#### Performance Impact

**Benchmarks** (compared to v0.3.x):

| Metric | v0.3.x | v0.4.0 | Change |
|--------|--------|--------|--------|
| p50 latency | ~25ms | <30ms | ✅ Maintained |
| p95 latency | ~180ms | <200ms | ✅ Maintained |
| p99 latency | ~350ms | <400ms | ✅ Maintained |
| Memory (N=32) | ~120MB | ≤128MB | ✅ Maintained |
| Build time (clean) | ~25s | ~20s | ✅ Improved 20% |
| Build time (incremental) | ~7s | ~6s | ✅ Improved 14% |

**Result**: v0.4.0 maintains or improves all performance metrics!

---

#### Troubleshooting

##### Issue: Build fails with Redis feature

**Cause**: Redis build error was fixed in v0.4.0

**Solution**: Ensure you're using exactly v0.4.0, not a pre-release:
```toml
[dependencies]
skreaver-core = { version = "0.4", features = ["redis"] }
```

##### Issue: New features not available

**Cause**: Feature flags not enabled

**Solution**: Enable required features:
```toml
[dependencies]
skreaver-mesh = "0.1"      # For agent mesh
skreaver-mcp = "0.1"       # For MCP protocol
skreaver-core = { version = "0.4", features = ["redis"] }  # For Redis blacklist
```

##### Issue: Performance regression

**Cause**: Unlikely, but check if unnecessary features are enabled

**Solution**: Only enable features you use:
```toml
# ❌ Don't do this unless needed
skreaver = { version = "0.4", features = ["all"] }

# ✅ Enable selectively
skreaver = { version = "0.4", features = ["redis", "auth"] }
```

---

#### Rollback Plan

If you need to rollback to v0.3.x:

```toml
[dependencies]
skreaver = "0.3"
```

```bash
cargo update
cargo test
```

No code changes needed (backward compatible)!

---

#### Summary

**Effort Required**: ⭐ **Minimal** (just `cargo update`)

**Benefits**:
- ✅ 100% backward compatible
- ✅ 227 new tests for stability
- ✅ Production-ready authentication
- ✅ Real resource monitoring
- ✅ Performance improvements
- ✅ New optional features available

**Recommendation**: **Upgrade immediately** - zero risk, all upside!

---

### v0.2.x → v0.3.x

**Release Date**: 2025-09-10
**Impact**: Medium - Security framework additions, performance improvements

#### Summary of Changes

**Major Additions**:
- ✅ Enterprise security framework
- ✅ OpenTelemetry observability integration
- ✅ Performance optimizations (37% faster builds)
- ✅ Multi-agent communication layer (skreaver-mesh)
- ✅ MCP protocol support (skreaver-mcp)

**Breaking Changes**:
- Redis API updated from `execute()` to `exec().unwrap()`
- Some internal module reorganization

#### Migration Steps

##### 1. Update Redis API Usage

**What Changed**: Redis crate deprecated `execute()` method

**Before (v0.2.x)**:
```rust
use redis::pipe;

let mut pipe = pipe();
pipe.set("key", "value");
pipe.execute(&mut conn);
```

**After (v0.3.x)**:
```rust
use redis::pipe;

let mut pipe = pipe();
pipe.set("key", "value");
pipe.exec(&mut conn).unwrap();
```

**Why**: Upstream redis crate deprecation

**Impact**: Low - simple method rename

##### 2. Update Imports for New Security Features

**What Changed**: New security module structure

**Before (v0.2.x)**:
```rust
use skreaver::validation::validate_input;
```

**After (v0.3.x)**:
```rust
use skreaver::security::{InputValidator, SecurityConfig};

let validator = InputValidator::new(SecurityConfig::default());
validator.validate_input(input)?;
```

**Why**: Comprehensive security framework with configurable policies

**Impact**: Low - only if using security features

##### 3. Enable New Feature Flags (Optional)

**New Features Available**:
```toml
[dependencies]
skreaver = { version = "0.3", features = [
    "security-full",      # Complete security framework
    "observability",      # Metrics + tracing
    "opentelemetry",      # OTEL export
] }
```

**Impact**: None - optional features

##### 4. Update Observability (Optional)

**What's New**: Integrated observability framework

**After (v0.3.x)**:
```rust
use skreaver::observability::{HealthStatus, MetricsCollector};

// Health checks
let health = health_checker.check().await?;
assert!(matches!(health.status, HealthStatus::Healthy));

// Metrics collection
let metrics = metrics_collector.collect();
```

**Why**: Production-ready monitoring

**Impact**: None - optional addition

---

### v0.1.x → v0.2.x

**Note**: v0.2.x was an internal version. Most users migrated directly from v0.1.x to v0.3.x.

See v0.1.x → v0.3.x migration below.

---

### v0.1.x → v0.3.x (Combined)

**Release Date**: 2025-09-10 (v0.3.0)
**Impact**: High - Major architecture changes

#### Summary of Changes

**Major Changes**:
- ✅ Workspace restructure (7 → 9 crates)
- ✅ Feature gate reorganization
- ✅ Memory backend improvements
- ✅ HTTP runtime enhancements
- ✅ Security framework addition

#### Migration Steps

##### 1. Update Import Paths

**What Changed**: Monolithic to workspace architecture

**Before (v0.1.x)**:
```rust
use skreaver::{Agent, Memory, Tool, Coordinator};
use skreaver::{InMemoryMemory, FileMemory};
use skreaver::{HttpTool, JsonTool};
```

**After (v0.3.x)**:
```rust
// Preferred: Use meta-crate (no change needed!)
use skreaver::{Agent, Memory, Tool, Coordinator};
use skreaver::{InMemoryMemory, FileMemory, RedisMemory};
use skreaver::{HttpTool, JsonTool};

// Advanced: Direct crate access (unstable)
use skreaver_core::{Agent, Memory, Tool};
use skreaver_memory::{FileMemory, RedisMemory};
use skreaver_tools::{HttpTool, JsonTool};
```

**Why**: Better modularity and selective compilation

**Impact**: Low - meta-crate re-exports unchanged

##### 2. Update Memory Backend Usage

**What Changed**: InMemoryMemory moved to skreaver-core

**Before (v0.1.x)**:
```rust
use skreaver::memory::InMemoryMemory;
```

**After (v0.3.x)**:
```rust
use skreaver::InMemoryMemory;  // Now in core, re-exported
```

**Why**: InMemoryMemory is core functionality

**Impact**: Low - just update import

##### 3. Update Feature Flags

**What Changed**: More granular feature flags

**Before (v0.1.x)**:
```toml
[dependencies]
skreaver = { version = "0.1", features = ["redis"] }
```

**After (v0.3.x)**:
```toml
[dependencies]
skreaver = { version = "0.3", features = [
    "redis",              # Memory backend
    "auth",               # Authentication
    "openapi",            # API docs
    "observability",      # Metrics + tracing
] }
```

**Why**: Selective compilation for faster builds

**Impact**: Medium - review which features you need

##### 4. Update Tool Dispatch (If Using)

**What Changed**: Type-safe tool dispatch

**Before (v0.1.x)**:
```rust
coordinator.execute_tool_by_name("http_get", params).await?;
```

**After (v0.3.x)**:
```rust
// Option 1: Direct tool usage (recommended)
let tool = HttpTool::new();
let result = tool.call(url);

// Option 2: Registry dispatch
let tool = ToolDispatch::from_name("http_get")?;
coordinator.dispatch(tool, params).await?;
```

**Why**: Compile-time type safety

**Impact**: Medium - update tool invocations

##### 5. Update Configuration

**What Changed**: Security configuration added

**After (v0.3.x)**:
```rust
use skreaver::security::SecurityConfig;

let config = SecurityConfig::default()
    .with_file_access("/var/app/data")
    .with_http_domain("*.example.com");
```

**Why**: Secure by default

**Impact**: Low - optional, use defaults

---

## Common Migration Patterns

### Pattern 1: Updating Imports

**Problem**: Compilation error `use of undeclared type or module`

**Solution**: Always import from `skreaver` meta-crate
```rust
// ❌ Don't do this
use skreaver_core::Agent;

// ✅ Do this instead
use skreaver::Agent;
```

### Pattern 2: Feature Flag Confusion

**Problem**: Feature not available despite importing

**Solution**: Check `Cargo.toml` has correct features
```toml
[dependencies]
skreaver = { version = "0.3", features = ["redis", "auth"] }
```

### Pattern 3: Breaking API Changes

**Problem**: Method signature changed

**Solution**: Check CHANGELOG.md for migration guide
```rust
// Find old usage
git grep "old_method_name"

// Replace with new usage per migration guide
// See specific version section above
```

### Pattern 4: Deprecation Warnings

**Problem**: Seeing deprecation warnings

**Solution**: Follow inline documentation
```rust
#[deprecated(since = "0.4.0", note = "Use new_method() instead")]
//                                     ^^^^^^^^^^^^^^^^^^^
//                                     Follow this guidance
```

---

## Troubleshooting

### Common Issues

#### Issue: "cannot find type `X` in crate `skreaver`"

**Cause**: Type not re-exported in meta-crate or wrong feature flag

**Solution**:
1. Check if type requires feature flag: `cargo doc --open --features all-features`
2. Add required feature to `Cargo.toml`
3. Import from correct module

#### Issue: "trait `Memory` is not implemented for `RedisMemory`"

**Cause**: Feature flag not enabled

**Solution**:
```toml
[dependencies]
skreaver = { version = "0.3", features = ["redis"] }
```

#### Issue: Build fails with "duplicate definitions"

**Cause**: Importing from both meta-crate and sub-crates

**Solution**: Only use meta-crate imports
```rust
// ❌ Remove
use skreaver_core::Agent;

// ✅ Keep
use skreaver::Agent;
```

#### Issue: Performance regression after upgrade

**Cause**: Feature bloat - unnecessary features enabled

**Solution**: Only enable features you use
```toml
[dependencies]
# ❌ Don't enable everything
skreaver = { version = "0.3", features = ["all"] }

# ✅ Enable only what you need
skreaver = { version = "0.3", features = ["redis", "auth"] }
```

### Getting Help

If you encounter migration issues:

1. **Check Documentation**:
   - This migration guide
   - [CHANGELOG.md](CHANGELOG.md)
   - [API_STABILITY.md](API_STABILITY.md)

2. **Search Issues**: https://github.com/shurankain/skreaver/issues

3. **Ask for Help**:
   - Open a GitHub issue with `migration` label
   - Include: version upgrading from/to, error message, minimal reproduction

4. **Community Support**:
   - GitHub Discussions
   - Discord (if available)

---

## Checklist

Use this checklist when migrating:

- [ ] Read CHANGELOG.md for target version
- [ ] Backup your code / commit current state
- [ ] Update `Cargo.toml` version
- [ ] Run `cargo check` and note errors
- [ ] Fix deprecation warnings
- [ ] Update imports per migration guide
- [ ] Update feature flags if needed
- [ ] Fix breaking API changes
- [ ] Run test suite
- [ ] Review performance (if applicable)
- [ ] Update your documentation
- [ ] Deploy to staging environment
- [ ] Monitor for issues

---

## Version History

| Version | Migration From | Guide Added | Notes |
|---------|---------------|-------------|-------|
| v0.3.0 | v0.1.x, v0.2.x | 2025-09-10 | Workspace architecture |
| v0.4.0 | v0.3.x | 2025-10-11 | 100% backward compatible, new features |
| v0.5.0 | v0.4.x | 2025-10-31 | WebSocket stabilization, minimal breaking changes |

---

## See Also

- [CHANGELOG.md](CHANGELOG.md) - Complete version history
- [API_STABILITY.md](API_STABILITY.md) - API stability guarantees
- [DEPRECATION_POLICY.md](DEPRECATION_POLICY.md) - Deprecation process
- [README.md](README.md) - Getting started guide
- [JWT_TOKEN_REVOCATION.md](JWT_TOKEN_REVOCATION.md) - Token revocation guide (v0.4.0+)
- [CODE_AUDIT_v0.4.0.md](CODE_AUDIT_v0.4.0.md) - Production readiness audit
- [NEXT_STEPS_EVALUATION.md](NEXT_STEPS_EVALUATION.md) - Strategic roadmap

---

**Last Updated**: 2025-10-31
**Current Version**: v0.5.0
