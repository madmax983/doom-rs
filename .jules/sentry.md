## Sentry's Journal

**Goal:** Provide context for testing strategy.
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

## 2026-04-06 - Remove unwrap from walkline trigger classification
**Learning:** Found a `.unwrap()` panic risk inside `dispatch_walk_lines` where `classify_trigger` was called twice: once to filter, and again later where it was unwrapped. While the filter conceptually guaranteed `Some`, it's a "ticking time bomb" if the logic ever decoupled.
**Action:** Instead of just testing the explosion, "removed the fuse" by extracting the `TriggerType` directly in the `filter_map` (using a pattern guard `Some(trigger) if matches!(...)`) and passing it through the tuple to the downstream consumer, avoiding the second lookup and the `unwrap()` entirely.
