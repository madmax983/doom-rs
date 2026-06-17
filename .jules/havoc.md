## 2024-05-18 - [Havoc: OOM on TEXTURE1 parser]
**Learning:** Doom's TEXTURE1 parsing uses a direct 4-byte `num_textures` read to allocate `Vec::with_capacity(num_textures)`. Fuzzing this length with large values triggers an immediate OOM.
**Action:** Use `.min(data.len() / 4)` to clamp lengths derived from WAD/lump headers, preventing massive allocations while still ensuring we parse valid entries up to the slice boundary.

**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust,  attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use  when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.
**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust, `Vec::with_capacity` attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use `.min(REASONABLE_CAPACITY)` when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.
**[Map Analyzer Graph Traversal Memory Exhaustion]**
**Learning:** Untrusted level topologies can present huge, cyclic, or disjointed graphs containing millions of empty/dummy sectors. `chokepoints()` and `isolated_areas()` used a `while let Some(...) = stack.pop()` algorithm allocating `HashSet` and tracking paths per node. Unbounded inputs force exponential RAM usage (OOM) or extreme iteration times, bringing the whole server/app down.
**Action:** Always place an explicit upper bound on traversals in data structures derived from IO. I clamped graph node limits to `65536` for tactical analysis—the absolute bounds of the `doom-types` system. This safely handles `doom-app` logic and gracefully yields a `Result::Err` so callers can discard analysis on massive invalid levels without panicking.
