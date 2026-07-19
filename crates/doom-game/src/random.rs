//! Combat randomness utilities built on top of `GameState::p_random`.
//!
//! All functions here consume RNG bytes from the deterministic `DoomRng` inside
//! `GameState`, ensuring identical results across network peers and demo
//! playback.

use crate::state::GameState;

use std::cell::RefCell;

// ---------------------------------------------------------------------------
// RNG call-tracing (opt-in microscope for demo-sync work)
// ---------------------------------------------------------------------------

/// A single recorded `P_Random` draw.
pub struct RngTraceEntry {
    /// `level_time` at the moment of the draw.
    pub leveltime: u32,
    /// Global draw sequence number (0-based, across the whole replay).
    pub seq: u32,
    /// The value returned by the draw.
    pub retval: u8,
    /// Best-effort caller symbol (first game frame outside the RNG module).
    pub caller: String,
}

struct RngTraceState {
    entries: Vec<RngTraceEntry>,
    leveltime: u32,
    seq: u32,
    /// Active caller-label context stack (top = innermost). Populated by
    /// `RngCtx` guards so the recorded caller is exact regardless of release
    /// inlining (which makes backtrace symbols unreliable).
    ctx: Vec<&'static str>,
}

thread_local! {
    static RNG_TRACE: RefCell<Option<RngTraceState>> = const { RefCell::new(None) };
}

/// RAII guard that pushes a caller-label context for RNG tracing and pops it on
/// drop. Cheap no-op when tracing is disabled. Use at the top of any function
/// that (directly or transitively) draws `P_Random`, so the trace attributes
/// draws to the correct vanilla category.
#[must_use]
pub struct RngCtx {
    active: bool,
}

/// Push a caller-label onto the RNG-trace context stack.
#[inline]
pub fn rng_ctx(label: &'static str) -> RngCtx {
    let active = RNG_TRACE.with(|t| {
        if let Some(s) = t.borrow_mut().as_mut() {
            s.ctx.push(label);
            true
        } else {
            false
        }
    });
    RngCtx { active }
}

impl Drop for RngCtx {
    #[inline]
    fn drop(&mut self) {
        if self.active {
            RNG_TRACE.with(|t| {
                if let Some(s) = t.borrow_mut().as_mut() {
                    s.ctx.pop();
                }
            });
        }
    }
}

/// Begin recording every `P_Random` draw on this thread.
pub fn rng_trace_enable() {
    RNG_TRACE.with(|t| {
        *t.borrow_mut() = Some(RngTraceState {
            entries: Vec::new(),
            leveltime: 0,
            seq: 0,
            ctx: Vec::new(),
        });
    });
}

/// Update the `level_time` stamp attached to subsequent draws.
pub fn rng_trace_set_leveltime(leveltime: u32) {
    RNG_TRACE.with(|t| {
        if let Some(s) = t.borrow_mut().as_mut() {
            s.leveltime = leveltime;
        }
    });
}

/// Stop recording and return all captured draws (empty if never enabled).
pub fn rng_trace_take() -> Vec<RngTraceEntry> {
    RNG_TRACE.with(|t| t.borrow_mut().take().map(|s| s.entries).unwrap_or_default())
}

/// Extract the most relevant caller symbol from a captured backtrace.
///
/// Walks the frames top-to-bottom and returns the first `doom_game` symbol
/// that is not part of the RNG plumbing itself.
fn caller_from_backtrace() -> String {
    let bt = std::backtrace::Backtrace::force_capture();
    let text = format!("{bt}");
    for line in text.lines() {
        let line = line.trim();
        let sym = match line.find(": ") {
            Some(pos) => line[pos + 2..].trim(),
            None => continue,
        };
        if !sym.contains("doom_game") {
            continue;
        }
        if sym.contains("::random::")
            || sym.contains("next_byte")
            || sym.contains("p_random")
            || sym.contains("p_subrandom")
            || sym.contains("::p_random")
            || sym.contains("rng_trace")
        {
            continue;
        }
        let sym = sym.strip_suffix('"').unwrap_or(sym);
        let cleaned = match sym.rfind("::h") {
            Some(pos) if sym[pos + 3..].chars().all(|c| c.is_ascii_hexdigit()) => &sym[..pos],
            _ => sym,
        };
        // Reduce to the last path segment (the bare fn name) and map it onto the
        // vanilla p_enemy.c / p_inter.c function category so the per-tic caller
        // list is directly comparable against the chocolate-doom RNG summary.
        let seg = cleaned.rsplit("::").next().unwrap_or(cleaned);
        return map_caller(seg).to_string();
    }
    "?".to_string()
}

