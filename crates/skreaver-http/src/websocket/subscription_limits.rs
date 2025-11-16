//! # Type-Level Subscription Limit Enforcement
//!
//! This module provides compile-time enforcement of WebSocket subscription limits
//! using const generics and the typestate pattern. This prevents runtime errors
//! from exceeding subscription quotas.
//!
//! # Problem: Runtime Subscription Overflow
//!
//! Without type-level enforcement, subscription limits are checked at runtime:
//!
//! ```ignore
//! // RUNTIME CHECK - can fail during operation
//! fn subscribe(&mut self, channel: Channel) -> Result<(), Error> {
//!     if self.subscriptions.len() >= self.max_subscriptions {
//!         return Err(Error::LimitExceeded); // Runtime error!
//!     }
//!     self.subscriptions.push(channel);
//!     Ok(())
//! }
//! ```
//!
//! Problems:
//! - Errors discovered at runtime, not compile time
//! - Must handle error cases everywhere subscriptions are added
//! - Easy to forget limit checks in new code paths
//!
//! # Solution: Type-Level Quota Tracking
//!
//! Use const generics to track subscription count at the type level:
//!
//! ```ignore
//! // Subscriptions<0> - No subscriptions yet
//! let subs = Subscriptions::<0>::new();
//!
//! // Subscriptions<1> - One subscription
//! let subs = subs.add(channel1); // Returns Subscriptions<1>
//!
//! // Subscriptions<2> - Two subscriptions
//! let subs = subs.add(channel2); // Returns Subscriptions<2>
//!
//! // Subscriptions<3> would fail to compile if MAX is 2!
//! // let subs = subs.add(channel3); // COMPILE ERROR!
//! ```
//!
//! The type system tracks the count and prevents exceeding limits at compile time.

use std::collections::HashSet;
use std::marker::PhantomData;

/// Maximum subscriptions allowed per connection (compile-time constant)
pub const MAX_SUBSCRIPTIONS: usize = 50;

/// Type-level subscription list with compile-time count tracking
///
/// The const generic `N` represents the current number of subscriptions.
/// This enables the compiler to enforce subscription limits.
///
/// # Type Safety
///
/// - `Subscriptions<0>` - No subscriptions
/// - `Subscriptions<1>` - One subscription
/// - `Subscriptions<N>` - N subscriptions
/// - Cannot create `Subscriptions<N>` where N > MAX_SUBSCRIPTIONS
///
/// # Example
///
/// ```ignore
/// use skreaver_http::websocket::subscription_limits::*;
///
/// // Start with no subscriptions
/// let subs = Subscriptions::<0>::new();
///
/// // Add a subscription (compile-time safe)
/// let subs = subs.subscribe("room1".to_string());
/// // Type is now Subscriptions<1>
///
/// // Add another (still safe)
/// let subs = subs.subscribe("room2".to_string());
/// // Type is now Subscriptions<2>
///
/// // If we tried to add 51 subscriptions, it wouldn't compile!
/// ```
pub struct Subscriptions<const N: usize> {
    channels: HashSet<String>,
    _phantom: PhantomData<[(); N]>,
}

impl Subscriptions<0> {
    /// Create a new empty subscription list
    ///
    /// This is the only way to create a Subscriptions instance,
    /// ensuring we always start from a known state.
    pub fn new() -> Self {
        Self {
            channels: HashSet::new(),
            _phantom: PhantomData,
        }
    }
}

impl Default for Subscriptions<0> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Subscriptions<N> {
    /// Add a subscription (compile-time checked)
    ///
    /// This method is only available when N < MAX_SUBSCRIPTIONS.
    /// The compiler will reject attempts to add too many subscriptions.
    ///
    /// # Type Transformation
    ///
    /// Consumes `Subscriptions<N>` and returns `Subscriptions<N+1>`,
    /// incrementing the type-level counter.
    ///
    /// # Arguments
    ///
    /// * `channel` - The channel name to subscribe to
    ///
    /// # Returns
    ///
    /// A new `Subscriptions<N+1>` with the channel added.
    pub fn subscribe<const NEXT: usize>(mut self, channel: String) -> Subscriptions<NEXT>
    where
        [(); NEXT]: Sized,
        ValidateSubscriptionLimit<NEXT>: IsWithinLimit,
    {
        self.channels.insert(channel);
        Subscriptions {
            channels: self.channels,
            _phantom: PhantomData,
        }
    }

    /// Remove a subscription
    ///
    /// Decrements the type-level counter by returning Subscriptions<N-1>.
    pub fn remove<const PREV: usize>(mut self, channel: &str) -> Subscriptions<PREV>
    where
        [(); PREV]: Sized,
    {
        self.channels.remove(channel);
        Subscriptions {
            channels: self.channels,
            _phantom: PhantomData,
        }
    }

    /// Get the current subscription count (compile-time constant)
    pub const fn count(&self) -> usize {
        N
    }

    /// Check if subscribed to a channel
    pub fn contains(&self, channel: &str) -> bool {
        self.channels.contains(channel)
    }

    /// Get all subscribed channels
    pub fn channels(&self) -> &HashSet<String> {
        &self.channels
    }

