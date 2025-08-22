//! Hints to the compiler that affects how code should be emitted or optimized.

/// Do the same as [`assert_unchecked`](std::hint::assert_unchecked), but instead of UB,
/// it panics with `debug_assertions`.
///
/// # Panics
///
/// It panics with `debug_assertions` if `cond` is `false`.
#[inline(always)]
#[track_caller]
#[allow(unused_variables, reason = "It contains #[cfg(debug_assertions)]")]
pub fn assert_hint(cond: bool, debug_msg: &str) {
    if cfg!(debug_assertions) {
        assert!(cond, "{debug_msg}");
    } else {
        unsafe { std::hint::assert_unchecked(cond) };
    }
}

/// Do the same as [`unreachable_unchecked`](std::hint::unreachable_unchecked), but instead of UB,
/// it panics with `debug_assertions`.
///
/// # Panics
///
/// It panics with `debug_assertions`.
#[inline(always)]
#[track_caller]
#[allow(unused_variables, reason = "It contains #[cfg(debug_assertions)]")]
pub fn unreachable_hint() -> ! {
    if cfg!(debug_assertions) {
        unreachable!();
    } else {
        unsafe { std::hint::unreachable_unchecked() }
    }
}

/// Indicate that a given branch is **not** likely to be taken, relatively speaking.
#[inline(always)]
#[cold]
pub const fn cold_path() {}

/// Indicate that a given condition is likely to be true.
#[inline(always)]
pub const fn likely(b: bool) -> bool {
    if !b {
        cold_path();
    }

    b
}

/// Indicate that a given condition is likely to be false.
#[inline(always)]
pub const fn unlikely(b: bool) -> bool {
    if b {
        cold_path();
    }

    b
}

/// A trait that is implemented by [`Option`] and [`Result`].
pub trait UnwrapOrPanic<T> {
    /// Unwraps a value, panicking if it is [`None`] or [`Err`].
    fn unwrap_or_panic(self, message: &'static str) -> T;
    /// Unwraps a value, UB if it is [`None`] or [`Err`].
    ///
    /// # Safety
    ///
    /// This function is unsafe because it can break the program if the provided value is used
    /// with wrong arguments.
    /// The caller __must__ be sure that the value is not `None` or `Err`.
    unsafe fn unwrap_unchecked(self) -> T;
    // `unwrap_or_bug_hint` is not described here not to extend the `Option` and `Result` even more
}

impl<T> UnwrapOrPanic<T> for Option<T> {
    #[inline(always)]
    fn unwrap_or_panic(self, message: &'static str) -> T {
        self.unwrap_or_else(|| panic!("{message}"))
    }

    #[inline(always)]
    unsafe fn unwrap_unchecked(self) -> T {
        unsafe { self.unwrap_unchecked() }
    }
}

impl<T, E> UnwrapOrPanic<T> for Result<T, E> {
    #[inline(always)]
    fn unwrap_or_panic(self, message: &'static str) -> T {
        self.unwrap_or_else(|_| panic!("{message}"))
    }

    #[inline(always)]
    unsafe fn unwrap_unchecked(self) -> T {
        unsafe { self.unwrap_unchecked() }
    }
}

/// Unwraps a value, panicking if it is [`None`] or [`Err`] with `debug_assertions`.
///
/// Else hints to the compiler that the value is not `None` or `Err`.
#[track_caller]
pub fn unwrap_or_bug_hint<T>(item: impl UnwrapOrPanic<T>) -> T {
    if cfg!(debug_assertions) {
        item.unwrap_or_panic("`unwrap_or_bug_hint` has been failed. It is a bug.")
    } else {
        unsafe { item.unwrap_unchecked() }
    }
}

/// Unwraps a value, panicking if it is [`None`] or [`Err`] with `debug_assertions`.
///
/// Else hints to the compiler that the value is not `None` or `Err`.
#[allow(unused_variables, reason = "It contains #[cfg(debug_assertions)]")]
#[track_caller]
pub fn unwrap_or_bug_message_hint<T>(item: impl UnwrapOrPanic<T>, message: &'static str) -> T {
    if cfg!(debug_assertions) {
        item.unwrap_or_panic(message)
    } else {
        unsafe { item.unwrap_unchecked() }
    }
}