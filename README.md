# orengine-utils

This repository provides utilities for building high-performance applications.

- **[`hints`](./src/hints.rs)** — compiler hints that affect how code is emitted or optimized.
- **[`backoff`](./src/backoff.rs)** — includes the [`Backoff`](./src/backoff.rs) structure for 
   implementing retry/backoff strategies.
- **[`cache_padded`](./src/cache_padded.rs)** — cache-padded atomic types and the `generate_cache_padded_type!` macro.
- **[`light_arc`](./src/light_arc.rs)** — provides the [`LightArc`](./src/light_arc.rs) type, 
    a lightweight reference-counted smart pointer.
- **[`instant`](./src/instant.rs)** — provides the [`OrengineInstant`](./src/instant.rs) type for
    efficient time handling and compact time representation.  
- **[`array_queue`](./src/array_queue.rs)** — provides the [`ArrayQueue`](./src/array_queue.rs) type, 
    an array-based queue implementation.
- **[`vec_queue`](./src/vec_queue.rs)** — provides the [`VecQueue`](./src/vec_queue.rs) type,
    an vector-based queue implementation.
- **[`config_macro`](./src/config_macro.rs)** — provides the `config_target_pointer_width_64`, 
    `config_target_pointer_width_32`, and `config_target_pointer_width_16` macros, 
    which are used to right compile the program based on the target platform.
