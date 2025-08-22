//! This module contains the [`OrengineInstant`].
use std::fmt;
use std::mem::MaybeUninit;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::time::{Duration, Instant as StdInstant};

/// A monotone clock. It can be converted to/from `std::time::Instant`.
///
/// On Unix-like systems, it weights 8 bytes.
/// On others, it is a wrapper around `std::time::Instant`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrengineInstant {
    #[cfg(not(unix))]
    instant: StdInstant,
    #[cfg(unix)]
    instant: u64,
}

impl OrengineInstant {
    /// Creates a new `OrengineInstant` from a `u64`.
    ///
    /// # Panics
    ///
    /// It panics if it is called not on `unix`.
    #[cfg(unix)]
    pub fn from_u64(instant: u64) -> Self {
        if cfg!(not(unix)) {
            panic!("`from_u64` can be called only on UNIX.");
        }

        Self { instant }
    }

    /// Converts the `OrengineInstant` into a `u64`.
    ///
    /// # Panics
    ///
    /// It panics if it is called not on `unix`.
    #[cfg(unix)]
    pub fn into_u64(self) -> u64 {
        if cfg!(not(unix)) {
            panic!("`into_u64` can be called only on UNIX.");
        }

        self.instant
    }

    /// Returns the current `monotonic` instant.
    pub fn now() -> Self {
        #[cfg(not(unix))]
        return Self {
            instant: StdInstant::now(),
        };

        #[allow(clippy::cast_sign_loss, reason = "It can't be negative")]
        #[cfg(unix)]
        {
            let mut ts_ = MaybeUninit::<libc::timespec>::uninit();
            unsafe {
                libc::clock_gettime(libc::CLOCK_MONOTONIC, ts_.as_mut_ptr());
            }
            let ts = unsafe { ts_.assume_init() };

            Self {
                instant: ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64,
            }
        }
    }

    /// Returns the amount of time elapsed from another instant to this one, or `None` if that
    /// instant is earlier than this one.
    ///
    /// Due to `monotonicity bugs`, even under correct logical ordering of the passed `Instant`s,
    /// this method can return `None`.
    pub fn checked_duration_since(&self, earlier: impl Into<StdInstant>) -> Option<Duration> {
        #[cfg(not(unix))]
        {
            self.instant.checked_duration_since(earlier.into())
        }

        #[cfg(unix)]
        {
            Some(Duration::from_nanos(
                self.instant - Self::from(earlier.into()).instant,
            ))
        }
    }

    /// Returns the amount of time elapsed from another instant to this one, or zero duration if
    /// that instant is earlier than this one.
    ///
    /// Due to `monotonicity bugs`, even under correct logical ordering of the passed `Instant`s,
    /// this method can return `None`.
    pub fn saturating_duration_since(&self, earlier: impl Into<StdInstant>) -> Duration {
        self.checked_duration_since(earlier.into())
            .unwrap_or_default()
    }

    /// Returns the amount of time elapsed from another instant to this one, or zero duration if
    /// that instant is earlier than this one.
    ///
    /// Due to `monotonicity bugs`, even under correct logical ordering of the passed `Instant`s,
    /// this method can return `None`.
    pub fn duration_since(&self, earlier: impl Into<StdInstant>) -> Duration {
        self.saturating_duration_since(earlier.into())
    }

    /// Returns the amount of time elapsed since this instant was created.
    pub fn elapsed(&self) -> Duration {
        Self::now() - *self
    }

    /// Returns `Some(t)` where `t` is the time `self + duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        #[cfg(not(unix))]
        {
            Some(Self {
                instant: self.instant.checked_add(duration)?,
            })
        }

        #[cfg(unix)]
        {
            let total_nanos = u64::try_from(duration.as_nanos()).ok()?;

            Some(Self {
                instant: self.instant.checked_add(total_nanos)?,
            })
        }
    }

    /// Returns `Some(t)` where `t` is the time `self - duration` if `t` can be represented as
    /// `Instant` (which means it's inside the bounds of the underlying data structure), `None`
    /// otherwise.
    pub fn checked_sub(&self, duration: Duration) -> Option<Self> {
        #[cfg(not(unix))]
        {
            Some(Self {
                instant: self.instant.checked_sub(duration)?,
            })
        }

        #[cfg(unix)]
        {
            let total_nanos = u64::try_from(duration.as_nanos()).ok()?;

            Some(Self {
                instant: self.instant.checked_sub(total_nanos)?,
            })
        }
    }
}

impl Add<Duration> for OrengineInstant {
    type Output = Self;

