//! Provides cache-padded atomic types.
use core::sync::atomic::{
    AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicPtr, AtomicU16, AtomicU32,
    AtomicU64, AtomicU8, AtomicUsize,
};

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "powerpc64",
))]
/// A cache line size for the architecture that the application is compiled for.
pub const CACHE_LINE_SIZE: usize = 128;

#[cfg(any(
    target_arch = "arm",
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6",
    target_arch = "sparc",
    target_arch = "hexagon",
))]
/// A cache line size for the architecture that the application is compiled for.
pub const CACHE_LINE_SIZE: usize = 32;

#[cfg(target_arch = "m68k")]
/// A cache line size for the architecture that the application is compiled for.
pub const CACHE_LINE_SIZE: usize = 16;

#[cfg(target_arch = "s390x")]
/// A cache line size for the architecture that the application is compiled for.
pub const CACHE_LINE_SIZE: usize = 256;

#[cfg(not(any(
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
)))]
/// A cache line size for the architecture that the application is compiled for.
pub const CACHE_LINE_SIZE: usize = 64;

/// Generates a cache padded type.
/// It accepts a name of the new type, the inner type, and the default function.
///
/// The new type can be dereferenced to the inner type.
#[macro_export]
macro_rules! generate_cache_padded_type {
    ($name:ident < $($generics:tt),* >, $atomic:ident < $($atomic_generics:tt),* >, $default:block) => {
        impl < $($generics),* > $name < $($generics),* > {
            /// Creates a new cache padded inner type.
            pub const fn new() -> Self {
                Self {
                    inner_type: $default,
                    _align: std::mem::MaybeUninit::uninit(),
                }
            }
        }

        impl < $($generics),* > std::ops::Deref for $name < $($generics),* > {
            type Target = $atomic < $($atomic_generics),* >;

            fn deref(&self) -> &Self::Target {
                &self.inner_type
            }
        }

        impl < $($generics),* > std::ops::DerefMut for $name < $($generics),* > {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner_type
            }
        }

        impl < $($generics),* > Default for $name < $($generics),* > {
            fn default() -> Self {
                Self::new()
            }
        }
    };
    ($name:ident, $atomic:ident, $default:block) => {
        /// Cache padded inner type. Can be dereferenced to the inner type.
        pub struct $name {
            inner_type: $atomic,
            _align: core::mem::MaybeUninit<
                [u8; if size_of::<$atomic>() > $crate::cache_padded::CACHE_LINE_SIZE {
                    0
                } else {
                    $crate::cache_padded::CACHE_LINE_SIZE - size_of::<$atomic>()
                }],
            >,
        }

        impl $name {
            /// Creates a new cache padded inner type.
            pub const fn new() -> Self {
                Self {
                    inner_type: $default,
                    _align: core::mem::MaybeUninit::uninit(),
                }
            }
        }

        impl core::ops::Deref for $name {
            type Target = $atomic;

            fn deref(&self) -> &Self::Target {
                &self.inner_type
            }
        }

        impl core::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner_type
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

generate_cache_padded_type!(CachePaddedAtomicU8, AtomicU8, { AtomicU8::new(0) });
generate_cache_padded_type!(CachePaddedAtomicU16, AtomicU16, { AtomicU16::new(0) });
generate_cache_padded_type!(CachePaddedAtomicU32, AtomicU32, { AtomicU32::new(0) });
generate_cache_padded_type!(CachePaddedAtomicU64, AtomicU64, { AtomicU64::new(0) });
generate_cache_padded_type!(CachePaddedAtomicUsize, AtomicUsize, { AtomicUsize::new(0) });

generate_cache_padded_type!(CachePaddedAtomicI8, AtomicI8, { AtomicI8::new(0) });
generate_cache_padded_type!(CachePaddedAtomicI16, AtomicI16, { AtomicI16::new(0) });
generate_cache_padded_type!(CachePaddedAtomicI32, AtomicI32, { AtomicI32::new(0) });
generate_cache_padded_type!(CachePaddedAtomicI64, AtomicI64, { AtomicI64::new(0) });
generate_cache_padded_type!(CachePaddedAtomicIsize, AtomicIsize, { AtomicIsize::new(0) });

/// Cache padded inner type. Can be dereferenced to the inner type.
pub(crate) struct CachePaddedAtomicPtr<T> {
    inner_type: AtomicPtr<T>,
    _align: core::mem::MaybeUninit<[u8; CACHE_LINE_SIZE - size_of::<usize>()]>,
}

impl<T> CachePaddedAtomicPtr<T> {
    /// Creates a new cache padded inner type.
    pub const fn new() -> Self {
        Self {
            inner_type: AtomicPtr::new(core::ptr::null_mut()),
            _align: core::mem::MaybeUninit::uninit(),
        }
    }
}

impl<T> core::ops::Deref for CachePaddedAtomicPtr<T> {
    type Target = AtomicPtr<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner_type
    }
}

impl<T> core::ops::DerefMut for CachePaddedAtomicPtr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner_type
    }
}

impl<T> Default for CachePaddedAtomicPtr<T> {
    fn default() -> Self {
        Self::new()
    }
}
