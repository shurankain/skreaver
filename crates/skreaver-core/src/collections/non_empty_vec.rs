//! Non-empty vector implementation.
//!
//! `NonEmptyVec<T>` is a vector that is guaranteed to contain at least one element.
//! This provides compile-time safety for operations that require non-empty collections.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Index;

/// A vector that is guaranteed to contain at least one element.
///
/// `NonEmptyVec<T>` wraps a regular `Vec<T>` but ensures it is never empty by
/// requiring at least one element at construction time. This makes invalid
/// states unrepresentable and eliminates the need for runtime checks.
///
/// # Example
///
/// ```rust
/// use skreaver_core::collections::NonEmptyVec;
///
/// // Construction requires at least one element
/// let vec = NonEmptyVec::new(1, vec![2, 3]);
/// assert_eq!(vec.head(), &1);
/// assert_eq!(vec.tail(), &[2, 3]);
/// assert_eq!(vec.len(), 3);
///
/// // Can also construct from a single element
/// let single = NonEmptyVec::singleton(42);
/// assert_eq!(single.head(), &42);
/// assert!(single.tail().is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NonEmptyVec<T> {
    head: T,
    tail: Vec<T>,
}

impl<T> NonEmptyVec<T> {
    /// Create a new non-empty vector with a head element and optional tail elements.
    ///
    /// # Parameters
    ///
    /// * `head` - The first element (guaranteed to exist)
    /// * `tail` - Additional elements to append after the head
    ///
    /// # Returns
    ///
    /// A new `NonEmptyVec<T>` containing at least the head element
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::collections::NonEmptyVec;
    ///
    /// let vec = NonEmptyVec::new("first", vec!["second", "third"]);
    /// assert_eq!(vec.len(), 3);
    /// ```
    pub fn new(head: T, tail: Vec<T>) -> Self {
        NonEmptyVec { head, tail }
    }

    /// Create a non-empty vector with a single element.
    ///
    /// # Parameters
    ///
    /// * `value` - The single element to store
    ///
    /// # Returns
    ///
    /// A new `NonEmptyVec<T>` containing only the provided element
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::collections::NonEmptyVec;
    ///
    /// let vec = NonEmptyVec::singleton(42);
    /// assert_eq!(vec.len(), 1);
    /// assert_eq!(vec.head(), &42);
    /// ```
    pub fn singleton(value: T) -> Self {
        NonEmptyVec {
            head: value,
            tail: Vec::new(),
        }
    }

    /// Get a reference to the first element.
    ///
    /// This operation is guaranteed to succeed since the vector is non-empty.
    ///
    /// # Returns
    ///
    /// A reference to the head element
    pub fn head(&self) -> &T {
        &self.head
    }

    /// Get a mutable reference to the first element.
    ///
    /// # Returns
    ///
    /// A mutable reference to the head element
    pub fn head_mut(&mut self) -> &mut T {
        &mut self.head
    }

    /// Get a slice of all elements after the head.
    ///
    /// # Returns
    ///
    /// A slice containing all tail elements (may be empty)
    pub fn tail(&self) -> &[T] {
        &self.tail
    }

    /// Get a mutable slice of all elements after the head.
    ///
    /// # Returns
    ///
    /// A mutable slice containing all tail elements
    pub fn tail_mut(&mut self) -> &mut [T] {
        &mut self.tail
    }

