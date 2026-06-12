**DehPatch parser panic due to float cast overflow**
**Learning:** `f64` parsed from a giant string of digits (e.g. `100` characters) successfully parses into an extremely large floating point value, which then panics or produces unexpected results when forcefully cast back down to `i32` or `usize` if the logic relies on standard clamped bounds but forgets how massive numbers behave inside casts.
**Action:** Add length limits on incoming float-like strings or explicitly bound checks prior to mapping the error so it doesn't cause out-of-bounds panics or infinite behavior inside `parse_float_fallback`.

**DoomRs Savegame read_bytes unbounded OOM/Truncated logic**
**Learning:** Deserializers often trust user-provided lengths for strings/vectors. Reading a 4-byte value and treating it directly as `usize` for a subsequent array slice causes OOMs or massive allocations when given garbage fuzz data like `0xFFFFFFFF`.
**Action:** Always clamp arbitrary length headers (like `name_len > 4096`) to sane bounds before allocating or advancing cursors.

**DFS MapAnalyzer stack overflow on linear graphs**
**Learning:** Iterative DFS needs careful parent/children tracking. If the `children_map` isn't bounded or the back-edges evaluation takes excessively long, an extremely deep linear map (10000+ nodes) can blow up the internal stack or lead to exponential backtracking if poorly formulated.
**Action:** Simplify iterative DFS logic and ensure node traversals pop/push safely without retaining excessive duplicate state on the stack.

**Loom Deadlock on AudioDriver**
**Learning:** (Tested, but didn't repro. Null driver is currently safe).
**Action:** Always verify `loom` output carefully with `cfg(loom)` paths.
