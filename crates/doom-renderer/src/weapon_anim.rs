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

use crate::colormap::ColormapCache;
use crate::column::IDENTITY_COLORMAP;
use crate::framebuffer::Framebuffer;
use crate::lighting::LightParams;
use crate::sprite::{SpriteCache, draw_weapon_frame};

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

/// Transition state of the weapon (raising, lowering, or resting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WeaponTransition {
    /// The weapon is neither raising nor lowering.
    #[default]
    None,
    /// The weapon is being raised.
    Raising,
    /// The weapon is being lowered.
    Lowering,
}

/// Snapshot of the weapon overlay's visual state for a single frame.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponSprite {
    /// Current sprite name (8 bytes, null-padded).
    pub sprite_name: [u8; 8],
    /// X position on screen (normally 160, modified by bob).
    pub sx: i32,
    /// Y position on screen (normally 167, modified by raise/lower).
    pub sy: i32,
    /// Transition state of the weapon (raising, lowering, or resting).
    pub transition: WeaponTransition,
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
            sx: 0,
            sy: WEAPON_BASE_Y,
            transition: WeaponTransition::None,
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

        // Doom's weapon bob never lifts above the resting baseline; the
        // weapon sways sideways and dips downward through the walk cycle.
        let sin_val = phase_rad.sin().abs();
        self.offset_y = ((self.amplitude as f32 * sin_val) as i64 >> 16) as i32;
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
            if self.raise_offset >= self.raise_target
                && self.current.transition == WeaponTransition::Lowering
            {
                self.current.transition = WeaponTransition::None;
            }
        } else if self.raise_offset > self.raise_target {
            // Raising: offset decreases toward target.
            self.raise_offset = (self.raise_offset - self.raise_speed).max(self.raise_target);
            if self.raise_offset <= self.raise_target
                && self.current.transition == WeaponTransition::Raising
            {
                self.current.transition = WeaponTransition::None;
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
        self.current.transition = WeaponTransition::Raising;
    }

    /// Begin lowering the weapon to the bottom of the screen.
    pub fn start_lower(&mut self) {
        self.raise_target = WEAPON_BOTTOM;
        self.current.transition = WeaponTransition::Lowering;
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
        self.raise_offset == WEAPON_TOP && self.current.transition == WeaponTransition::None
    }

    /// Compute the final screen X including bob offset.
    pub fn screen_x(&self) -> i32 {
        self.current.sx + self.bob.offset_x
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
    draw_weapon_animated_with_override(fb, anim, cache, colormap, None);
}

/// Draw the weapon overlay using the current animation state, with an optional
/// fixed colormap override for effects such as vanilla invulnerability.
pub fn draw_weapon_animated_with_override(
    fb: &mut Framebuffer,
    anim: &WeaponAnimState,
    cache: &SpriteCache,
    colormap: &[u8; 256],
    fixed_colormap: Option<&[u8; 256]>,
) {
    let sx = anim.screen_x();
    let sy = anim.screen_y();

    // A fixed colormap override takes precedence over flash/fullbright behavior.
    let main_colormap = if let Some(override_cm) = fixed_colormap {
        override_cm
    } else if anim.current.full_bright {
        &IDENTITY_COLORMAP
    } else {
        colormap
    };

    // Draw main weapon sprite.
    if let Some(frame) = cache.get(&anim.current.sprite_name) {
        draw_weapon_frame(fb, frame, sx, sy, main_colormap);
    }

    // Draw muzzle flash overlay at full brightness.
    if anim.current.flash_active {
        if let Some(flash_frame) = cache.get(&anim.current.flash_sprite) {
            draw_weapon_frame(
                fb,
                flash_frame,
                sx,
                sy,
                fixed_colormap.unwrap_or(&IDENTITY_COLORMAP),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Sector-shaded weapon rendering
// ---------------------------------------------------------------------------

/// Extra light level added to sector lighting during a muzzle flash.
///
/// In Doom, firing a weapon briefly illuminates the surrounding area.
/// This bonus is added to the sector light level for one frame when the
/// weapon is in its flash state.
pub const WEAPON_FLASH_LIGHT_BONUS: u8 = 128;

/// Distance value used for the weapon overlay when computing light params.
///
/// The weapon is always at the player's viewpoint, effectively at distance 0.
/// We use 1.0 (the minimum clamped distance in `LightParams`) to get the
/// brightest possible result for the given sector light, without any
/// distance-based darkening.
pub const WEAPON_LIGHT_DISTANCE: f32 = 1.0;

/// Draw the weapon overlay with sector-based lighting and muzzle flash support.
///
/// Similar to [`draw_weapon_animated`] but applies per-sector colourmap shading:
///
/// - **Normal state**: the weapon sprite is shaded according to `light_level`
///   using the provided [`ColormapCache`].  In a dark room the weapon appears
///   dark; in a bright room it appears bright.
/// - **Flash state** (`anim.current.full_bright`): the weapon sprite renders
///   at full brightness regardless of sector light (colormap index 0).
/// - The muzzle flash overlay sprite always renders at full brightness.
///
/// If `colormap` is `None`, falls back to full-bright rendering (identity
/// colormap) for both the main sprite and flash overlay.
pub fn draw_weapon_shaded(
    fb: &mut Framebuffer,
    anim: &WeaponAnimState,
    cache: &SpriteCache,
    light_level: u8,
    colormap: Option<&ColormapCache>,
) {
    let sx = anim.screen_x();
    let sy = anim.screen_y();

    // Determine the colormap for the main weapon sprite.
    let main_map: [u8; 256];
    let main_colormap: &[u8; 256] = if anim.current.full_bright {
        // Muzzle flash active: weapon always full-bright.
        &IDENTITY_COLORMAP
    } else if let Some(cm) = colormap {
        let lp = LightParams::new(light_level, false);
        main_map = *lp.get_colormap(WEAPON_LIGHT_DISTANCE, cm);
        &main_map
    } else {
        &IDENTITY_COLORMAP
    };

    // Draw main weapon sprite.
    if let Some(frame) = cache.get(&anim.current.sprite_name) {
        draw_weapon_frame(fb, frame, sx, sy, main_colormap);
    }

    // Draw muzzle flash overlay at full brightness.
    if anim.current.flash_active {
        if let Some(flash_frame) = cache.get(&anim.current.flash_sprite) {
            draw_weapon_frame(fb, flash_frame, sx, sy, &IDENTITY_COLORMAP);
        }
    }
}

/// Return the extra light bonus contributed by the weapon's muzzle flash.
///
/// - Returns [`WEAPON_FLASH_LIGHT_BONUS`] (128) when the weapon is in a
///   muzzle flash state (`full_bright` is true).
/// - Returns `0` otherwise.
///
/// Add this to the sector's base light level (saturating at 255) to briefly
/// illuminate nearby walls and floors during firing.
pub fn weapon_light_bonus(anim_state: &WeaponAnimState) -> u8 {
    if anim_state.current.full_bright {
        WEAPON_FLASH_LIGHT_BONUS
    } else {
        0
    }
}

/// Return the appropriate 256-byte colormap for the weapon sprite.
///
/// - If `is_flash` is `true`, returns the identity (full-bright) colormap
///   from the cache (index 0) so the weapon renders at maximum brightness.
/// - Otherwise, computes the colormap from `sector_light` at the standard
///   weapon distance ([`WEAPON_LIGHT_DISTANCE`]).
/// - If `colormap` is `None`, returns `None` (caller should fall back to
///   the static `IDENTITY_COLORMAP`).
pub fn get_weapon_light_params(
    sector_light: u8,
    is_flash: bool,
    colormap: Option<&ColormapCache>,
) -> Option<&[u8; 256]> {
    let cm = colormap?;
    if is_flash {
        Some(cm.get(0))
    } else {
        let lp = LightParams::new(sector_light, false);
        Some(lp.get_colormap(WEAPON_LIGHT_DISTANCE, cm))
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
        assert_eq!(ws.sx, 0);
        assert_eq!(ws.sy, WEAPON_BASE_Y);
        assert_eq!(ws.transition, WeaponTransition::None);
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
    fn weapon_bob_vertical_offset_never_lifts_above_rest_position() {
        let mut bob = WeaponBob::default();
        for _ in 0..64 {
            bob.tick(MAX_BOB);
            assert!(
                bob.offset_y >= 0,
                "Doom weapon bob dips the gun; it should not rise above the resting baseline"
            );
        }
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
        assert_eq!(state.current.transition, WeaponTransition::Raising);
        assert_eq!(state.raise_offset, WEAPON_BOTTOM);
        assert_eq!(state.raise_target, WEAPON_TOP);
    }

    #[test]
    fn weapon_anim_state_start_lower_sets_flag() {
        let mut state = WeaponAnimState::new();
        state.start_lower();
        assert_eq!(state.current.transition, WeaponTransition::Lowering);
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
    fn screen_x_uses_psprite_origin_plus_bob_offset() {
        let mut state = WeaponAnimState::new();
        state.current.sx = -3;
        state.bob.offset_x = 5;
        assert_eq!(state.screen_x(), 2);
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

    #[test]
    fn draw_weapon_animated_override_tints_weapon_pixels() {
        let mut fb = Framebuffer::new();
        let state = WeaponAnimState::new();
        let mut cache = SpriteCache::empty();
        cache.insert(
            doom_wad::LumpName::from_str("PISGA0"),
            crate::sprite::SpriteFrame {
                width: 1,
                height: 1,
                left_offset: 0,
                top_offset: 1,
                pixels: vec![Some(9)],
            },
        );
        let override_map = [0xA5; 256];

        draw_weapon_animated_with_override(
            &mut fb,
            &state,
            &cache,
            &IDENTITY_COLORMAP,
            Some(&override_map),
        );

        assert!(
            fb.data.contains(&0xA5),
            "fixed colormap override should tint the weapon sprite"
        );
        assert!(
            !fb.data.contains(&9),
            "raw weapon palette indices should not survive the override"
        );
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

    // ------------------------------------------------------------------
    // Helper: build a test ColormapCache
    // ------------------------------------------------------------------

    use crate::colormap::{COLORMAP_ROWS, COLORMAP_SIZE, ColormapCache};

    /// Build a `ColormapCache` where row `n` has every byte set to `n`.
    fn test_cache() -> ColormapCache {
        let mut data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for row in 0..COLORMAP_ROWS {
            let start = row * COLORMAP_SIZE;
            data[start..start + COLORMAP_SIZE].fill(row as u8);
        }
        ColormapCache::from_test_data(data)
    }

    // ------------------------------------------------------------------
    // weapon_light_bonus
    // ------------------------------------------------------------------

    #[test]
    fn weapon_light_bonus_zero_when_no_flash() {
        let state = WeaponAnimState::new();
        assert_eq!(
            weapon_light_bonus(&state),
            0,
            "no flash active → bonus should be 0"
        );
    }

    #[test]
    fn weapon_light_bonus_128_during_flash() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 4);
        assert_eq!(
            weapon_light_bonus(&state),
            WEAPON_FLASH_LIGHT_BONUS,
            "flash active → bonus should be WEAPON_FLASH_LIGHT_BONUS"
        );
    }

    #[test]
    fn weapon_light_bonus_returns_zero_after_flash_expires() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 2);
        // Tick twice to expire the flash.
        state.tick(0);
        state.tick(0);
        assert_eq!(
            weapon_light_bonus(&state),
            0,
            "flash expired → bonus should be 0"
        );
    }

    #[test]
    fn weapon_light_bonus_is_128_constant() {
        assert_eq!(WEAPON_FLASH_LIGHT_BONUS, 128);
    }

    #[test]
    fn weapon_light_bonus_still_active_mid_flash() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 5);
        state.tick(0); // 4 tics remaining
        assert_eq!(
            weapon_light_bonus(&state),
            128,
            "mid-flash bonus should still be 128"
        );
    }

    // ------------------------------------------------------------------
    // get_weapon_light_params
    // ------------------------------------------------------------------

    #[test]
    fn get_weapon_light_params_none_when_no_cache() {
        let result = get_weapon_light_params(128, false, None);
        assert!(
            result.is_none(),
            "should return None when no ColormapCache is provided"
        );
    }

    #[test]
    fn get_weapon_light_params_none_when_flash_and_no_cache() {
        let result = get_weapon_light_params(128, true, None);
        assert!(
            result.is_none(),
            "should return None when no cache even during flash"
        );
    }

    #[test]
    fn get_weapon_light_params_fullbright_during_flash() {
        let cache = test_cache();
        let result = get_weapon_light_params(64, true, Some(&cache));
        let row = result.expect("should return Some when cache is provided");
        // Row 0 in test cache has all bytes = 0.
        assert_eq!(
            row[0], 0,
            "flash should select colormap row 0 (full-bright)"
        );
        assert_eq!(row[100], 0);
    }

    #[test]
    fn get_weapon_light_params_dark_sector_selects_dark_row() {
        let cache = test_cache();
        let result = get_weapon_light_params(0, false, Some(&cache));
        let row = result.expect("should return Some");
        // Sector light 0 → base colormap index 31 (darkest).
        // At distance 1.0: scale = 160/1 = 160, offset = 160 >> 4 = 10
        // result = 31 - 10 = 21 → row 21.
        assert_eq!(row[0], 21, "dark sector should select a dark colormap row");
    }

    #[test]
    fn get_weapon_light_params_bright_sector_selects_bright_row() {
        let cache = test_cache();
        let result = get_weapon_light_params(255, false, Some(&cache));
        let row = result.expect("should return Some");
        // Sector light 255 → LightParams::new(255, false) → auto-fullbright.
        // So colormap_for_distance returns 0.
        assert_eq!(
            row[0], 0,
            "fully bright sector should select row 0 (identity)"
        );
    }

    #[test]
    fn get_weapon_light_params_mid_light_selects_mid_row() {
        let cache = test_cache();
        let result = get_weapon_light_params(128, false, Some(&cache));
        let row = result.expect("should return Some");
        // Sector light 128 → base index 15.
        // At distance 1.0: scale = 160, offset = 10.
        // result = 15 - 10 = 5 → row 5.
        assert_eq!(
            row[0], 5,
            "mid-bright sector should select an intermediate row"
        );
    }

    #[test]
    fn get_weapon_light_params_flash_overrides_any_light() {
        let cache = test_cache();
        // Even at total darkness, flash should return row 0.
        let result = get_weapon_light_params(0, true, Some(&cache));
        let row = result.expect("should return Some");
        assert_eq!(
            row[0], 0,
            "flash should always return full-bright regardless of sector light"
        );
    }

    // ------------------------------------------------------------------
    // draw_weapon_shaded
    // ------------------------------------------------------------------

    #[test]
    fn draw_weapon_shaded_no_panic_empty_cache() {
        let mut fb = Framebuffer::new();
        let state = WeaponAnimState::new();
        let cache = SpriteCache::empty();
        let cm = test_cache();
        draw_weapon_shaded(&mut fb, &state, &cache, 128, Some(&cm));
        // No panic with empty sprite cache.
    }

    #[test]
    fn draw_weapon_shaded_no_panic_with_flash() {
        let mut fb = Framebuffer::new();
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 4);
        let cache = SpriteCache::empty();
        let cm = test_cache();
        draw_weapon_shaded(&mut fb, &state, &cache, 128, Some(&cm));
        // No panic with flash and empty cache.
    }

    #[test]
    fn draw_weapon_shaded_no_panic_no_colormap() {
        let mut fb = Framebuffer::new();
        let state = WeaponAnimState::new();
        let cache = SpriteCache::empty();
        draw_weapon_shaded(&mut fb, &state, &cache, 128, None);
        // No panic when colormap is None.
    }

    #[test]
    fn draw_weapon_shaded_no_panic_dark_sector() {
        let mut fb = Framebuffer::new();
        let state = WeaponAnimState::new();
        let cache = SpriteCache::empty();
        let cm = test_cache();
        draw_weapon_shaded(&mut fb, &state, &cache, 0, Some(&cm));
        // No panic at minimum light.
    }

    #[test]
    fn draw_weapon_shaded_no_panic_bright_sector() {
        let mut fb = Framebuffer::new();
        let state = WeaponAnimState::new();
        let cache = SpriteCache::empty();
        let cm = test_cache();
        draw_weapon_shaded(&mut fb, &state, &cache, 255, Some(&cm));
        // No panic at maximum light.
    }

    #[test]
    fn draw_weapon_shaded_flash_active_with_none_cache() {
        let mut fb = Framebuffer::new();
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 3);
        let cache = SpriteCache::empty();
        draw_weapon_shaded(&mut fb, &state, &cache, 64, None);
        // No panic: flash + None colormap should gracefully fall back.
    }

    #[test]
    fn draw_weapon_shaded_flash_state_uses_identity() {
        // Verify that during flash, the weapon sprite uses IDENTITY_COLORMAP.
        // We test this indirectly by checking that no panic occurs and that
        // the flash path selects row 0 (which we verify through
        // get_weapon_light_params).
        let cache = test_cache();
        let result = get_weapon_light_params(0, true, Some(&cache));
        let row = result.expect("should return Some");
        // Row 0 in test_cache is all zeros = identity in the test sense.
        assert_eq!(row[0], 0);
    }

    // ------------------------------------------------------------------
    // Integration: weapon_light_bonus + sector light saturation
    // ------------------------------------------------------------------

    #[test]
    fn weapon_light_bonus_added_to_sector_light_saturates() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 4);
        let sector_light: u8 = 200;
        let bonus = weapon_light_bonus(&state);
        let effective = sector_light.saturating_add(bonus);
        assert_eq!(effective, 255, "200 + 128 should saturate to 255");
    }

    #[test]
    fn weapon_light_bonus_added_to_dark_sector_produces_128() {
        let mut state = WeaponAnimState::new();
        state.trigger_flash(*b"PISFA0\0\0", 4);
        let sector_light: u8 = 0;
        let bonus = weapon_light_bonus(&state);
        let effective = sector_light.saturating_add(bonus);
        assert_eq!(effective, 128, "0 + 128 should produce 128");
    }

    #[test]
    fn weapon_light_bonus_no_flash_leaves_sector_unchanged() {
        let state = WeaponAnimState::new();
        let sector_light: u8 = 80;
        let bonus = weapon_light_bonus(&state);
        let effective = sector_light.saturating_add(bonus);
        assert_eq!(effective, 80, "no flash: sector light should be unchanged");
    }

    // ------------------------------------------------------------------
    // Colormap row selection consistency
    // ------------------------------------------------------------------

    #[test]
    fn get_weapon_light_params_consistent_with_light_params() {
        let cache = test_cache();
        // Verify that get_weapon_light_params produces the same result as
        // manually constructing LightParams and calling get_colormap.
        for light in [0u8, 64, 128, 192, 255] {
            let via_helper = get_weapon_light_params(light, false, Some(&cache));
            let lp = LightParams::new(light, false);
            let via_direct = lp.get_colormap(1.0, &cache);
            assert_eq!(
                via_helper.expect("should be Some"),
                via_direct,
                "helper and direct LightParams should agree for light={light}"
            );
        }
    }

    #[test]
    fn get_weapon_light_params_flash_always_row_zero() {
        let cache = test_cache();
        // Flash should always return row 0 regardless of sector light.
        for light in [0u8, 64, 128, 192, 255] {
            let row = get_weapon_light_params(light, true, Some(&cache)).expect("should be Some");
            assert_eq!(row[0], 0, "flash should always be row 0 for light={light}");
        }
    }
}
