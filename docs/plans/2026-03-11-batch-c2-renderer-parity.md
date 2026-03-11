# Batch C2 Renderer Parity Implementation Plan

Status: Completed on 2026-03-11 for visplane reuse, sky projection, and renderer-side sector ownership hardening. Raw subsector-order parity remains open.

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Finish the deeper renderer parity work from the Chocolate Doom audit by fixing visplane reuse, sky vertical projection, and subsector/sector-order assumptions without destabilizing the playable slice.

**Architecture:** Keep the software renderer structure intact, but make three targeted changes. First, teach visplane allocation to reuse a matching plane when the overlap window is still empty, matching Doom's `R_CheckPlane` shape instead of splitting on any raw range overlap. Second, replace the current "top half of the screen only" sky V mapping with a Doom-shaped horizon-relative projection. Third, remove ad hoc nearest-first seg resorting inside subsectors and lean on BSP/subsector order plus the map's own sector ownership data instead of renderer heuristics.

**Tech Stack:** Rust workspace crates `doom-renderer` and `doom-map`, unit tests with `cargo test`.

---

## Outcome

- `r_check_plane()` now behaves like Doom's allocator for the current plane: reuse the passed-in plane when its overlap window is empty, otherwise allocate a fresh sibling instead of scavenging some older matching plane.
- Sky V mapping now uses Doom-shaped `skytexturemid` behavior instead of stretching the top half of the screen and clamping the bottom half to the final texel.
- Renderer-side player sector lookup now prefers the map-owned subsector sector and only falls back to the older nearest-seg heuristic for malformed or synthetic no-BSP scenes.
- Subsector raw-order parity was investigated but not landed. Swapped same-subsector portal torture cases still depend on the hardening sort, so that mismatch remains explicitly open instead of being “fixed” by regressing clipping.

### Task 1: Visplane Reuse Matches Doom

**Files:**
- Modify: `crates/doom-renderer/src/visplane.rs`
- Test: `crates/doom-renderer/src/visplane.rs`

**Step 1: Write the failing tests**

- Add a regression proving `r_check_plane()` reuses an existing plane when the requested union range overlaps the plane bounds but the overlapped columns are still empty.
- Add a second regression proving it still splits when the overlapped columns are already occupied.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-renderer visplane_reuse -- --nocapture
```

Expected: the reuse case fails because the current code treats any occupied column anywhere in `start_x..=end_x` as a conflict.

**Step 3: Write the minimal implementation**

- Rework `VisplaneSet::r_check_plane()` so it checks the Doom-style union range between the plane's current `min_x..max_x` and the requested `start_x..end_x`.
- Reuse the plane when the overlapping range inside that union is empty.
- Split only when the overlap window contains already-occupied columns.
- Keep the existing per-column top/bottom storage and `r_find_plane()` keying unchanged.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-renderer visplane_reuse -- --nocapture
```

Expected: the reuse and split regressions both pass.

Result:

- Landed in `crates/doom-renderer/src/visplane.rs`.
- The focused visplane reuse regressions pass, including the Doom-shaped “conflict allocates a fresh plane even when another matching sibling exists” case.

### Task 2: Sky Vertical Projection Uses Doom-Shaped Horizon Mapping

**Files:**
- Modify: `crates/doom-renderer/src/sky.rs`
- Modify: `crates/doom-renderer/src/render.rs`
- Test: `crates/doom-renderer/src/sky.rs`

**Step 1: Write the failing tests**

- Add a regression proving sky rows below the horizon are not clamped to the bottom texel.
- Add a regression proving the vertical mapping is centered around the screen horizon rather than a hardcoded "top 100 rows" scale.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-renderer sky_vertical_mapping -- --nocapture
```

Expected: failure because the current code clamps everything below row 99 to the final sky texel.

**Step 3: Write the minimal implementation**

- Introduce a small helper in `sky.rs` that maps screen row to sky texture row using a horizon-relative formula instead of `y * tex_h / 100`.
- Use that helper in both `draw_sky_columns()` and `draw_sky_coverage_columns()`.
- Only touch `render.rs` if the helper needs explicit horizon input; otherwise keep the render call surface unchanged.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-renderer sky_vertical_mapping -- --nocapture
```

