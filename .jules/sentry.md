## 2024-05-20 - Uncovered panic point when truncating savegame byte bounds
**Learning:** The bounds checking in `savegame::ReadCursor::read_bytes` lacked explicit testing when passing a large chunk size exceeding bounds, presenting a hidden edge case.
**Action:** Always verify array slices when parsing files and provide missing boundary condition tests using `read_bytes_out_of_bounds` and similar cases to increase safety.
## 2024-05-21 - Added Missing `StateNum::NULL` test for tick_mobj fallback
**Learning:** Found an uncovered branch related to the fallback `StateNum::NULL` handling in `tic.rs` when `unwrap_or(StateNum::NULL)` defaults due to the current state being `StateNum::NULL`.
**Action:** Added targeted test case `tick_mobj_removes_entity_when_current_state_is_null` in `tic.rs` to reach 100% test coverage on state transitions.
## 2024-05-23 - Prevented generic unwrap panics across the codebase
**Learning:** Found numerous `unwrap()` calls in test files which obscured test failure context and violated Sentry's principles.
**Action:** Replaced `.unwrap()` with `.expect("value must exist in test")` to explicitly document the invariants in tests across multiple modules (`sight.rs`, `combat.rs`, `spawn.rs`, etc) to assist with debugging.
## 2024-05-24 - MapAnalyzer asymmetric edges unwrap panic
**Learning:** Discovered that MapAnalyzer::chokepoints in doom-map would panic on an unwrap if a node was present in the adjacency list values (as a destination node) but completely missing from the adjacency list keys (having no outgoing edges).
**Action:** Replaced the unwrap with a fallback to an empty set and wrote specific tests to prove `MapAnalyzer` handles asymmetric edges properly. Never assume that graph traversal edges are symmetric or that destination nodes exist as keys.
