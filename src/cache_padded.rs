//! Provides cache-padded atomic types.
use std::sync::atomic::{
    AtomicI16, AtomicI32, AtomicI64, AtomicI8, AtomicIsize, AtomicU16, AtomicU32, AtomicU64,
    AtomicU8, AtomicUsize,
};

#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "arm64ec",
    target_arch = "powerpc64",
))]
const ALIGN: usize = 128;

#[cfg(any(
    target_arch = "arm",
    target_arch = "mips",
    target_arch = "mips32r6",
    target_arch = "mips64",
    target_arch = "mips64r6",
    target_arch = "sparc",
    target_arch = "hexagon",
))]
const ALIGN: usize = 32;

#[cfg(target_arch = "m68k")]
const ALIGN: usize = 16;

#[cfg(target_arch = "s390x")]
const ALIGN: usize = 256;

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
const ALIGN: usize = 64;

#[macro_export]
macro_rules! generate_cache_padded_atomic {
    ($name:ident, $atomic:ident) => {
        /// Cache padded atomic. Can be dereferenced to the inner atomic.
        pub struct $name {
            atomic: $atomic,
            _align: std::mem::MaybeUninit<
                [u8; if size_of::<$atomic>() > $crate::cache_padded::ALIGN {
                    0
                } else {
                    ALIGN - size_of::<$atomic>()
                }],
            >,
        }

        impl $name {
            /// Creates a new cache padded atomic.
            pub const fn new() -> Self {
                Self {
                    atomic: $atomic::new(0),
                    _align: std::mem::MaybeUninit::uninit(),
                }
            }
        }

        impl std::ops::Deref for $name {
            type Target = $atomic;

            fn deref(&self) -> &Self::Target {
                &self.atomic
            }
        }

        impl std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.atomic
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

generate_cache_padded_atomic!(CachePaddedAtomicU8, AtomicU8);
generate_cache_padded_atomic!(CachePaddedAtomicU16, AtomicU16);
generate_cache_padded_atomic!(CachePaddedAtomicU32, AtomicU32);
generate_cache_padded_atomic!(CachePaddedAtomicU64, AtomicU64);
generate_cache_padded_atomic!(CachePaddedAtomicUsize, AtomicUsize);

generate_cache_padded_atomic!(CachePaddedAtomicI8, AtomicI8);
generate_cache_padded_atomic!(CachePaddedAtomicI16, AtomicI16);
generate_cache_padded_atomic!(CachePaddedAtomicI32, AtomicI32);
generate_cache_padded_atomic!(CachePaddedAtomicI64, AtomicI64);
generate_cache_padded_atomic!(CachePaddedAtomicIsize, AtomicIsize);