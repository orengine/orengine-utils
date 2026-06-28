//! Fast non-cryptographic pseudo-random number generators.
//!
//! Provides xorshift-based PRNGs for `u32` and `u64` in two flavors:
//!
//! - **Stateful** ([`cheap_random_with_current_u32`], [`cheap_random_with_current_u64`]):
//!   the caller owns the state. Use these when you need a reproducible
//!   sequence or when thread-local storage is unavailable (e.g. `no_std`).
//!
//! - **Thread-local** ([`cheap_random_u32`], [`cheap_random_u64`]):
//!   state is managed automatically per-thread. Available only when the
//!   `no_std` feature is disabled.
//!
//! Neither generator is suitable for cryptographic use. Both produce
//! statistically adequate randomness for tasks such as treap priorities,
//! random sampling, and load balancing.

use core::num::{NonZeroU32, NonZeroU64, Wrapping};

/// Advances `current` with a 32-bit xorshift step and returns the new value.
///
/// Suitable for use as a treap priority generator or any place a fast,
/// non-cryptographic `u32` sequence is needed.
///
/// # Example
///
/// ```rust
/// use orengine_utils::cheap_random::cheap_random_with_current_u32;
///
/// let mut state = core::num::NonZeroU32::new(1).unwrap();
///
/// let a = cheap_random_with_current_u32(&mut state);
/// let b = cheap_random_with_current_u32(&mut state);
///
/// assert_ne!(a, b);
/// ```
pub fn cheap_random_with_current_u32(current: &mut NonZeroU32) -> u32 {
    let mut x = Wrapping(current.get());

    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;

    *current = unsafe { NonZeroU32::new_unchecked(x.0) };

    current.get()
}

/// Returns a random `u32` from a thread-local xorshift state.
///
/// The state is initialized once per thread and advances automatically.
/// Equivalent to calling [`cheap_random_with_current_u32`] on a hidden
/// per-thread [`NonZeroU32`].
///
/// # Example
///
/// ```rust
/// use orengine_utils::cheap_random::cheap_random_u32;
///
/// let a = cheap_random_u32();
/// let b = cheap_random_u32();
///
/// assert_ne!(a, b);
/// ```
#[cfg(not(feature = "no_std"))]
pub fn cheap_random_u32() -> u32 {
    use core::cell::Cell;

    thread_local! {
        static RNG: Cell<NonZeroU32> = const {
            Cell::new(NonZeroU32::new(451_842_549).unwrap())
        };
    }

    RNG.with(|rng| unsafe { cheap_random_with_current_u32(&mut *rng.as_ptr()) })
}

/// Advances `current` with a 64-bit xorshift step and returns the new value.
///
/// Suitable for any place a fast, non-cryptographic `u64` sequence is needed.
///
/// # Example
///
/// ```rust
/// use orengine_utils::cheap_random::cheap_random_with_current_u64;
///
/// let mut state = core::num::NonZeroU64::new(1).unwrap();
///
/// let a = cheap_random_with_current_u64(&mut state);
/// let b = cheap_random_with_current_u64(&mut state);
///
/// assert_ne!(a, b);
/// ```
pub fn cheap_random_with_current_u64(current: &mut NonZeroU64) -> u64 {
    let mut x = Wrapping(current.get());

    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;

    *current = unsafe { NonZeroU64::new_unchecked(x.0) };

    current.get()
}

/// Returns a random `u64` from a thread-local xorshift state.
///
/// The state is initialized once per thread and advances automatically.
/// Equivalent to calling [`cheap_random_with_current_u64`] on a hidden
/// per-thread [`NonZeroU64`].
///
/// # Example
///
/// ```rust
/// use orengine_utils::cheap_random::cheap_random_u64;
///
/// let a = cheap_random_u64();
/// let b = cheap_random_u64();
///
/// assert_ne!(a, b);
/// ```
#[cfg(not(feature = "no_std"))]
pub fn cheap_random_u64() -> u64 {
    use core::cell::Cell;

    thread_local! {
        static RNG: Cell<NonZeroU64> = const {
            Cell::new(NonZeroU64::new(7_948_160_987_894_135_694).unwrap())
        };
    }

    RNG.with(|rng| unsafe { cheap_random_with_current_u64(&mut *rng.as_ptr()) })
}