    /// # Panics
    ///
    /// This function may panic if the resulting point in time cannot be represented by the
    /// underlying data structure. See [`OrengineInstant::checked_add`] for a version without a panic.
    fn add(self, other: Duration) -> Self {
        self.checked_add(other)
            .expect("overflow when adding duration to instant")
    }
}

impl AddAssign<Duration> for OrengineInstant {
    fn add_assign(&mut self, other: Duration) {
        // This is not millennium-safe, but I think that's OK. :)
        *self = self
            .checked_add(other)
            .expect("overflow when adding duration to instant");
    }
}

impl Sub<Duration> for OrengineInstant {
    type Output = Self;

    fn sub(self, other: Duration) -> Self {
        self.checked_sub(other)
            .expect("overflow when subtracting duration from instant")
    }
}

impl SubAssign<Duration> for OrengineInstant {
    fn sub_assign(&mut self, other: Duration) {
        // This is not millennium-safe, but I think that's OK. :)
        *self = self
            .checked_sub(other)
            .expect("overflow when subtracting duration from instant");
    }
}

impl Sub<Self> for OrengineInstant {
    type Output = Duration;

    /// Returns the amount of time elapsed from another instant to this one,
    /// or zero duration if that instant is later than this one.
    fn sub(self, other: Self) -> Duration {
        self.duration_since(other)
    }
}

impl fmt::Debug for OrengineInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.instant.fmt(f)
    }
}

#[cfg(unix)]
mod unix_time {
    // std::time::Instant is represented as
    // struct Nanoseconds(u32);
    //
    // struct Timespec {
    //     tv_sec: i64,
    //     tv_nsec: Nanoseconds,
    // }

    // struct Instant {
    //     t: Timespec,
    // }

    pub(crate) struct Nanoseconds(pub(crate) u32);

    pub(crate) struct Timespec {
        pub(crate) tv_sec: i64,
        pub(crate) tv_nsec: Nanoseconds,
    }
}

impl From<OrengineInstant> for std::time::Instant {
    fn from(val: OrengineInstant) -> Self {
        #[cfg(not(unix))]
        {
            val.instant
        }

        #[cfg(unix)]
        {
            let dur = Duration::from_nanos(val.instant);

            unsafe {
                #[allow(clippy::transmute_undefined_repr, reason = "False positive")]
                #[allow(clippy::cast_possible_wrap, reason = "It is fine for our century")]
                std::mem::transmute(unix_time::Timespec {
                    tv_sec: dur.as_secs() as i64,
                    tv_nsec: unix_time::Nanoseconds(dur.subsec_nanos()),
                })
            }
        }
    }
}

impl From<std::time::Instant> for OrengineInstant {
    #[allow(clippy::cast_sign_loss, reason = "It is never negative")]
    fn from(val: std::time::Instant) -> Self {
        #[cfg(not(unix))]
        {
            Self { instant: val }
        }

        #[cfg(unix)]
        {
            #[allow(clippy::transmute_undefined_repr, reason = "False positive")]
            let ts: unix_time::Timespec = unsafe { std::mem::transmute(val) };

            Self {
                instant: ts.tv_sec as u64 * 1_000_000_000 + u64::from(ts.tv_nsec.0),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrengineInstant;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_into_std_instant() {
        let instant = OrengineInstant::now();
        let std_instant: std::time::Instant = instant.into();
        let instant_from_std: OrengineInstant = std_instant.into();

        assert_eq!(instant, instant_from_std);

        thread::sleep(Duration::from_millis(1));

        let now = OrengineInstant::now();

        assert_eq!(now.duration_since(instant), now.duration_since(std_instant));
    }

    #[test]
    fn test_from_std_instant() {
        let std_instant: std::time::Instant = std::time::Instant::now();
        let instant: OrengineInstant = std_instant.into();
        let std_instant_from_instant: std::time::Instant = instant.into();

        assert_eq!(std_instant, std_instant_from_instant);
    }

    #[test]
    fn test_instant_ordering() {
        let instant1: OrengineInstant = std::time::Instant::now().into();
        let instant2 = instant1;

        assert_eq!(instant1, instant2);

        let instant3 = instant1 + Duration::from_millis(1);
        let instant4 = instant1 + Duration::from_millis(2);

        assert!(instant1 < instant3);
        assert!(instant3 < instant4);

        let mut btree = std::collections::BTreeSet::new();

        assert!(btree.insert(instant1));
        assert!(btree.insert(instant3));
        assert!(btree.insert(instant4));

        assert_eq!(btree.pop_first(), Some(instant1));
        assert_eq!(btree.pop_first(), Some(instant3));
        assert_eq!(btree.pop_first(), Some(instant4));
    }
}