/// Map a doom-rs function name onto the vanilla P_Random-caller category used
/// by the chocolate-doom trace, so per-tic `funcs` lists line up 1:1.
fn map_caller(seg: &str) -> &str {
    match seg {
        "spawn_level_things" => "spawn",
        "tick_sector_lights" | "t_light_flash" | "tick_light_flash" => "T_LightFlash",
        "a_look" | "transition_to_see_state" => "A_Look",
        "a_chase" => "A_Chase",
        "p_new_chase_dir" => "P_NewChaseDir",
        "p_try_walk" => "P_TryWalk",
        "p_move" => "P_Move",
        "p_check_missile_range" => "P_CheckMissileRange",
        "a_face_target" => "A_FaceTarget",
        "a_pos_attack" => "A_PosAttack",
        "a_spos_attack" => "A_SPosAttack",
        "a_troop_attack" => "A_TroopAttack",
        "a_sarg_attack" => "A_SargAttack",
        "a_cpos_attack" => "A_CPosAttack",
        "a_pain" => "A_Pain",
        "a_scream" => "A_Scream",
        "p_damage_mobj" | "damage_mobj" => "P_DamageMobj",
        "p_kill_mobj" | "kill_mobj" => "P_KillMobj",
        "p_gun_shot" | "gun_shot" => "P_GunShot",
        "p_sub_random" | "p_subrandom" => "P_SubRandom",
        "p_spawn_puff" | "spawn_puff" => "P_SpawnPuff",
        "p_spawn_blood" | "spawn_blood" => "P_SpawnBlood",
        "p_aim_line_attack" | "p_line_attack" => "P_LineAttack",
        other => other,
    }
}

/// Record a single draw if tracing is active (called from `next_byte`).
#[inline]
fn rng_trace_record(retval: u8) {
    // Prefer the explicit context-stack label; only fall back to the (slow,
    // inlining-sensitive) backtrace when no guard is active.
    let ctx_label = RNG_TRACE.with(|t| t.borrow().as_ref().map(|s| s.ctx.last().copied()));
    let Some(ctx_label) = ctx_label else {
        return; // tracing disabled
    };
    let caller = match ctx_label {
        Some(label) => label.to_string(),
        None => caller_from_backtrace(),
    };
    RNG_TRACE.with(|t| {
        if let Some(s) = t.borrow_mut().as_mut() {
            let seq = s.seq;
            s.seq += 1;
            s.entries.push(RngTraceEntry {
                leveltime: s.leveltime,
                seq,
                retval,
                caller,
            });
        }
    });
}

/// Doom's original 256-entry pseudo-random number table (from `m_random.c`).
///
/// The sequence is deterministic and identical on all network peers, making
/// it safe to use inside the game simulation.  `DoomRng::next_byte()` returns
/// successive bytes from this table, wrapping at index 255.
pub static RNG_TABLE: [u8; 256] = [
    0, 8, 109, 220, 222, 241, 149, 107, 75, 248, 254, 140, 16, 66, 74, 21, 211, 47, 80, 242, 154,
    27, 205, 128, 161, 89, 77, 36, 95, 110, 85, 48, 212, 140, 211, 249, 22, 79, 200, 50, 28, 188,
    52, 140, 202, 120, 68, 145, 62, 70, 184, 190, 91, 197, 152, 224, 149, 104, 25, 178, 252, 182,
    202, 182, 141, 197, 4, 81, 181, 242, 145, 42, 39, 227, 156, 198, 225, 193, 219, 93, 122, 175,
    249, 0, 175, 143, 70, 239, 46, 246, 163, 53, 163, 109, 168, 135, 2, 235, 25, 92, 20, 145, 138,
    77, 69, 166, 78, 176, 173, 212, 166, 113, 94, 161, 41, 50, 239, 49, 111, 164, 70, 60, 2, 37,
    171, 75, 136, 156, 11, 56, 42, 146, 138, 229, 73, 146, 77, 61, 98, 196, 135, 106, 63, 197, 195,
    86, 96, 203, 113, 101, 170, 247, 181, 113, 80, 250, 108, 7, 255, 237, 129, 226, 79, 107, 112,
    166, 103, 241, 24, 223, 239, 120, 198, 58, 60, 82, 128, 3, 184, 66, 143, 224, 145, 224, 81,
    206, 163, 45, 63, 90, 168, 114, 59, 33, 159, 95, 28, 139, 123, 98, 125, 196, 15, 70, 194, 253,
    54, 14, 109, 226, 71, 17, 161, 93, 186, 87, 244, 138, 20, 52, 123, 251, 26, 36, 17, 46, 52,
    231, 232, 76, 31, 221, 84, 37, 216, 165, 212, 106, 197, 242, 98, 43, 39, 175, 254, 145, 190,
    84, 118, 222, 187, 136, 120, 163, 236, 249,
];

