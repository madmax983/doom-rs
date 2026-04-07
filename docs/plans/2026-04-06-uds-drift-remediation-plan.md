# UDS Drift Remediation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a strict vanilla compatibility profile that eliminates the confirmed UDS drift in demos, special colormap behavior, and PWAD sprite/flat loading, while making savegame compatibility an explicit supported mode instead of accidental drift.

**Architecture:** Introduce one shared compatibility-profile enum in `doom-types`, thread it from `doom-app` CLI into demo, renderer, and resource-loading entry points, and keep today's behavior as `Extended` until strict mode is green. Fix demo parity first because it is the clearest byte-level incompatibility and the best validation harness for later gameplay/RNG work.

**Tech Stack:** Rust 2024 workspace, `clap`, `cargo test`, existing `doom-demo`, `doom-app`, `doom-game`, `doom-renderer`, `doom-wad`, and `doom-types` crates.

## Execution Status

- `Task 0` complete in `9c48e11` (`test: restore doom-wad lib test compilation`)
- `Task 1` complete in `a353289` (`feat: add compatibility profile plumbing`)
- `Task 2` complete in `1680e73` (`feat: make demo tic payload vanilla-compatible`)
- `Task 3` complete in `3c7cf4b` and tightened in `9163a10`
- `Task 4` complete in `8395eba` (`feat: add strict special colormap handling`)
- `Task 5` complete in `d6deab1` and `ed6ee4a`
- `Task 6` complete in `0950e19` with API cleanup follow-up `b9749be`
- `Task 7` is the doc closeout pass captured by the commit that updates this file

---

## Issue Map

- `UDS-01` P0: demo tic payload is wrong. `crates/doom-demo/src/ticcmd.rs` stores two bytes of turn and drops buttons, but vanilla LMP uses four bytes per tic command as `forwardmove`, `sidemove`, one-byte turn, and one-byte buttons.
- `UDS-02` P0: there is no explicit compatibility boundary. Strict-vanilla and extended/source-port behavior are currently mixed together in the default code path.
- `UDS-03` P1: special `COLORMAP` rows are approximated instead of being faithfully used.
- `UDS-04` P1: stacked PWAD sprite/flat loading is always enabled, which does not match vanilla-era behavior.
- `UDS-05` P1: savegames are intentionally custom (`DRS1`) but undocumented as such at runtime and not switchable to a vanilla-compatible path.
- `UDS-06` P2: `cargo test -p doom-wad --lib` is blocked by a duplicate test name, so the verification harness is currently limping.

## Recommended Compatibility Policy

Implement two modes:

- `Extended` (default): preserve today's modernized behavior
- `VanillaStrict`: enforce vanilla demo bytes, faithful special colormap use, and vanilla-compatible resource-loading behavior

Do not try to rewrite the whole engine into one monolithic "always vanilla" path. That would break current conveniences and make future drift impossible to reason about.

## Task 0: Restore the WAD Test Harness

**Files:**
- Modify: `crates/doom-wad/src/stack.rs`
- Test: `crates/doom-wad/src/stack.rs`

**Step 1: Write the tiny failing change request down in the code review notes**

Rename one of the duplicate `map_lump_group_searches_pwads_first_udmf` tests so the crate compiles again. Do not change behavior in this task.

**Step 2: Run the current crate test command to verify the failure is real**

Run: `cargo test -p doom-wad --lib`
Expected: FAIL with `E0428` duplicate definition in `crates/doom-wad/src/stack.rs`

**Step 3: Apply the minimal fix**

Change only the duplicate test function name. Leave the assertions alone.

**Step 4: Re-run the crate tests**

Run: `cargo test -p doom-wad --lib`
Expected: PASS or at least advance to the next real failure instead of dying in name-resolution hell

**Step 5: Commit**

```bash
git add crates/doom-wad/src/stack.rs
git commit -m "test: restore doom-wad lib test compilation"
```

## Task 1: Add an Explicit Compatibility Profile

**Files:**
- Create: `crates/doom-types/src/compat.rs`
- Modify: `crates/doom-types/src/lib.rs`
- Modify: `crates/doom-app/src/main.rs`
- Modify: `crates/doom-app/src/demo_mode.rs`
- Modify: `crates/doom-renderer/src/colormap.rs`
- Modify: `crates/doom-renderer/src/flat_cache.rs`
- Modify: `crates/doom-renderer/src/sprite.rs`
- Test: `crates/doom-app/src/main.rs`

**Step 1: Write the failing CLI tests**

Add tests in `crates/doom-app/src/main.rs` for:

- default mode parses as `Extended`
- `--compat vanilla-strict` parses correctly
- invalid `--compat` values fail with a clear clap error

Example target shape:

```rust
assert_eq!(args.compat, CompatibilityProfile::Extended);
assert_eq!(args.compat, CompatibilityProfile::VanillaStrict);
```

**Step 2: Run only the new parsing tests**

Run: `cargo test -p doom-app compat --lib`
Expected: FAIL because `CompatibilityProfile` and the new CLI flag do not exist yet

**Step 3: Add the shared enum**

Create `crates/doom-types/src/compat.rs`:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompatibilityProfile {
    #[default]
    Extended,
    VanillaStrict,
}
```

Re-export it from `crates/doom-types/src/lib.rs`.

**Step 4: Thread the profile through app construction**

Add a `--compat` CLI flag in `crates/doom-app/src/main.rs`, defaulting to `extended`, and pass the resulting enum into the demo/renderer/resource-loading code paths that will need it in later tasks. Do not change behavior yet beyond plumbing.

**Step 5: Re-run the parsing tests**

Run: `cargo test -p doom-app compat --lib`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/doom-types/src/compat.rs crates/doom-types/src/lib.rs crates/doom-app/src/main.rs crates/doom-app/src/demo_mode.rs crates/doom-renderer/src/colormap.rs crates/doom-renderer/src/flat_cache.rs crates/doom-renderer/src/sprite.rs
git commit -m "feat: add compatibility profile plumbing"
```

## Task 2: Fix Demo Tic Byte Semantics

**Files:**
- Modify: `crates/doom-demo/src/ticcmd.rs`
- Modify: `crates/doom-demo/src/recorder.rs`
- Modify: `crates/doom-demo/src/player.rs`
- Modify: `crates/doom-demo/src/lib.rs`
- Modify: `crates/doom-app/src/demo_mode.rs`
- Modify: `crates/doom-app/src/net_mode.rs`
- Test: `crates/doom-demo/src/ticcmd.rs`
- Test: `crates/doom-demo/src/player.rs`

**Step 1: Write failing byte-accurate tests**

Add tests that prove the strict profile uses:

- byte 0: `forward_move`
- byte 1: `side_move`
- byte 2: one-byte demo turn
- byte 3: `buttons`

Add a regression that `BT_ATTACK`, `BT_USE`, and weapon-change bits survive `TicCmd -> DemoTicCmd -> TicCmd`.

Example:

```rust
let tic = TicCmd {
    forward_move: 10,
    side_move: -5,
    angle_turn: 0x1200,
    buttons: bt::BT_ATTACK | bt::BT_USE,
    ..Default::default()
};
let demo = DemoTicCmd::from_ticcmd(&tic);
assert_eq!(demo.to_bytes(), [10u8, 251u8, 0x12, bt::BT_ATTACK | bt::BT_USE]);
let back = demo.to_ticcmd();
assert_eq!(back.angle_turn, 0x1200);
assert_eq!(back.buttons, bt::BT_ATTACK | bt::BT_USE);
```

**Step 2: Run the demo crate tests**

Run: `cargo test -p doom-demo --lib`
Expected: FAIL because the current format stores `i16 angle_turn` and zeros `buttons`

**Step 3: Change the demo tic representation**

Refactor `DemoTicCmd` in `crates/doom-demo/src/ticcmd.rs` so strict demo bytes mirror vanilla semantics:

- store a one-byte demo-turn field instead of a two-byte `i16`
- preserve `buttons`
- keep `chatchar` out of the LMP stream

When converting:

- `from_ticcmd()` must quantize the engine's `i16 angle_turn` to the one-byte demo form
- `to_ticcmd()` must reconstruct the engine turn shape from that one-byte field

Document the lossy conversion explicitly.

**Step 4: Update recorder and player**

Adjust `crates/doom-demo/src/recorder.rs` and `crates/doom-demo/src/player.rs` so they serialize and parse the corrected four-byte payload without changing header size or terminator behavior.

**Step 5: Re-run demo tests**

