//! Weapon sprite animation: bob, raise/lower, and muzzle flash.
//!
//! This module drives the visual side of weapon rendering.  The game logic
//! (doom-game) decides *which* weapon is active and when to fire; this module
//! translates that into screen-space positions with lateral sway (bob),
//! vertical raise/lower transitions, and muzzle-flash overlays.
//!
//! # Overview
//!
//! - [`WeaponBob`] computes frame-by-frame lateral and vertical offsets that
//!   sway the weapon sprite in rhythm with the player's movement speed.
//! - [`WeaponSprite`] stores the current sprite name, screen position, and
//!   flash state for a single weapon overlay.
//! - [`WeaponAnimState`] combines the above into a full animation controller
//!   that external code advances once per game tic.
//! - [`draw_weapon_animated`] renders the current state into a [`Framebuffer`].

use crate::column::IDENTITY_COLORMAP;
use crate::framebuffer::Framebuffer;
use crate::sprite::{SpriteCache, draw_sprite};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Horizontal center of the weapon overlay (screen X).
pub const WEAPON_BASE_X: i32 = 160;

/// Vertical baseline of the weapon overlay (screen Y).
pub const WEAPON_BASE_Y: i32 = 167;

/// Y offset when the weapon is fully lowered (off-screen below the view).
pub const WEAPON_BOTTOM: i32 = 128;

/// Y offset when the weapon is fully raised (normal play position).
pub const WEAPON_TOP: i32 = 0;

/// Pixels per tic for raise / lower transitions.
pub const RAISE_SPEED: i32 = 6;

/// Maximum bob amplitude (16 in 16.16 fixed-point = `0x10_0000`).
pub const MAX_BOB: i32 = 0x10_0000;

/// Phase advance per tic.
///
/// ANG90 / 20 in BAM (Binary Angle Measure) units:
/// ANG90 = 0x4000_0000, so ANG90 / 20 = 0x0333_3333 (~4.5 degrees/tic).
const ANGLE_PER_TIC: u32 = 0x0333_3333;

// ---------------------------------------------------------------------------
// WeaponSprite
// ---------------------------------------------------------------------------

/// Snapshot of the weapon overlay's visual state for a single frame.
#[derive(Clone, Debug)]
pub struct WeaponSprite {
    /// Current sprite name (8 bytes, null-padded).
    pub sprite_name: [u8; 8],
    /// X position on screen (normally 160, modified by bob).
    pub sx: i32,
    /// Y position on screen (normally 167, modified by raise/lower).
    pub sy: i32,
    /// Is the weapon currently being raised?
    pub raising: bool,
    /// Is the weapon currently being lowered?
    pub lowering: bool,
    /// Muzzle flash active (draws bright overlay).
    pub flash_active: bool,
    /// Flash sprite name (if different from main sprite).
    pub flash_sprite: [u8; 8],
    /// Flash tics remaining.
    pub flash_tics: u32,
    /// Full bright flag (weapon flash always draws at full brightness).
    pub full_bright: bool,
}

impl Default for WeaponSprite {
    fn default() -> Self {
        Self {
            sprite_name: *b"PISGA0\0\0",
            sx: WEAPON_BASE_X,
            sy: WEAPON_BASE_Y,
            raising: false,
            lowering: false,
            flash_active: false,
            flash_sprite: [0u8; 8],
            flash_tics: 0,
            full_bright: false,
        }
    }
}

// ---------------------------------------------------------------------------
// WeaponBob
// ---------------------------------------------------------------------------

/// Lateral and vertical sway that moves the weapon sprite during walking.
///
/// The bob is purely cosmetic and has no effect on game state.  Phase advances
/// each tic; amplitude tracks the player's movement speed (clamped to
/// [`MAX_BOB`]).  The offsets are computed using `f32` trigonometry because the
/// bob is a renderer-only effect and does not need deterministic fixed-point
/// reproducibility.
#[derive(Clone, Debug, Default)]
pub struct WeaponBob {
    /// Bob amplitude (increases with player speed, capped at [`MAX_BOB`]).
    pub amplitude: i32,
    /// Bob phase angle (BAM units, advances each tic).
    pub phase: u32,
    /// Horizontal bob offset (computed from amplitude + phase).
    pub offset_x: i32,
    /// Vertical bob offset (double-frequency, half-amplitude).
    pub offset_y: i32,
}

