## Sentry's Journal

**Goal:** Provide context for testing strategy.
## 2024-03-18 - Unreachable state in GamePhaseController

**Learning:** `GamePhaseController::tick_intermission` assumes it's only called when `self.phase` is `GamePhase::Intermission`. While this is guaranteed by the current call site (`tick()`), if someone were to call `tick_intermission` directly (or change `tick()`) while in another state, it would hit an `unreachable!()` panic.

**Action:** Added a specific `#[should_panic]` test `tick_intermission_unreachable_panic` to guarantee this invariant is protected. Similarly for `MobjSlab::alloc`'s free-list checking logic.

## 2024-03-22 - Out of sync serialization logic for MobjKind
**Learning:** Adding new variants to `MobjKind` in `mobj.rs` requires manually updating the serialization mapping in `mobj_kind_from_u16` (`savegame.rs`). This manual mapping drifted from reality, meaning new mobj kinds (like `VileFire` and `BossCube`) would fail to load from savegames (returning `SaveError::Truncated`).
**Action:** Updated the test `mobj_kind_roundtrip` to cover the new bounds. In the future, prefer table-driven or derived serialization for such enums to prevent drift.