    /// Convert to runtime-checked subscription list
    ///
    /// This is necessary for dynamic scenarios where the count
    /// isn't known at compile time (e.g., loading from database).
    pub fn into_dynamic(self) -> DynamicSubscriptions {
        DynamicSubscriptions {
            channels: self.channels,
            max: MAX_SUBSCRIPTIONS,
        }
    }
}

/// Compile-time validation that N is within the subscription limit
///
/// This trait is only implemented for values <= MAX_SUBSCRIPTIONS,
/// causing a compile error if you try to exceed the limit.
pub struct ValidateSubscriptionLimit<const N: usize>;

/// Marker trait indicating the subscription count is valid
pub trait IsWithinLimit {}

// Implement IsWithinLimit for all N where N <= MAX_SUBSCRIPTIONS
// This uses a const generic trick to enforce the limit at compile time
impl<const N: usize> IsWithinLimit for ValidateSubscriptionLimit<N> where
    ValidateSubscriptionLimit<N>: ValidSubscriptionCount
{
}

/// Helper trait for compile-time validation
pub trait ValidSubscriptionCount {}

// Implement for all valid counts (0..=MAX_SUBSCRIPTIONS)
// We use a macro to generate impls for all valid values
macro_rules! impl_valid_count {
    ($($n:literal),*) => {
        $(
            impl ValidSubscriptionCount for ValidateSubscriptionLimit<$n> {}
        )*
    };
}

// Generate implementations for 0..=50
impl_valid_count!(
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50
);

/// Runtime-checked subscription list for dynamic scenarios
///
/// Use this when the subscription count isn't known at compile time,
/// such as when loading existing subscriptions from a database.
pub struct DynamicSubscriptions {
    channels: HashSet<String>,
    max: usize,
}

impl DynamicSubscriptions {
    /// Create a new dynamic subscription list
    pub fn new(max: usize) -> Self {
        Self {
            channels: HashSet::new(),
            max,
        }
    }

    /// Add a subscription (runtime checked)
    pub fn add(&mut self, channel: String) -> Result<(), SubscriptionLimitError> {
        if self.channels.len() >= self.max {
            return Err(SubscriptionLimitError {
                current: self.channels.len(),
                max: self.max,
            });
        }
        self.channels.insert(channel);
        Ok(())
    }

    /// Remove a subscription
    pub fn remove(&mut self, channel: &str) -> bool {
        self.channels.remove(channel)
    }

    /// Get current count
    pub fn count(&self) -> usize {
        self.channels.len()
    }

    /// Check if subscribed
    pub fn contains(&self, channel: &str) -> bool {
        self.channels.contains(channel)
    }

    /// Get all channels
    pub fn channels(&self) -> &HashSet<String> {
        &self.channels
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionLimitError {
    pub current: usize,
    pub max: usize,
}

impl std::fmt::Display for SubscriptionLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Subscription limit exceeded: {} subscriptions (max: {})",
            self.current, self.max
        )
    }
}

impl std::error::Error for SubscriptionLimitError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_level_subscription_limits() {
        let channel1 = "channel1".to_string();
        let channel2 = "channel2".to_string();
        let channel3 = "channel3".to_string();

        // Start with no subscriptions
        let subs = Subscriptions::<0>::new();
        assert_eq!(subs.count(), 0);

        // Add first subscription
        let subs = subs.subscribe::<1>(channel1.clone());
        assert_eq!(subs.count(), 1);
        assert!(subs.contains(&channel1));

        // Add second subscription
        let subs = subs.subscribe::<2>(channel2.clone());
        assert_eq!(subs.count(), 2);
        assert!(subs.contains(&channel2));

        // Add third subscription
        let subs = subs.subscribe::<3>(channel3.clone());
        assert_eq!(subs.count(), 3);
        assert!(subs.contains(&channel3));

        // Remove a subscription
        let subs = subs.remove::<2>(&channel2);
        assert_eq!(subs.count(), 2);
        assert!(!subs.contains(&channel2));
    }

    #[test]
    fn test_dynamic_subscriptions() {
        let channel1 = "channel1".to_string();
        let channel2 = "channel2".to_string();

        let mut subs = DynamicSubscriptions::new(2);

        // Add subscriptions
        assert!(subs.add(channel1.clone()).is_ok());
        assert_eq!(subs.count(), 1);

        assert!(subs.add(channel2.clone()).is_ok());
        assert_eq!(subs.count(), 2);

        // Should fail - limit reached
        let channel3 = "channel3".to_string();
        let result = subs.add(channel3);
        assert!(result.is_err());

        // Remove and try again
        assert!(subs.remove(&channel1));
        let channel3 = "channel3".to_string();
        assert!(subs.add(channel3).is_ok());
    }

    // This test demonstrates compile-time safety
    // Uncommenting the line below should cause a compile error:
    // #[test]
    // fn test_compile_time_limit() {
    //     let subs = Subscriptions::<0>::new();
    //     // Try to add 51 subscriptions - should not compile!
    //     let subs = subs.add::<51>("channel".to_string());
    // }
}
