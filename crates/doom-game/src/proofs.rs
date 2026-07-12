//! Verus proofs for the deterministic Doom RNG.
//!
//! These proofs are verified with the Verus verifier, NOT `rustc`. Run
//! `verus --crate-type lib crates/doom-game/src/proofs.rs` to verify (the
//! `doom-game` build script runs exactly this when `DOOM_VERIFY=1`).
//!
//! They are about the *shape* of the real code in `random.rs` / `state.rs`:
//! - `DoomRng::next_byte` advances `self.index = (self.index + 1) & 255` and
//!   reads `RNG_TABLE[index]` (a 256-entry `[u8; 256]` table);
//! - `GameState::p_subrandom` returns `p_random() as i32 - p_random() as i32`.
//!
//! The concrete 256 byte values of `RNG_TABLE` live in `random.rs` and are
//! pinned byte-for-byte by the `rng_table_matches_vanilla_anchor_values` unit
//! test; here the table is modelled as an abstract 256-element sequence, which
//! is the exact shape these safety/determinism properties depend on.

// Only compiled when verus processes this file.
#![cfg(verus_keep_ghost)]

use vstd::prelude::*;

verus! {

/// Models `random.rs::DoomRng::next_byte`'s index update
/// `self.index = (self.index + 1) & 255`.
pub open spec fn rng_next_index(i: u32) -> u32 {
    add(i, 1) & 255
}

/// Models the table read `RNG_TABLE[self.index]` over an abstract 256-entry
/// table `t` (the real `RNG_TABLE: [u8; 256]`).
pub open spec fn rng_out(t: Seq<u8>, i: u32) -> u8 {
    t[rng_next_index(i) as int]
}

/// Table index is ALWAYS a valid subscript into the 256-entry `RNG_TABLE`.
///
/// Models `random.rs::next_byte`: the advanced index `(index + 1) & 255` is
/// `< 256` for every `u32`, so `RNG_TABLE[self.index as usize]` can never index
/// out of bounds (no panic), regardless of the incoming index value.
proof fn lemma_rng_index_in_bounds(i: u32)
    ensures
        rng_next_index(i) < 256,
{
    assert((add(i, 1) & 255) < 256) by (bit_vector);
}

/// The RNG index stays in `0..256` across calls and wraps mod 256.
///
/// Models `random.rs::next_byte`: given the documented invariant that
/// `index ∈ 0..256`, after the update the index is still `< 256`; it increments
/// by one, and steps from `255` back to `0` (the mod-256 wrap).
proof fn lemma_rng_index_mod256(i: u32)
    requires
        i < 256,
    ensures
        rng_next_index(i) < 256,
        i == 255 ==> rng_next_index(i) == 0,
        i < 255 ==> rng_next_index(i) == i + 1,
{
    assert((add(i, 1) & 255) < 256) by (bit_vector);
    assert(i == 255 ==> (add(i, 1) & 255) == 0) by (bit_vector);
    assert(i < 255 ==> (add(i, 1) & 255) == i + 1) by (bit_vector);
}

/// Determinism: `next_byte`'s result is a pure function of the index alone.
///
/// Models `random.rs::next_byte`. With a fixed table `t`, the returned byte
/// depends only on the current `index` (no hidden state) — equal indices give
/// equal outputs — and the read is always in bounds. This is what makes the
/// RNG replay bit-identically across peers and demo playback.
proof fn lemma_rng_deterministic(t: Seq<u8>, i: u32, j: u32)
    requires
        t.len() == 256,
        i == j,
    ensures
        0 <= rng_next_index(i) < t.len(),
        rng_out(t, i) == rng_out(t, j),
{
    assert((add(i, 1) & 255) < 256) by (bit_vector);
}

/// `p_subrandom` stays within `[-255, 255]`.
///
/// Models `state.rs::p_subrandom` = `p_random() as i32 - p_random() as i32`.
/// Both draws are bytes (`u8 ∈ [0, 255]`), so their difference is bounded by
/// `±255` — the invariant relied on by `p_missile_angle_spread` (`<< 20`).
proof fn lemma_p_subrandom_range(a: u8, b: u8)
    ensures
        -255 <= (a as int) - (b as int) <= 255,
{
}

} // verus!