    /// Get the number of elements in the vector.
    ///
    /// This is always at least 1.
    ///
    /// # Returns
    ///
    /// The total number of elements (head + tail)
    ///
    /// # Note
    ///
    /// `is_empty()` is not provided because NonEmptyVec is guaranteed to never be empty.
    /// Use `is_singleton()` to check if the vector contains exactly one element.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        1 + self.tail.len()
    }

    /// Check if the vector contains only one element.
    ///
    /// # Returns
    ///
    /// `true` if the vector contains only the head element
    pub fn is_singleton(&self) -> bool {
        self.tail.is_empty()
    }

    /// Append an element to the end of the vector.
    ///
    /// # Parameters
    ///
    /// * `value` - The element to append
    pub fn push(&mut self, value: T) {
        self.tail.push(value);
    }

    /// Remove and return the last element if the vector has more than one element.
    ///
    /// Returns `None` if the vector only contains the head element, as removing
    /// it would violate the non-empty guarantee.
    ///
    /// # Returns
    ///
    /// `Some(T)` if there are tail elements, `None` if only the head remains
    pub fn pop(&mut self) -> Option<T> {
        self.tail.pop()
    }

    /// Get an iterator over references to the elements.
    ///
    /// # Returns
    ///
    /// An iterator yielding references to all elements
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.head).chain(self.tail.iter())
    }

    /// Get an iterator over mutable references to the elements.
    ///
    /// # Returns
    ///
    /// An iterator yielding mutable references to all elements
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        std::iter::once(&mut self.head).chain(self.tail.iter_mut())
    }

    /// Convert the non-empty vector into a regular `Vec<T>`.
    ///
    /// # Returns
    ///
    /// A `Vec<T>` containing all elements
    pub fn into_vec(self) -> Vec<T> {
        let mut vec = Vec::with_capacity(1 + self.tail.len());
        vec.push(self.head);
        vec.extend(self.tail);
        vec
    }

    /// Get a reference to an element at the given index.
    ///
    /// # Parameters
    ///
    /// * `index` - The index to access
    ///
    /// # Returns
    ///
    /// `Some(&T)` if the index is valid, `None` otherwise
    pub fn get(&self, index: usize) -> Option<&T> {
        if index == 0 {
            Some(&self.head)
        } else {
            self.tail.get(index - 1)
        }
    }

    /// Get a mutable reference to an element at the given index.
    ///
    /// # Parameters
    ///
    /// * `index` - The index to access
    ///
    /// # Returns
    ///
    /// `Some(&mut T)` if the index is valid, `None` otherwise
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index == 0 {
            Some(&mut self.head)
        } else {
            self.tail.get_mut(index - 1)
        }
    }

    /// Get a reference to the first element (alias for `head`).
    ///
    /// Provided for consistency with standard `Vec` API.
    ///
    /// # Returns
    ///
    /// A reference to the first element
    pub fn first(&self) -> &T {
        &self.head
    }

    /// Get a reference to the last element.
    ///
    /// # Returns
    ///
    /// A reference to the last element
    pub fn last(&self) -> &T {
        self.tail.last().unwrap_or(&self.head)
    }

    /// Get a mutable reference to the last element.
    ///
    /// # Returns
    ///
    /// A mutable reference to the last element
    pub fn last_mut(&mut self) -> &mut T {
        // SAFETY: tail.last_mut() returns None only when tail is empty,
        // in which case we return head instead
        self.tail.last_mut().unwrap_or(&mut self.head)
    }
}

impl<T> TryFrom<Vec<T>> for NonEmptyVec<T> {
    type Error = EmptyVecError;

    /// Try to convert a `Vec<T>` into a `NonEmptyVec<T>`.
    ///
    /// # Returns
    ///
    /// `Ok(NonEmptyVec<T>)` if the vector is non-empty, `Err(EmptyVecError)` otherwise
    fn try_from(mut vec: Vec<T>) -> Result<Self, Self::Error> {
        if vec.is_empty() {
            Err(EmptyVecError)
        } else {
            let head = vec.remove(0);
            Ok(NonEmptyVec { head, tail: vec })
        }
    }
}

impl<T> From<NonEmptyVec<T>> for Vec<T> {
    fn from(non_empty: NonEmptyVec<T>) -> Self {
        non_empty.into_vec()
    }
}

impl<T> Index<usize> for NonEmptyVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("index out of bounds")
    }
}

impl<T> IntoIterator for NonEmptyVec<T> {
    type Item = T;
    type IntoIter = std::iter::Chain<std::iter::Once<T>, std::vec::IntoIter<T>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.head).chain(self.tail)
    }
}

