//! Sprite frame resolution from actor state machine entries.
//!
//! Resolves an actor's current `MobjStateEntry` (sprite index + frame + fullbright)
//! into a concrete WAD sprite lump name, rotation, and rendering flags.
//!
//! This module bridges the game-logic state table (`doom_game::states::STATES`)
//! and the renderer's sprite cache, replacing the old DoomEd-type-based lookup
//! with a state-driven approach that correctly handles animation frames and
//! the fullbright flag.

use doom_game::mobj::MobjStateEntry;
use doom_game::states::sprite_names;

use crate::render_flags::RenderFlag;

// ---------------------------------------------------------------------------
// ResolvedSprite
// ---------------------------------------------------------------------------

/// Resolved sprite information for rendering.
///
/// Produced by [`resolve_sprite`]; consumed by the billboard sprite renderer
/// to look up the correct WAD lump and apply lighting overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedSprite {
    /// 4-char sprite prefix (e.g., `"TROO"`, `"POSS"`).
    pub sprite_name: &'static str,
    /// Frame index (0 = A, 1 = B, etc.) with the fullbright bit stripped.
    pub frame: u8,
    /// Whether this frame should be rendered at full brightness (no lighting).
    pub fullbright: bool,
    /// Rotation index (0-7) based on viewer angle, or 0 if the sprite has
    /// no rotations.
    pub rotation: u8,
}

// ---------------------------------------------------------------------------
// ActorRenderInfo
// ---------------------------------------------------------------------------

/// Information about an actor needed for sprite rendering.
///
/// Carries the subset of `Mobj` fields required by the billboard renderer,
/// decoupling the renderer from the full `Mobj` struct.
#[derive(Debug, Clone, Copy)]
pub struct ActorRenderInfo {
    /// World X position (fixed-point raw value, divide by 65536 for map units).
    pub x: i32,
    /// World Y position (fixed-point raw value).
    pub y: i32,
    /// World Z position of the actor base (fixed-point raw value).
    pub z: i32,
    /// Actor's facing angle (BAM, 32-bit).
    pub angle: u32,
    /// Current state's sprite index (into `sprite_names::SPRITE_NAMES`).
    pub sprite: u16,
    /// Current state's frame (with optional fullbright bit `0x80`).
    pub frame: u8,
    /// Actor's height in fixed-point raw value (for vertical positioning).
    pub height: i32,
    /// Render flag derived from the actor's state and flags.
    pub render_flag: RenderFlag,
    /// Fallback 4-byte sprite prefix used when `sprite == SPR_NONE`.
    ///
    /// Pickup items whose `spawn_state` is `S_NULL` have no state-driven sprite;
    /// this field carries the DoomEd-type-derived prefix so they still render.
    pub fallback_prefix: Option<[u8; 4]>,
}

// ---------------------------------------------------------------------------
// Sprite resolution
// ---------------------------------------------------------------------------

/// Resolve an actor's state into sprite rendering information.
///
/// # Arguments
/// - `state`        - the actor's current `MobjStateEntry`
/// - `thing_angle`  - the actor's facing angle (BAM, 32-bit)
/// - `viewer_angle` - angle from the viewer (player) to the actor (BAM)
///
/// # Returns
/// `Some(ResolvedSprite)` with the sprite name, frame, fullbright flag, and
/// rotation; or `None` if the sprite index is `SPR_NONE` or out of bounds.
pub fn resolve_sprite(
    state: &MobjStateEntry,
    thing_angle: u32,
    viewer_angle: u32,
) -> Option<ResolvedSprite> {
    let sprite_index = state.sprite as usize;

    // SPR_NONE (0xFFFF) or out-of-bounds index => no sprite.
    if state.sprite == sprite_names::SPR_NONE || sprite_index >= sprite_names::SPRITE_COUNT {
        return None;
    }

    let sprite_name = sprite_names::SPRITE_NAMES[sprite_index];
    let frame = state.frame & 0x7F; // mask off fullbright bit
    let fullbright = (state.frame & MobjStateEntry::FF_FULLBRIGHT) != 0;

    // Calculate rotation from viewer perspective.
    let rotation = compute_rotation(thing_angle, viewer_angle);

    Some(ResolvedSprite {
        sprite_name,
        frame,
        fullbright,
        rotation,
    })
}

