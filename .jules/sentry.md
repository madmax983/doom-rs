## 2024-03-18 - Unreachable state in GamePhaseController

**Learning:** `GamePhaseController::tick_intermission` assumes it's only called when `self.phase` is `GamePhase::Intermission`. While this is guaranteed by the current call site (`tick()`), if someone were to call `tick_intermission` directly (or change `tick()`) while in another state, it would hit an `unreachable!()` panic.

**Action:** Added a specific `#[should_panic]` test `tick_intermission_unreachable_panic` to guarantee this invariant is protected. Similarly for `MobjSlab::alloc`'s free-list checking logic.
## 2024-03-18 - Missing edge case testing in WAD directory lookups

**Learning:** `WadFile::find_lump_data` and `WadFile::lumps_between` lacked tests verifying their behavior when requested lumps weren't found or for boundary conditions like zero-sized marker lumps (e.g. `F_START`/`F_END`). Similarly, `WadStack` lacked tests for checking if it actually detects a valid `has_iwad` and retrieving ordered lumps.
**Action:** Added targeted test cases `map_lump_group_missing_lumps_returns_none`, `find_lump_data_returns_none_if_missing`, `lumps_between_returns_correct_range` in `WadFile`. For `WadStack`, added `has_iwad_detects_base_wad`, `all_lumps_iterates_in_order`, and `map_lump_group_searches_pwads_first`. For `LumpDef`, added basic checks `lump_def_end` and `lump_def_is_marker`. This improves branch coverage significantly for wad parsing and searching.
## 2026-03-30 - Prevent panic on out-of-bounds rendering of CogmindFrame
**Learning:** Direct slice indexing `self.cells[idx]` within `CogmindFrame::get` caused a panic if `Widget::render` queried out of bounds due to structural or manual bugs elsewhere.
**Action:** Use safe safe `.get(idx)` instead, which resolves to `None` gracefully.
## 2024-04-03 - [Testing Parsing Boundaries in doom-wad]
**Learning:** Found uncovered edge cases related to directory parsing where `infotableofs` could be negative, causing unexpected cast bugs or errors. In `doom-wad`, testing parsing logic requires explicitly feeding negative metadata offsets and validating `thiserror` types.
**Action:** Always check array bounds casts and offsets in parsing headers. Use `matches!(result, Err(WadError::DirectoryOutOfBounds { .. }))` for checking struct enum variants with fields.
## 2026-04-06 - Missing invalid state transitions to `StateNum::NULL` test

**Learning:** `tick_mobj` handles transitioning a mobj's state when its tics reach zero. If the lookup for `next_state` returns `None` (e.g. invalid state index), it falls back to `StateNum::NULL`. The transition to `NULL` correctly removes the mobj because `p_set_mobj_state` returns `false` for `NULL`, but this fallback path was completely uncovered by tests.
**Action:** When a fallback value like `StateNum::NULL` is provided in an `unwrap_or`, add a targeted unit test to ensure the fallback actually executes and does the right thing (e.g. removes the entity).

## 2024-04-08 - Added Missing `StateNum::NULL` test
**Learning:** Found uncovered branches related to the fallback `StateNum::NULL` handling in `tic.rs` when `unwrap_or(StateNum::NULL)` defaults.
**Action:** Added targeted test cases `tick_mobj_with_invalid_next_state_removes_entity` and `advance_mobj_state_with_invalid_state_holds_forever` in `tic.rs` to reach 100% test coverage on state transitions.
## 2026-04-10 - Replace unwrap() with expect()
**Learning:** Removing unwrap() reduces panic points and gives better crash context via panic messages, especially in file parsing logic.
**Action:** Replaced unwrap() with expect() in wad.rs, main.rs, savegame.rs, and net_mode.rs.
## 2024-04-10 - SaveError Refactoring
**Learning:** Replaced manual `Display` implementation for `SaveError` with `#[derive(thiserror::Error)]`. Used tests to verify correct routing of format decoding (`SaveFormat` formatting, too short save detection, magic byte mismatches) for both regular saves and `doomrs` specific loading.
**Action:** Always prefer `thiserror` when modifying error types that have simple manual implementations.

