# Skreaver Migration Guide

> **Current Version**: v0.3.0
> **Last Updated**: 2025-10-08

This document provides step-by-step migration instructions for upgrading between Skreaver versions.

---

## Table of Contents

- [Overview](#overview)
- [Migration Strategy](#migration-strategy)
- [Version-Specific Guides](#version-specific-guides)
  - [v0.3.x → v0.4.x](#v03x--v04x-planned)
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

### v0.3.x → v0.4.x (Planned)

**Status**: Not yet released
**Target Date**: TBD
**Breaking Changes**: TBD

This section will be populated when v0.4.0 is released.

**Expected Changes**:
- API stability finalization
- Possible deprecations based on feedback
- Type safety improvements

**Preparation**:
- Review [API_STABILITY.md](API_STABILITY.md)
- Check for deprecation warnings in v0.3.x
- Join discussions about proposed changes

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
| v0.4.0 | v0.3.x | TBD | API stability finalization |

---

## See Also

- [CHANGELOG.md](CHANGELOG.md) - Complete version history
- [API_STABILITY.md](API_STABILITY.md) - API stability guarantees
- [DEPRECATION_POLICY.md](DEPRECATION_POLICY.md) - Deprecation process
- [README.md](README.md) - Getting started guide

---

**Last Updated**: 2025-10-08
**Next Update**: With v0.4.0 release