/// Deterministic Doom RNG.
///
/// Wraps an index into `RNG_TABLE`, advancing by 1 each call (mod 256).
/// The index is part of `GameState` and is included in snapshots.
#[derive(Clone, Debug, Default)]
pub struct DoomRng {
    /// Current position in `RNG_TABLE` (always in `0..256`).
    index: u32,
}

impl DoomRng {
    /// Create a new RNG at index 0 (start of table).
    pub fn new() -> Self {
        Self { index: 0 }
    }

    /// Return the next random byte and advance the index.
    ///
    /// Port of `P_Random()` from `m_random.c`: the index is incremented
    /// **before** the table read, so from index 0 the first call returns
    /// `RNG_TABLE[1]` (matching vanilla after `M_ClearRandom`).
    // Verified in proofs.rs::lemma_rng_index_in_bounds (the advanced index is
    // always < 256, so RNG_TABLE[index] never panics), lemma_rng_index_mod256
    // (index stays in 0..256 and wraps mod 256), and lemma_rng_deterministic
    // (the returned byte is a pure function of the index — bit-identical replay).
    #[inline]
    pub fn next_byte(&mut self) -> u8 {
        self.index = (self.index + 1) & 255;
        let val = RNG_TABLE[self.index as usize];
        rng_trace_record(val);
        val
    }

    /// Current table index (for snapshot / serialization).
    #[inline]
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Restore to a specific index (for snapshot restore).
    #[inline]
    pub fn set_index(&mut self, index: u32) {
        self.index = index & 255;
    }
}

// ---------------------------------------------------------------------------
// Damage variance
// ---------------------------------------------------------------------------

/// Apply Doom's standard damage multiplier to a base damage value.
///
/// Formula: `base_damage * ((p_random() % 8) + 1)`.
/// The result is in `[base_damage * 1, base_damage * 8]`.
///
/// Port of the inline `damage *= ...` patterns found throughout `p_map.c`,
/// `p_inter.c`, and weapon code.
pub fn p_damage_with_variance(gs: &mut GameState, base_damage: i32) -> i32 {
    if base_damage == 0 {
        return 0;
    }
    let multiplier = (gs.p_random() as i32 % 8) + 1;
    base_damage * multiplier
}

// ---------------------------------------------------------------------------
// Random tic offset
// ---------------------------------------------------------------------------

/// Add a random 0-3 tic offset to `base_tics`.
///
/// Used by monster see/chase states to desynchronize animations of monsters
/// that spawn at the same time.  Not applied to attack/pain/death states.
///
/// Port of the `P_Random()&3` patterns in `P_SetMobjState`.
pub fn randomize_tics(gs: &mut GameState, base_tics: i32) -> i32 {
    base_tics + (gs.p_random() as i32 & 3)
}

// ---------------------------------------------------------------------------
// Random chance check
// ---------------------------------------------------------------------------

/// Return `true` if `p_random() < threshold`.
///
/// Threshold 0 always returns `false`; threshold 255 returns `true` for all
/// table entries except the single entry with value 255 (which returns
/// `false` for threshold 255).
///
/// Used for pain chance, dodge chance, monster infighting probability, etc.
pub fn p_random_chance(gs: &mut GameState, threshold: u8) -> bool {
    gs.p_random() < threshold
}

// ---------------------------------------------------------------------------
// Missile angle spread
// ---------------------------------------------------------------------------

