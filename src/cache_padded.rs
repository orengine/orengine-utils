//! Provides cache-padded atomic types.
//!
//! # Example
//!
//! ```
//! use orengine_utils::cache_padded::{CachePadded, CachePaddedAtomicUsize};
//! use core::sync::atomic::{AtomicUsize, Ordering};
//!
//! // Using CachePaddedAtomicUsize type alias
//! let counter = CachePaddedAtomicUsize::new(AtomicUsize::new(0));
//!
//! counter.fetch_add(1, Ordering::Relaxed);
//!
//! assert_eq!(counter.load(Ordering::Relaxed), 1);
//!
//! // Using CachePadded with a custom type
//! let padded_value = CachePadded::new(42);
//! assert_eq!(*padded_value, 42);
//! ```
// This code is forked from crossbeam: https://github.com/crossbeam-rs/crossbeam/blob/master/crossbeam-utils/src/cache_padded.rs
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{
    AtomicBool, AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicPtr, AtomicU16,
    AtomicU32, AtomicU64, AtomicU8, AtomicUsize,
};

/// Pads and aligns a value to the length of a cache line.
///
/// In concurrent programming, sometimes it is desirable to make sure commonly accessed pieces of
/// data are not placed into the same cache line. Updating an atomic value invalidates the whole
/// cache line it belongs to, which makes the next access to the same cache line slower for other
/// CPU cores. Use `CachePadded` to ensure updating one piece of data doesn't invalidate other
/// cached data.
///
/// # Size and alignment
///
/// Cache lines are assumed to be N bytes long, depending on the architecture:
///
/// * On x86-64, aarch64, and powerpc64, N = 128.
/// * On arm, mips, mips64, sparc, and hexagon, N = 32.
/// * On m68k, N = 16.
/// * On s390x, N = 256.
/// * On all others, N = 64.
///
/// Note that N is just a reasonable guess and is not guaranteed to match the actual cache line
/// length of the machine the program is running on. On modern Intel architectures, spatial
/// prefetcher is pulling pairs of 64-byte cache lines at a time, so we pessimistically assume that
/// cache lines are 128 bytes long.
///
/// The size of `CachePadded<T>` is the smallest multiple of N bytes large enough to accommodate
/// a value of type `T`.
///
/// The alignment of `CachePadded<T>` is the maximum of N bytes and the alignment of `T`.
///
/// # Examples
///
/// Alignment and padding:
///
/// ```
/// use orengine_utils::cache_padded::CachePadded;
///
/// let array = [CachePadded::new(1i8), CachePadded::new(2i8)];
/// let addr1 = &*array[0] as *const i8 as usize;
/// let addr2 = &*array[1] as *const i8 as usize;
///
/// assert!(addr2 - addr1 >= 32);
/// assert_eq!(addr1 % 32, 0);
/// assert_eq!(addr2 % 32, 0);
/// ```
///
/// When building a concurrent queue with a head and a tail index, it is wise to place them in
/// different cache lines so that concurrent threads pushing and popping elements don't invalidate
/// each other's cache lines:
///
/// ```
/// use orengine_utils::cache_padded::CachePadded;
/// use core::sync::atomic::AtomicUsize;
///
/// struct Queue<T> {
///     head: CachePadded<AtomicUsize>,
///     tail: CachePadded<AtomicUsize>,
///     buffer: *mut T,
/// }
/// ```
#[derive(Clone, Copy, Default, Hash, PartialEq, Eq)]
// Starting from Intel's Sandy Bridge, spatial prefetcher is now pulling pairs of 64-byte cache
// lines at a time, so we have to align to 128 bytes rather than 64.
//
// Sources:
// - https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-optimization-manual.pdf
// - https://github.com/facebook/folly/blob/1b5288e6eea6df074758f877c849b6e73bbb9fbb/folly/lang/Align.h#L107
//
// aarch64/arm64ec's big.LITTLE architecture has asymmetric cores and "big" cores have 128-byte cache line size.
//
// Sources:
// - https://www.mono-project.com/news/2016/09/12/arm64-icache/
//
// powerpc64 has 128-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_ppc64x.go#L9
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/powerpc/include/asm/cache.h#L26
#[cfg_attr(
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm64ec",
        target_arch = "powerpc64",
    ),
    repr(align(128))
)]
// arm, mips, mips64, sparc, and hexagon have 32-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_arm.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mipsle.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips64x.go#L9
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/sparc/include/asm/cache.h#L17
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/hexagon/include/asm/cache.h#L12
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc",
        target_arch = "hexagon",
    ),
    repr(align(32))
)]
// m68k has a 16-byte cache line size.
//
// Sources:
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/m68k/include/asm/cache.h#L9
#[cfg_attr(target_arch = "m68k", repr(align(16)))]
// s390x has 256-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_s390x.go#L7
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/s390/include/asm/cache.h#L13
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
// x86, wasm, riscv, and sparc64 have 64-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/dda2991c2ea0c5914714469c4defc2562a907230/src/internal/cpu/cpu_x86.go#L9
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_wasm.go#L7
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/riscv/include/asm/cache.h#L10
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/sparc/include/asm/cache.h#L19
//
// All others are assumed to have 64-byte cache line size.
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "arm64ec",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc",
        target_arch = "hexagon",
        target_arch = "m68k",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
