# Batch C1 Renderer Parity Implementation Plan

Status: Completed on 2026-03-11

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the highest-visibility remaining renderer parity issues from the Chocolate Doom audit: logical texture height pegging, map-specific sky selection, and masked midtexture ordering against sprites.

**Architecture:** Keep the changes targeted. Preserve the existing software renderer and framebuffer path, but separate logical texture height from padded cache height, select the sky texture from the level name instead of hardcoding `SKY1`, and move masked midtexture columns into a deferred depth-sorted pass that can interleave with sprite rendering.

**Tech Stack:** Rust workspace crates `doom-renderer` and `doom-app`, unit tests with `cargo test`.

## Outcome

- Pegging now uses `WallTexture.logical_height` instead of padded cache height in the wall paths that match Chocolate Doom's pegging logic.
- Sky selection now resolves from `sky_texture_name(level.name.as_str())` instead of hardcoding `SKY1`.
- Masked midtextures are collected during wall rendering and replayed in a deferred masked pass that depth-sorts with sprites.
- Renderer-side player sector lookup was hardened while landing these tests because an existing subsector-order regression exposed a bad sector-ownership assumption in synthetic leaves.

---

### Task 1: Pegging Uses Logical Texture Height

**Files:**
- Modify: `crates/doom-renderer/src/texture.rs`
- Modify: `crates/doom-renderer/src/render.rs`
- Test: `crates/doom-renderer/src/render.rs`

**Step 1: Write the failing test**

- Add a regression proving that a non-power-of-two upper texture pegs using its logical height rather than the padded cache height.

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doom-renderer logical_height_pegging -- --nocapture
```

Expected: failure because `texturemid` still uses padded cache height.

**Step 3: Write the minimal implementation**

- Add `logical_height` to `WallTexture`.
- Populate it from the original texture definition height.
- Keep `height` as padded storage height for column wrapping.
- Use `logical_height` only in the pegging formulas that currently misuse `tex.height`.

**Step 4: Run test to verify it passes**

Run:
```powershell
cargo test -p doom-renderer logical_height_pegging -- --nocapture
```

Expected: pegging regression passes.

Result:

- Implemented via `WallTexture.logical_height` in `crates/doom-renderer/src/texture.rs` and pegging updates in `crates/doom-renderer/src/render.rs`.
- The focused pegging regression passes. Its assertion shape is less dramatic than the sky and masked-midtexture tests, but it still exercises the logical-height path.

### Task 2: Map-Specific Sky Selection

**Files:**
- Modify: `crates/doom-renderer/src/render.rs`
- Test: `crates/doom-renderer/src/render.rs`

**Step 1: Write the failing test**

- Add a regression where the level name maps to `SKY2` and the texture cache contains distinct `SKY1` and `SKY2` columns; the rendered sky must use `SKY2`.

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doom-renderer map_specific_sky_selection -- --nocapture
```

Expected: failure because the renderer still fetches `SKY1` unconditionally.

**Step 3: Write the minimal implementation**

- Use `sky_texture_name(level.name.as_str())` when choosing the sky texture in `render_level()`.

**Step 4: Run test to verify it passes**

Run:
```powershell
cargo test -p doom-renderer map_specific_sky_selection -- --nocapture
```

Expected: sky selection regression passes.

Result:

- The renderer now chooses the sky texture from the level name, and the map-specific sky selection regression passes.

### Task 3: Deferred Masked Midtextures

**Files:**
- Modify: `crates/doom-renderer/src/render.rs`
- Modify: `crates/doom-renderer/src/sprite.rs`
- Modify: `crates/doom-app/src/main.rs`
- Test: `crates/doom-renderer/src/render.rs`

**Step 1: Write the failing test**

- Add an integration regression showing that a sprite behind a masked middle texture remains visible through transparent texels but is covered by opaque texels.

**Step 2: Run test to verify it fails**

Run:
```powershell
cargo test -p doom-renderer masked_midtexture_sprite_ordering -- --nocapture
```

Expected: failure because masked columns are still drawn inline before sprites.

**Step 3: Write the minimal implementation**

- Collect visible masked middle texture columns during wall rendering instead of drawing them immediately.
- Return those deferred masked columns from `render_level()`.
- Extend sprite rendering to interleave deferred masked columns with sprites by depth.
- Keep wall z-buffer and portal clip behavior unchanged.

**Step 4: Run test to verify it passes**

Run:
```powershell
cargo test -p doom-renderer masked_midtexture_sprite_ordering -- --nocapture
```

Expected: masked/sprite ordering regression passes.

Result:

- Deferred masked columns now come back through `RenderOut`, and sprite rendering interleaves them by depth in a post pass. The masked/sprite ordering regression passes.

### Task 4: Batch Verification

**Files:**
- Verify only

**Step 1: Run focused suites**

Run:
```powershell
cargo test -p doom-renderer logical_height_pegging -- --nocapture
cargo test -p doom-renderer map_specific_sky_selection -- --nocapture
cargo test -p doom-renderer masked_midtexture_sprite_ordering -- --nocapture
```

**Step 2: Run broader renderer/app suites**

Run:
```powershell
cargo fmt --all
cargo test -p doom-renderer --lib
cargo test -p doom-app
```

Expected: focused regressions and broader renderer/app suites stay green.

Actual verification run:

```powershell
cargo test -p doom-renderer logical_height_pegging -- --nocapture
cargo test -p doom-renderer map_specific_sky_selection -- --nocapture
cargo test -p doom-renderer masked_midtexture_sprite_ordering -- --nocapture
cargo test -p doom-renderer test_far_solid_wall_clipping_does_not_depend_on_subsector_seg_order -- --nocapture
cargo fmt --all
cargo test -p doom-renderer --lib
cargo test -p doom-app
```

Verification result:

- All focused Batch C1 regressions passed.
- `cargo test -p doom-renderer --lib` passed with 707 tests.
- `cargo test -p doom-app` passed with 91 tests.
- Existing unrelated warnings remain in the tree and were not part of this batch.
