## 2023-10-27 - Test Coverage vs Trivial Getters
**Learning:** Testing simple `raw()` getters or other one-line simple structural accessors provides diminishing returns for test coverage and violates the Sentry persona.
**Action:** Focus strictly on branches containing actual logic, error handling, formatting, parsing, limits, math, and edge cases (especially in `doom-wad` parsing!).

## 2023-10-27 - Global State in Unit Tests
**Learning:** Mutating global static state (e.g. `FINESINE` atomic flag) in a single test causes flakiness because `cargo test` runs all tests concurrently in the same process.
**Action:** Avoid writing tests that temporarily swap global state. If an initialization path needs testing, consider using synchronization primitives or architectural changes (which require permission per Sentry rules).
