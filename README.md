# orengine-utils

This repository provides utilities for building high-performance applications.

- **[`hints`](./src/hints.rs)** — compiler hints that affect how code is emitted or optimized.
- **[`backoff`](./src/backoff.rs)** — includes the [`Backoff`](./src/backoff.rs) structure for 
   implementing retry/backoff strategies.
- **[`cache_padded`](./src/cache_padded.rs)** — cache-padded atomic types and the `generate_cache_padded_atomic!` macro.
- **[`light_arc`](./src/light_arc.rs)** — provides the [`LightArc`](./src/light_arc.rs) type, 
    a lightweight reference-counted smart pointer.
- **[`instant`](./src/instant.rs)** — provides the [`OrengineInstant`](./src/instant.rs) type for
    efficient time handling and compact time representation.  