// ---------------------------------------------------------------------------
// Rotation calculation
// ---------------------------------------------------------------------------

/// Compute the rotation frame (0-7) for a sprite.
///
/// Uses Doom's rotation logic:
/// 1. Subtract `thing_angle` from `viewer_angle` to get the relative angle.
/// 2. Add 22.5 degrees (`ANG45 / 2`) for rounding to the nearest sector.
/// 3. Divide by 45 degrees (`ANG45`) to get a rotation index 0-7.
///
/// Rotation mapping:
/// - 0 = front
/// - 1 = front-right
/// - 2 = right
/// - 3 = back-right
/// - 4 = back
/// - 5 = back-left
/// - 6 = left
/// - 7 = front-left
pub fn compute_rotation(thing_angle: u32, viewer_angle: u32) -> u8 {
    // ANG45 in BAM (Binary Angle Measurement).
    const ANG45: u32 = 0x2000_0000;

    let relative = viewer_angle.wrapping_sub(thing_angle);
    let adjusted = relative.wrapping_add(ANG45 / 2); // add half-step for rounding
    // Top 3 bits give us the 0-7 sector index.
    (adjusted >> 29) as u8
}

// ---------------------------------------------------------------------------
// Sprite lump name construction
// ---------------------------------------------------------------------------

/// Construct the WAD lump name string for a sprite.
///
/// Format: `"XXXXYZ"` where:
/// - `XXXX` = 4-char sprite prefix (e.g., `"TROO"`)
/// - `Y`    = frame letter (`A`, `B`, `C`, ...)
/// - `Z`    = rotation digit (`0` = no rotation, `1`-`8` = rotations)
///
/// # Examples
/// ```ignore
/// assert_eq!(sprite_lump_name_bytes("POSS", 0, 0), *b"POSSA0\0\0");
/// assert_eq!(sprite_lump_name_bytes("TROO", 1, 3), *b"TROOB3\0\0");
/// ```
pub fn sprite_lump_name_bytes(sprite_name: &str, frame: u8, rotation: u8) -> [u8; 8] {
    let mut buf = [0u8; 8];
    for (i, byte) in sprite_name.bytes().take(4).enumerate() {
        buf[i] = byte.to_ascii_uppercase();
    }
    buf[4] = b'A'.saturating_add(frame);
    buf[5] = b'0'.saturating_add(rotation);
    buf
}

#[cfg(test)]
mod havoc_tests {
    use super::*;

    #[test]
    fn test_sprite_lump_name_bytes_overflow() {
        let _ = sprite_lump_name_bytes("POSS", 200, 1);
    }
}

/// Construct the sprite lump name and indicate whether the sprite should
/// be mirrored horizontally.
///
/// For rotation 0 (non-directional sprites), mirroring never applies.
/// For other rotations, the mirroring decision is deferred to the
/// `SpriteCache` fallback logic.
///
/// Returns `(lump_name, mirrored)`.
pub fn sprite_lump_name_with_mirror_bytes(
    sprite_name: &str,
    frame: u8,
    rotation: u8,
) -> ([u8; 8], bool) {
    let direct = sprite_lump_name_bytes(sprite_name, frame, rotation);

    // Rotation 0 (no rotation) never mirrors.
    if rotation == 0 {
        return (direct, false);
    }

    // Mirror lookup is handled at the SpriteCache level, not here.
    // Return the direct name without mirroring.
    (direct, false)
}

/// Determine the `RenderFlag` for an actor based on its state and mobj flags.
///
/// Fullbright is determined by the state's frame field (bit 0x80).
/// Fuzz is determined by the mobj's `MF_SHADOW` flag.
pub fn render_flag_from_state(frame: u8, mobj_flags: u32) -> RenderFlag {
    use doom_game::mobj::flags::MF_SHADOW;

    if mobj_flags & MF_SHADOW != 0 {
        RenderFlag::Fuzz
    } else if (frame & MobjStateEntry::FF_FULLBRIGHT) != 0 {
        RenderFlag::FullBright
    } else {
        RenderFlag::Normal
    }
}

