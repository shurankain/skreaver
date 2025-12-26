# Migration Guide for Skreaver Core 0.6.0

This document outlines breaking changes and migration steps for upgrading to v0.6.0.

## IdValidationError Removal (LOW-4)

**Status:** Deprecated in v0.5.0, will be **REMOVED** in v0.6.0

**Replacement:** Use `ValidationError` from `crate::validation` module instead.

### What's Changing

The `IdValidationError` type is being removed to reduce duplication and simplify the codebase. All functionality is available through the more flexible `ValidationError` type.

### Migration Steps

#### 1. Update imports

```rust
// Before (v0.5.0)
use skreaver_core::IdValidationError;

// After (v0.6.0)
use skreaver_core::validation::ValidationError;
```

#### 2. Update function signatures

```rust
// Before (v0.5.0)
fn validate_id(id: &str) -> Result<String, IdValidationError> {
    // ...
}

// After (v0.6.0)
fn validate_id(id: &str) -> Result<String, ValidationError> {
    // ...
}
```

#### 3. Update error matching

The error variants are identical, so only the type name changes:

```rust
// Before (v0.5.0)
match result {
    Err(IdValidationError::Empty) => { /* handle */ }
    Err(IdValidationError::TooLong { length, max }) => { /* handle */ }
    Ok(value) => { /* handle */ }
}

// After (v0.6.0)
match result {
    Err(ValidationError::Empty) => { /* handle */ }
    Err(ValidationError::TooLong { length, max }) => { /* handle */ }
    Ok(value) => { /* handle */ }
}
```

#### 4. Update error conversions

If you have custom error types that convert from `IdValidationError`:

```rust
// Before (v0.5.0)
impl From<IdValidationError> for MyError {
    fn from(err: IdValidationError) -> Self {
        match err {
            IdValidationError::Empty => MyError::InvalidId("empty"),
            // ...
        }
    }
}

// After (v0.6.0)
impl From<ValidationError> for MyError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::Empty => MyError::InvalidId("empty"),
            // ...
        }
    }
}
```

### Compatibility Notes

- **v0.5.0:** Both `IdValidationError` and `ValidationError` are available. `From` traits provide automatic conversion between them.
- **v0.6.0:** Only `ValidationError` will be available. Code must be migrated before upgrading.

### Automated Migration

For codebases with many usages, you can use `sed` or your editor's find-replace:

```bash
# Find all occurrences
rg "IdValidationError" --type rust

# Replace in files (verify first!)
find . -name "*.rs" -type f -exec sed -i '' 's/IdValidationError/ValidationError/g' {} +
find . -name "*.rs" -type f -exec sed -i '' 's/use skreaver_core::IdValidationError/use skreaver_core::validation::ValidationError/g' {} +
```

**Important:** Review all changes after automated replacement to ensure correctness.

### Timeline

- **v0.5.0** (Current): `IdValidationError` deprecated with warnings
- **v0.6.0** (Target): `IdValidationError` removed entirely

### Need Help?

If you encounter issues during migration:
1. Check the type documentation in `crate::validation`
2. Review the examples in `crate::identifiers::validation`
3. Open an issue at https://github.com/shurankain/skreaver/issues

---

**Note:** This migration affects public API. Ensure all downstream dependencies are updated before releasing v0.6.0.
