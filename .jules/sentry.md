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
## 2025-02-28 - [Robust Test Injection]
**Learning:** When injecting new tests into a `mod tests` block located at the end of a Rust file, Python scripts attempting regex or string replacement on brace pairs are prone to causing brace matching errors during compilation (`error: this file contains an unclosed delimiter`).
**Action:** Avoid brittle Python scripts for this purpose. Instead, use a bash approach: remove the file's final closing brace with `sed -i '$d' file.rs`, append the new test functions using `cat << 'EOF' >> file.rs`, and finish with the closing `}`.