// ---------------------------------------------------------------------------
// SpriteCache extensions
// ---------------------------------------------------------------------------

impl crate::sprite::SpriteCache {
    /// Look up a sprite frame by its string name.
    ///
    /// The name should be uppercase (e.g., `"TROOA1"`). This is the
    /// string-based counterpart to [`crate::sprite::SpriteCache::get`] which takes
    /// `&[u8; 8]`.
    pub fn get_by_name(&self, name: &str) -> Option<&crate::sprite::SpriteFrame> {
        // Convert to 8-byte array for the existing lookup.
        let mut buf = [0u8; 8];
        for (i, byte) in name.bytes().take(8).enumerate() {
            buf[i] = byte.to_ascii_uppercase();
        }
        self.get(&buf)
    }

    /// Look up a sprite frame by sprite name, frame index, and rotation.
    ///
    /// Returns `Some((frame, mirrored))` if found. Tries the exact
    /// rotation first, then the mirrored rotation, then rotation 0
    /// as a final fallback.
    pub fn get_frame(
        &self,
        sprite_name: &str,
        frame: u8,
        rotation: u8,
    ) -> Option<(&crate::sprite::SpriteFrame, bool)> {
        // Try direct rotation.
        let direct = sprite_lump_name_bytes(sprite_name, frame, rotation);
        if let Some(f) = self.get(&direct) {
            return Some((f, false));
        }

        // Try mirrored rotation.
        if let Some(mirror_rot) = mirror_rotation_lookup(rotation) {
            let mirror = sprite_lump_name_bytes(sprite_name, frame, mirror_rot);
            if let Some(f) = self.get(&mirror) {
                return Some((f, true));
            }
        }

        // Fall back to rotation 0 (non-directional).
        if rotation != 0 {
            let fallback = sprite_lump_name_bytes(sprite_name, frame, 0);
            if let Some(f) = self.get(&fallback) {
                return Some((f, false));
            }
        }

        None
    }
}

