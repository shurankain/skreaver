//! Non-empty queue implementation for tool execution.
//!
//! `NonEmptyQueue<T>` is a FIFO queue that is guaranteed to contain at least one element.
//! This is particularly useful for tool execution pipelines where we need to ensure
//! there is always at least one tool to execute.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;

/// A FIFO queue that is guaranteed to contain at least one element.
///
/// `NonEmptyQueue<T>` provides queue operations (enqueue/dequeue) while maintaining
/// the invariant that the queue is never empty. This makes it ideal for tool execution
/// pipelines where we need compile-time guarantees about the presence of tools.
///
/// # Example
///
/// ```rust
/// use skreaver_core::collections::NonEmptyQueue;
///
/// // Construction requires at least one element
/// let mut queue = NonEmptyQueue::new("first", vec!["second", "third"]);
/// assert_eq!(queue.len(), 3);
///
/// // Dequeue removes elements until only one remains
/// assert_eq!(queue.dequeue(), Some("first"));
/// assert_eq!(queue.dequeue(), Some("second"));
/// assert_eq!(queue.dequeue(), None); // Can't remove the last element
/// assert_eq!(queue.peek(), &"third");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonEmptyQueue<T> {
    /// The first element in the queue (guaranteed to exist)
    head: T,
    /// Remaining elements in FIFO order
    tail: VecDeque<T>,
}

impl<T> NonEmptyQueue<T> {
    /// Create a new non-empty queue with a head element and optional tail elements.
    ///
    /// # Parameters
    ///
    /// * `head` - The first element in the queue
    /// * `tail` - Additional elements to add to the queue
    ///
    /// # Returns
    ///
    /// A new `NonEmptyQueue<T>` containing at least the head element
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::collections::NonEmptyQueue;
    ///
    /// let queue = NonEmptyQueue::new(1, vec![2, 3]);
    /// assert_eq!(queue.len(), 3);
    /// ```
    pub fn new(head: T, tail: Vec<T>) -> Self {
        NonEmptyQueue {
            head,
            tail: VecDeque::from(tail),
        }
    }

    /// Create a non-empty queue with a single element.
    ///
    /// # Parameters
    ///
    /// * `value` - The single element to store
    ///
    /// # Returns
    ///
    /// A new `NonEmptyQueue<T>` containing only the provided element
    ///
    /// # Example
    ///
    /// ```rust
    /// use skreaver_core::collections::NonEmptyQueue;
    ///
    /// let queue = NonEmptyQueue::singleton(42);
    /// assert_eq!(queue.len(), 1);
    /// assert_eq!(queue.peek(), &42);
    /// ```
    pub fn singleton(value: T) -> Self {
        NonEmptyQueue {
            head: value,
            tail: VecDeque::new(),
        }
    }

    /// Get a reference to the front element without removing it.
    ///
    /// This operation is guaranteed to succeed since the queue is non-empty.
    ///
    /// # Returns
    ///
    /// A reference to the front element
    pub fn peek(&self) -> &T {
        &self.head
    }

    /// Get a mutable reference to the front element.
    ///
    /// # Returns
    ///
    /// A mutable reference to the front element
    pub fn peek_mut(&mut self) -> &mut T {
        &mut self.head
    }

    /// Add an element to the back of the queue.
    ///
    /// # Parameters
    ///
    /// * `value` - The element to add
    pub fn enqueue(&mut self, value: T) {
        self.tail.push_back(value);
    }

    /// Remove and return the front element if the queue has more than one element.
    ///
    /// Returns `None` if the queue contains only the head element, as removing
    /// it would violate the non-empty guarantee.
    ///
    /// # Returns
    ///
    /// `Some(T)` if there are tail elements, `None` if only the head remains
    pub fn dequeue(&mut self) -> Option<T> {
        if self.tail.is_empty() {
            None
        } else {
            let old_head = std::mem::replace(&mut self.head, self.tail.pop_front().unwrap());
            Some(old_head)
        }
    }

