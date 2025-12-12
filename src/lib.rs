//! This crate provides some useful utilities.
//!
//! - The [`hints module`](hints) provides hints to the compiler that affects how code
//!   should be emitted or optimized.
//! - The [`backoff module`](backoff) provides the [`Backoff`](backoff::Backoff) structure.
//! - The [`cache_padded module`](cache_padded) provides cache-padded atomics types and
//!   the [`CachePadded`] wrapper.
//! - The [`light_arc module`](light_arc) provides the [`LightArc`](light_arc::LightArc) type.
//! - The [`OrengineInstant`] that is a monotone clock that weights 8 bytes on Unix-like systems.
//! - The [`ArrayQueue`] that is an array-based queue implementation.
//! - The [`VecQueue`] that is a vector-based queue implementation.
//! - The [`NumberKeyMap`] that is a compact open-addressing map specialized for `usize`
//!   keys optimized for zero-misses and so optimized for 99+% reading operations.
//! - Configuration macros that are used to right compile the program based on the target platform
//!   such as [`config_target_pointer_width_64`], [`config_target_pointer_width_32`], and
//!   [`config_target_pointer_width_16`].

#![cfg_attr(feature = "no_std", no_std)]
#![deny(clippy::all)]
#![deny(clippy::assertions_on_result_states)]
#![deny(clippy::match_wild_err_arm)]
#![deny(clippy::allow_attributes_without_reason)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![allow(async_fn_in_trait, reason = "It improves readability.")]
#![allow(
    clippy::missing_const_for_fn,
    reason = "Since we cannot make a constant function non-constant after its release,
    we need to look for a reason to make it constant, and not vice versa."
)]
#![allow(clippy::inline_always, reason = "We write highly optimized code.")]
#![allow(
    clippy::must_use_candidate,
    reason = "It is better to developer think about it."
)]
#![allow(
    clippy::module_name_repetitions,
    reason = "This is acceptable most of the time."
)]
#![allow(
    clippy::missing_errors_doc,
    reason = "Unless the error is something special,
    the developer should document it."
)]
#![allow(clippy::redundant_pub_crate, reason = "It improves readability.")]
#![allow(clippy::struct_field_names, reason = "It improves readability.")]
#![allow(
    clippy::module_inception,
    reason = "It is fine if a file in has the same mane as a module."
)]
#![allow(clippy::if_not_else, reason = "It improves readability.")]
#![allow(
    rustdoc::private_intra_doc_links,
    reason = "It allows to create more readable docs."
)]
#![allow(
    clippy::negative_feature_names,
    reason = "It is needed to allow the `no_std` feature."
)]

extern crate alloc;

mod array_buffer;
mod array_queue;
pub mod backoff;
pub mod cache_padded;
mod clear_with;
mod config_macro;
pub mod hints;
#[cfg(not(feature = "no_std"))]
mod instant;
pub mod light_arc;
pub mod number_key_map;
mod vec_queue;

pub use array_buffer::ArrayBuffer;
pub use array_queue::ArrayQueue;
pub use clear_with::*;
#[cfg(not(feature = "no_std"))]
pub use instant::OrengineInstant;
#[cfg(not(feature = "no_std"))]
pub use number_key_map::NumberKeyMap;
pub use vec_queue::VecQueue;
