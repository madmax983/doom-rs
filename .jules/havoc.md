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
**OOM in `tokenize_response_file`**
**Learning:** `tokenize_response_file` used `content.chars().collect::<Vec<char>>()` to iterate over characters which triggers an O(N) heap allocation, leading to OOM on very large response files.
**Action:** Replace string collection and indexed traversal with `char_indices().peekable()` to iterate over bounds directly and slice the original `&str`.

**No Unhandled Aborts in string unwraps (`doom_game::savegame`)**
**Learning:** Evaluated how string representations are handled during Savegame loads (e.g. `desc_str` strings in test bounds and `unwrap_or("")`). All cases safely map to empty strings if UTF-8 coercion fails, mitigating runtime aborts.
**Action:** Always safely fallback using `.unwrap_or("")` when parsing string-like metadata from unknown bytes.

**No Unhandled Aborts Found in `doom-net` unwraps**
**Learning:** Evaluated how `unwrap_or`, `unwrap`, and `expect` are used inside the `doom-net` parsing logic (e.g. `TicPacket::from_bytes`). All occurrences inside network handling are checked results matching test bounds constraints, mitigating malicious network inputs aborting the engine unexpectedly.
**Action:** Continue to bound networking packets carefully.

**Audio System deadlocks under Loom permutations**
**Learning:** `audio_cmd_thread` uses a standard `std::sync::mpsc::Receiver::recv()` which blocks indefinitely. When run under Loom permutations with threads simulating drops, this blocks the entire permutation checker, essentially timing out/deadlocking the test runner.
**Action:** Do not use `loom` for testing standard library MPSC channels unless custom drop-aware or loom-specific alternatives are implemented in the main code.