Expected: the new sky vertical regressions pass.

Result:

- Landed in `crates/doom-renderer/src/sky.rs`.
- Sky rows now advance across the full screen and wrap by logical height instead of smearing the final texel below the horizon.

### Task 3: Remove Subsector Seg Resorting And Renderer Sector Heuristics

**Files:**
- Modify: `crates/doom-renderer/src/seg.rs`
- Modify: `crates/doom-renderer/src/render.rs`
- Test: `crates/doom-renderer/src/seg.rs`
- Test: `crates/doom-renderer/src/render.rs`

**Step 1: Write the failing tests**

- Add a regression proving subsector seg traversal preserves WAD/BSP subsector order instead of resorting by synthetic nearest-first distance.
- Add a renderer regression across multiple player positions/angles showing far-wall clipping remains stable after removing that sort.
- Add a focused regression for `player_sector_index()` showing player sector resolution comes from map ownership instead of nearest seg heuristics inside a mixed subsector.

**Step 2: Run tests to verify they fail**

Run:
```powershell
cargo test -p doom-renderer subsector_seg_order -- --nocapture
cargo test -p doom-renderer player_sector_index -- --nocapture
```

Expected: failure because subsector segs are still being explicitly resorted and player sector lookup still guesses from nearby seg geometry.

**Step 3: Write the minimal implementation**

- Remove `ordered_subsector_segs()` distance sorting and iterate subsector segs in stored order.
- Keep BSP near/far child traversal, but stop imposing extra intra-subsector ordering.
- Simplify `player_sector_index()` to prefer map-owned sector lookup instead of nearest-seg scoring, while preserving safe fallback behavior for malformed synthetic data.

**Step 4: Run tests to verify they pass**

Run:
```powershell
cargo test -p doom-renderer subsector_seg_order -- --nocapture
cargo test -p doom-renderer player_sector_index -- --nocapture
```

Expected: seg-order and player-sector regressions pass.

Result:

- `player_sector_index()` was hardened in `crates/doom-renderer/src/render.rs` to prefer `Level::sector_index_at()` and only fall back to nearest-seg scoring for malformed or synthetic levels.
- The `player_sector_index` regression passes.
- Removing subsector seg resorting did not survive renderer verification: swapped same-subsector portal torture scenes still leaked or over-clipped when the hardening sort was removed. The sorter stays for now, and the new renderer regressions document that dependency explicitly.

### Task 4: Batch Verification And Audit Update

**Files:**
- Modify: `docs/plans/2026-03-10-chocolate-doom-parity-audit.md`
- Verify only: `crates/doom-renderer`
- Verify only: `crates/doom-app`

**Step 1: Run focused regressions**

Run:
```powershell
cargo test -p doom-renderer visplane_reuse -- --nocapture
cargo test -p doom-renderer sky_vertical_mapping -- --nocapture
cargo test -p doom-renderer subsector_seg_order -- --nocapture
cargo test -p doom-renderer player_sector_index -- --nocapture
```

**Step 2: Run broader verification**

Run:
```powershell
cargo fmt --all
cargo test -p doom-renderer --lib
cargo test -p doom-app
```

Expected: focused Batch C2 regressions and the broader renderer/app suites pass.

Actual verification run:

```powershell
cargo test -p doom-renderer visplane_reuse -- --nocapture
cargo test -p doom-renderer sky_vertical_mapping -- --nocapture
cargo test -p doom-renderer player_sector_index -- --nocapture
cargo test -p doom-renderer test_far_solid_wall_clipping_does_not_depend_on_subsector_seg_order -- --nocapture
cargo test -p doom-renderer test_far_portal_visplanes_do_not_depend_on_subsector_seg_order -- --nocapture
cargo fmt --all
cargo test -p doom-renderer --lib
cargo test -p doom-app
```

Verification result:

- All focused Batch C2 regressions passed.
- `cargo test -p doom-renderer --lib` passed with 713 tests.
- `cargo test -p doom-app` passed with 93 tests.
- Existing unrelated warnings remain in the tree and were not part of this batch.

**Step 3: Update the audit log**

- Mark Batch C2 complete in `docs/plans/2026-03-10-chocolate-doom-parity-audit.md`.
- Record what remains explicitly open in renderer parity after this batch.
