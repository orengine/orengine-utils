//! This module provides a [`Backoff`] that can be used to busy-wait with
//! preemptive yield when it is necessary.
//!
//! It has the same API as `crossbeam::Backoff`.
use crate::hints::likely;
use core::cell::Cell;
use core::fmt;

const SPIN_LIMIT: u32 = 6;

/// Performs exponential backoff in spin loops.
///
/// Backing off in spin loops reduces contention and improves overall performance.
///
/// This primitive can execute *YIELD* and *PAUSE* instructions, yield the current thread to the OS
/// scheduler, and tell when it is a good time to block the thread using a different synchronization
/// mechanism. Each step of the back off procedure takes roughly twice as long as the previous
/// step.
pub struct Backoff {
    step: Cell<u32>,
}

impl Backoff {
    /// Creates a new `Backoff` instance.
    #[inline]
    pub fn new() -> Self {
        Self { step: Cell::new(0) }
    }

    /// Returns the current backoff step
    /// (how many times it has been called since the last [`reset`](Self::reset)).
    #[inline]
    pub fn step(&self) -> u32 {
        self.step.get()
    }

    /// Resets the backoff state.
    #[inline]
    pub fn reset(&self) {
        self.step.set(0);
    }

    /// Backs off in a lock-free loop.
    ///
    /// This method should be used when we need to retry an operation because another thread made
    /// progress.
    ///
    /// The processor may yield using the *YIELD* or *PAUSE* instruction.
    #[inline]
    pub fn spin(&self) {
        for _ in 0..1 << self.step.get().min(SPIN_LIMIT) {
            std::hint::spin_loop();
        }

        self.step.set(self.step.get() + 1);
    }

    /// It [`spins`](Self::spin) or calls the provided function if
    /// it [`should be used`](Self::is_completed).
    ///
    /// If the provided function is [`std::thread::yield_now`],
    /// then this function has the same behaviour as [`snooze`](Self::snooze).
    #[inline]
    pub fn spin_or<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        if likely(self.step.get() < SPIN_LIMIT) {
            for _ in 0..1 << self.step.get().min(SPIN_LIMIT) {
                std::hint::spin_loop();
            }
        } else {
            f();
        }

        self.step.set(self.step.get() + 1);
    }

    /// Backs off in a blocking loop.
    ///
    /// This method should be used when we need to wait for another thread to make progress.
    ///
    /// The processor may yield using the *YIELD* or *PAUSE* instruction, and the current thread
    /// may yield by giving up a timeslice to the OS scheduler.
    ///
    /// In `#[no_std]` environments, this method is equivalent to [`spin`].
    ///
    /// If possible, use [`is_completed`] to check when it is advised to stop using backoff and
    /// block the current thread using a different synchronization mechanism instead.
    ///
    /// [`spin`]: Backoff::spin
    /// [`is_completed`]: Backoff::is_completed
    #[inline]
    pub fn snooze(&self) {
        self.spin_or(std::thread::yield_now);
    }

    /// Returns `true` if exponential backoff has completed and blocking the thread is advised.
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.step.get() >= SPIN_LIMIT
    }
}

impl fmt::Debug for Backoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Backoff")
            .field("step", &self.step)
            .field("is_completed", &self.is_completed())
            .finish()
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}