//! Doom HUD status bar renderer.
//!
//! Renders the bottom 32 rows of the 320×200 framebuffer (rows 168–199).
//! Displays health, armor, ammo, weapon ownership, face placeholder, and keys.
//! No sprite/font data is required — numbers use a built-in 3×5 pixel font;
//! text labels are drawn as small filled rectangles (palette-colored blocks).

use crate::framebuffer::Framebuffer;
use doom_game::player::{
    AmmoType, KEY_BLUE_CARD, KEY_RED_CARD, KEY_YELLOW_CARD, PlayerState, WEAPON_AMMO,
};

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// First row of the status bar.
pub const STATUS_BAR_Y: usize = 168;
/// Height of the status bar in rows.
pub const STATUS_BAR_HEIGHT: usize = 32;

// ---------------------------------------------------------------------------
// Color palette indices
// ---------------------------------------------------------------------------

/// Dark gray — status bar background.
const COLOR_BG: u8 = 7;
/// Yellow-green — normal numbers (health, ammo, armor).
const COLOR_NUMBER: u8 = 80;
/// Red — low health / red key.
const COLOR_RED: u8 = 176;
/// Bright yellow — god-mode health / yellow key.
const COLOR_BRIGHT_YELLOW: u8 = 231;
/// Green — owned weapon slot indicator.
const COLOR_WEAPON_OWNED: u8 = 112;
/// Dark gray — unowned weapon slot indicator.
const COLOR_WEAPON_MISSING: u8 = 96;
/// Blue key color.
const COLOR_KEY_BLUE: u8 = 200;
/// Yellow key color.
const COLOR_KEY_YELLOW: u8 = 231;
/// Red key color.
const COLOR_KEY_RED: u8 = 176;
/// Face healthy (health > 75).
const COLOR_FACE_HEALTHY: u8 = 96;
/// Face hurt (25 < health ≤ 75).
const COLOR_FACE_HURT: u8 = 208;
/// Face critical (health ≤ 25).
const COLOR_FACE_CRIT: u8 = 176;
/// Face god mode.
const COLOR_FACE_GOD: u8 = 231;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Draw the Doom HUD status bar into the bottom 32 rows of `fb`.
///
/// # Parameters
/// - `fb`       — mutable framebuffer; pixels in rows 168–199 will be overwritten.
/// - `player`   — current player state (health, ammo, armor, weapons, keys).
/// - `god_mode` — when `true`, override health color to bright yellow and use god face.
pub fn draw_status_bar(fb: &mut Framebuffer, player: &PlayerState, god_mode: bool) {
    // --- Background fill ---
    fb.fill_rect(0, STATUS_BAR_Y, 320, STATUS_BAR_HEIGHT, COLOR_BG);

    // Vertical center for numbers within the bar.
    let base_y = STATUS_BAR_Y + 12;

    // --- AMMO (x=0..42) ---
    let ammo = current_weapon_ammo(player);
    let ammo_color = if ammo < 10 { COLOR_RED } else { COLOR_NUMBER };
    draw_number(fb, 4, base_y, ammo, ammo_color);

    // --- HEALTH (x=48..104) ---
    let health = player.health();
    let health_color = if god_mode {
        COLOR_BRIGHT_YELLOW
    } else if health < 25 {
        COLOR_RED
    } else {
        COLOR_NUMBER
    };
    draw_number(fb, 52, base_y, health, health_color);

    // --- ARMS (x=104..165) — weapon ownership indicators for slots 2–7 ---
    for slot in 2..=7usize {
        let col = 104 + (slot - 2) * 10;
        let row = STATUS_BAR_Y + 8;
        let owned = player.weapons.get(slot).copied().unwrap_or(false);
        let color = if owned {
            COLOR_WEAPON_OWNED
        } else {
            COLOR_WEAPON_MISSING
        };
        // 5×5 filled square per weapon slot.
        for dy in 0..5usize {
            for dx in 0..5usize {
                fb.set_pixel(col + dx, row + dy, color);
            }
        }
    }

    // --- FACE placeholder (x=165..221) — solid block colored by health ---
    let face_color = if god_mode {
        COLOR_FACE_GOD
    } else if health > 75 {
        COLOR_FACE_HEALTHY
    } else if health > 25 {
        COLOR_FACE_HURT
    } else {
        COLOR_FACE_CRIT
    };
    fb.fill_rect(165, STATUS_BAR_Y + 4, 56, 24, face_color);

    // --- ARMOR (x=221..267) ---
    let armor = player.armor();
    draw_number(fb, 225, base_y, armor, COLOR_NUMBER);

    // --- KEYS (x=268..319) ---
    draw_key_indicator(
        fb,
        270,
        STATUS_BAR_Y + 6,
        COLOR_KEY_BLUE,
        player.keys & KEY_BLUE_CARD != 0,
    );
    draw_key_indicator(
        fb,
        270,
        STATUS_BAR_Y + 14,
        COLOR_KEY_YELLOW,
        player.keys & KEY_YELLOW_CARD != 0,
    );
    draw_key_indicator(
        fb,
        270,
        STATUS_BAR_Y + 22,
        COLOR_KEY_RED,
        player.keys & KEY_RED_CARD != 0,
    );
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Draw a single decimal digit (0–9) at pixel `(x, y)` using a 3×5 bitmap font.
///
/// Silently ignores digits > 9.  All pixel writes are bounds-checked.
fn draw_digit(fb: &mut Framebuffer, x: usize, y: usize, digit: u8, color: u8) {
    // 3-wide, 5-tall bitmaps.  Bit 2 is the leftmost column, bit 0 the rightmost.
    const DIGITS: [[u8; 5]; 10] = [
        [0b111, 0b101, 0b101, 0b101, 0b111], // 0
        [0b010, 0b110, 0b010, 0b010, 0b111], // 1
        [0b111, 0b001, 0b111, 0b100, 0b111], // 2
        [0b111, 0b001, 0b111, 0b001, 0b111], // 3
        [0b101, 0b101, 0b111, 0b001, 0b001], // 4
        [0b111, 0b100, 0b111, 0b001, 0b111], // 5
        [0b111, 0b100, 0b111, 0b101, 0b111], // 6
        [0b111, 0b001, 0b001, 0b001, 0b001], // 7
        [0b111, 0b101, 0b111, 0b101, 0b111], // 8
        [0b111, 0b101, 0b111, 0b001, 0b111], // 9
    ];
    if digit > 9 {
        return;
    }
    let bits = &DIGITS[digit as usize];
    for (row, &mask) in bits.iter().enumerate() {
        for col in 0..3usize {
            if mask & (1 << (2 - col)) != 0 {
                let px = x + col;
                let py = y + row;
                // Framebuffer::set_pixel already bounds-checks, but we mirror
                // the check here for clarity and to avoid needless calls.
                if px < 320 && py < 200 {
                    fb.set_pixel(px, py, color);
                }
            }
        }
    }
}

/// Draw an integer (0–999, clamped) right-justified into a three-digit field
/// starting at pixel `(x, y)`.  Digits are 3px wide with 1px gaps (4px stride).
///
/// Leading zeros are suppressed: "42" draws at x+4 and x+8; "7" draws at x+8.
fn draw_number(fb: &mut Framebuffer, x: usize, y: usize, value: i32, color: u8) {
    let clamped = value.clamp(0, 999) as u32;
    let hundreds = (clamped / 100) as u8;
    let tens = ((clamped / 10) % 10) as u8;
    let ones = (clamped % 10) as u8;

    if hundreds > 0 {
        draw_digit(fb, x, y, hundreds, color);
    }
    if hundreds > 0 || tens > 0 {
        draw_digit(fb, x + 4, y, tens, color);
    }
    draw_digit(fb, x + 8, y, ones, color);
}

/// Draw a 6×6 key indicator square at `(x, y)`.
///
/// If `owned` is `true` the square is filled with `color`; otherwise it is
/// filled with the background color (invisible / not collected).
fn draw_key_indicator(fb: &mut Framebuffer, x: usize, y: usize, color: u8, owned: bool) {
    let c = if owned { color } else { COLOR_BG };
    for dy in 0..6usize {
        for dx in 0..6usize {
            fb.set_pixel(x + dx, y + dy, c);
        }
    }
}

/// Return the ammo count for the player's currently equipped weapon.
///
/// Melee weapons (Fist, Chainsaw) have `AmmoType::None` and return 0.
fn current_weapon_ammo(player: &PlayerState) -> i32 {
    let weapon_idx = player.weapon as usize;
    // WEAPON_AMMO is indexed by weapon number; guard against out-of-range.
    let ammo_type = WEAPON_AMMO
        .get(weapon_idx)
        .copied()
        .unwrap_or(AmmoType::None);
    match ammo_type {
        AmmoType::None => 0,
        _ => {
            // ammo_type as usize gives the pool index (Bullets=0, Shells=1, …).
            player.ammo(ammo_type as usize) as i32
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::player::PlayerState;

    // Helper: a default (pistol-start) player.
    fn default_player() -> PlayerState {
        PlayerState::default()
    }

    #[test]
    fn draw_status_bar_does_not_panic() {
        let mut fb = Framebuffer::new();
        let player = default_player();
        // Should complete without panicking for normal and god-mode states.
        draw_status_bar(&mut fb, &player, false);
        draw_status_bar(&mut fb, &player, true);
    }

    #[test]
    fn status_bar_fills_bottom_rows() {
        let mut fb = Framebuffer::new();
        let player = default_player();
        draw_status_bar(&mut fb, &player, false);
        // Row 168 must not be all zeros — background fill (color 7) was applied.
        let row_start = STATUS_BAR_Y * 320;
        let row = &fb.data[row_start..row_start + 320];
        assert!(
            row.iter().any(|&b| b != 0),
            "Status bar row 168 should not be all zeros after draw_status_bar"
        );
    }

    #[test]
    fn draw_digit_zero_sets_correct_pixels() {
        let mut fb = Framebuffer::new();
        // Draw digit '0' at (0,0) with color 1.
        draw_digit(&mut fb, 0, 0, 0, 1);

        // Digit '0' bitmap: top row = 0b111 → pixels (0,0),(1,0),(2,0) all set.
        assert_eq!(fb.get_pixel(0, 0), Some(1), "top-left corner of '0'");
        assert_eq!(fb.get_pixel(1, 0), Some(1), "top-middle of '0'");
        assert_eq!(fb.get_pixel(2, 0), Some(1), "top-right of '0'");

        // Middle row (row 2) of '0' = 0b101 → (0,2) and (2,2) set, (1,2) clear.
        assert_eq!(fb.get_pixel(0, 2), Some(1), "mid-left of '0'");
        assert_eq!(
            fb.get_pixel(1, 2),
            Some(0),
            "mid-center of '0' should be gap"
        );
        assert_eq!(fb.get_pixel(2, 2), Some(1), "mid-right of '0'");
    }

    #[test]
    fn draw_number_clamps_negative() {
        let mut fb = Framebuffer::new();
        // Negative values must not panic; they are clamped to 0 → draws "0".
        draw_number(&mut fb, 0, 0, -5, 1);
        // After drawing '0', the top row pixels must be set.
        assert_eq!(fb.get_pixel(8, 0), Some(1), "ones digit of clamped '0'");
    }

    #[test]
    fn draw_number_clamps_large() {
        let mut fb = Framebuffer::new();
        // Values > 999 must be clamped to 999 without panicking.
        draw_number(&mut fb, 0, 0, 99_999, 1);
        // 999 → hundreds=9, tens=9, ones=9; top row of each digit is 0b111.
        assert_eq!(fb.get_pixel(0, 0), Some(1), "hundreds digit top-left");
        assert_eq!(fb.get_pixel(4, 0), Some(1), "tens digit top-left");
        assert_eq!(fb.get_pixel(8, 0), Some(1), "ones digit top-left");
    }
}