## 2024-04-11 - Adding tests for get_alive_target_with_pos
**Learning:** Discovered a lack of test coverage for the `get_alive_target_with_pos` function in `actions.rs`, which is used in actor logic.
**Action:** Added unit tests to ensure `get_alive_target_with_pos` correctly handles cases with no target, a dead target, and a valid alive target. This prevents regressions in monster targeting logic.
## 2023-10-27 - [doom-net] Improved RollbackManager and packet edge case tests
**Learning:** Found coverage gaps in `rollback.rs` around the rollback depth boundary conditions (it could potentially search back too far if not bounded), and `packet.rs` around missing data handling in `from_bytes` near the end of a command chunk.
**Action:** Wrote `get_inputs_predicts_stops_at_tic_0` boundary test to verify predictions properly avoid underflow and stop at tic 0. Wrote `from_bytes_rejects_missing_data_at_cmd_boundary` to ensure packet deserialization gracefully rejects malformed or truncated payload buffers near command boundary offsets. Fixed a `clippy::clone-on-copy` issue in `flat_cache.rs`.
## 2024-04-15 - Replace unwrap() with expect() in domain structs and tests
**Learning:** Found scattered instances of `.unwrap()` and `.unwrap_err()` in map parsing logic, primitives, and game logic, which can obscure test failure context or lead to uninformative panics.
**Action:** Replaced `.unwrap()` and `.unwrap_err()` with `.expect()` or `.expect_err()` to enforce providing explicit failure messages, making assertions clearer when parsing WAD data or managing the audio system.
## 2026-04-17 - Update oldest_tic in InputLog
**Learning:** InputLog::oldest_tic() returned 0 due to missing state update implementation which went undiscovered.
**Action:** Add unit tests to check state mutation methods verify public API changes are observed.
## 2026-04-20 - Replace unwrap calls with expect in tests
**Learning:** Found scattered instances of `.unwrap()` in test files (`savegame.rs`, `actions.rs`, `driver.rs`, etc) that obscured test failure context by panic-ing with a generic error message, which violates Sentry's principle that tests should provide meaningful context. Added targeted `havoc` testing to `MapAnalyzer` in `doom-map` to prove graph analysis is resilient against malformed/unconnected topological map data.
**Action:** Always replace `unwrap()` with `expect()` in tests to explicitly document the invariant and assist debugging. Always construct intentionally malformed inputs when testing analysis routines.
## 2026-04-20 - Uncovered panic point when truncating savegame byte bounds
**Learning:** The bounds checking in `savegame::ReadCursor::read_bytes` lacked explicit testing when passing a large chunk size exceeding bounds, presenting a hidden edge case.
**Action:** Always verify array slices when parsing files and provide missing boundary condition tests using `read_bytes_out_of_bounds` and similar cases to increase safety.
## 2024-05-18 - Graceful degradation for unconnected/malformed topologies

**Learning:** `MapAnalyzer::chokepoints` previously lacked proper testing to verify resilience against malformed graphs, particularly graphs that included asymmetric connections or nonexistent child nodes in the adjacency list.
**Action:** Adding tests like `havoc_test_analyzer_does_not_panic_on_asymmetric_edges` and `havoc_test_analyzer_missing_back_edges` proactively protects analysis functions against dirty maps without failing safely.
## 2026-04-23 - Added Missing `StateNum::NULL` test for tick_mobj fallback
**Learning:** Found an uncovered branch related to the fallback `StateNum::NULL` handling in `tic.rs` when `unwrap_or(StateNum::NULL)` defaults due to the current state being `StateNum::NULL`.
**Action:** Added targeted test case `tick_mobj_removes_entity_when_current_state_is_null` in `tic.rs` to reach 100% test coverage on state transitions.
## 2024-04-25 - Prevented generic unwrap panics across the codebase
**Learning:** Found numerous `unwrap()` calls in test files which obscured test failure context and violated Sentry's principles.
**Action:** Replaced `.unwrap()` with `.expect("value must exist in test")` to explicitly document the invariants in tests across multiple modules (`sight.rs`, `combat.rs`, `spawn.rs`, etc) to assist with debugging.
## 2024-06-29 - Fixed unwrap panics in weapon psprite state ticking
**Learning:** The psprite state ticking logic implicitly assumed that the transition arrays are well-formed. However, game states can contain out-of-bounds indices (for example via bad demo/save data). `StateNum::NULL` provides a clean fallback to prevent crashes.
**Action:** Wrote a test case `tick_psprite_slot_invalid_state_transitions_to_null` that injects an invalid psprite state and verifies the state drops to NULL safely without panicking.