    /// Get the number of elements in the queue.
    ///
    /// This is always at least 1.
    ///
    /// # Returns
    ///
    /// The total number of elements
    ///
    /// # Note
    ///
    /// `is_empty()` is not provided because NonEmptyQueue is guaranteed to never be empty.
    /// Use `is_singleton()` to check if the queue contains exactly one element.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        1 + self.tail.len()
    }

    /// Check if the queue contains only one element.
    ///
    /// # Returns
    ///
    /// `true` if the queue contains only the head element
    pub fn is_singleton(&self) -> bool {
        self.tail.is_empty()
    }

    /// Get an iterator over references to the elements in queue order.
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

    /// Convert the queue into a `Vec<T>` in queue order.
    ///
    /// # Returns
    ///
    /// A `Vec<T>` containing all elements in FIFO order
    pub fn into_vec(self) -> Vec<T> {
        let mut vec = Vec::with_capacity(1 + self.tail.len());
        vec.push(self.head);
        vec.extend(self.tail);
        vec
    }

    /// Convert the queue into a `VecDeque<T>`.
    ///
    /// # Returns
    ///
    /// A `VecDeque<T>` containing all elements in FIFO order
    pub fn into_deque(self) -> VecDeque<T> {
        let mut deque = VecDeque::with_capacity(1 + self.tail.len());
        deque.push_back(self.head);
        deque.extend(self.tail);
        deque
    }

    /// Get a reference to the element at the given index.
    ///
    /// Index 0 refers to the front of the queue.
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

    /// Get the front element (alias for `peek`).
    ///
    /// # Returns
    ///
    /// A reference to the front element
    pub fn front(&self) -> &T {
        &self.head
    }

    /// Get the back element.
    ///
    /// # Returns
    ///
    /// A reference to the back element
    pub fn back(&self) -> &T {
        self.tail.back().unwrap_or(&self.head)
    }
}

impl<T> TryFrom<Vec<T>> for NonEmptyQueue<T> {
    type Error = EmptyQueueError;

    /// Try to convert a `Vec<T>` into a `NonEmptyQueue<T>`.
    ///
    /// # Returns
    ///
    /// `Ok(NonEmptyQueue<T>)` if the vector is non-empty, `Err(EmptyQueueError)` otherwise
    fn try_from(mut vec: Vec<T>) -> Result<Self, Self::Error> {
        if vec.is_empty() {
            Err(EmptyQueueError)
        } else {
            let head = vec.remove(0);
            Ok(NonEmptyQueue {
                head,
                tail: VecDeque::from(vec),
            })
        }
    }
}

impl<T> TryFrom<VecDeque<T>> for NonEmptyQueue<T> {
    type Error = EmptyQueueError;

    /// Try to convert a `VecDeque<T>` into a `NonEmptyQueue<T>`.
    ///
    /// # Returns
    ///
    /// `Ok(NonEmptyQueue<T>)` if the deque is non-empty, `Err(EmptyQueueError)` otherwise
    fn try_from(mut deque: VecDeque<T>) -> Result<Self, Self::Error> {
        if let Some(head) = deque.pop_front() {
            Ok(NonEmptyQueue { head, tail: deque })
        } else {
            Err(EmptyQueueError)
        }
    }
}

impl<T> From<NonEmptyQueue<T>> for Vec<T> {
    fn from(queue: NonEmptyQueue<T>) -> Self {
        queue.into_vec()
    }
}

impl<T> From<NonEmptyQueue<T>> for VecDeque<T> {
    fn from(queue: NonEmptyQueue<T>) -> Self {
        queue.into_deque()
    }
}

impl<T> IntoIterator for NonEmptyQueue<T> {
    type Item = T;
    type IntoIter = std::iter::Chain<std::iter::Once<T>, std::collections::vec_deque::IntoIter<T>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.head).chain(self.tail)
    }
}

impl<'a, T> IntoIterator for &'a NonEmptyQueue<T> {
    type Item = &'a T;
    type IntoIter =
        std::iter::Chain<std::iter::Once<&'a T>, std::collections::vec_deque::Iter<'a, T>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(&self.head).chain(self.tail.iter())
    }
}