/// Compute a random angle spread for inaccurate monster projectiles.
///
/// Returns `p_subrandom() << 20` — a BAM-compatible angle delta suitable for
/// adding to a monster's aim angle to simulate weapon inaccuracy.
///
/// Used by zombiemen, shotgun guys, and other hitscan monsters.
pub fn p_missile_angle_spread(gs: &mut GameState) -> i32 {
    gs.p_subrandom() << 20
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    // -----------------------------------------------------------------------
    // p_random basics
    // -----------------------------------------------------------------------

    #[test]
    fn p_random_returns_first_table_entry() {
        // Vanilla P_Random increments the index BEFORE reading, so after
        // M_ClearRandom (index 0) the first call returns RNG_TABLE[1].
        let mut gs = GameState::new("test");
        let val = gs.p_random();
        assert_eq!(val, RNG_TABLE[1], "first p_random must return RNG_TABLE[1]");
    }

    #[test]
    fn p_random_advances_rng_index() {
        let mut gs = GameState::new("test");
        gs.p_random();
        assert_eq!(
            gs.rng.index(),
            1,
            "rng index must be 1 after one p_random call"
        );
    }

    #[test]
    fn p_random_wraps_at_256() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            gs.p_random();
        }
        assert_eq!(
            gs.rng.index(),
            0,
            "rng index must wrap to 0 after 256 p_random calls"
        );
        // With increment-before-read, the next call returns RNG_TABLE[1] again.
        let val = gs.p_random();
        assert_eq!(val, RNG_TABLE[1]);
    }

    #[test]
    fn p_random_is_deterministic() {
        let mut gs1 = GameState::new("test");
        let mut gs2 = GameState::new("test");
        let seq1: Vec<u8> = (0..50).map(|_| gs1.p_random()).collect();
        let seq2: Vec<u8> = (0..50).map(|_| gs2.p_random()).collect();
        assert_eq!(
            seq1, seq2,
            "two fresh GameStates must produce identical sequences"
        );
    }

    #[test]
    fn multiple_p_random_calls_produce_different_values() {
        let mut gs = GameState::new("test");
        let a = gs.p_random();
        let b = gs.p_random();
        // First call -> RNG_TABLE[1]=8, second -> RNG_TABLE[2]=109 — they differ.
        assert_ne!(
            a, b,
            "consecutive p_random calls should produce different values"
        );
    }

    #[test]
    fn rng_table_matches_vanilla_anchor_values() {
        // Byte-for-byte anchors from chocolate-doom src/doom/m_random.c rndtable[].
        // A single wrong byte breaks demo sync, so pin known indices.
        assert_eq!(RNG_TABLE[0], 0, "rndtable[0]");
        assert_eq!(RNG_TABLE[1], 8, "rndtable[1]");
        assert_eq!(RNG_TABLE[2], 109, "rndtable[2]");
        assert_eq!(RNG_TABLE[23], 128, "rndtable[23]");
        assert_eq!(RNG_TABLE[255], 249, "rndtable[255] (last entry)");
        assert_eq!(RNG_TABLE.len(), 256, "table must have 256 entries");
    }

    // -----------------------------------------------------------------------
    // p_random_range
    // -----------------------------------------------------------------------

    #[test]
    fn p_random_range_returns_value_within_bounds() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let val = gs.p_random_range(10, 20);
            assert!(val >= 10, "p_random_range value {val} must be >= 10");
            assert!(val <= 20, "p_random_range value {val} must be <= 20");
        }
    }

    #[test]
    fn p_random_range_min_equals_max_returns_that_value() {
        let mut gs = GameState::new("test");
        for _ in 0..10 {
            let val = gs.p_random_range(42, 42);
            assert_eq!(val, 42, "min==max must always return that value");
        }
    }

    // -----------------------------------------------------------------------
    // p_subrandom
    // -----------------------------------------------------------------------

    #[test]
    fn p_subrandom_range_is_minus255_to_255() {
        let mut gs = GameState::new("test");
        for _ in 0..512 {
            let val = gs.p_subrandom();
            assert!(
                (-255..=255).contains(&val),
                "p_subrandom value {val} must be in [-255, 255]"
            );
        }
    }

    #[test]
    fn p_subrandom_is_symmetric_around_zero_on_average() {
        let mut gs = GameState::new("test");
        let sum: i64 = (0..1024).map(|_| gs.p_subrandom() as i64).sum();
        // With 1024 samples, the absolute average should be reasonably small.
        // We allow generous bounds since the table is small and will cycle.
        let avg = sum.abs() as f64 / 1024.0;
        assert!(
            avg < 50.0,
            "p_subrandom average magnitude {avg} should be small, indicating symmetry"
        );
    }

    // -----------------------------------------------------------------------
    // p_damage_with_variance
    // -----------------------------------------------------------------------

    #[test]
    fn damage_variance_applies_multiplier_1_to_8() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let dmg = p_damage_with_variance(&mut gs, 10);
            assert!(dmg >= 10, "damage {dmg} must be >= base 10");
            assert!(dmg <= 80, "damage {dmg} must be <= base*8 = 80");
            assert_eq!(dmg % 10, 0, "damage {dmg} must be a multiple of base 10");
        }
    }

    #[test]
    fn damage_variance_with_base_zero_returns_zero() {
        let mut gs = GameState::new("test");
        let dmg = p_damage_with_variance(&mut gs, 0);
        assert_eq!(dmg, 0, "base 0 must return 0");
    }

    #[test]
    fn damage_variance_with_base_10_returns_10_to_80() {
        let mut gs = GameState::new("test");
        let mut min_seen = i32::MAX;
        let mut max_seen = i32::MIN;
        for _ in 0..256 {
            let dmg = p_damage_with_variance(&mut gs, 10);
            min_seen = min_seen.min(dmg);
            max_seen = max_seen.max(dmg);
        }
        assert!(min_seen >= 10, "min damage {min_seen} must be >= 10");
        assert!(max_seen <= 80, "max damage {max_seen} must be <= 80");
    }

    // -----------------------------------------------------------------------
    // Pain chance (tested via damage_mobj — see combat.rs tests)
    // We test the core p_random_chance helper here.
    // -----------------------------------------------------------------------

    #[test]
    fn p_random_chance_threshold_0_returns_false() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            assert!(
                !p_random_chance(&mut gs, 0),
                "threshold 0 must always return false"
            );
        }
    }

    #[test]
    fn p_random_chance_threshold_255_returns_true_for_most_values() {
        let mut gs = GameState::new("test");
        let true_count: usize = (0..256).filter(|_| p_random_chance(&mut gs, 255)).count();
        // Only entries with value 255 in the table return false.
        // There is 1 entry with value 255 in the table (index 33).
        assert!(
            true_count >= 250,
            "threshold 255 should return true for most values, got {true_count}/256"
        );
    }

    // -----------------------------------------------------------------------
    // randomize_tics
    // -----------------------------------------------------------------------

    #[test]
    fn randomize_tics_adds_0_to_3_to_base() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let result = randomize_tics(&mut gs, 10);
            assert!(result >= 10, "result {result} must be >= base 10");
            assert!(result <= 13, "result {result} must be <= base+3 = 13");
        }
    }

    #[test]
    fn randomize_tics_with_base_0_returns_0_to_3() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let result = randomize_tics(&mut gs, 0);
            assert!(result >= 0, "result {result} must be >= 0");
            assert!(result <= 3, "result {result} must be <= 3");
        }
    }

    // -----------------------------------------------------------------------
    // p_missile_angle_spread
    // -----------------------------------------------------------------------

    #[test]
    fn missile_angle_spread_returns_bam_compatible_values() {
        let mut gs = GameState::new("test");
        for _ in 0..256 {
            let spread = p_missile_angle_spread(&mut gs);
            // p_subrandom is [-255, 255], shifted left 20 bits.
            let max_mag = 255i32 << 20;
            assert!(
                spread.abs() <= max_mag,
                "spread {spread} magnitude must be <= {max_mag}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // RNG determinism across save/load
    // -----------------------------------------------------------------------

    #[test]
    fn rng_state_is_deterministic_across_save_load() {
        let mut gs = GameState::new("test");
        // Consume some random bytes.
        for _ in 0..77 {
            gs.p_random();
        }
        // Save the RNG index.
        let saved_index = gs.rng.index();
        // Generate a sequence.
        let seq1: Vec<u8> = (0..20).map(|_| gs.p_random()).collect();

        // Simulate restore: create fresh state, set index to saved value.
        let mut gs2 = GameState::new("test");
        gs2.rng.set_index(saved_index);
        let seq2: Vec<u8> = (0..20).map(|_| gs2.p_random()).collect();

        assert_eq!(
            seq1, seq2,
            "RNG must replay identically after restoring saved index"
        );
    }
}
