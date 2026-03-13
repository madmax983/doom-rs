# Chocolate Doom Parity Closeout And Remaining Work

Date: 2026-03-13
Branch: `feat/batch-c2-renderer-parity`
Base trunk before merge: `6611734`

## Purpose

This document is the short-form follow-up to [2026-03-10-chocolate-doom-parity-audit.md](C:/Users/markm/doom-rs/docs/plans/2026-03-10-chocolate-doom-parity-audit.md).

It exists to answer two practical questions:

1. What parity work has landed on the current branch since `trunk`?
2. What parity work is still intentionally open after this pass?

## Landed Since Trunk

The current branch contains the following parity and bug-fix slice on top of `trunk`:

- `e93d737` `feat: land batch c2 renderer parity fixes`
- `6bc38ac` `feat: land batch a2 and b2 missile parity`
- `1d63de5` `feat: add vertical hitscan parity`
- `a298e84` `feat: add bullet autoaim parity`
- `e1b5f72` `fix: restore monster behavior and death presentation`
- `2e56807` `fix: sync player health and improve ai logging`
- `29aa4e8` `fix: restore level exit transitions`
- `d4f60c5` `fix: seal two-sided door floor leaks`
- `549bfb9` `fix: land pending gameplay bug fixes`
- `be31294` `feat: advance psprite parity`
- `35a5d1a` `feat: improve weapon state parity`
- `a33a1c1` `feat: advance psprite and sound parity`
- `ed9b91f` `fix: restore weapon overlay positioning`
- `cde5aec` `style: format remaining parity edits`

## What This Branch Actually Improved

### Gameplay and interactions

- Locked-door feedback is now player-only and Doom-shaped.
- Walk-trigger processing moved back into the game tick instead of app-layer glue.
- Door opening behavior is no longer sluggish or mis-targeted.
- Stair descent support-floor handling no longer traps the player on step transitions.
- Periodic damaging floors no longer double-apply damage.
- Level exits now reach intermission and next-map flow again.

### Monsters, combat, and spawn

- Monster wakeup, retaliation, and sound propagation are much closer to vanilla.
- `P_CheckMissileRange` edge cases now cover vanilla Arch-Vile and Revenant behavior.
- Hitscan moved from the old flat 2D approximation to a Doom-shaped vertical slope clip.
- Player bullet weapons now use center/right/left autoaim probing.
- Spawn logic respects floor support, ceiling-spawn flags, and blocked Nightmare respawns better.
- Player health is synced between `PlayerState` and the player mobj again, which fixes AI behavior that depended on the wrong truth.
- Projectile spawn/default state issues are fixed, so imp fireballs and similar missiles render and behave correctly again.

### Renderer

- Batch C2 landed sky projection, visplane reuse hardening, and renderer-side sector ownership fixes.
- Door floor leaks and several portal/window clipping lies were fixed.
- Weapon overlay placement now uses the correct psprite origin path instead of drifting right or floating like a cursed severed hand.
- Weapon bob now behaves more like Doom and no longer lifts the overlay above rest.

### Frontend and audio

- Psprite/weapon presentation is materially closer to `p_pspr.c`, including better lower/raise and reload sequencing.
- Weapon audio is tied to actual fire events instead of stale input guesses.
- Monster sound origins now reuse channels more like Doom instead of stacking nonsense from the same actor.

## Remaining Parity Debt

These are the real remaining items after this pass, grouped by likely impact.

### High-value remaining source-faithfulness

- Gameplay specials: exact pathological `spechit` encounter ordering is still open.
- Combat: deeper refire/state nuance is still not fully source-identical.
- Combat: damage-table parity is still only partially audited.
- Combat: the current bullet autoaim probe is documented as an approximation; a stricter split between slope probing and actual fire angle may still be needed.
- Spawn: broader map-thing spawn parity and remaining flag-specific edge cases still need a sweep.

### Renderer debt

- Raw subsector-order parity is still not safe to land.
- The hardening sort in `crates/doom-renderer/src/seg.rs` is still covering a real renderer weakness in nasty same-subsector cases.
- Deeper visplane and sky parity still remain beyond the currently landed fixes.
- Residual sprite clip edge cases can still occur when clip state is borrowed from the wrong sector context.

### Audio and meta systems

- Long-lived sound spatial refresh is still simplified compared to Doom's `S_UpdateSounds`.
- True attract-mode demo playback is still missing.
- Input and demo-loop parity has not had the same depth of audit as gameplay/rendering.

### Still basically untouched

- Savegame parity
- Demo parity as a recording/playback system
- RNG parity as a dedicated pass

## Suggested Remaining Order

1. Finish gameplay/combat cleanup:
   exact `spechit` ordering, refire nuance, damage-table pass, spawn edge cases
2. Finish renderer cleanup:
   subsector raw-order parity, remaining visplane/sky details, residual sprite clip cases
3. Finish audio/demo parity:
   long-lived sound updates, attract-mode demo playback, deeper input/demo audit
4. Finish save/demo/RNG parity as a dedicated final pass

## Merge Note

This branch is merge-worthy as a stable playable slice.

The remaining debt is mostly deep source-faithfulness and edge-case cleanup, not the obvious broken-playability issues that were blocking the branch earlier.