impl WeaponBob {
    /// Advance one tic of bob state.
    ///
    /// `player_speed` should be the magnitude of the player's velocity in
    /// 16.16 fixed-point units.  Zero speed produces zero offset.
    pub fn tick(&mut self, player_speed: i32) {
        self.amplitude = player_speed.min(MAX_BOB);
        self.phase = self.phase.wrapping_add(ANGLE_PER_TIC);

        // Convert BAM phase to radians.
        let phase_rad = bam_to_radians(self.phase);

        // Lateral sway: cos(phase) * amplitude >> 16.
        let cos_val = phase_rad.cos();
        self.offset_x = ((self.amplitude as f32 * cos_val) as i64 >> 16) as i32;

        // Vertical bounce: sin(2 * phase) * amplitude >> 17.
        // Double frequency, half amplitude produces a bounce rhythm.
        let sin_val = (phase_rad * 2.0).sin();
        self.offset_y = ((self.amplitude as f32 * sin_val) as i64 >> 17) as i32;
    }

    /// Reset all bob state to zero.
    pub fn reset(&mut self) {
        self.amplitude = 0;
        self.phase = 0;
        self.offset_x = 0;
        self.offset_y = 0;
    }
}

/// Convert a BAM angle to radians.
#[inline]
fn bam_to_radians(bam: u32) -> f32 {
    bam as f32 * (2.0 * std::f32::consts::PI / (u32::MAX as f32 + 1.0))
}

// ---------------------------------------------------------------------------
// WeaponAnimState
// ---------------------------------------------------------------------------

/// Full weapon animation controller: bob + raise/lower + muzzle flash.
///
/// Call [`tick`](Self::tick) once per game tic to advance the animation.
/// Call [`draw_weapon_animated`] to render the result into a framebuffer.
#[derive(Clone, Debug)]
pub struct WeaponAnimState {
    /// Current weapon overlay state.
    pub current: WeaponSprite,
    /// Lateral/vertical bob state.
    pub bob: WeaponBob,
    /// Y offset for raise/lower animation (0 = fully raised, [`WEAPON_BOTTOM`] = fully lowered).
    pub raise_offset: i32,
    /// Target Y for raise (0 = fully raised, [`WEAPON_BOTTOM`] = fully lowered).
    pub raise_target: i32,
    /// Speed of raise/lower (pixels per tic).
    pub raise_speed: i32,
}

impl WeaponAnimState {
    /// Create a new animation state with the pistol sprite, fully raised.
    pub fn new() -> Self {
        Self {
            current: WeaponSprite::default(),
            bob: WeaponBob::default(),
            raise_offset: WEAPON_TOP,
            raise_target: WEAPON_TOP,
            raise_speed: RAISE_SPEED,
        }
    }

    /// Advance one game tic: bob, raise/lower, flash countdown.
    pub fn tick(&mut self, player_speed: i32) {
        // Advance bob.
        self.bob.tick(player_speed);

        // Advance raise/lower.
        if self.raise_offset < self.raise_target {
            // Lowering: offset increases toward target.
            self.raise_offset = (self.raise_offset + self.raise_speed).min(self.raise_target);
            if self.raise_offset >= self.raise_target {
                self.current.lowering = false;
            }
        } else if self.raise_offset > self.raise_target {
            // Raising: offset decreases toward target.
            self.raise_offset = (self.raise_offset - self.raise_speed).max(self.raise_target);
            if self.raise_offset <= self.raise_target {
                self.current.raising = false;
            }
        }

        // Advance flash countdown.
        if self.current.flash_tics > 0 {
            self.current.flash_tics -= 1;
            if self.current.flash_tics == 0 {
                self.current.flash_active = false;
                self.current.full_bright = false;
            }
        }

        // Update screen position from computed values.
        self.current.sx = self.screen_x();
        self.current.sy = self.screen_y();
    }

