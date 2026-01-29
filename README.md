# orengine-utils

This repository provides utilities for building high-performance applications.

- **[`hints`](./src/hints.rs)** — compiler hints that affect how code is emitted or optimized.
- **[`backoff`](./src/backoff.rs)** — includes the [`Backoff`](./src/backoff.rs) structure for 
   implementing retry/backoff strategies.
- **[`cache_padded`](./src/cache_padded.rs)** — The [`cache_padded module`](./src/cache_padded.rs) provides cache-padded
  atomics types and
  the [`CachePadded`](./src/cache_padded.rs) wrapper.
- **[`light_arc`](./src/light_arc.rs)** — provides the [`LightArc`](./src/light_arc.rs) type, 
  a lightweight reference-counted smart pointer.
- **[`instant`](./src/instant.rs)** — provides the [`OrengineInstant`](./src/instant.rs) type for
  efficient time handling and compact time representation. It is unavalible with the `no_std` feature.
- **[`array_queue`](./src/array_queue.rs)** — provides the [`ArrayQueue`](./src/array_queue.rs) type, 
  an array-based queue implementation.
- **[`vec_queue`](./src/vec_queue.rs)** — provides the [`VecQueue`](./src/vec_queue.rs) type,
  a vector-based queue implementation.
- **[`numa`](./src/numa.rs)** — provides sufficient utilities for working with NUMA nodes.
- **[`config_macro`](./src/config_macro.rs)** — provides the `config_target_pointer_width_64`, 
    `config_target_pointer_width_32`, and `config_target_pointer_width_16` macros, 
  which are used to right compile the program based on the target platform.
- **[`number_key_map`](./src/number_key_map.rs)** This module provides the [`NumberKeyMap`](./src/number_key_map.rs) struct,
  a compact open-addressing map specialized for `usize` keys optimized for zero-misses and so optimized 
  for 99+% reading operations.

# `no-std`

It provides the `no_std` feature, that makes it use `core` and `alloc` crates instead of `std`.
With this feature, this crate provides almost all the functionality. It excludes the `instant` module.