Run: `cargo test -p doom-demo --lib`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/doom-demo/src/ticcmd.rs crates/doom-demo/src/recorder.rs crates/doom-demo/src/player.rs crates/doom-demo/src/lib.rs crates/doom-app/src/demo_mode.rs crates/doom-app/src/net_mode.rs
git commit -m "feat: make demo tic payload vanilla-compatible"
```

## Task 3: Add Demo Interop and App-Level Regression Coverage

**Files:**
- Modify: `crates/doom-app/src/demo_mode.rs`
- Modify: `crates/doom-app/src/main.rs`
- Create: `crates/doom-demo/tests/vanilla_bytes.rs`
- Test: `crates/doom-app/src/demo_mode.rs`
- Test: `crates/doom-demo/tests/vanilla_bytes.rs`

**Step 1: Write failing end-to-end demo tests**

Add:

- one golden-byte test in `crates/doom-demo/tests/vanilla_bytes.rs`
- one playback test in `crates/doom-app/src/demo_mode.rs` proving a recorded `BT_ATTACK` command actually reaches game logic
- one playback test proving a `BT_USE` command is not silently dropped

Use hard-coded byte arrays first. Do not block this task on external fixture collection.

**Step 2: Run the focused tests**

Run: `cargo test -p doom-demo --test vanilla_bytes`
Run: `cargo test -p doom-app demo_mode --lib`
Expected: FAIL until the wrappers and golden bytes match the corrected semantics

**Step 3: Wire strict-mode record/playback**

Update `crates/doom-app/src/main.rs` and `crates/doom-app/src/demo_mode.rs` so:

- `--record` writes strict bytes when `--compat vanilla-strict` is active
- `--playdemo` reads strict bytes and preserves action buttons during playback
- `Extended` mode keeps today's behavior only if you intentionally still want it

If the code would otherwise fork badly, prefer always-correct demo bytes and delete the legacy branch.

**Step 4: Re-run the focused tests**

Run: `cargo test -p doom-demo --test vanilla_bytes`
Run: `cargo test -p doom-app demo_mode --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/doom-demo/tests/vanilla_bytes.rs crates/doom-app/src/demo_mode.rs crates/doom-app/src/main.rs
git commit -m "test: add demo interop regressions"
```

## Task 4: Make Special COLORMAP Rows Faithful in Strict Mode

**Files:**
- Modify: `crates/doom-renderer/src/colormap.rs`
- Modify: `crates/doom-renderer/src/render.rs`
- Modify: `crates/doom-renderer/src/sprite.rs`
- Modify: `crates/doom-renderer/src/weapon_anim.rs`
- Modify: `crates/doom-app/src/main.rs`
- Test: `crates/doom-renderer/src/colormap.rs`
- Test: `crates/doom-renderer/src/render.rs`

**Step 1: Write failing colormap tests**

Add tests that:

- load a synthetic `COLORMAP` where row 32 contains a sentinel value
- verify strict mode can return row 32 without clamping to `0..31`
- verify extended mode can still use the current synthetic fallback if needed

Example target shape:

```rust
let row = cache.special_row(32, CompatibilityProfile::VanillaStrict);
assert_eq!(row[0], 0xA5);
```

**Step 2: Run renderer colormap tests**

Run: `cargo test -p doom-renderer colormap --lib`
Expected: FAIL because the current API clamps light rows and does not expose strict special-row access

**Step 3: Add explicit special-row APIs**

In `crates/doom-renderer/src/colormap.rs`:

- keep the normal lighting helper for `0..31`
- add a non-clamping helper for row lookup
- make invulnerability behavior profile-aware

Do not overload one method with magic indexes. Name the special-row path clearly.

**Step 4: Audit render call sites**

Update `render.rs`, `sprite.rs`, `weapon_anim.rs`, and any app-level selection in `crates/doom-app/src/main.rs` so strict mode uses the real special row path where appropriate. Leave extended mode behavior unchanged unless the old path is pure dead weight.

**Step 5: Re-run renderer tests**

Run: `cargo test -p doom-renderer colormap --lib`
Run: `cargo test -p doom-renderer render --lib`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/doom-renderer/src/colormap.rs crates/doom-renderer/src/render.rs crates/doom-renderer/src/sprite.rs crates/doom-renderer/src/weapon_anim.rs crates/doom-app/src/main.rs
git commit -m "feat: add strict special colormap handling"
```

## Task 5: Add Vanilla-Strict Sprite and Flat Loading

**Files:**
- Modify: `crates/doom-renderer/src/flat_cache.rs`
- Modify: `crates/doom-renderer/src/sprite.rs`
- Modify: `crates/doom-wad/src/stack.rs`
- Modify: `crates/doom-app/src/main.rs`
- Test: `crates/doom-renderer/src/flat_cache.rs`
- Test: `crates/doom-renderer/src/sprite.rs`

**Step 1: Write failing resource-loading regressions**

Add tests that build small in-memory IWAD/PWAD stacks and verify:

- strict mode does not automatically honor `FF_START`/`FF_END`
- strict mode does not automatically honor `SS_START`/`SS_END`
- extended mode still keeps stacked override behavior

Do not guess at subtle vanilla semantics from memory. For any ambiguous case, match Chocolate Doom behavior and record the fixture in the test comment.

**Step 2: Run the focused tests**

Run: `cargo test -p doom-renderer flat_cache --lib`
Run: `cargo test -p doom-renderer sprite --lib`
Expected: FAIL because the current loaders always apply extended stacked-marker logic

**Step 3: Split the loader entry points**

Refactor the public APIs so callers can choose:

- `load_from_stack_extended(...)`
- `load_from_stack_strict(...)`

or an equivalent profile-driven API.

