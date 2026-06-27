**[loom-deadlock-doom-audio]**
**Learning:** `loom` can effectively detect deadlocks caused by concurrent calls to non-atomic shared structures in `Mutex`. In `doom-audio` testing, `SfxMixer::stop_all()` used an iterator approach to assign `None` to channels which compiled fine under normal conditions, but raised issues when analyzed for data races/deadlocks or linting (`clippy::manual_slice_fill`). It could have masked more serious concurrency flaws if other threads accessed partial state.
**Action:** Always prefer atomic-like mass assignments (`fill`) when working inside a lock to minimize the critical section duration and prevent partial state exposure, especially when fuzzing or using `loom`.

**[fuzzer-oom-dehacked-buffer]**
**Learning:** Parsing untrusted payloads with lengths directly feeding `Vec::with_capacity` (or iterating bounds) is a prime target for OOMs from fuzzers (or malicious actors). The engine was protected against OOM in Dehacked strings via physical bounds checking (`count.min(data.len() / entry_size)`), demonstrating the need to validate logical limits against actual byte constraints.
**Action:** Never trust parsed integer lengths. Always constrain allocations by physical limits derived from the payload size before allocating.

**[unnecessary-min-or-max]**
**Learning:** Fuzzers and edge-case testing might highlight redundant code paths that do nothing because the bounds check is logically impossible to fail (e.g., `max(0)` on an unsigned integer or an already-checked condition).
**Action:** Remove redundant logic that clutters the execution path.