impl<'a, T> IntoIterator for &'a NonEmptyVec<T> {
    type Item = &'a T;
    type IntoIter = std::iter::Chain<std::iter::Once<&'a T>, std::slice::Iter<'a, T>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.head).chain(self.tail.iter())
    }
}

impl<T: fmt::Display> fmt::Display for NonEmptyVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}", self.head)?;
        for item in &self.tail {
            write!(f, ", {}", item)?;
        }
        write!(f, "]")
    }
}

/// Error type for attempting to create a `NonEmptyVec` from an empty `Vec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyVecError;

impl fmt::Display for EmptyVecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cannot create NonEmptyVec from empty Vec")
    }
}

impl std::error::Error for EmptyVecError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_nonempty_vec() {
        let vec = NonEmptyVec::new(1, vec![2, 3]);
        assert_eq!(vec.head(), &1);
        assert_eq!(vec.tail(), &[2, 3]);
        assert_eq!(vec.len(), 3);
    }

    #[test]
    fn singleton_creates_single_element() {
        let vec = NonEmptyVec::singleton(42);
        assert_eq!(vec.head(), &42);
        assert!(vec.tail().is_empty());
        assert_eq!(vec.len(), 1);
        assert!(vec.is_singleton());
    }

    #[test]
    fn push_appends_elements() {
        let mut vec = NonEmptyVec::singleton(1);
        vec.push(2);
        vec.push(3);
        assert_eq!(vec.len(), 3);
        assert_eq!(vec.tail(), &[2, 3]);
    }

    #[test]
    fn pop_removes_tail_elements() {
        let mut vec = NonEmptyVec::new(1, vec![2, 3]);
        assert_eq!(vec.pop(), Some(3));
        assert_eq!(vec.pop(), Some(2));
        assert_eq!(vec.pop(), None); // Can't remove head
        assert_eq!(vec.len(), 1);
    }

    #[test]
    fn iter_iterates_all_elements() {
        let vec = NonEmptyVec::new(1, vec![2, 3]);
        let collected: Vec<_> = vec.iter().copied().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn into_vec_converts_correctly() {
        let vec = NonEmptyVec::new(1, vec![2, 3]);
        let regular_vec: Vec<_> = vec.into_vec();
        assert_eq!(regular_vec, vec![1, 2, 3]);
    }

    #[test]
    fn try_from_vec_succeeds_for_nonempty() {
        let vec = vec![1, 2, 3];
        let non_empty = NonEmptyVec::try_from(vec).unwrap();
        assert_eq!(non_empty.head(), &1);
        assert_eq!(non_empty.tail(), &[2, 3]);
    }

    #[test]
    fn try_from_vec_fails_for_empty() {
        let vec: Vec<i32> = vec![];
        let result = NonEmptyVec::try_from(vec);
        assert!(result.is_err());
    }

    #[test]
    fn get_returns_correct_elements() {
        let vec = NonEmptyVec::new(1, vec![2, 3]);
        assert_eq!(vec.get(0), Some(&1));
        assert_eq!(vec.get(1), Some(&2));
        assert_eq!(vec.get(2), Some(&3));
        assert_eq!(vec.get(3), None);
    }

    #[test]
    fn first_and_last_work_correctly() {
        let vec = NonEmptyVec::new(1, vec![2, 3]);
        assert_eq!(vec.first(), &1);
        assert_eq!(vec.last(), &3);

        let single = NonEmptyVec::singleton(42);
        assert_eq!(single.first(), &42);
        assert_eq!(single.last(), &42);
    }

    #[test]
    fn index_operator_works() {
        let vec = NonEmptyVec::new(10, vec![20, 30]);
        assert_eq!(vec[0], 10);
        assert_eq!(vec[1], 20);
        assert_eq!(vec[2], 30);
    }

    #[test]
    #[should_panic(expected = "index out of bounds")]
    fn index_operator_panics_on_invalid_index() {
        let vec = NonEmptyVec::singleton(1);
        let _ = vec[5];
    }
}