Keep the implementation localized to the cache loaders. Do not spread marker-interpretation policy into unrelated code.

**Step 4: Re-run the focused tests**

Run: `cargo test -p doom-renderer flat_cache --lib`
Run: `cargo test -p doom-renderer sprite --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/doom-renderer/src/flat_cache.rs crates/doom-renderer/src/sprite.rs crates/doom-wad/src/stack.rs crates/doom-app/src/main.rs
git commit -m "feat: add vanilla-strict resource loading"
```

## Task 6: Split Savegame Policy from Savegame Format

**Files:**
- Create: `crates/doom-game/src/savegame_vanilla.rs`
- Modify: `crates/doom-game/src/savegame.rs`
- Modify: `crates/doom-game/src/lib.rs`
- Modify: `crates/doom-app/src/savegame.rs`
- Modify: `crates/doom-app/src/main.rs`
- Modify: `docs/plans/vantage-spec-savegame-parity.md`
- Test: `crates/doom-game/src/savegame.rs`
- Test: `crates/doom-app/src/savegame.rs`

**Step 1: Write failing format-selection tests**

Add tests that prove:

- extended mode still writes and reads `DRS1`
- strict mode does not route through the custom serializer
- app-level load logic can distinguish the two formats explicitly

Use an enum like:

```rust
pub enum SaveFormat {
    DoomRs,
    VanillaDsg,
}
```

**Step 2: Run targeted save tests**

Run: `cargo test -p doom-game savegame --lib`
Run: `cargo test -p doom-app savegame --lib`
Expected: FAIL because only the custom format exists today

**Step 3: Land the format boundary before the parser**

First refactor the existing custom path behind an explicit format-selection API. Do not start implementing vanilla serialization until the boundary exists cleanly.

**Step 4: Implement the vanilla path in a separate module**

Create `crates/doom-game/src/savegame_vanilla.rs` and keep it isolated from the current `DRS1` implementation. Mirror Chocolate Doom/original-source save layout, not the incomplete UDS chapter 9 mirror.

Use fixture-backed tests wherever possible. If fixture files are not yet available, create minimal hard-coded binary samples with strong comments that name the source behavior they represent.

**Step 5: Re-run targeted save tests**

Run: `cargo test -p doom-game savegame --lib`
Run: `cargo test -p doom-app savegame --lib`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/doom-game/src/savegame.rs crates/doom-game/src/savegame_vanilla.rs crates/doom-game/src/lib.rs crates/doom-app/src/savegame.rs crates/doom-app/src/main.rs docs/plans/vantage-spec-savegame-parity.md
git commit -m "feat: split custom and vanilla savegame formats"
```

## Task 7: Close the Audit Loop

**Files:**
- Modify: `docs/plans/2026-04-06-unofficial-doom-specs-parity-matrix.md`
- Modify: `docs/plans/2026-04-06-uds-drift-remediation-plan.md`
- Modify: `docs/plans/vantage-spec-demo-parity.md`
- Modify: `docs/plans/vantage-spec-rng-parity.md`

**Step 1: Update the docs after each subsystem lands**

Change status labels in the parity matrix from `Drifted`/`Partial` to `Aligned` only when:

- the code is merged
- the targeted tests pass
- one end-to-end smoke path proves the feature works

**Step 2: Run the final verification sweep**

Run:

```bash
cargo test -p doom-wad --lib
cargo test -p doom-demo --lib
cargo test -p doom-renderer --lib
cargo test -p doom-game savegame --lib
cargo test -p doom-app --lib
```

Expected: all pass

Status: verification completed on branch during remediation closeout.

**Step 3: Update the remaining parity notes**

Make `vantage-spec-demo-parity.md` and `vantage-spec-rng-parity.md` reflect the new reality:

- demo byte compatibility fixed
- RNG work still depends on long-run deterministic validation, not just the table

Status: completed in the closeout doc update.

**Step 4: Commit**

```bash
git add docs/plans/2026-04-06-unofficial-doom-specs-parity-matrix.md docs/plans/2026-04-06-uds-drift-remediation-plan.md docs/plans/vantage-spec-demo-parity.md docs/plans/vantage-spec-rng-parity.md
git commit -m "docs: close uds drift remediation loop"
```

## Notes For The Implementer

- Do demo work before savegame work. Demo parity has a much better test loop and will expose real determinism bugs faster.
- Keep `Extended` as the default until strict mode is fully usable. Breaking today's mod-friendly path prematurely would be self-owning.
- Treat savegames as a separate deliverable even if the compatibility seam lands early. They are the murkiest part of the audit and should not block demo/resource parity.
- If strict sprite/flat semantics turn out to be nastier than the UDS prose suggests, record the exact Chocolate Doom behavior in tests and let the tests become the real spec.