impl<T: fmt::Display> fmt::Display for NonEmptyQueue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Queue[{}", self.head)?;
        for item in &self.tail {
            write!(f, ", {}", item)?;
        }
        write!(f, "]")
    }
}

/// Error type for attempting to create a `NonEmptyQueue` from an empty collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmptyQueueError;

impl fmt::Display for EmptyQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cannot create NonEmptyQueue from empty collection")
    }
}

impl std::error::Error for EmptyQueueError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_nonempty_queue() {
        let queue = NonEmptyQueue::new(1, vec![2, 3]);
        assert_eq!(queue.peek(), &1);
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn singleton_creates_single_element() {
        let queue = NonEmptyQueue::singleton(42);
        assert_eq!(queue.peek(), &42);
        assert_eq!(queue.len(), 1);
        assert!(queue.is_singleton());
    }

    #[test]
    fn enqueue_adds_to_back() {
        let mut queue = NonEmptyQueue::singleton(1);
        queue.enqueue(2);
        queue.enqueue(3);
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.peek(), &1);
        assert_eq!(queue.back(), &3);
    }

    #[test]
    fn dequeue_removes_from_front() {
        let mut queue = NonEmptyQueue::new(1, vec![2, 3]);
        assert_eq!(queue.dequeue(), Some(1));
        assert_eq!(queue.peek(), &2);
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.peek(), &3);
        assert_eq!(queue.dequeue(), None); // Can't remove last element
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn iter_iterates_in_queue_order() {
        let queue = NonEmptyQueue::new(1, vec![2, 3]);
        let collected: Vec<_> = queue.iter().copied().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn into_vec_converts_correctly() {
        let queue = NonEmptyQueue::new(1, vec![2, 3]);
        let vec: Vec<_> = queue.into_vec();
        assert_eq!(vec, vec![1, 2, 3]);
    }

    #[test]
    fn try_from_vec_succeeds_for_nonempty() {
        let vec = vec![1, 2, 3];
        let queue = NonEmptyQueue::try_from(vec).unwrap();
        assert_eq!(queue.peek(), &1);
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn try_from_vec_fails_for_empty() {
        let vec: Vec<i32> = vec![];
        let result = NonEmptyQueue::try_from(vec);
        assert!(result.is_err());
    }

    #[test]
    fn try_from_deque_succeeds_for_nonempty() {
        let mut deque = VecDeque::new();
        deque.push_back(1);
        deque.push_back(2);
        let queue = NonEmptyQueue::try_from(deque).unwrap();
        assert_eq!(queue.peek(), &1);
    }

    #[test]
    fn try_from_deque_fails_for_empty() {
        let deque: VecDeque<i32> = VecDeque::new();
        let result = NonEmptyQueue::try_from(deque);
        assert!(result.is_err());
    }

    #[test]
    fn get_returns_correct_elements() {
        let queue = NonEmptyQueue::new(1, vec![2, 3]);
        assert_eq!(queue.get(0), Some(&1));
        assert_eq!(queue.get(1), Some(&2));
        assert_eq!(queue.get(2), Some(&3));
        assert_eq!(queue.get(3), None);
    }

    #[test]
    fn front_and_back_work_correctly() {
        let queue = NonEmptyQueue::new(1, vec![2, 3]);
        assert_eq!(queue.front(), &1);
        assert_eq!(queue.back(), &3);

        let single = NonEmptyQueue::singleton(42);
        assert_eq!(single.front(), &42);
        assert_eq!(single.back(), &42);
    }

    #[test]
    fn fifo_behavior() {
        let mut queue = NonEmptyQueue::singleton(1);
        queue.enqueue(2);
        queue.enqueue(3);

        assert_eq!(queue.dequeue(), Some(1));
        queue.enqueue(4);
        assert_eq!(queue.dequeue(), Some(2));
        assert_eq!(queue.dequeue(), Some(3));
        assert_eq!(queue.peek(), &4);
    }
}
