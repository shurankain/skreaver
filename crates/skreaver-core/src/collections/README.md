# Non-Empty Collections

This module provides collection types that guarantee non-emptiness at compile time, making invalid states unrepresentable and improving API safety.

## Overview

The collections module introduces two key types:

- **`NonEmptyVec<T>`** - A vector guaranteed to contain at least one element
- **`NonEmptyQueue<T>`** - A FIFO queue guaranteed to contain at least one element

## Key Benefits

1. **Compile-Time Safety**: Invalid empty states are impossible to construct
2. **Zero-Cost Abstraction**: No runtime overhead over standard collections
3. **Ergonomic API**: Familiar methods from standard collections plus safe guarantees
4. **Type Safety**: APIs can require non-empty inputs without runtime checks

## Usage Examples

### NonEmptyVec

```rust
use skreaver_core::collections::NonEmptyVec;

// Create with head and tail
let vec = NonEmptyVec::new(1, vec![2, 3]);
assert_eq!(vec.head(), &1);
assert_eq!(vec.len(), 3);

// Create singleton
let single = NonEmptyVec::singleton(42);
assert!(single.is_singleton());

// Safe operations
let mut vec = NonEmptyVec::new(1, vec![2]);
vec.push(3);  // Always safe
vec.pop();    // Returns Option, protects last element
```

### NonEmptyQueue

```rust
use skreaver_core::collections::NonEmptyQueue;

// Create queue
let mut queue = NonEmptyQueue::new("task1", vec!["task2"]);

// FIFO operations
queue.enqueue("task3");
let first = queue.dequeue(); // Returns Some until only one remains
let last = queue.peek();     // Always returns a reference
```

### Type-Safe APIs

```rust
// Function requiring at least one element
fn process_batch(items: NonEmptyVec<String>) -> String {
    // Can safely access head without Option
    let first = items.head();
    format!("Processing {}", first)
}

// Tool execution pipeline
fn execute_tools(mut queue: NonEmptyQueue<Tool>) {
    loop {
        let tool = queue.peek();
        execute(tool);

        if queue.dequeue().is_none() {
            break;  // Last tool processed
        }
    }
}
```

### Conversion

```rust
// Try converting from standard collections
let vec = vec![1, 2, 3];
let non_empty = NonEmptyVec::try_from(vec)?;

// Convert back to standard collections
let regular_vec: Vec<_> = non_empty.into_vec();

// Using From trait
let queue: NonEmptyQueue<_> = NonEmptyQueue::new(1, vec![2, 3]);
let deque: VecDeque<_> = queue.into();
```

## Features

### NonEmptyVec

- `new(head, tail)` - Create from head and tail elements
- `singleton(value)` - Create single-element vector
- `head()` / `head_mut()` - Access first element (always succeeds)
- `tail()` / `tail_mut()` - Access remaining elements
- `push(value)` - Append element
- `pop()` - Remove last element (returns `None` if singleton)
- `iter()` / `iter_mut()` - Iterate over elements
- `get(index)` / `get_mut(index)` - Safe indexed access
- `first()` / `last()` - Access boundary elements
- Index operator `[usize]` - Panicking indexed access

### NonEmptyQueue

- `new(head, tail)` - Create from head and tail elements
- `singleton(value)` - Create single-element queue
- `peek()` / `peek_mut()` - Access front element (always succeeds)
- `enqueue(value)` - Add element to back
- `dequeue()` - Remove front element (returns `None` if singleton)
- `iter()` / `iter_mut()` - Iterate in queue order
- `front()` / `back()` - Access boundary elements
- `get(index)` - Safe indexed access

### Shared Features

- Serde support (`Serialize`, `Deserialize`)
- `TryFrom<Vec<T>>` / `TryFrom<VecDeque<T>>` conversions
- `Into<Vec<T>>` / `Into<VecDeque<T>>` conversions
- `IntoIterator` implementations
- `Display` formatting
- `Clone`, `Debug`, `PartialEq`, `Eq`, `Hash` derives

## Design Decisions

### Why not use the `nonempty` crate?

We implemented custom types for:
1. Full control over API surface and behavior
2. Integration with serde without feature gates
3. Specific optimizations for our use cases
4. Consistency with project patterns

### API Design

- **Protection of Invariants**: Operations like `pop()` and `dequeue()` return `Option` rather than panicking, preserving the non-empty guarantee
- **Ergonomic Accessors**: `head()`, `peek()`, `first()`, `last()` always succeed and return references
- **Familiar Interface**: Methods mirror standard library collections where possible
- **Zero Unsafe Code**: Implementation uses safe Rust exclusively

## Use Cases in Skreaver

1. **Tool Execution Pipelines**: Ensure at least one tool is always queued
2. **Agent Configurations**: Require at least one configuration option
3. **Batch Processing**: Guarantee non-empty batches for processing
4. **Type-Safe APIs**: Express requirements in types rather than runtime checks

## Performance

Both types are zero-cost abstractions:
- `NonEmptyVec<T>` wraps `(T, Vec<T>)` - same size as `Vec<T>` for non-empty cases
- `NonEmptyQueue<T>` wraps `(T, VecDeque<T>)` - same performance as `VecDeque<T>`
- No heap allocation overhead beyond the underlying collection
- All operations have the same complexity as the standard collections

## Error Handling

Attempting to create from empty collections returns typed errors:
- `EmptyVecError` - Cannot create `NonEmptyVec` from empty `Vec`
- `EmptyQueueError` - Cannot create `NonEmptyQueue` from empty collection

## Testing

The module includes comprehensive tests covering:
- Creation and construction methods
- Mutation operations (push, pop, enqueue, dequeue)
- Iteration and conversion
- Boundary conditions (singleton cases)
- Error cases (empty collection conversion)
- FIFO behavior for queues

## Example

See `examples/nonempty_collections.rs` for a complete demonstration of all features.

## Future Enhancements

Potential additions for future versions:
- Additional collection types (NonEmptySet, NonEmptyMap)
- More efficient specialized operations
- Additional trait implementations (FromIterator, Extend)
- Iterator adaptors preserving non-emptiness