# Skreaver Deprecation Policy

> **Version**: 1.0
> **Effective Date**: 2025-10-08
> **Current Release**: v0.5.0

---

## Overview

This document defines how Skreaver manages API deprecations to provide a smooth upgrade path for users while allowing the framework to evolve.

---

## Table of Contents

- [Principles](#principles)
- [Deprecation Process](#deprecation-process)
- [Timeline Requirements](#timeline-requirements)
- [Deprecation Syntax](#deprecation-syntax)
- [Migration Support](#migration-support)
- [Current Deprecations](#current-deprecations)
- [Examples](#examples)

---

## Principles

### Core Values

1. **User-First**: Never break user code without warning and migration path
2. **Transparency**: Clear communication about what's deprecated and why
3. **Gradual Transition**: Provide ample time for migration
4. **Complete Information**: Always provide replacement APIs and migration guides

### Deprecation vs. Breaking Changes

| Approach | Definition | Timeline | User Impact |
|----------|------------|----------|-------------|
| **Deprecation** | Mark API as obsolete but keep it working | 1+ minor versions | Warnings, code still works |
| **Breaking Change** | Remove or change API incompatibly | Next major version | Code stops compiling |

**Our Approach**: Always deprecate before removing (except for internal APIs)

---

## Deprecation Process

### Step 1: Proposal & Review

**Who**: Maintainers and community
**When**: Before deprecation

**Actions**:
1. Open GitHub issue with deprecation proposal
2. Document:
   - What is being deprecated
   - Why it's being deprecated
   - What replaces it
   - Migration path
3. Gather community feedback
4. Approve or revise proposal

**Template**:
```markdown
## Deprecation Proposal: [API Name]

### What
`function_name()` in `skreaver::module`

### Why
- Performance issues with current implementation
- Better alternative exists: `new_function_name()`
- Design inconsistency with rest of API

### Replacement
Use `new_function_name()` instead

### Migration
```rust
// Old
let result = function_name(arg);

// New
let result = new_function_name(arg);
```

### Timeline
- Deprecate in: v0.4.0
- Remove in: v0.5.0 (1 minor version grace period)
```

### Step 2: Implementation

**Who**: Maintainers
**When**: Next minor release

**Actions**:
1. Add `#[deprecated]` attribute
2. Update rustdoc documentation
3. Add migration guide to docs
4. Update CHANGELOG.md
5. Update MIGRATION.md

**Code changes**:
```rust
#[deprecated(since = "0.4.0", note = "Use `new_api()` instead. See MIGRATION.md for details.")]
pub fn old_api() -> Result<(), Error> {
    // Keep implementation working
}
```

### Step 3: Communication

**Who**: Maintainers
**When**: With release

**Actions**:
1. **CHANGELOG.md** entry:
   ```markdown
   ### Deprecated
   - `old_api()`: Deprecated in favor of `new_api()`.
     Will be removed in v0.5.0. See MIGRATION.md.
   ```

2. **Release notes**: Highlight deprecations prominently

3. **Migration guide**: Add detailed migration instructions

4. **Community announcement**: Blog post or discussion thread

### Step 4: Monitoring

**Who**: Maintainers & Community
**When**: During grace period

**Actions**:
1. Monitor for migration issues
2. Provide support for migrations
3. Update migration guide based on feedback
4. Consider extending grace period if needed

### Step 5: Removal

**Who**: Maintainers
**When**: After grace period

**Actions**:
1. Remove deprecated API completely
2. Update documentation
3. Update CHANGELOG.md with removal notice
4. Update MIGRATION.md
5. Bump version appropriately (major for stable API)

---

## Timeline Requirements

### Pre-1.0 (Current)

**Format**: `0.MINOR.PATCH`

| Stability Level | Deprecation Period | Removal Version |
|----------------|-------------------|-----------------|
| **Stable API** (from `skreaver`) | Minimum 1 minor version | Next minor version |
| **Unstable Feature** (`unstable-*`) | No grace period required | Any version |
| **Internal API** (direct crate imports) | No grace period required | Any version |

**Example**:
- v0.4.0: Deprecate `old_api()`
- v0.4.x: Grace period (warnings only)
- v0.5.0: Remove `old_api()`

### Post-1.0 (Future)

**Format**: `MAJOR.MINOR.PATCH`

| Stability Level | Deprecation Period | Removal Version |
|----------------|-------------------|-----------------|
| **Stable API** | Minimum 1 major version OR 6 months | Next major version |
| **Unstable Feature** | Minimum 1 minor version | Next major version |
| **Internal API** | No grace period required | Any version |

**Example**:
- v1.0.0: Deprecate `old_api()`
- v1.x.x: Grace period (minimum 6 months)
- v2.0.0: Remove `old_api()`

---

## Deprecation Syntax

### Function Deprecation

```rust
#[deprecated(
    since = "0.4.0",
    note = "Use `execute_tool_typed()` instead. See MIGRATION.md for details."
)]
pub fn execute_tool_by_name(&self, name: &str) -> Result<String, Error> {
    // Implementation continues to work
}
```

### Struct/Type Deprecation

```rust
#[deprecated(
    since = "0.4.0",
    note = "Use `ToolDispatch` for type-safe tool selection"
)]
pub struct ToolName(String);
```

### Module Deprecation

```rust
#[deprecated(since = "0.4.0", note = "Use `skreaver::security` module instead")]
pub mod old_security {
    // Re-export new types for compatibility
    pub use crate::security::*;
}
```

### Trait Method Deprecation

```rust
pub trait Tool {
    #[deprecated(since = "0.4.0", note = "Implement `call_validated()` instead")]
    fn call(&self, input: String) -> ExecutionResult {
        // Default implementation for backwards compatibility
        self.call_validated(ValidatedInput::new(input).unwrap_or_default())
    }

    fn call_validated(&self, input: ValidatedInput) -> ExecutionResult;
}
```

### Feature Flag Deprecation

```toml
# Cargo.toml
[features]
# Deprecated features
old-feature = []  # Deprecated in 0.4.0, use 'new-feature' instead
```

```rust
// src/lib.rs
#[cfg(feature = "old-feature")]
compile_error!(
    "Feature 'old-feature' is deprecated in v0.4.0. \
     Use 'new-feature' instead. \
     See MIGRATION.md for details."
);
```

---

## Migration Support

### Documentation Requirements

Every deprecation MUST include:

1. **Rustdoc Warning**:
   ```rust
   /// Execute a tool by name
   ///
   /// # Deprecated
   /// This function is deprecated in favor of [`execute_tool_typed`].
   /// It will be removed in v0.5.0.
   ///
   /// ## Migration
   /// ```rust
   /// // Old
   /// coordinator.execute_tool_by_name("http_get", input)?;
   ///
   /// // New
   /// let tool = ToolDispatch::from_name("http_get")?;
   /// coordinator.execute_tool_typed(tool, input)?;
   /// ```
   #[deprecated(since = "0.4.0", note = "Use execute_tool_typed() instead")]
   pub fn execute_tool_by_name(&self, name: &str) -> Result<String, Error> {
       // ...
   }
   ```

2. **CHANGELOG.md Entry**:
   ```markdown
   ### Deprecated

   - `Coordinator::execute_tool_by_name()`: Deprecated in favor of type-safe
     `execute_tool_typed()`. Will be removed in v0.5.0.

     **Migration**: See MIGRATION.md for code examples.
   ```

3. **MIGRATION.md Section**:
   ```markdown
   ## Deprecation: execute_tool_by_name()

   **Deprecated in**: v0.4.0
   **Removed in**: v0.5.0
   **Reason**: Type safety - prevents runtime errors from invalid tool names

   ### Before
   ```rust
   coordinator.execute_tool_by_name("http_get", "https://api.example.com")?;
   ```

   ### After
   ```rust
   let tool = ToolDispatch::from_name("http_get")?;
   coordinator.execute_tool_typed(tool, "https://api.example.com")?;
   ```

   ### Rationale
   The new API catches invalid tool names at compile time or explicitly at
   `from_name()` call site, preventing silent failures later in execution.
   ```

### Migration Tools

We provide:
- **Automated migration helpers**: When possible, provide conversion functions
- **Compatibility shims**: Wrapper functions that call new API
- **Example code**: Before/after examples in documentation
- **Migration scripts**: For complex migrations (when feasible)

---

## Current Deprecations

**As of v0.5.0**: No deprecated APIs

All APIs in v0.5.0 are current. This section will be updated as deprecations are introduced.

---

## Examples

### Example 1: Function Rename

**Scenario**: Rename function for clarity

**Implementation**:
```rust
// In v0.4.0 - Deprecate old name
#[deprecated(since = "0.4.0", note = "Renamed to `create_snapshot_async()`")]
pub fn create_snapshot(&self) -> Result<Snapshot, Error> {
    self.create_snapshot_async()
}

// New name
pub fn create_snapshot_async(&self) -> Result<Snapshot, Error> {
    // Implementation
}
```

**Documentation**:
```markdown
### CHANGELOG.md (v0.4.0)
#### Deprecated
- `Memory::create_snapshot()`: Renamed to `create_snapshot_async()`
  for clarity. Will be removed in v0.5.0.

### MIGRATION.md
## create_snapshot() → create_snapshot_async()

Simply rename the method call:
```rust
// Before
let snapshot = memory.create_snapshot()?;

// After
let snapshot = memory.create_snapshot_async()?;
```
```

### Example 2: API Redesign

**Scenario**: Change function signature for type safety

**Implementation**:
```rust
// In v0.4.0 - Deprecate old API
#[deprecated(
    since = "0.4.0",
    note = "Use `execute_tool(ToolDispatch, input)` for type safety"
)]
pub fn execute_tool_by_name(&self, name: &str, input: String)
    -> Result<String, Error>
{
    // Bridge to new API for compatibility
    let tool = ToolDispatch::from_name(name)
        .map_err(|e| Error::ToolNotFound(name.to_string()))?;
    self.execute_tool(tool, input)
}

// New API
pub fn execute_tool(&self, tool: ToolDispatch, input: String)
    -> Result<String, Error>
{
    // New implementation
}
```

**Documentation**: Full migration guide in MIGRATION.md with rationale

### Example 3: Struct Replacement

**Scenario**: Replace struct with better design

**Implementation**:
```rust
// In v0.4.0 - Deprecate old struct
#[deprecated(since = "0.4.0", note = "Use `SecurityConfig` instead")]
pub struct Config {
    // Keep fields for backwards compatibility
}

impl Config {
    /// Convert to new SecurityConfig
    pub fn into_security_config(self) -> SecurityConfig {
        SecurityConfig {
            // Map fields
        }
    }
}

// New struct
pub struct SecurityConfig {
    // Improved design
}
```

---

## Enforcement

### CI Checks

Our CI pipeline:
1. **Runs with `RUSTFLAGS="-D warnings"`**: Deprecation warnings become errors in CI
2. **cargo-semver-checks**: Detects accidental removals without deprecation
3. **Documentation tests**: Ensures migration examples stay valid

### Review Process

All PRs that deprecate APIs must:
1. Include deprecation attribute with version and note
2. Update CHANGELOG.md
3. Add migration guide to MIGRATION.md or inline docs
4. Include rationale for deprecation
5. Provide migration time estimate
6. Get approval from maintainers

---

## Exceptions

### Emergency Deprecations

For critical security issues:
- May deprecate and remove in same version
- Must provide immediate migration path
- Announced prominently in security advisory

### Unstable Features

Features marked `unstable-*`:
- No deprecation period required
- May be removed without warning
- Users accept this when enabling the feature

### Internal APIs

Direct crate imports (`skreaver_core::*`):
- No deprecation period required
- Subject to change at any time
- Not covered by this policy

---

## Feedback

### Questions About Deprecations

If a deprecation:
- Is unclear
- Lacks migration information
- Causes significant problems
- Needs timeline extension

Please:
1. File an issue at https://github.com/shurankain/skreaver/issues
2. Tag with `deprecation` label
3. Provide specific use case details

### Requesting Deprecation Extensions

If you need more time to migrate:
1. Open an issue before removal version is released
2. Explain what blockers exist
3. Estimate time needed
4. We'll consider extending grace period

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2025-10-08 | Initial deprecation policy |

---

## See Also

- [API_STABILITY.md](API_STABILITY.md) - API stability guarantees
- [CHANGELOG.md](CHANGELOG.md) - All changes including deprecations
- [MIGRATION.md](MIGRATION.md) - Migration guides (to be created)
- [Semantic Versioning 2.0.0](https://semver.org/) - Versioning standard

---

**Last Updated**: 2025-10-31
**Next Review**: Before v0.6.0 release