    /// Begin raising the weapon from the bottom of the screen.
    pub fn start_raise(&mut self) {
        self.raise_offset = WEAPON_BOTTOM;
        self.raise_target = WEAPON_TOP;
        self.current.raising = true;
        self.current.lowering = false;
    }

    /// Begin lowering the weapon to the bottom of the screen.
    pub fn start_lower(&mut self) {
        self.raise_target = WEAPON_BOTTOM;
        self.current.lowering = true;
        self.current.raising = false;
    }

    /// Start a muzzle-flash overlay.
    pub fn trigger_flash(&mut self, sprite_name: [u8; 8], duration: u32) {
        self.current.flash_sprite = sprite_name;
        self.current.flash_tics = duration;
        self.current.flash_active = true;
        self.current.full_bright = true;
    }

    /// Returns `true` when the weapon is fully raised and not transitioning.
    pub fn is_ready(&self) -> bool {
        self.raise_offset == WEAPON_TOP && !self.current.raising && !self.current.lowering
    }

    /// Compute the final screen X including bob offset.
    pub fn screen_x(&self) -> i32 {
        WEAPON_BASE_X + self.bob.offset_x
    }

    /// Compute the final screen Y including bob offset and raise offset.
    pub fn screen_y(&self) -> i32 {
        WEAPON_BASE_Y + self.bob.offset_y + self.raise_offset
    }
}

impl Default for WeaponAnimState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Weapon sprite name table
// ---------------------------------------------------------------------------

/// Return the default sprite lump name for a weapon ID.
///
/// Standard Doom weapon IDs:
/// 0 = fist, 1 = pistol, 2 = shotgun, 3 = chaingun, 4 = rocket launcher,
/// 5 = plasma rifle, 6 = BFG 9000, 7 = chainsaw, 8 = super shotgun.
/// Unknown IDs fall back to the pistol sprite.
pub fn weapon_sprite_name(weapon_id: u8) -> [u8; 8] {
    match weapon_id {
        0 => *b"PUNGA0\0\0", // Fist/punch
        1 => *b"PISGA0\0\0", // Pistol
        2 => *b"SHTGA0\0\0", // Shotgun
        3 => *b"CHGGA0\0\0", // Chaingun
        4 => *b"ROCKA0\0\0", // Rocket launcher
        5 => *b"PLSGA0\0\0", // Plasma
        6 => *b"BFGGA0\0\0", // BFG
        7 => *b"SAWGA0\0\0", // Chainsaw
        8 => *b"SHT2A0\0\0", // Super shotgun
        _ => *b"PISGA0\0\0", // fallback
    }
}

// ---------------------------------------------------------------------------
// Animated weapon renderer
// ---------------------------------------------------------------------------

