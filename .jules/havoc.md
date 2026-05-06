## 2024-05-18 - [Havoc: OOM on TEXTURE1 parser]
**Learning:** Doom's TEXTURE1 parsing uses a direct 4-byte `num_textures` read to allocate `Vec::with_capacity(num_textures)`. Fuzzing this length with large values triggers an immediate OOM.
**Action:** Use `.min(data.len() / 4)` to clamp lengths derived from WAD/lump headers, preventing massive allocations while still ensuring we parse valid entries up to the slice boundary.

**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust,  attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use  when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.
**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust, `Vec::with_capacity` attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use `.min(REASONABLE_CAPACITY)` when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.
**Loom testing on sfx_mixer**
**Learning:** loom can test data races and deadlocks. Added tests to the SfxMixer since it is shared across threads via Arc<Mutex<>>. Verified that basic play/mix operations and updates are safe.
**Action:** Always add loom::model tests for new concurrent structures.
