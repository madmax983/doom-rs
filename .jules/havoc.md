**Overflow in Combat radius_attack**
**Learning:** Found an integer overflow where multiplying maximum `i32` damage by `(radius - dist)` would easily panic standard `i32` bounds if the values were maliciously large (like from a fuzz target).
**Action:** Always cast bounds and modifiers to `i64` before multiplication during distance scaling, then clamp to `i32::MIN..i32::MAX` before casting back.
**Zero-Duration MUS Score Infinite Loop**
**Learning:** `while` loops dependent on iterating through a sequence to reach an end state can infinitely loop if the state resets entirely within a single iteration block because of 0-time advances (e.g. an event duration of 0 causing a full loop wrapping).
**Action:** Always validate that event sequences have a > 0 minimum duration, or impose a maximum iteration limit within loops that advance time-based state.

**[Havoc: OOM and Arithmetic Overflows in Audio Processors]**
**Learning:** Basic audio sampling processing math and absolute MIDI event time accumulations are prone to `u64` and `u32` overflows when dealing with unverified parameters like massive input sample rates (`u32::MAX`) or extreme chunk lengths, or sample rate of 0 dividing by zero and causing out of bounds allocations.
**Action:** Always use `.saturating_add()`, `.saturating_mul()`, and `.max(1)` clamps defensively around hardware-driven mathematical constraints.
**Blockmap OOB Silent Corruption**
**Learning:** Using `.unwrap_or(0)` on missing offsets in blockmap causes silent fallback to the file header instead of gracefully failing.
**Action:** Replace missing offset fallbacks with explicitly starting iteration at the end of the file or returning empty.
**Audio Mutex Deadlock Analysis**
**Learning:** Evaluated `loom` feature for synchronization primitive correctness in `doom-audio`. The use of standard Mutex inside the audio callback is generally a risk (priority inversion), but safe from deadlocks as verified by `loom` since locks are not acquired sequentially.
**Action:** When validating thread-safe state wrappers, attach `loom` harnesses to assert no deadlock interleavings exist.
