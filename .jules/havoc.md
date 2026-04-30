## 2024-05-18 - [Havoc: OOM on TEXTURE1 parser]
**Learning:** Doom's TEXTURE1 parsing uses a direct 4-byte `num_textures` read to allocate `Vec::with_capacity(num_textures)`. Fuzzing this length with large values triggers an immediate OOM.
**Action:** Use `.min(data.len() / 4)` to clamp lengths derived from WAD/lump headers, preventing massive allocations while still ensuring we parse valid entries up to the slice boundary.

**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust,  attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use  when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.
**Havoc: Bounds-checking allocations**
**Learning:** Uncapped allocations driven by input (like network packets or save files) can cause AddressSanitizer/allocator Out-Of-Memory errors and Denial of Service. In Rust, `Vec::with_capacity` attempts to allocate the requested size immediately, leading to massive memory usage when the capacity is arbitrary.
**Action:** Use `.min(REASONABLE_CAPACITY)` when reserving memory based on input-controlled sizes. Limit capacities on things like Network rollbacks or save game parsers.

## 2024-05-02 - Removed unsafe mutable static trig initialization
**Learning:** `doom-types/src/angle.rs` contained a `static mut SINE_TABLE` initialized at runtime using an `AtomicBool` guard (`init_trig_tables`). This required `unsafe` blocks sprinkled throughout every downstream crate (`doom-app`, `doom-game`, `doom-renderer`, `doom-tui`) just to call `.sin()` or `.cos()`. It was brittle, caused race condition paranoia, and required careful initialization ordering (e.g. `INIT.call_once(|| ...)`).
**Action:** Replaced runtime trig initialization with a compile-time static array. Wrote a python script to precompute the `[Fixed16_16; 8192]` table and embedded it as `static SINE_TABLE` in `trig_table.rs` via `include!`. This allowed making `sin()` and `cos()` completely pure and safe functions, removing `init_trig_tables` entirely, and deleting all associated `unsafe` blocks and `std::sync::Once` initialization patterns across the entire codebase.

## 2024-05-02 - Loom verified AudioDriver thread safety
**Learning:** `doom-audio/src/driver.rs` uses an `Arc<Mutex<SfxMixer>>` shared between the background audio thread and the event loop. The `SfxMixer::mix` function runs in the tight background loop while `SfxMixer::play` and `SfxMixer::stop_all` run from the main thread.
**Action:** Added `havoc_loom_deadlock_test` to `doom-audio` to verify that concurrent calls to `mix()` and `stop_all()` on cloned `driver.mixer` locks do not deadlock. Loom verified thread safety successfully.