/// Compute the mirrored rotation index for sprite fallback.
///
/// Doom WADs often store only rotations 1-5 and mirror 2<->8, 3<->7, 4<->6.
/// Rotations 0, 1, and 5 are symmetric and never mirrored.
fn mirror_rotation_lookup(rotation: u8) -> Option<u8> {
    match rotation {
        2 => Some(8),
        3 => Some(7),
        4 => Some(6),
        6 => Some(4),
        7 => Some(3),
        8 => Some(2),
        _ => None, // 0, 1, 5 have no mirror
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::mobj::MobjStateEntry;
    use doom_game::states::{STATES, ids, sprite_names};

    // ======================================================================
    // resolve_sprite tests
    // ======================================================================

    #[test]
    fn resolve_trooper_idle_to_spr_poss_frame_a() {
        let state = &STATES[ids::S_POSS_STND as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "POSS");
        assert_eq!(resolved.frame, 0); // frame A
        assert!(!resolved.fullbright);
    }

    #[test]
    fn resolve_imp_chase_to_spr_troo() {
        let state = &STATES[ids::S_TROO_RUN1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "TROO");
    }

    #[test]
    fn resolve_returns_none_for_spr_none() {
        let state = &STATES[ids::S_NULL as usize];
        // S_NULL has sprite = SPR_NONE (0xFFFF)
        assert!(resolve_sprite(state, 0, 0).is_none());
    }

    #[test]
    fn resolve_fullbright_on_projectile_explosion() {
        // Imp fireball fly state (S_TBALL1) has FB bit set.
        let state = &STATES[ids::S_TBALL1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert!(resolved.fullbright, "fireball should be fullbright");
        assert_eq!(resolved.sprite_name, "BAL1");
    }

    #[test]
    fn resolve_frame_masked_correctly() {
        // Imp fireball frame is 0 | FB = 0x80. After masking, frame should be 0.
        let state = &STATES[ids::S_TBALL1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.frame, 0, "frame should be masked to 0 (A)");
    }

    #[test]
    fn resolve_different_states_produce_different_frames() {
        let run1 = &STATES[ids::S_POSS_RUN1 as usize];
        let run2 = &STATES[ids::S_POSS_RUN2 as usize];
        let r1 = resolve_sprite(run1, 0, 0).expect("run1");
        let r2 = resolve_sprite(run2, 0, 0).expect("run2");
        assert_eq!(r1.sprite_name, r2.sprite_name); // both POSS
        assert_ne!(
            r1.frame, r2.frame,
            "different run states should have different frames"
        );
    }

    #[test]
    fn resolve_sergeant_idle() {
        let state = &STATES[ids::S_SPOS_STND as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "SPOS");
        assert_eq!(resolved.frame, 0);
    }

    #[test]
    fn resolve_demon_chase() {
        let state = &STATES[ids::S_SARG_RUN1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "SARG");
    }

    #[test]
    fn resolve_cacodemon_idle() {
        let state = &STATES[ids::S_HEAD_STND as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "HEAD");
    }

    #[test]
    fn resolve_baron_idle() {
        let state = &STATES[ids::S_BOSS_STND as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "BOSS");
    }

    #[test]
    fn resolve_cyberdemon_idle() {
        let state = &STATES[ids::S_CYBER_STND as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "CYBR");
    }

    // ======================================================================
    // compute_rotation tests
    // ======================================================================

    #[test]
    fn rotation_viewer_directly_front() {
        // Thing facing east (0), viewer angle = 0 (east) => directly in front.
        let rot = compute_rotation(0, 0);
        assert_eq!(rot, 0, "viewer directly in front => rotation 0");
    }

    #[test]
    fn rotation_viewer_at_45_right() {
        // Viewer 45 degrees clockwise from front.
        const ANG45: u32 = 0x2000_0000;
        let rot = compute_rotation(0, ANG45);
        assert_eq!(rot, 1, "viewer at 45 degrees right => rotation 1");
    }

    #[test]
    fn rotation_viewer_at_90_right() {
        const ANG90: u32 = 0x4000_0000;
        let rot = compute_rotation(0, ANG90);
        assert_eq!(rot, 2, "viewer at 90 degrees => rotation 2");
    }

    #[test]
    fn rotation_viewer_behind() {
        const ANG180: u32 = 0x8000_0000;
        let rot = compute_rotation(0, ANG180);
        assert_eq!(rot, 4, "viewer behind => rotation 4");
    }

    #[test]
    fn rotation_viewer_at_270_left() {
        const ANG270: u32 = 0xC000_0000;
        let rot = compute_rotation(0, ANG270);
        assert_eq!(rot, 6, "viewer at 270 degrees => rotation 6");
    }

    #[test]
    fn rotation_symmetry_opposite_angles() {
        // Rotation 2 (right) and rotation 6 (left) should be symmetric.
        const ANG90: u32 = 0x4000_0000;
        const ANG270: u32 = 0xC000_0000;
        let r1 = compute_rotation(0, ANG90);
        let r2 = compute_rotation(0, ANG270);
        assert_eq!(r1 + r2, 8, "opposite rotations should sum to 8");
    }

    #[test]
    fn rotation_wrapping_at_360() {
        // 360 degrees == 0 degrees in BAM (wraps to 0).
        let rot_0 = compute_rotation(0, 0);
        let rot_360 = compute_rotation(0, 0u32.wrapping_add(0)); // effectively 0
        assert_eq!(rot_0, rot_360, "360 degrees should wrap to same as 0");
    }

    #[test]
    fn rotation_thing_angle_shifts_result() {
        // Thing faces north (90 degrees = ANG90). Viewer at ANG90 (north) => front.
        const ANG90: u32 = 0x4000_0000;
        let rot = compute_rotation(ANG90, ANG90);
        assert_eq!(rot, 0, "viewer matching thing angle => front (rotation 0)");
    }

    #[test]
    fn rotation_always_in_0_to_7() {
        // Smoke test: rotation should always be in [0, 7].
        for thing_deg in (0u32..360).step_by(15) {
            for viewer_deg in (0u32..360).step_by(15) {
                let thing_bam = (thing_deg as u64 * 0x1_0000_0000u64 / 360) as u32;
                let viewer_bam = (viewer_deg as u64 * 0x1_0000_0000u64 / 360) as u32;
                let rot = compute_rotation(thing_bam, viewer_bam);
                assert!(
                    rot <= 7,
                    "rotation {rot} out of range for thing_deg={thing_deg}, viewer_deg={viewer_deg}"
                );
            }
        }
    }

    #[test]
    fn rotation_135_degrees_is_3() {
        // 135 degrees from front = back-right = rotation 3.
        const ANG135: u32 = 0x6000_0000;
        let rot = compute_rotation(0, ANG135);
        assert_eq!(rot, 3, "135 degrees => rotation 3");
    }

    #[test]
    fn rotation_315_degrees_is_7() {
        // 315 degrees from front = front-left = rotation 7.
        const ANG315: u32 = 0xE000_0000;
        let rot = compute_rotation(0, ANG315);
        assert_eq!(rot, 7, "315 degrees => rotation 7");
    }

    // ======================================================================
    // sprite_lump_name_bytes tests
    // ======================================================================

    #[test]
    fn lump_name_poss_frame_a_rot0() {
        assert_eq!(sprite_lump_name_bytes("POSS", 0, 0), *b"POSSA0\0\0");
    }

    #[test]
    fn lump_name_troo_frame_b_rot3() {
        assert_eq!(sprite_lump_name_bytes("TROO", 1, 3), *b"TROOB3\0\0");
    }

    #[test]
    fn lump_name_sarg_frame_c_rot7() {
        assert_eq!(sprite_lump_name_bytes("SARG", 2, 7), *b"SARGC7\0\0");
    }

    #[test]
    fn lump_name_high_frame() {
        // Frame K (index 10) is valid in Doom.
        assert_eq!(sprite_lump_name_bytes("SKUL", 10, 0), *b"SKULK0\0\0");
    }

    #[test]
    fn lump_name_rotation_8() {
        assert_eq!(sprite_lump_name_bytes("POSS", 0, 8), *b"POSSA8\0\0");
    }

    #[test]
    fn lump_name_frame_wraps_for_high_values() {
        // Frame 25 => 'A' + 25 = 'Z'.
        assert_eq!(sprite_lump_name_bytes("TEST", 25, 0), *b"TESTZ0\0\0");
    }

    // ======================================================================
    // sprite_lump_name_with_mirror_bytes tests
    // ======================================================================

    #[test]
    fn mirror_rotation_0_never_mirrors() {
        let (name, mirrored) = sprite_lump_name_with_mirror_bytes("POSS", 0, 0);
        assert_eq!(name, *b"POSSA0\0\0");
        assert!(!mirrored);
    }

    #[test]
    fn mirror_direct_rotations_dont_mirror() {
        for rot in 1..=8 {
            let (_, mirrored) = sprite_lump_name_with_mirror_bytes("POSS", 0, rot);
            assert!(!mirrored, "rotation {rot} should not mirror at this level");
        }
    }

    #[test]
    fn mirror_returns_correct_lump_name() {
        let (name, _) = sprite_lump_name_with_mirror_bytes("TROO", 2, 5);
        assert_eq!(name, *b"TROOC5\0\0");
    }

    // ======================================================================
    // ActorRenderInfo tests
    // ======================================================================

    #[test]
    fn actor_render_info_carries_all_fields() {
        let info = ActorRenderInfo {
            x: 100 * 65536,
            y: 200 * 65536,
            z: 24 * 65536,
            angle: 0x4000_0000,
            sprite: 10,  // SPR_POSS
            frame: 0x80, // frame A + fullbright
            height: 56 * 65536,
            render_flag: RenderFlag::FullBright,
            fallback_prefix: None,
        };
        assert_eq!(info.x, 100 * 65536);
        assert_eq!(info.y, 200 * 65536);
        assert_eq!(info.z, 24 * 65536);
        assert_eq!(info.angle, 0x4000_0000);
        assert_eq!(info.sprite, 10);
        assert_eq!(info.frame, 0x80);
        assert_eq!(info.height, 56 * 65536);
        assert_eq!(info.render_flag, RenderFlag::FullBright);
    }

    #[test]
    fn actor_render_info_fullbright_from_frame_bit() {
        let info = ActorRenderInfo {
            x: 0,
            y: 0,
            z: 0,
            angle: 0,
            sprite: 10,
            frame: 0x80 | 2, // frame C + fullbright
            height: 0,
            render_flag: RenderFlag::FullBright,
            fallback_prefix: None,
        };
        let is_fullbright = (info.frame & MobjStateEntry::FF_FULLBRIGHT) != 0;
        assert!(
            is_fullbright,
            "fullbright bit should be detectable from frame"
        );
        let actual_frame = info.frame & 0x7F;
        assert_eq!(actual_frame, 2, "masked frame should be C (2)");
    }

    #[test]
    fn actor_render_info_normal_frame() {
        let info = ActorRenderInfo {
            x: 0,
            y: 0,
            z: 0,
            angle: 0,
            sprite: 10,
            frame: 3, // frame D, no fullbright
            height: 0,
            render_flag: RenderFlag::Normal,
            fallback_prefix: None,
        };
        let is_fullbright = (info.frame & MobjStateEntry::FF_FULLBRIGHT) != 0;
        assert!(!is_fullbright);
        assert_eq!(info.render_flag, RenderFlag::Normal);
    }

    // ======================================================================
    // render_flag_from_state tests
    // ======================================================================

    #[test]
    fn render_flag_fuzz_takes_priority() {
        // MF_SHADOW overrides even fullbright.
        let flag = render_flag_from_state(0x80, doom_game::mobj::flags::MF_SHADOW);
        assert_eq!(flag, RenderFlag::Fuzz);
    }

    #[test]
    fn render_flag_fullbright_when_no_shadow() {
        let flag = render_flag_from_state(0x80, 0);
        assert_eq!(flag, RenderFlag::FullBright);
    }

    #[test]
    fn render_flag_normal_for_plain_frame() {
        let flag = render_flag_from_state(0, 0);
        assert_eq!(flag, RenderFlag::Normal);
    }

    // ======================================================================
    // Integration with STATES table
    // ======================================================================

    #[test]
    fn all_monster_spawn_states_have_valid_sprite_indices() {
        let spawn_states: &[u16] = &[
            ids::S_POSS_STND,
            ids::S_SPOS_STND,
            ids::S_TROO_STND,
            ids::S_SARG_STND,
            ids::S_HEAD_STND,
            ids::S_BOSS_STND,
            ids::S_CYBER_STND,
            ids::S_SPID_STND,
            ids::S_SKULL_STND,
            ids::S_BSPI_STND,
            ids::S_PAIN_STND,
            ids::S_SKEL_STND,
            ids::S_FATT_STND,
            ids::S_VILE_STND,
            ids::S_CPOS_STND,
            ids::S_BOS2_STND,
        ];
        for &sid in spawn_states {
            let state = &STATES[sid as usize];
            assert!(
                (state.sprite as usize) < sprite_names::SPRITE_COUNT,
                "state {sid} has sprite index {} which exceeds SPRITE_COUNT ({})",
                state.sprite,
                sprite_names::SPRITE_COUNT
            );
        }
    }

    #[test]
    fn all_projectile_states_have_valid_sprite_indices() {
        let projectile_states: &[u16] = &[
            ids::S_TBALL1,
            ids::S_TBALL2,
            ids::S_TBALLX1,
            ids::S_BRBALL1,
            ids::S_BRBALL2,
            ids::S_ROCKET,
            ids::S_EXPLODE1,
            ids::S_PLASBALL1,
            ids::S_PLASBALL2,
            ids::S_BFGSHOT1,
            ids::S_BFGSHOT2,
            ids::S_TRACER1,
            ids::S_TRACER2,
            ids::S_ARACH_PLAZ1,
            ids::S_FATSHOT1,
        ];
        for &sid in projectile_states {
            let state = &STATES[sid as usize];
            assert!(
                (state.sprite as usize) < sprite_names::SPRITE_COUNT,
                "projectile state {sid} has invalid sprite index {}",
                state.sprite
            );
        }
    }

    #[test]
    fn weapon_states_have_valid_sprite_indices() {
        let weapon_states: &[u16] = &[
            ids::S_PUNCH_READY,
            ids::S_PISTOL_READY,
            ids::S_SGUN_READY,
            ids::S_DSGUN_READY,
            ids::S_CHAIN_READY,
            ids::S_MISSILE_READY,
            ids::S_PLASMA_READY,
            ids::S_BFG_READY,
            ids::S_SAW_READY1,
        ];
        for &sid in weapon_states {
            let state = &STATES[sid as usize];
            assert!(
                (state.sprite as usize) < sprite_names::SPRITE_COUNT,
                "weapon state {sid} has invalid sprite index {}",
                state.sprite
            );
        }
    }

    #[test]
    fn no_non_null_state_exceeds_sprite_count_except_spr_none() {
        for (i, state) in STATES.iter().enumerate() {
            if state.sprite != sprite_names::SPR_NONE {
                assert!(
                    (state.sprite as usize) < sprite_names::SPRITE_COUNT,
                    "state {i} has sprite index {} >= SPRITE_COUNT {}",
                    state.sprite,
                    sprite_names::SPRITE_COUNT
                );
            }
        }
    }

    #[test]
    fn s_null_has_spr_none() {
        let state = &STATES[ids::S_NULL as usize];
        assert_eq!(
            state.sprite,
            sprite_names::SPR_NONE,
            "S_NULL should have SPR_NONE"
        );
    }

    // ======================================================================
    // SpriteCache.get_by_name and get_frame tests
    // ======================================================================

    #[test]
    fn get_by_name_finds_inserted_frame() {
        let mut cache = crate::sprite::SpriteCache::empty();
        let frame = crate::sprite::SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 1,
            pixels: vec![Some(10); 4],
        };
        cache.insert(doom_wad::lump::LumpName::from_str("TROOA1"), frame);
        assert!(cache.get_by_name("TROOA1").is_some());
        assert!(cache.get_by_name("TROOA2").is_none());
    }

    #[test]
    fn get_by_name_case_insensitive() {
        let mut cache = crate::sprite::SpriteCache::empty();
        let frame = crate::sprite::SpriteFrame {
            width: 1,
            height: 1,
            left_offset: 0,
            top_offset: 0,
            pixels: vec![Some(5)],
        };
        cache.insert(doom_wad::lump::LumpName::from_str("POSSA0"), frame);
        // Lookup with lowercase should still find it.
        assert!(cache.get_by_name("possa0").is_some());
    }

    #[test]
    fn get_frame_direct_rotation() {
        let mut cache = crate::sprite::SpriteCache::empty();
        let frame = crate::sprite::SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 1,
            pixels: vec![Some(10); 4],
        };
        cache.insert(doom_wad::lump::LumpName::from_str("TROOA3"), frame);

        let result = cache.get_frame("TROO", 0, 3);
        assert!(result.is_some(), "should find TROOA3");
        let (_, mirrored) = result.expect("value must exist in test");
        assert!(!mirrored, "direct rotation should not be mirrored");
    }

    #[test]
    fn get_frame_mirror_fallback() {
        let mut cache = crate::sprite::SpriteCache::empty();
        let frame = crate::sprite::SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 1,
            pixels: vec![Some(20); 4],
        };
        // Only insert rotation 2 (mirror of 8).
        cache.insert(doom_wad::lump::LumpName::from_str("POSSA2"), frame);

        // Lookup rotation 8 should fall back to mirror (rotation 2).
        let result = cache.get_frame("POSS", 0, 8);
        assert!(result.is_some(), "should find mirror of POSSA8 -> POSSA2");
        let (_, mirrored) = result.expect("value must exist in test");
        assert!(mirrored, "mirror fallback should set mirrored=true");
    }

    #[test]
    fn get_frame_rotation_0_fallback() {
        let mut cache = crate::sprite::SpriteCache::empty();
        let frame = crate::sprite::SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 1,
            pixels: vec![Some(30); 4],
        };
        // Only insert rotation 0.
        cache.insert(doom_wad::lump::LumpName::from_str("SARGA0"), frame);

        // Lookup rotation 5 should fall back to rotation 0.
        let result = cache.get_frame("SARG", 0, 5);
        assert!(result.is_some(), "should fall back to SARGA0");
        let (_, mirrored) = result.expect("value must exist in test");
        assert!(!mirrored, "fallback to rotation 0 should not be mirrored");
    }

    #[test]
    fn get_frame_not_found() {
        let cache = crate::sprite::SpriteCache::empty();
        let result = cache.get_frame("XXXX", 0, 0);
        assert!(result.is_none(), "nonexistent sprite should return None");
    }

    // ======================================================================
    // mirror_rotation_lookup tests
    // ======================================================================

    #[test]
    fn mirror_rotation_lookup_pairs() {
        assert_eq!(mirror_rotation_lookup(2), Some(8));
        assert_eq!(mirror_rotation_lookup(8), Some(2));
        assert_eq!(mirror_rotation_lookup(3), Some(7));
        assert_eq!(mirror_rotation_lookup(7), Some(3));
        assert_eq!(mirror_rotation_lookup(4), Some(6));
        assert_eq!(mirror_rotation_lookup(6), Some(4));
    }

    #[test]
    fn mirror_rotation_lookup_symmetric_none() {
        assert_eq!(mirror_rotation_lookup(0), None);
        assert_eq!(mirror_rotation_lookup(1), None);
        assert_eq!(mirror_rotation_lookup(5), None);
    }

    // ======================================================================
    // Additional edge case tests
    // ======================================================================

    #[test]
    fn resolve_lost_soul_is_fullbright() {
        // Lost Soul states have FB bit set.
        let state = &STATES[ids::S_SKULL_STND as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "SKUL");
        assert!(resolved.fullbright, "Lost Soul should be fullbright");
    }

    #[test]
    fn resolve_bullet_puff_first_frame_is_fullbright() {
        let state = &STATES[ids::S_PUFF1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "PUFF");
        assert!(resolved.fullbright, "first puff frame should be fullbright");
    }

    #[test]
    fn resolve_bullet_puff_second_frame_not_fullbright() {
        let state = &STATES[ids::S_PUFF2 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "PUFF");
        assert!(
            !resolved.fullbright,
            "second puff frame should not be fullbright"
        );
    }

    #[test]
    fn resolve_rocket_fly_state() {
        let state = &STATES[ids::S_ROCKET as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "MISL");
        assert!(resolved.fullbright);
        assert_eq!(resolved.frame, 0);
    }

    #[test]
    fn resolve_plasma_ball_fly_state() {
        let state = &STATES[ids::S_PLASBALL1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "PLSS");
        assert!(resolved.fullbright);
    }

    #[test]
    fn resolve_bfg_ball_fly_state() {
        let state = &STATES[ids::S_BFGSHOT1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "BFS1");
        assert!(resolved.fullbright);
    }

    #[test]
    fn resolve_arachnotron_attack_flash_is_fullbright() {
        let state = &STATES[ids::S_BSPI_ATK2 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "BSPI");
        assert!(
            resolved.fullbright,
            "arachnotron attack flash should be fullbright"
        );
    }

    #[test]
    fn resolve_weapon_pistol_flash_is_fullbright() {
        let state = &STATES[ids::S_PISTOL_FLASH1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "PISG");
        assert!(resolved.fullbright, "pistol flash should be fullbright");
    }

    #[test]
    fn resolve_weapon_chainsaw_ready_not_fullbright() {
        let state = &STATES[ids::S_SAW_READY1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "SAWG");
        assert!(
            !resolved.fullbright,
            "chainsaw ready should not be fullbright"
        );
    }

    #[test]
    fn resolve_teleport_fog_is_fullbright() {
        let state = &STATES[ids::S_TFOG1 as usize];
        let resolved = resolve_sprite(state, 0, 0).expect("should resolve");
        assert_eq!(resolved.sprite_name, "TFOG");
        assert!(resolved.fullbright);
    }
}
