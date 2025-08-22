//! This crate provides some useful utilities.
//!
//! - The [`hints module`](hints) provides hints to the compiler that affects how code
//!   should be emitted or optimized.
//! - The [`backoff module`](backoff) provides the [`Backoff`](backoff::Backoff) structure.
//! - The [`cache_padded module`](cache_padded) provides cache-padded atomics types and
//!   the [`generate_cache_padded_type`] macro.
//! - The [`light_arc module`](light arc) provides the [`LightArc`](light_arc::LightArc) type.
//! - The [`instant module`](instant) provides the [`OrengineInstant`](instant::OrengineInstant)
//!   type.

pub mod hints;
pub mod backoff;
pub mod cache_padded;
pub mod light_arc;
pub mod instant;