//! This crate provides some useful utilities.
//!
//! - The [`hints module`](hints) provides hints to the compiler that affects how code
//!   should be emitted or optimized.
//! - The [`backoff module`](backoff) provides the [`Backoff`](backoff::Backoff) structure.
//! - The [`cache_padded module`](cache_padded) provides cache-padded atomics types and
//!   the [`generate_cache_padded_type`] macro.
//! - The [`light_arc module`](light arc) provides the [`LightArc`](light_arc::LightArc) type.
//! - The [`OrengineInstant`] that is a monotone clock that weights 8 bytes on Unix-like systems.
//! - The [`ArrayQueue`] that is an array-based queue implementation.
//! - Configuration macros that are used to right compile the program based on the target platform
//!   such as [`config_target_pointer_width_64`], [`config_target_pointer_width_32`], and
//!   [`config_target_pointer_width_16`].

pub mod hints;
pub mod backoff;
pub mod cache_padded;
pub mod light_arc;
mod instant;
mod clear_with;
mod array_queue;
mod config_macro;

pub use clear_with::*;
pub use array_queue::ArrayQueue;
pub use instant::OrengineInstant;