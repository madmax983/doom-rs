## 2024-05-18 - [Havoc: OOM on TEXTURE1 parser]
**Learning:** Doom's TEXTURE1 parsing uses a direct 4-byte `num_textures` read to allocate `Vec::with_capacity(num_textures)`. Fuzzing this length with large values triggers an immediate OOM.
**Action:** Use `.min(data.len() / 4)` to clamp lengths derived from WAD/lump headers, preventing massive allocations while still ensuring we parse valid entries up to the slice boundary.

**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust,  attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use  when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.
**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust, `Vec::with_capacity` attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use `.min(REASONABLE_CAPACITY)` when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.
**[BspTree Cyclic Denial of Service]
**Learning:** Legacy format parsers (like Doom WAD loaders) often validate bounds (e.g. `node_idx < N_NODES`), but fail to validate graph properties (e.g. DAG constraints). If a node points to itself, a simple tree traversal (`point_in_subsector` or `max_depth`) will hang or stack overflow, enabling a Denial of Service via a maliciously crafted WAD.
**Action:** Implement a bounded Depth-First Search (DFS) with a cycle-detection stack (`validate_acyclic`) *after* basic bounds checking during the parsing/validation phase to catch infinite loops before they happen.