pub struct CachePadded<T> {
    value: T,
}

unsafe impl<T: Send> Send for CachePadded<T> {}
unsafe impl<T: Sync> Sync for CachePadded<T> {}

impl<T> CachePadded<T> {
    /// Pads and aligns a value to the length of a cache line.
    ///
    /// # Examples
    ///
    /// ```
    /// use orengine_utils::cache_padded::CachePadded;
    ///
    /// let padded_value = CachePadded::new(1);
    /// ```
    pub const fn new(t: T) -> Self {
        Self { value: t }
    }

    /// Returns the inner value.
    ///
    /// # Examples
    ///
    /// ```
    /// use orengine_utils::cache_padded::CachePadded;
    ///
    /// let padded_value = CachePadded::new(7);
    /// let value = padded_value.into_inner();
    ///
    /// assert_eq!(value, 7);
    /// ```
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> Deref for CachePadded<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> DerefMut for CachePadded<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T: fmt::Debug> fmt::Debug for CachePadded<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachePadded")
            .field("value", &self.value)
            .finish()
    }
}

impl<T> From<T> for CachePadded<T> {
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

impl<T: fmt::Display> fmt::Display for CachePadded<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.value, f)
    }
}

macro_rules! cache_padded_atomic_number {
    ($name:ident, $atomic_type:ident, $number_type:ident) => {
        #[allow(
            rustdoc::redundant_explicit_links,
            reason = "It is needed for right IDE doc formating"
        )]
        #[doc = concat!(
            "Alias to [`CachePadded`](CachePadded)`<`[`", stringify!($atomic_type), "`]`>`."
        )]
        pub struct $name(CachePadded<$atomic_type>);

        impl $name {
             #[doc = concat!(
                 "Creates a new [`CachePadded`](CachePadded)`<`[`", stringify!($atomic_type), "`]`>`."
             )]
            #[inline(always)]
            pub const fn new(t: $number_type) -> Self {
                Self($crate::cache_padded::CachePadded::new($atomic_type::new(t)))
            }
        }

        impl core::ops::Deref for $name {
            type Target = $atomic_type;

            fn deref(&self) -> &$atomic_type {
                &self.0
            }
        }

        impl core::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut $atomic_type {
                &mut self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new($number_type::default())
            }
        }
    };
}

cache_padded_atomic_number!(CachePaddedAtomicU8, AtomicU8, u8);
cache_padded_atomic_number!(CachePaddedAtomicU16, AtomicU16, u16);
cache_padded_atomic_number!(CachePaddedAtomicU32, AtomicU32, u32);
cache_padded_atomic_number!(CachePaddedAtomicU64, AtomicU64, u64);
cache_padded_atomic_number!(CachePaddedAtomicUsize, AtomicUsize, usize);

cache_padded_atomic_number!(CachePaddedAtomicI8, AtomicI8, i8);
cache_padded_atomic_number!(CachePaddedAtomicI16, AtomicI16, i16);
cache_padded_atomic_number!(CachePaddedAtomicI32, AtomicI32, i32);
cache_padded_atomic_number!(CachePaddedAtomicI64, AtomicI64, i64);
cache_padded_atomic_number!(CachePaddedAtomicIsize, AtomicIsize, isize);

cache_padded_atomic_number!(CachePaddedAtomicBool, AtomicBool, bool);

#[allow(
    rustdoc::redundant_explicit_links,
    reason = "It is needed for right IDE doc formating"
)]
/// Alias to [`CachePadded`](CachePadded)`<`[`AtomicPtr`](AtomicPtr)`<T>>`.
pub struct CachePaddedAtomicPtr<T>(CachePadded<AtomicPtr<T>>);

impl<T> CachePaddedAtomicPtr<T> {
    /// Creates a new [`CachePadded`](CachePadded)`<`[`AtomicPtr`](AtomicPtr)`<T>>`.
    #[inline(always)]
    pub const fn new(ptr: *mut T) -> Self {
        Self(CachePadded::new(AtomicPtr::new(ptr)))
    }
}

impl<T> Deref for CachePaddedAtomicPtr<T> {
    type Target = AtomicPtr<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for CachePaddedAtomicPtr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Generates a cache padded type.
/// It accepts a name of the new type, the inner type, and the default function.
///
/// The new type can be dereferenced to the inner type.
///
/// # Deprecated
///
/// This macro is deprecated, use the [`CachePadded`] wrapper instead.
#[macro_export]
#[deprecated]
macro_rules! generate_cache_padded_type {
    ($name:ident, $atomic:ident, $default:block) => {
        /// Cache padded inner type. Can be dereferenced to the inner type.
        pub struct $name(CachePadded<$atomic>);

        impl $name {
            /// Creates a new cache padded inner type.
            pub const fn new() -> Self {
                Self(CachePadded::new($default))
            }
        }

        impl Deref for $name {
            type Target = $atomic;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}