/// Draw the weapon overlay using the current animation state.
///
/// Draws the main weapon sprite at the position computed by [`WeaponAnimState`],
/// then overlays the muzzle flash sprite (at full brightness) if active.
///
/// Does nothing for sprites not found in the cache (matches vanilla behaviour).
pub fn draw_weapon_animated(
    fb: &mut Framebuffer,
    anim: &WeaponAnimState,
    cache: &SpriteCache,
    colormap: &[u8; 256],
) {
    let sx = anim.screen_x();
    let sy = anim.screen_y();

    // Choose the colormap: full-bright during flash, otherwise caller's.
    let main_colormap = if anim.current.full_bright {
        &IDENTITY_COLORMAP
    } else {
        colormap
    };

    // Draw main weapon sprite.
    if let Some(frame) = cache.get(&anim.current.sprite_name) {
        draw_sprite(fb, frame, sx, sy, main_colormap);
    }

    // Draw muzzle flash overlay at full brightness.
    if anim.current.flash_active {
        if let Some(flash_frame) = cache.get(&anim.current.flash_sprite) {
            draw_sprite(fb, flash_frame, sx, sy, &IDENTITY_COLORMAP);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------
    // WeaponSprite
    // ------------------------------------------------------------------

    #[test]
    fn weapon_sprite_default_values() {
        let ws = WeaponSprite::default();
        assert_eq!(ws.sprite_name, *b"PISGA0\0\0");
        assert_eq!(ws.sx, WEAPON_BASE_X);
        assert_eq!(ws.sy, WEAPON_BASE_Y);
        assert!(!ws.raising);
        assert!(!ws.lowering);
        assert!(!ws.flash_active);
        assert_eq!(ws.flash_tics, 0);
        assert!(!ws.full_bright);
    }

    #[test]
    fn weapon_sprite_clone() {
        let ws = WeaponSprite::default();
        let ws2 = ws.clone();
        assert_eq!(ws2.sprite_name, ws.sprite_name);
    }

    // ------------------------------------------------------------------
    // WeaponBob
    // ------------------------------------------------------------------

    #[test]
    fn weapon_bob_tick_advances_phase() {
        let mut bob = WeaponBob::default();
        let phase_before = bob.phase;
        bob.tick(0x1_0000); // some nonzero speed
        assert_ne!(bob.phase, phase_before, "phase should advance after tick");
        assert_eq!(bob.phase, ANGLE_PER_TIC);
    }

    #[test]
    fn weapon_bob_tick_computes_offsets_based_on_speed() {
        let mut bob = WeaponBob::default();
        // Give it a moderate speed and tick several times to get past phase 0.
        for _ in 0..5 {
            bob.tick(0x8_0000); // half of MAX_BOB
        }
        // At phase = 5 * ANGLE_PER_TIC, cos and sin are nonzero, so offsets
        // should be nonzero.
        assert!(
            bob.offset_x != 0 || bob.offset_y != 0,
            "at least one bob offset should be nonzero with speed and phase"
        );
    }

    #[test]
    fn weapon_bob_zero_speed_produces_zero_offset() {
        let mut bob = WeaponBob::default();
        bob.tick(0);
        assert_eq!(bob.offset_x, 0, "zero speed → zero X offset");
        assert_eq!(bob.offset_y, 0, "zero speed → zero Y offset");
    }

    #[test]
    fn weapon_bob_amplitude_capped_at_max_bob() {
        let mut bob = WeaponBob::default();
        bob.tick(MAX_BOB * 2); // way above cap
        assert_eq!(bob.amplitude, MAX_BOB, "amplitude should clamp to MAX_BOB");
    }

    #[test]
    fn weapon_bob_reset_clears_all_state() {
        let mut bob = WeaponBob::default();
        bob.tick(0x5_0000);
        bob.tick(0x5_0000);
        bob.reset();
        assert_eq!(bob.amplitude, 0);
        assert_eq!(bob.phase, 0);
        assert_eq!(bob.offset_x, 0);
        assert_eq!(bob.offset_y, 0);
    }

    // ------------------------------------------------------------------
    // WeaponAnimState
    // ------------------------------------------------------------------

    #[test]
    fn weapon_anim_state_new_starts_with_pistol() {
        let state = WeaponAnimState::new();
        assert_eq!(state.current.sprite_name, *b"PISGA0\0\0");
        assert_eq!(state.raise_offset, WEAPON_TOP);
        assert_eq!(state.raise_target, WEAPON_TOP);
    }

    #[test]
    fn weapon_anim_state_start_raise_sets_flag() {
        let mut state = WeaponAnimState::new();
        state.start_raise();
        assert!(state.current.raising);
        assert!(!state.current.lowering);
        assert_eq!(state.raise_offset, WEAPON_BOTTOM);
        assert_eq!(state.raise_target, WEAPON_TOP);
    }

    #[test]
    fn weapon_anim_state_start_lower_sets_flag() {
        let mut state = WeaponAnimState::new();
        state.start_lower();
        assert!(state.current.lowering);
        assert!(!state.current.raising);
        assert_eq!(state.raise_target, WEAPON_BOTTOM);
    }

    #[test]
    fn weapon_anim_tick_advances_raise_toward_target() {
        let mut state = WeaponAnimState::new();
        state.start_raise(); // raise_offset = WEAPON_BOTTOM, target = WEAPON_TOP
        let initial = state.raise_offset;
        state.tick(0);
        // After one tick, raise_offset should have decreased by RAISE_SPEED.
        assert_eq!(
            state.raise_offset,
            initial - RAISE_SPEED,
            "raise offset should decrease by RAISE_SPEED per tic"
        );
    }

    #[test]
    fn weapon_anim_tick_advances_lower_toward_target() {
        let mut state = WeaponAnimState::new();
        state.start_lower(); // raise_offset = 0, target = WEAPON_BOTTOM
        state.tick(0);
        assert_eq!(
            state.raise_offset, RAISE_SPEED,
            "lower offset should increase by RAISE_SPEED per tic"
        );
    }

    #[test]
    fn weapon_anim_fully_raised_is_ready() {
        let state = WeaponAnimState::new();
        assert!(
            state.is_ready(),
            "newly created state should be ready (fully raised)"
        );
    }

    #[test]
    fn weapon_anim_during_raise_not_ready() {
        let mut state = WeaponAnimState::new();
        state.start_raise();
        assert!(
            !state.is_ready(),
            "weapon should not be ready while raising"
        );
    }

    #[test]
    fn weapon_anim_during_lower_not_ready() {
        let mut state = WeaponAnimState::new();
        state.start_lower();
        state.tick(0); // advance one tic so offset != 0
        assert!(
            !state.is_ready(),
            "weapon should not be ready while lowering"
        );
    }

    #[test]
    fn weapon_anim_raise_completes() {
        let mut state = WeaponAnimState::new();
        state.start_raise();
        // Tick enough times to fully raise.
        for _ in 0..100 {
            state.tick(0);
        }
        assert!(state.is_ready(), "weapon should be ready after full raise");
        assert_eq!(state.raise_offset, WEAPON_TOP);
    }

    // ------------------------------------------------------------------
    // Flash
    // ------------------------------------------------------------------

    #[test]
    fn trigger_flash_sets_active_and_duration() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 4);
        assert!(state.current.flash_active);
        assert!(state.current.full_bright);
        assert_eq!(state.current.flash_tics, 4);
        assert_eq!(state.current.flash_sprite, *b"PISFA0\0\0");
    }

    #[test]
    fn flash_tics_decrement_each_tick() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 3);
        state.tick(0);
        assert_eq!(state.current.flash_tics, 2);
        state.tick(0);
        assert_eq!(state.current.flash_tics, 1);
    }

    #[test]
    fn flash_deactivates_when_tics_reach_zero() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 2);
        state.tick(0);
        assert!(
            state.current.flash_active,
            "flash should still be active after 1 of 2 tics"
        );
        state.tick(0);
        assert!(
            !state.current.flash_active,
            "flash should deactivate when tics reach 0"
        );
        assert!(!state.current.full_bright);
    }

    // ------------------------------------------------------------------
    // Screen position
    // ------------------------------------------------------------------

    #[test]
    fn screen_x_includes_bob_offset() {
        let mut state = WeaponAnimState::new();
        state.bob.offset_x = 5;
        assert_eq!(state.screen_x(), WEAPON_BASE_X + 5);
    }

    #[test]
    fn screen_y_includes_bob_offset_and_raise_offset() {
        let mut state = WeaponAnimState::new();
        state.bob.offset_y = 3;
        state.raise_offset = 10;
        assert_eq!(state.screen_y(), WEAPON_BASE_Y + 3 + 10);
    }

    #[test]
    fn screen_y_default_is_base_y() {
        let state = WeaponAnimState::new();
        assert_eq!(state.screen_y(), WEAPON_BASE_Y);
    }

    // ------------------------------------------------------------------
    // weapon_sprite_name
    // ------------------------------------------------------------------

    #[test]
    fn weapon_sprite_name_returns_correct_names() {
        assert_eq!(weapon_sprite_name(0), *b"PUNGA0\0\0");
        assert_eq!(weapon_sprite_name(1), *b"PISGA0\0\0");
        assert_eq!(weapon_sprite_name(2), *b"SHTGA0\0\0");
        assert_eq!(weapon_sprite_name(3), *b"CHGGA0\0\0");
        assert_eq!(weapon_sprite_name(4), *b"ROCKA0\0\0");
        assert_eq!(weapon_sprite_name(5), *b"PLSGA0\0\0");
        assert_eq!(weapon_sprite_name(6), *b"BFGGA0\0\0");
        assert_eq!(weapon_sprite_name(7), *b"SAWGA0\0\0");
        assert_eq!(weapon_sprite_name(8), *b"SHT2A0\0\0");
    }

    #[test]
    fn weapon_sprite_name_fallback_for_unknown() {
        assert_eq!(weapon_sprite_name(9), *b"PISGA0\0\0");
        assert_eq!(weapon_sprite_name(255), *b"PISGA0\0\0");
    }

    // ------------------------------------------------------------------
    // draw_weapon_animated
    // ------------------------------------------------------------------

    #[test]
    fn draw_weapon_animated_no_panic_empty_cache() {
        let mut fb = Framebuffer::new();
        let state = WeaponAnimState::new();
        let cache = SpriteCache::empty();
        draw_weapon_animated(&mut fb, &state, &cache, &IDENTITY_COLORMAP);
        // Should not panic — sprite not found is silently skipped.
    }

    #[test]
    fn draw_weapon_animated_no_panic_with_flash() {
        let mut fb = Framebuffer::new();
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 4);
        let cache = SpriteCache::empty();
        draw_weapon_animated(&mut fb, &state, &cache, &IDENTITY_COLORMAP);
        // Should not panic even with flash active and missing sprites.
    }

    // ------------------------------------------------------------------
    // Constants
    // ------------------------------------------------------------------

    #[test]
    fn constants_have_expected_values() {
        assert_eq!(WEAPON_BASE_X, 160);
        assert_eq!(WEAPON_BASE_Y, 167);
        assert_eq!(WEAPON_BOTTOM, 128);
        assert_eq!(WEAPON_TOP, 0);
        assert_eq!(RAISE_SPEED, 6);
        assert_eq!(MAX_BOB, 0x10_0000);
    }

    // ------------------------------------------------------------------
    // Edge cases
    // ------------------------------------------------------------------

    #[test]
    fn weapon_bob_multiple_ticks_phase_wraps() {
        let mut bob = WeaponBob::default();
        // Tick many times — phase wraps around u32 without panic.
        for _ in 0..10_000 {
            bob.tick(0x4_0000);
        }
        // No panic, phase is some valid u32.
        let _ = bob.phase;
    }

    #[test]
    fn weapon_anim_state_default_equals_new() {
        let d = WeaponAnimState::default();
        let n = WeaponAnimState::new();
        assert_eq!(d.raise_offset, n.raise_offset);
        assert_eq!(d.raise_target, n.raise_target);
        assert_eq!(d.raise_speed, n.raise_speed);
        assert_eq!(d.current.sprite_name, n.current.sprite_name);
    }

    #[test]
    fn bam_to_radians_zero_is_zero() {
        let r = bam_to_radians(0);
        assert!((r - 0.0).abs() < 1e-6);
    }

    #[test]
    fn bam_to_radians_half_circle() {
        let r = bam_to_radians(0x8000_0000);
        assert!(
            (r - std::f32::consts::PI).abs() < 0.01,
            "half of u32 range should be ~PI, got {}",
            r
        );
    }
}
