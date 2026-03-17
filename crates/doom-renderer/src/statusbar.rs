//! Doom HUD status bar renderer (ST_ module).
//!
//! Renders the bottom 32 rows of the 320x200 framebuffer (rows 168-199).
//! Displays health, armor, ammo, weapon ownership, face placeholder, keys,
//! ammo tally, and text labels using built-in hardcoded bitmap fonts.
//!
//! # Layout (left to right, all in the bottom 32 rows):
//!
//! | Region     | X range   | Content                              |
//! |------------|-----------|--------------------------------------|
//! | Ammo count | 2..43     | Current weapon ammo (large yellow)   |
//! | Health     | 48..103   | "HEALTH" label + percentage (red)    |
//! | Arms       | 104..143  | 3x3 weapon grid (1-7)               |
//! | Face       | 144..183  | Mugshot placeholder rectangle        |
//! | Armor      | 184..243  | "ARMOR" label + percentage (green)   |
//! | Keys       | 244..271  | 3 key card slots (blue/yellow/red)   |
//! | Ammo tally | 272..319  | 4 rows: ammo/maxammo per type        |

use crate::framebuffer::Framebuffer;
use crate::patch_cache::PatchCache;
use doom_game::face::{FaceState, face_patch_name};
use doom_game::player::{
    AmmoType, KEY_BLUE_CARD, KEY_BLUE_SKULL, KEY_RED_CARD, KEY_RED_SKULL, KEY_YELLOW_CARD,
    KEY_YELLOW_SKULL, PlayerState, WEAPON_AMMO,
};
use doom_wad::WadStack;

// ---------------------------------------------------------------------------
// Layout constants
// ---------------------------------------------------------------------------

/// First row of the status bar.
pub const STATUS_BAR_Y: usize = 168;
/// Height of the status bar in rows.
pub const STATUS_BAR_HEIGHT: usize = 32;
/// Framebuffer width.
const FB_W: usize = 320;

// ---------------------------------------------------------------------------
// Color palette indices
// ---------------------------------------------------------------------------

/// Black -- status bar background.
const COLOR_BG: u8 = 0;
/// Yellow -- ammo count, bright indicators.
const COLOR_YELLOW: u8 = 231;
/// Red -- health numbers, red key.
const COLOR_RED: u8 = 176;
/// Green -- armor numbers, green border, healthy face.
const COLOR_GREEN: u8 = 112;
/// Dark gray -- unowned weapon, dim indicators.
const COLOR_DIM: u8 = 96;
/// Blue key color.
const COLOR_KEY_BLUE: u8 = 200;
/// Yellow key color.
const COLOR_KEY_YELLOW: u8 = 231;
/// Red key color.
const COLOR_KEY_RED: u8 = 176;
/// Label text color (medium gray).
const COLOR_LABEL: u8 = 4;
/// Ammo tally number color (light gray).
const COLOR_TALLY: u8 = 80;
/// Face border / outline color.
const COLOR_FACE_BORDER: u8 = 96;
/// Face skin color (flesh tone).
const COLOR_FACE_SKIN: u8 = 80;
/// Face feature color (eyes, mouth).
const COLOR_FACE_FEATURE: u8 = 0;

// ---------------------------------------------------------------------------
// StatusBarData -- decoupled from doom-game types
// ---------------------------------------------------------------------------

/// Input data for the status bar renderer.
///
/// Provides a clean interface that does not depend on `doom-game` types directly,
/// allowing the renderer to be tested and used independently.
#[derive(Debug, Clone, Default)]
pub struct StatusBarData {
    /// Player health (0-200 typical, can be negative when dead).
    pub health: i32,
    /// Player armor points.
    pub armor: i32,
    /// Armor type: 0 = none, 1 = green security armor, 2 = blue combat armor.
    pub armor_type: u8,
    /// Ammo count for the currently ready weapon.
    pub ammo_current: u32,
    /// All ammo pool counts: [bullets, shells, cells, rockets].
    pub ammo: [u32; 4],
    /// Maximum ammo for each type.
    pub max_ammo: [u32; 4],
    /// Currently equipped weapon index (0-8).
    pub ready_weapon: usize,
    /// Owned weapons bitmask-style array (index 0-8).
    pub weapons: [bool; 9],
    /// Key bitmask (same as PlayerState::keys).
    pub keys: u8,
    /// Mugshot frame index (0-7, placeholder for future sprite-based faces).
    pub face_index: u8,
}

impl StatusBarData {
    /// Construct `StatusBarData` from a `PlayerState` reference.
    pub fn from_player(player: &PlayerState) -> Self {
        let weapon_idx = player.weapon as usize;
        let ammo_type = WEAPON_AMMO
            .get(weapon_idx)
            .copied()
            .unwrap_or(AmmoType::None);
        let ammo_current = match ammo_type {
            AmmoType::None => 0,
            _ => player.ammo(ammo_type as usize),
        };

        Self {
            health: player.health(),
            armor: player.armor(),
            armor_type: player.armor_type,
            ammo_current,
            ammo: [
                player.ammo(0),
                player.ammo(1),
                player.ammo(2),
                player.ammo(3),
            ],
            max_ammo: player.max_ammo,
            ready_weapon: weapon_idx,
            weapons: player.weapons,
            keys: player.keys,
            face_index: 0,
        }
    }
}

// ===========================================================================
// Digit bitmaps -- 7 pixels wide x 9 pixels tall
// ===========================================================================

/// 7x9 digit bitmaps. Each `[u8; 9]` is 9 rows; each byte uses bits 6..0
/// (bit 6 = leftmost column, bit 0 = rightmost column).
const DIGIT_BITMAPS: [[u8; 9]; 10] = [
    // 0
    [
        0b0111110, // .XXXXX.
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b0111110, // .XXXXX.
    ],
    // 1
    [
        0b0001100, // ...XX..
        0b0011100, // ..XXX..
        0b0101100, // .X.XX..
        0b0001100, // ...XX..
        0b0001100, // ...XX..
        0b0001100, // ...XX..
        0b0001100, // ...XX..
        0b0001100, // ...XX..
        0b0111111, // .XXXXXX
    ],
    // 2
    [
        0b0111110, // .XXXXX.
        0b1100011, // XX...XX
        0b0000011, // .....XX
        0b0000110, // ....XX.
        0b0001100, // ...XX..
        0b0011000, // ..XX...
        0b0110000, // .XX....
        0b1100000, // XX.....
        0b1111111, // XXXXXXX
    ],
    // 3
    [
        0b0111110, // .XXXXX.
        0b1100011, // XX...XX
        0b0000011, // .....XX
        0b0000011, // .....XX
        0b0011110, // ..XXXX.
        0b0000011, // .....XX
        0b0000011, // .....XX
        0b1100011, // XX...XX
        0b0111110, // .XXXXX.
    ],
    // 4
    [
        0b0000110, // ....XX.
        0b0001110, // ...XXX.
        0b0010110, // ..X.XX.
        0b0100110, // .X..XX.
        0b1000110, // X...XX.
        0b1111111, // XXXXXXX
        0b0000110, // ....XX.
        0b0000110, // ....XX.
        0b0000110, // ....XX.
    ],
    // 5
    [
        0b1111111, // XXXXXXX
        0b1100000, // XX.....
        0b1100000, // XX.....
        0b1111110, // XXXXXX.
        0b0000011, // .....XX
        0b0000011, // .....XX
        0b0000011, // .....XX
        0b1100011, // XX...XX
        0b0111110, // .XXXXX.
    ],
    // 6
    [
        0b0111110, // .XXXXX.
        0b1100011, // XX...XX
        0b1100000, // XX.....
        0b1100000, // XX.....
        0b1111110, // XXXXXX.
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b0111110, // .XXXXX.
    ],
    // 7
    [
        0b1111111, // XXXXXXX
        0b0000011, // .....XX
        0b0000110, // ....XX.
        0b0001100, // ...XX..
        0b0011000, // ..XX...
        0b0011000, // ..XX...
        0b0011000, // ..XX...
        0b0011000, // ..XX...
        0b0011000, // ..XX...
    ],
    // 8
    [
        0b0111110, // .XXXXX.
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b0111110, // .XXXXX.
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b0111110, // .XXXXX.
    ],
    // 9
    [
        0b0111110, // .XXXXX.
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b1100011, // XX...XX
        0b0111111, // .XXXXXX
        0b0000011, // .....XX
        0b0000011, // .....XX
        0b1100011, // XX...XX
        0b0111110, // .XXXXX.
    ],
];

/// Width of a large digit in pixels.
pub const DIGIT_W: i32 = 7;
/// Height of a large digit in pixels.
pub const DIGIT_H: i32 = 9;

// ===========================================================================
// Letter bitmaps -- 5 pixels wide x 7 pixels tall (uppercase A-Z)
// ===========================================================================

/// 5x7 letter bitmaps for uppercase A-Z. Each `[u8; 7]` is 7 rows; each byte
/// uses bits 4..0 (bit 4 = leftmost column, bit 0 = rightmost column).
const LETTER_BITMAPS: [[u8; 7]; 26] = [
    // A
    [
        0b01110, // .XXX.
        0b10001, // X...X
        0b10001, // X...X
        0b11111, // XXXXX
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
    ],
    // B
    [
        0b11110, // XXXX.
        0b10001, // X...X
        0b10001, // X...X
        0b11110, // XXXX.
        0b10001, // X...X
        0b10001, // X...X
        0b11110, // XXXX.
    ],
    // C
    [
        0b01110, // .XXX.
        0b10001, // X...X
        0b10000, // X....
        0b10000, // X....
        0b10000, // X....
        0b10001, // X...X
        0b01110, // .XXX.
    ],
    // D
    [
        0b11110, // XXXX.
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b11110, // XXXX.
    ],
    // E
    [
        0b11111, // XXXXX
        0b10000, // X....
        0b10000, // X....
        0b11110, // XXXX.
        0b10000, // X....
        0b10000, // X....
        0b11111, // XXXXX
    ],
    // F
    [
        0b11111, // XXXXX
        0b10000, // X....
        0b10000, // X....
        0b11110, // XXXX.
        0b10000, // X....
        0b10000, // X....
        0b10000, // X....
    ],
    // G
    [
        0b01110, // .XXX.
        0b10001, // X...X
        0b10000, // X....
        0b10111, // X.XXX
        0b10001, // X...X
        0b10001, // X...X
        0b01110, // .XXX.
    ],
    // H
    [
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b11111, // XXXXX
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
    ],
    // I
    [
        0b01110, // .XXX.
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
        0b01110, // .XXX.
    ],
    // J
    [
        0b00111, // ..XXX
        0b00010, // ...X.
        0b00010, // ...X.
        0b00010, // ...X.
        0b00010, // ...X.
        0b10010, // X..X.
        0b01100, // .XX..
    ],
    // K
    [
        0b10001, // X...X
        0b10010, // X..X.
        0b10100, // X.X..
        0b11000, // XX...
        0b10100, // X.X..
        0b10010, // X..X.
        0b10001, // X...X
    ],
    // L
    [
        0b10000, // X....
        0b10000, // X....
        0b10000, // X....
        0b10000, // X....
        0b10000, // X....
        0b10000, // X....
        0b11111, // XXXXX
    ],
    // M
    [
        0b10001, // X...X
        0b11011, // XX.XX
        0b10101, // X.X.X
        0b10101, // X.X.X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
    ],
    // N
    [
        0b10001, // X...X
        0b11001, // XX..X
        0b10101, // X.X.X
        0b10011, // X..XX
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
    ],
    // O
    [
        0b01110, // .XXX.
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b01110, // .XXX.
    ],
    // P
    [
        0b11110, // XXXX.
        0b10001, // X...X
        0b10001, // X...X
        0b11110, // XXXX.
        0b10000, // X....
        0b10000, // X....
        0b10000, // X....
    ],
    // Q
    [
        0b01110, // .XXX.
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10101, // X.X.X
        0b10010, // X..X.
        0b01101, // .XX.X
    ],
    // R
    [
        0b11110, // XXXX.
        0b10001, // X...X
        0b10001, // X...X
        0b11110, // XXXX.
        0b10100, // X.X..
        0b10010, // X..X.
        0b10001, // X...X
    ],
    // S
    [
        0b01110, // .XXX.
        0b10001, // X...X
        0b10000, // X....
        0b01110, // .XXX.
        0b00001, // ....X
        0b10001, // X...X
        0b01110, // .XXX.
    ],
    // T
    [
        0b11111, // XXXXX
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
    ],
    // U
    [
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b01110, // .XXX.
    ],
    // V
    [
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b01010, // .X.X.
        0b01010, // .X.X.
        0b00100, // ..X..
    ],
    // W
    [
        0b10001, // X...X
        0b10001, // X...X
        0b10001, // X...X
        0b10101, // X.X.X
        0b10101, // X.X.X
        0b11011, // XX.XX
        0b10001, // X...X
    ],
    // X
    [
        0b10001, // X...X
        0b10001, // X...X
        0b01010, // .X.X.
        0b00100, // ..X..
        0b01010, // .X.X.
        0b10001, // X...X
        0b10001, // X...X
    ],
    // Y
    [
        0b10001, // X...X
        0b10001, // X...X
        0b01010, // .X.X.
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
        0b00100, // ..X..
    ],
    // Z
    [
        0b11111, // XXXXX
        0b00001, // ....X
        0b00010, // ...X.
        0b00100, // ..X..
        0b01000, // .X...
        0b10000, // X....
        0b11111, // XXXXX
    ],
];

/// Width of a letter in pixels.
pub const LETTER_W: i32 = 5;
/// Height of a letter in pixels.
pub const LETTER_H: i32 = 7;

// ===========================================================================
// Small digit bitmaps for ammo tally -- 3 pixels wide x 5 pixels tall
// ===========================================================================

/// 3x5 digit bitmaps for the ammo tally (compact). Bits 2..0 per row.
const SMALL_DIGITS: [[u8; 5]; 10] = [
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

// ===========================================================================
// Pixel drawing primitives
// ===========================================================================

/// Set a single pixel, ignoring out-of-bounds coordinates.
#[inline]
fn put_pixel(fb: &mut Framebuffer, x: i32, y: i32, color: u8) {
    if x >= 0 && y >= 0 && (x as usize) < FB_W && (y as usize) < 200 {
        fb.set_pixel(x as usize, y as usize, color);
    }
}

// ===========================================================================
// Digit rendering (large 7x9 font)
// ===========================================================================

/// Draw a single large digit (0-9) at pixel position `(x, y)`.
///
/// Uses the 7x9 `DIGIT_BITMAPS` font. Silently ignores digits > 9.
pub fn draw_digit(fb: &mut Framebuffer, x: i32, y: i32, digit: u8, color: u8) {
    if digit > 9 {
        return;
    }
    let bits = &DIGIT_BITMAPS[digit as usize];
    for (row, &mask) in bits.iter().enumerate() {
        for col in 0..DIGIT_W {
            if mask & (1 << (6 - col)) != 0 {
                put_pixel(fb, x + col, y + row as i32, color);
            }
        }
    }
}

/// Draw an integer right-aligned in a field of `width` digits.
///
/// Uses the large 7x9 font. Negative values are displayed as 0.
/// Each digit occupies `DIGIT_W + 1` = 8 pixels horizontally (7 px digit + 1 px gap).
pub fn draw_number(fb: &mut Framebuffer, x: i32, y: i32, value: i32, width: usize, color: u8) {
    let clamped = if value < 0 { 0u32 } else { value as u32 };
    // Extract individual digits.
    let mut digits = [0u8; 10];
    let mut num = clamped;
    let mut count = 0usize;
    if num == 0 {
        digits[0] = 0;
        count = 1;
    } else {
        while num > 0 && count < 10 {
            digits[count] = (num % 10) as u8;
            num /= 10;
            count += 1;
        }
    }

    // Draw right-aligned within `width` slots.
    let stride = DIGIT_W + 1; // 8 px per digit position
    for (i, &digit) in digits.iter().enumerate().take(width) {
        let slot_x = x + ((width - 1 - i) as i32) * stride;
        if i < count {
            draw_digit(fb, slot_x, y, digit, color);
        }
        // Leading positions with no digit are left blank (background).
    }
}

// ===========================================================================
// Small digit rendering (3x5 font for ammo tally)
// ===========================================================================

/// Draw a single small digit (0-9) at pixel position `(x, y)` using the 3x5 font.
fn draw_small_digit(fb: &mut Framebuffer, x: i32, y: i32, digit: u8, color: u8) {
    if digit > 9 {
        return;
    }
    let bits = &SMALL_DIGITS[digit as usize];
    for (row, &mask) in bits.iter().enumerate() {
        for col in 0..3i32 {
            if mask & (1 << (2 - col)) != 0 {
                put_pixel(fb, x + col, y + row as i32, color);
            }
        }
    }
}

/// Draw a small right-aligned number with up to `width` digits (3x5 font).
fn draw_small_number(fb: &mut Framebuffer, x: i32, y: i32, value: u32, width: usize, color: u8) {
    let mut digits = [0u8; 10];
    let mut num = value;
    let mut count = 0usize;
    if num == 0 {
        digits[0] = 0;
        count = 1;
    } else {
        while num > 0 && count < 10 {
            digits[count] = (num % 10) as u8;
            num /= 10;
            count += 1;
        }
    }
    let stride = 4i32; // 3 px + 1 px gap
    for (i, &digit) in digits.iter().enumerate().take(width) {
        let slot_x = x + ((width - 1 - i) as i32) * stride;
        if i < count {
            draw_small_digit(fb, slot_x, y, digit, color);
        }
    }
}

// ===========================================================================
// Character / text rendering (5x7 letter font)
// ===========================================================================

/// Draw a single uppercase character at `(x, y)` using the 5x7 letter font.
///
/// Supports A-Z (case-insensitive) and digits 0-9. Other characters are
/// treated as a space (no pixels drawn). The `/` character draws a slash.
pub fn draw_char(fb: &mut Framebuffer, x: i32, y: i32, ch: u8, color: u8) {
    let upper = ch.to_ascii_uppercase();
    if upper.is_ascii_uppercase() {
        let idx = (upper - b'A') as usize;
        let bits = &LETTER_BITMAPS[idx];
        for (row, &mask) in bits.iter().enumerate() {
            for col in 0..LETTER_W {
                if mask & (1 << (4 - col)) != 0 {
                    put_pixel(fb, x + col, y + row as i32, color);
                }
            }
        }
    } else if upper.is_ascii_digit() {
        // Re-use small digit bitmaps for inline text digits.
        draw_small_digit(fb, x + 1, y + 1, upper - b'0', color);
    } else if upper == b'/' {
        // Simple slash glyph for "ammo/max" display.
        for i in 0..5i32 {
            put_pixel(fb, x + 4 - i, y + i + 1, color);
        }
    }
    // Space and other characters: no pixels drawn.
}

/// Draw a text string at `(x, y)` using the 5x7 letter font.
///
/// Each character occupies 6 pixels horizontally (5 px glyph + 1 px gap).
pub fn draw_text(fb: &mut Framebuffer, x: i32, y: i32, text: &[u8], color: u8) {
    let stride = LETTER_W + 1; // 6 px per character
    for (i, &ch) in text.iter().enumerate() {
        draw_char(fb, x + (i as i32) * stride, y, ch, color);
    }
}

// ===========================================================================
// Key card rendering
// ===========================================================================

/// Draw the three key card indicator slots.
///
/// Three slots stacked vertically (8 px each), showing blue/yellow/red.
/// Cards and skulls of the same color are merged (either unlocks the slot).
/// Filled rectangle if owned; empty bordered rectangle if not.
pub fn draw_keys(fb: &mut Framebuffer, x: i32, y: i32, keys: u8) {
    let slots: [(u8, u8, u8); 3] = [
        (KEY_BLUE_CARD, KEY_BLUE_SKULL, COLOR_KEY_BLUE),
        (KEY_YELLOW_CARD, KEY_YELLOW_SKULL, COLOR_KEY_YELLOW),
        (KEY_RED_CARD, KEY_RED_SKULL, COLOR_KEY_RED),
    ];

    for (i, &(card_bit, skull_bit, color)) in slots.iter().enumerate() {
        let ky = y + (i as i32) * 10;
        let owned = keys & (card_bit | skull_bit) != 0;
        if owned {
            // Filled rectangle.
            for dy in 0..8i32 {
                for dx in 0..8i32 {
                    put_pixel(fb, x + dx, ky + dy, color);
                }
            }
        } else {
            // Empty bordered rectangle (1-pixel border, hollow inside).
            for dx in 0..8i32 {
                put_pixel(fb, x + dx, ky, COLOR_DIM);
                put_pixel(fb, x + dx, ky + 7, COLOR_DIM);
            }
            for dy in 1..7i32 {
                put_pixel(fb, x, ky + dy, COLOR_DIM);
                put_pixel(fb, x + 7, ky + dy, COLOR_DIM);
            }
        }
    }
}

// ===========================================================================
// Face placeholder
// ===========================================================================

/// Draw a simple face placeholder based on health level.
///
/// - health > 60: happy face (smile)
/// - health > 20: neutral face (straight line mouth)
/// - health <= 20: sad face (frown)
///
/// The face is drawn inside a bordered rectangle.
pub fn draw_face(fb: &mut Framebuffer, x: i32, y: i32, face_index: u8, health: i32) {
    let _ = face_index; // Reserved for future sprite-based faces.
    let w = 38i32;
    let h = 28i32;

    // Border.
    let border_color = if health > 60 {
        COLOR_GREEN
    } else if health > 20 {
        COLOR_FACE_BORDER
    } else {
        COLOR_RED
    };

    // Top and bottom border.
    for dx in 0..w {
        put_pixel(fb, x + dx, y, border_color);
        put_pixel(fb, x + dx, y + h - 1, border_color);
    }
    // Left and right border.
    for dy in 1..h - 1 {
        put_pixel(fb, x, y + dy, border_color);
        put_pixel(fb, x + w - 1, y + dy, border_color);
    }
    // Fill interior with skin color.
    for dy in 1..h - 1 {
        for dx in 1..w - 1 {
            put_pixel(fb, x + dx, y + dy, COLOR_FACE_SKIN);
        }
    }

    // Eyes (2x2 blocks).
    let eye_y = y + 8;
    let left_eye_x = x + 10;
    let right_eye_x = x + 26;
    for dy in 0..2i32 {
        for dx in 0..2i32 {
            put_pixel(fb, left_eye_x + dx, eye_y + dy, COLOR_FACE_FEATURE);
            put_pixel(fb, right_eye_x + dx, eye_y + dy, COLOR_FACE_FEATURE);
        }
    }

    // Mouth -- depends on health.
    let mouth_y = y + 18;
    let mouth_cx = x + w / 2;
    if health > 60 {
        // Happy: upward curve (smile).
        put_pixel(fb, mouth_cx - 5, mouth_y, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 4, mouth_y + 1, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 3, mouth_y + 2, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 2, mouth_y + 2, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 1, mouth_y + 2, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx, mouth_y + 2, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 1, mouth_y + 2, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 2, mouth_y + 2, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 3, mouth_y + 1, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 4, mouth_y, COLOR_FACE_FEATURE);
    } else if health > 20 {
        // Neutral: straight line.
        for dx in -4..=4i32 {
            put_pixel(fb, mouth_cx + dx, mouth_y + 1, COLOR_FACE_FEATURE);
        }
    } else {
        // Sad: downward curve (frown).
        put_pixel(fb, mouth_cx - 5, mouth_y + 2, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 4, mouth_y + 1, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 3, mouth_y, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 2, mouth_y, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx - 1, mouth_y, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx, mouth_y, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 1, mouth_y, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 2, mouth_y, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 3, mouth_y + 1, COLOR_FACE_FEATURE);
        put_pixel(fb, mouth_cx + 4, mouth_y + 2, COLOR_FACE_FEATURE);
    }
}

// ===========================================================================
// Weapon grid (ARMS)
// ===========================================================================

/// Draw the ARMS weapon grid showing weapon slots 1-7.
///
/// 3 columns x 3 rows (slot 8 unused, slot 0 = fist skipped from grid).
/// The ready weapon is highlighted, owned weapons are yellow, unowned are dim.
pub fn draw_arms(fb: &mut Framebuffer, x: i32, y: i32, weapons: &[bool; 9], ready: usize) {
    // Label.
    draw_text(fb, x + 2, y, b"ARMS", COLOR_LABEL);

    let grid_y = y + 9;
    let cell_w = 12i32;
    let cell_h = 7i32;

    // Weapon slots 1-7 arranged in a 3x3 grid (bottom-right cell empty).
    for slot in 1..=7usize {
        let grid_idx = slot - 1; // 0-6
        let col = (grid_idx % 3) as i32;
        let row = (grid_idx / 3) as i32;
        let cx = x + col * cell_w;
        let cy = grid_y + row * cell_h;

        let owned = weapons.get(slot).copied().unwrap_or(false);
        let is_ready = slot == ready;

        let color = if is_ready {
            COLOR_RED // Currently selected weapon highlighted in red.
        } else if owned {
            COLOR_YELLOW
        } else {
            COLOR_DIM
        };

        // Draw the slot number using the small font.
        draw_small_digit(fb, cx + 4, cy + 1, slot as u8, color);

        // Draw a small border around the ready weapon slot.
        if is_ready {
            for dx in 0..cell_w {
                put_pixel(fb, cx + dx, cy, color);
                put_pixel(fb, cx + dx, cy + cell_h - 1, color);
            }
            for dy in 1..cell_h - 1 {
                put_pixel(fb, cx, cy + dy, color);
                put_pixel(fb, cx + cell_w - 1, cy + dy, color);
            }
        }
    }
}

// ===========================================================================
// Ammo tally
// ===========================================================================

/// Ammo type short labels.
const AMMO_LABELS: [&[u8]; 4] = [b"BULL", b"SHEL", b"CELL", b"ROCK"];

/// Draw the ammo tally showing current/max for all 4 ammo types.
fn draw_ammo_tally(fb: &mut Framebuffer, x: i32, y: i32, ammo: &[u32; 4], max_ammo: &[u32; 4]) {
    let row_h = 7i32;
    for i in 0..4usize {
        let ry = y + (i as i32) * row_h;
        // Label (4 chars).
        draw_text(fb, x, ry, AMMO_LABELS[i], COLOR_LABEL);
        // Current ammo (right-aligned, 3 digits, at x+26).
        draw_small_number(fb, x + 26, ry + 1, ammo[i], 3, COLOR_TALLY);
        // Slash.
        put_pixel(fb, x + 39, ry + 2, COLOR_TALLY);
        put_pixel(fb, x + 40, ry + 3, COLOR_TALLY);
        // Max ammo (right-aligned, 3 digits, at x+42).
        draw_small_number(fb, x + 42, ry + 1, max_ammo[i], 3, COLOR_TALLY);
    }
}

// ===========================================================================
// Main entry points
// ===========================================================================

/// Draw the Doom HUD status bar using a `StatusBarData` struct.
///
/// Fills the bottom 32 rows (y=168..199) of the framebuffer.
pub fn draw_status_bar_data(fb: &mut Framebuffer, data: &StatusBarData) {
    // --- Background fill ---
    for y in STATUS_BAR_Y..STATUS_BAR_Y + STATUS_BAR_HEIGHT {
        let start = y * FB_W;
        let end = start + FB_W;
        fb.data[start..end].fill(COLOR_BG);
    }

    let bar_y = STATUS_BAR_Y as i32;

    // --- Ammo count (x=2..43): current weapon ammo, large yellow ---
    let ammo_color = if data.ammo_current < 10 {
        COLOR_RED
    } else {
        COLOR_YELLOW
    };
    draw_number(fb, 2, bar_y + 12, data.ammo_current as i32, 3, ammo_color);

    // --- Health (x=48..103): "HEALTH" label + percentage ---
    draw_text(fb, 48, bar_y + 2, b"HEALTH", COLOR_LABEL);
    let health_color = COLOR_RED;
    draw_number(fb, 52, bar_y + 12, data.health, 3, health_color);
    // Percent sign: small text glyph.
    draw_char(fb, 52 + 3 * 8, bar_y + 14, b'%', COLOR_RED);

    // --- Arms (x=104..143): weapon grid ---
    draw_arms(fb, 104, bar_y + 1, &data.weapons, data.ready_weapon);

    // --- Face (x=144..183): placeholder ---
    draw_face(fb, 144, bar_y + 2, data.face_index, data.health);

    // --- Armor (x=184..243): "ARMOR" label + percentage ---
    draw_text(fb, 186, bar_y + 2, b"ARMOR", COLOR_LABEL);
    draw_number(fb, 190, bar_y + 12, data.armor, 3, COLOR_GREEN);
    draw_char(fb, 190 + 3 * 8, bar_y + 14, b'%', COLOR_GREEN);

    // --- Keys (x=244..271): 3 key card slots ---
    draw_keys(fb, 248, bar_y + 2, data.keys);

    // --- Ammo tally (x=272..319): 4 rows ---
    draw_ammo_tally(fb, 272, bar_y + 2, &data.ammo, &data.max_ammo);
}

/// Draw the Doom HUD status bar (backward-compatible API).
///
/// This is the original interface that accepts `PlayerState` directly.
/// Internally converts to `StatusBarData` and delegates to `draw_status_bar_data`.
///
/// # Parameters
/// - `fb`       -- mutable framebuffer; pixels in rows 168-199 will be overwritten.
/// - `player`   -- current player state (health, ammo, armor, weapons, keys).
/// - `god_mode` -- when `true`, override health color to bright yellow and use god face.
pub fn draw_status_bar(fb: &mut Framebuffer, player: &PlayerState, god_mode: bool) {
    let data = StatusBarData::from_player(player);
    draw_status_bar_data(fb, &data);
    if god_mode {
        apply_god_mode_overlay(fb, data.health);
    }
}

/// Apply the god mode visual overlay on top of an already-drawn status bar.
///
/// Overdraws three regions with bright yellow (`COLOR_YELLOW`) to signal invincibility:
/// 1. The health number (same position as the normal red health display).
/// 2. The health percent sign.
/// 3. The face border rectangle, giving the mugshot a gold halo.
fn apply_god_mode_overlay(fb: &mut Framebuffer, health: i32) {
    let bar_y = STATUS_BAR_Y as i32;

    // 1. Redraw health number in yellow.
    draw_number(fb, 52, bar_y + 12, health, 3, COLOR_YELLOW);

    // 2. Redraw health percent sign in yellow.
    draw_char(fb, 52 + 3 * 8, bar_y + 14, b'%', COLOR_YELLOW);

    // 3. Redraw the face border rectangle in yellow.
    //    draw_face uses x=144, y=bar_y+2, w=38, h=28 (matching draw_status_bar_data).
    let face_x = 144i32;
    let face_y = bar_y + 2;
    let face_w = 38i32;
    let face_h = 28i32;
    for dx in 0..face_w {
        put_pixel(fb, face_x + dx, face_y, COLOR_YELLOW);
        put_pixel(fb, face_x + dx, face_y + face_h - 1, COLOR_YELLOW);
    }
    for dy in 1..face_h - 1 {
        put_pixel(fb, face_x, face_y + dy, COLOR_YELLOW);
        put_pixel(fb, face_x + face_w - 1, face_y + dy, COLOR_YELLOW);
    }
}

// ===========================================================================
// WAD patch-based status bar (parity renderer)
// ===========================================================================

/// Draw a right-aligned number using STTNUM (tall red) digit patches.
///
/// `right_x` is the right edge of the number field (matches vanilla `ST_AMMOX`,
/// `ST_HEALTHX`, `ST_ARMORX`).  Digit width is read from the actual patch so
/// any WAD variant works correctly.
pub fn draw_stnum(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    right_x: i32,
    y: i32,
    value: i32,
    max_digits: usize,
) {
    // Resolve digit width from the "0" patch (all STTNUM digits share width).
    let digit_w = cache
        .get("STTNUM0", wad)
        .map(|p| p.width as i32)
        .unwrap_or(14);

    let v = value.max(0) as u32;
    let mut digits = [0u8; 6];
    let mut count = 0;
    let mut n = v;
    if n == 0 {
        digits[0] = 0;
        count = 1;
    } else {
        while n > 0 && count < 6 {
            digits[count] = (n % 10) as u8;
            n /= 10;
            count += 1;
        }
    }

    // Draw right-to-left: ones digit at right_x - digit_w, etc.
    let mut x = right_x;
    for (i, digit) in digits.iter().copied().enumerate().take(max_digits) {
        x -= digit_w;
        if i < count {
            let name = format!("STTNUM{digit}");
            if let Some(patch) = cache.get(&name, wad) {
                let p = patch.clone();
                fb.draw_patch_vanilla(x, y, &p);
            }
        }
    }
}

/// Draw a right-aligned number using STYSNUM (small yellow) digit patches.
///
/// Used for the ammo tally columns (current/max ammo per type).
pub fn draw_stysnum(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    right_x: i32,
    y: i32,
    value: u32,
    max_digits: usize,
) {
    let digit_w = cache
        .get("STYSNUM0", wad)
        .map(|p| p.width as i32)
        .unwrap_or(7);

    let mut digits = [0u8; 4];
    let mut count = 0;
    let mut n = value;
    if n == 0 {
        digits[0] = 0;
        count = 1;
    } else {
        while n > 0 && count < 4 {
            digits[count] = (n % 10) as u8;
            n /= 10;
            count += 1;
        }
    }

    let mut x = right_x;
    for (i, digit) in digits.iter().copied().enumerate().take(max_digits) {
        x -= digit_w;
        if i < count {
            let name = format!("STYSNUM{digit}");
            if let Some(patch) = cache.get(&name, wad) {
                let p = patch.clone();
                fb.draw_patch_vanilla(x, y, &p);
            }
        }
    }
}

/// Draw the status bar using actual WAD patches.
///
/// All positions are exact values from vanilla Doom's `st_stuff.h`:
///   ST_AMMOX=44   ST_AMMOY=171
///   ST_HEALTHX=90 ST_HEALTHY=171
///   ST_ARMORX=221 ST_ARMORY=171
///   ST_FACEX=143  ST_FACEY=168
///   ST_ARMSBGX=104 ST_ARMSBGY=168
///   ST_KEY0Y=171  ST_KEY1Y=181  ST_KEY2Y=191
///   ST_AMMO0-3Y = 173,179,185,191  (bullets,shells,rockets,cells)
pub fn draw_status_bar_wad(
    fb: &mut Framebuffer,
    cache: &mut PatchCache,
    wad: &WadStack,
    data: &StatusBarData,
    face: &FaceState,
) {
    // Absolute y coordinates (screen space, not relative to bar).
    const AMY: i32 = 171; // ammo / health / armor number y
    const BAR: i32 = 168; // bar top y

    // 1. Background: STBAR centered on the 320px framebuffer.
    //    Widescreen WADs (Unity/KEX) ship a 576px-wide STBAR; vanilla is 320px.
    //    Either way, center it so the content aligns with our vanilla-coordinate elements.
    if let Some(patch) = cache.get("STBAR", wad) {
        let p = patch.clone();
        let bar_x = (320 - p.width as i32) / 2;
        fb.draw_patch(bar_x, BAR, &p);
    }

    // 2. Ammo — right edge x=44, y=171, 3 digits. (ST_AMMOX=44)
    draw_stnum(fb, cache, wad, 44, AMY, data.ammo_current as i32, 3);

    // 3. Health — right edge x=90, y=171, 3 digits + percent. (ST_HEALTHX=90)
    draw_stnum(fb, cache, wad, 90, AMY, data.health, 3);
    if let Some(pct) = cache.get("STTPRCNT", wad) {
        let p = pct.clone();
        fb.draw_patch_vanilla(90, AMY, &p);
    }

    // 4. Arms box — background at (104,168), weapon numbers in 3×2 grid.
    //    ST_ARMSBGX=104, ST_ARMSX=111, ST_ARMSXSPACE=12, ST_ARMSYSPACE=10
    if let Some(arms) = cache.get("STARMS", wad) {
        let p = arms.clone();
        fb.draw_patch_vanilla(104, BAR, &p);
    }
    let arm_xs = [111i32, 123, 135, 111, 123, 135];
    let arm_ys = [172i32, 172, 172, 182, 182, 182];
    for slot in 0..6usize {
        let weapon_num = slot + 2; // weapons 2-7
        let owned = data.weapons.get(weapon_num).copied().unwrap_or(false);
        if owned {
            let name = format!("STGNUM{weapon_num}");
            if let Some(patch) = cache.get(&name, wad) {
                let p = patch.clone();
                fb.draw_patch_vanilla(arm_xs[slot], arm_ys[slot], &p);
            }
        }
    }

    // 5. Face mugshot — ST_FACEX=143, ST_FACEY=168.
    let face_name = face_patch_name(face.kind);
    if let Some(patch) = cache.get(&face_name, wad) {
        let p = patch.clone();
        fb.draw_patch_vanilla(143, BAR, &p);
    }

    // 6. Armor — right edge x=221, y=171, 3 digits + percent. (ST_ARMORX=221)
    draw_stnum(fb, cache, wad, 221, AMY, data.armor, 3);
    if let Some(pct) = cache.get("STTPRCNT", wad) {
        let p = pct.clone();
        fb.draw_patch_vanilla(221, AMY, &p);
    }

    // 7. Keys — x=239, y=171/181/191. (ST_KEY0-2Y = 171,181,191)
    let key_bits = [
        (KEY_BLUE_CARD, 0usize, 171i32),
        (KEY_YELLOW_CARD, 1, 181),
        (KEY_RED_CARD, 2, 191),
        (KEY_BLUE_SKULL, 3, 171),
        (KEY_YELLOW_SKULL, 4, 181),
        (KEY_RED_SKULL, 5, 191),
    ];
    let mut slot_used = [false; 3];
    for (bit, idx, ky) in &key_bits {
        if data.keys & bit != 0 {
            let slot = idx % 3;
            if !slot_used[slot] {
                let name = format!("STKEYS{idx}");
                if let Some(patch) = cache.get(&name, wad) {
                    let p = patch.clone();
                    fb.draw_patch_vanilla(239, *ky, &p);
                }
                slot_used[slot] = true;
            }
        }
    }

    // 8. Ammo tally — vanilla y order: bullets=173, shells=179, rockets=185, cells=191.
    //    ST_AMMO0-3Y / ST_MAXAMMO0-3Y, right edges x=288 / x=314.
    //    Ammo array indices: 0=bullets, 1=shells, 2=cells, 3=rockets.
    //    Display order (Doom source): bullets, shells, rockets, cells.
    let tally_display = [(0usize, 173i32), (1, 179), (3, 185), (2, 191)];
    for (ammo_idx, ty) in &tally_display {
        let cur = data.ammo.get(*ammo_idx).copied().unwrap_or(0);
        let max = data.max_ammo.get(*ammo_idx).copied().unwrap_or(0);
        draw_stysnum(fb, cache, wad, 288, *ty, cur, 3);
        draw_stysnum(fb, cache, wad, 314, *ty, max, 3);
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::player::PlayerState;

    // Helper: a default (pistol-start) player.
    fn default_player() -> PlayerState {
        PlayerState::default()
    }

    // --- Test 1: StatusBarData::default() has zeroed fields ---
    #[test]
    fn status_bar_data_default_zeroed() {
        let d = StatusBarData::default();
        assert_eq!(d.health, 0);
        assert_eq!(d.armor, 0);
        assert_eq!(d.armor_type, 0);
        assert_eq!(d.ammo_current, 0);
        assert_eq!(d.ammo, [0; 4]);
        assert_eq!(d.max_ammo, [0; 4]);
        assert_eq!(d.ready_weapon, 0);
        assert_eq!(d.weapons, [false; 9]);
        assert_eq!(d.keys, 0);
        assert_eq!(d.face_index, 0);
    }

    // --- Test 2: draw_digit renders non-zero pixels ---
    #[test]
    fn draw_digit_renders_nonzero_pixels() {
        let mut fb = Framebuffer::new();
        draw_digit(&mut fb, 10, 10, 0, 42);
        // At least some pixels in the 7x9 region should be non-zero.
        let mut found = false;
        for dy in 0..DIGIT_H {
            for dx in 0..DIGIT_W {
                if fb.get_pixel((10 + dx) as usize, (10 + dy) as usize) == Some(42) {
                    found = true;
                }
            }
        }
        assert!(found, "draw_digit should render at least one pixel");
    }

    // --- Test 3: draw_digit for each digit 0-9 renders pixels ---
    #[test]
    fn draw_digit_all_digits_render() {
        for d in 0..=9u8 {
            let mut fb = Framebuffer::new();
            draw_digit(&mut fb, 0, 0, d, 1);
            let mut count = 0;
            for dy in 0..DIGIT_H {
                for dx in 0..DIGIT_W {
                    if fb.get_pixel(dx as usize, dy as usize) == Some(1) {
                        count += 1;
                    }
                }
            }
            assert!(
                count > 0,
                "digit {} should render at least one pixel, got {}",
                d,
                count
            );
        }
    }

    // --- Test 4: draw_number right-aligns correctly ---
    #[test]
    fn draw_number_right_aligns() {
        let mut fb = Framebuffer::new();
        // Draw "7" in a 3-digit field. It should appear in the rightmost slot.
        draw_number(&mut fb, 0, 0, 7, 3, 1);
        // Rightmost digit starts at x = 0 + (3-1)*8 = 16.
        // The leftmost slot (x=0) should be empty (all zero).
        let mut left_has_pixels = false;
        for dy in 0..DIGIT_H {
            for dx in 0..DIGIT_W {
                if fb.get_pixel(dx as usize, dy as usize) == Some(1) {
                    left_has_pixels = true;
                }
            }
        }
        assert!(
            !left_has_pixels,
            "leftmost digit slot should be empty for single-digit number"
        );

        // Rightmost slot should have pixels.
        let right_x = 2 * 8; // slot index 2, stride 8
        let mut right_has_pixels = false;
        for dy in 0..DIGIT_H {
            for dx in 0..DIGIT_W {
                if fb.get_pixel((right_x + dx) as usize, dy as usize) == Some(1) {
                    right_has_pixels = true;
                }
            }
        }
        assert!(
            right_has_pixels,
            "rightmost digit slot should have pixels for '7'"
        );
    }

    // --- Test 5: draw_number negative value shows as 0 ---
    #[test]
    fn draw_number_negative_shows_zero() {
        let mut fb_neg = Framebuffer::new();
        let mut fb_zero = Framebuffer::new();
        draw_number(&mut fb_neg, 0, 0, -42, 3, 1);
        draw_number(&mut fb_zero, 0, 0, 0, 3, 1);
        // Both should produce identical output.
        for y in 0..DIGIT_H {
            for x in 0..30i32 {
                assert_eq!(
                    fb_neg.get_pixel(x as usize, y as usize),
                    fb_zero.get_pixel(x as usize, y as usize),
                    "negative value should render same as 0 at ({}, {})",
                    x,
                    y
                );
            }
        }
    }

    // --- Test 6: draw_number with value=100, width=3 renders 3 digits ---
    #[test]
    fn draw_number_100_renders_three_digits() {
        let mut fb = Framebuffer::new();
        draw_number(&mut fb, 0, 0, 100, 3, 1);
        // All three digit slots should have pixels.
        for slot in 0..3usize {
            let sx = (slot as i32) * 8;
            let mut has_pixels = false;
            for dy in 0..DIGIT_H {
                for dx in 0..DIGIT_W {
                    if fb.get_pixel((sx + dx) as usize, dy as usize) == Some(1) {
                        has_pixels = true;
                    }
                }
            }
            assert!(
                has_pixels,
                "digit slot {} should have pixels for value 100",
                slot
            );
        }
    }

    // --- Test 7: draw_text renders non-zero pixels for "HEALTH" ---
    #[test]
    fn draw_text_health_renders() {
        let mut fb = Framebuffer::new();
        draw_text(&mut fb, 10, 10, b"HEALTH", 42);
        // Check that at least some pixels were drawn in the text region.
        let mut count = 0;
        for dy in 0..LETTER_H {
            // "HEALTH" = 6 chars * 6 px stride = 36 px wide.
            for dx in 0..36i32 {
                if fb.get_pixel((10 + dx) as usize, (10 + dy) as usize) == Some(42) {
                    count += 1;
                }
            }
        }
        assert!(
            count > 0,
            "draw_text 'HEALTH' should render pixels, got {}",
            count
        );
    }

    // --- Test 8: draw_status_bar doesn't panic with default data ---
    #[test]
    fn draw_status_bar_does_not_panic_default() {
        let mut fb = Framebuffer::new();
        let data = StatusBarData::default();
        draw_status_bar_data(&mut fb, &data);
    }

    // --- Test 9: draw_status_bar fills bottom 32 rows (not all black) ---
    #[test]
    fn draw_status_bar_fills_bottom_rows() {
        let mut fb = Framebuffer::new();
        let data = StatusBarData {
            health: 100,
            armor: 50,
            ammo_current: 42,
            ammo: [200, 50, 300, 50],
            max_ammo: [200, 50, 300, 50],
            weapons: [true, true, true, false, false, false, false, false, false],
            ..StatusBarData::default()
        };
        draw_status_bar_data(&mut fb, &data);

        // The status bar region should have some non-zero pixels (from text, numbers, etc.)
        let mut nonzero = 0;
        for y in STATUS_BAR_Y..STATUS_BAR_Y + STATUS_BAR_HEIGHT {
            for x in 0..FB_W {
                if fb.data[y * FB_W + x] != 0 {
                    nonzero += 1;
                }
            }
        }
        assert!(
            nonzero > 0,
            "Status bar should have non-zero pixels after drawing, got {}",
            nonzero
        );
    }

    // --- Test 10: draw_keys with all keys shows colored pixels ---
    #[test]
    fn draw_keys_all_keys_shows_color() {
        let mut fb = Framebuffer::new();
        let all_keys = KEY_BLUE_CARD | KEY_YELLOW_CARD | KEY_RED_CARD;
        draw_keys(&mut fb, 10, 10, all_keys);
        // Check for blue key pixels.
        let mut found_blue = false;
        for dy in 0..8i32 {
            for dx in 0..8i32 {
                if fb.get_pixel((10 + dx) as usize, (10 + dy) as usize) == Some(COLOR_KEY_BLUE) {
                    found_blue = true;
                }
            }
        }
        assert!(found_blue, "blue key should have colored pixels");
        // Check for yellow key pixels (second slot, y offset 10).
        let mut found_yellow = false;
        for dy in 0..8i32 {
            for dx in 0..8i32 {
                if fb.get_pixel((10 + dx) as usize, (20 + dy) as usize) == Some(COLOR_KEY_YELLOW) {
                    found_yellow = true;
                }
            }
        }
        assert!(found_yellow, "yellow key should have colored pixels");
    }

    // --- Test 11: draw_keys with no keys is minimal (just borders) ---
    #[test]
    fn draw_keys_no_keys_minimal() {
        let mut fb = Framebuffer::new();
        draw_keys(&mut fb, 10, 10, 0);
        // With no keys, we should see only border pixels (COLOR_DIM=96), no bright key colors.
        let mut found_bright = false;
        for dy in 0..30i32 {
            for dx in 0..8i32 {
                let px = fb.get_pixel((10 + dx) as usize, (10 + dy) as usize);
                if px == Some(COLOR_KEY_BLUE) || px == Some(COLOR_KEY_RED) {
                    // Note: COLOR_KEY_YELLOW == COLOR_YELLOW == 231 would only appear if owned.
                    // COLOR_KEY_RED and COLOR_KEY_BLUE should not appear as fill.
                    found_bright = true;
                }
            }
        }
        assert!(
            !found_bright,
            "no keys owned should not show bright key colors"
        );
    }

    // --- Test 12: draw_face doesn't panic for each health range ---
    #[test]
    fn draw_face_health_ranges_no_panic() {
        let mut fb = Framebuffer::new();
        // Happy face (health > 60).
        draw_face(&mut fb, 10, 10, 0, 100);
        // Neutral face (20 < health <= 60).
        draw_face(&mut fb, 10, 10, 0, 40);
        // Sad face (health <= 20).
        draw_face(&mut fb, 10, 10, 0, 10);
        // Zero health.
        draw_face(&mut fb, 10, 10, 0, 0);
        // Negative health.
        draw_face(&mut fb, 10, 10, 0, -10);
    }

    // --- Test 13: draw_arms shows ready weapon differently ---
    #[test]
    fn draw_arms_ready_weapon_differs() {
        let weapons = [true, true, true, true, false, false, false, false, false];
        let mut fb_ready2 = Framebuffer::new();
        let mut fb_ready3 = Framebuffer::new();
        draw_arms(&mut fb_ready2, 10, 10, &weapons, 2);
        draw_arms(&mut fb_ready3, 10, 10, &weapons, 3);
        // The two framebuffers should differ because different weapons are highlighted.
        let mut differ = false;
        for y in 10..42usize {
            for x in 10..50usize {
                if fb_ready2.get_pixel(x, y) != fb_ready3.get_pixel(x, y) {
                    differ = true;
                }
            }
        }
        assert!(
            differ,
            "draw_arms with different ready weapons should produce different output"
        );
    }

    // --- Test 14: draw_status_bar preserves top 168 rows ---
    #[test]
    fn draw_status_bar_preserves_top_rows() {
        let mut fb = Framebuffer::new();
        // Fill the top area with a sentinel value.
        for y in 0..STATUS_BAR_Y {
            for x in 0..FB_W {
                fb.data[y * FB_W + x] = 42;
            }
        }
        let data = StatusBarData::default();
        draw_status_bar_data(&mut fb, &data);
        // Verify top 168 rows are untouched.
        for y in 0..STATUS_BAR_Y {
            for x in 0..FB_W {
                assert_eq!(
                    fb.data[y * FB_W + x],
                    42,
                    "pixel ({}, {}) in game area was overwritten",
                    x,
                    y
                );
            }
        }
    }

    // --- Bonus: backward-compat draw_status_bar with PlayerState ---
    #[test]
    fn draw_status_bar_player_compat_no_panic() {
        let mut fb = Framebuffer::new();
        let player = default_player();
        draw_status_bar(&mut fb, &player, false);
        draw_status_bar(&mut fb, &player, true);
    }

    // --- Bonus: StatusBarData::from_player round-trips correctly ---
    #[test]
    fn status_bar_data_from_player() {
        let player = default_player();
        let data = StatusBarData::from_player(&player);
        assert_eq!(data.health, player.health());
        assert_eq!(data.armor, player.armor());
        assert_eq!(data.keys, player.keys);
        assert_eq!(data.weapons, player.weapons);
    }

    // --- Bonus: draw_char renders a letter ---
    #[test]
    fn draw_char_renders_a() {
        let mut fb = Framebuffer::new();
        draw_char(&mut fb, 0, 0, b'A', 1);
        let mut count = 0;
        for dy in 0..LETTER_H {
            for dx in 0..LETTER_W {
                if fb.get_pixel(dx as usize, dy as usize) == Some(1) {
                    count += 1;
                }
            }
        }
        assert!(count > 0, "draw_char 'A' should render pixels");
    }

    // --- Bonus: draw_number with large value clamps display ---
    #[test]
    fn draw_number_large_value_no_panic() {
        let mut fb = Framebuffer::new();
        // Should not panic even with a very large value.
        draw_number(&mut fb, 0, 0, 999_999, 3, 1);
        // With width=3 it still renders (just the lower 3 digits won't fit the
        // full number, but it should not panic).
    }

    // --- Test 15: god_mode=true causes yellow pixels in health number region ---
    #[test]
    fn god_mode_true_health_is_yellow() {
        let mut fb_god = Framebuffer::new();
        let mut fb_normal = Framebuffer::new();

        let data = StatusBarData {
            health: 100,
            ..StatusBarData::default()
        };

        // Draw normal bar, then apply the overlay manually for god mode.
        draw_status_bar_data(&mut fb_normal, &data);
        draw_status_bar_data(&mut fb_god, &data);
        apply_god_mode_overlay(&mut fb_god, data.health);

        // In the god framebuffer there must be at least one COLOR_YELLOW pixel
        // in the health number region (x=52..76, y=bar_y+12..bar_y+21).
        let bar_y = STATUS_BAR_Y;
        let mut found_yellow_god = false;
        let mut found_red_god = false;
        for dy in 0..DIGIT_H as usize {
            for dx in 0..(3 * 8usize) {
                let px = fb_god.get_pixel(52 + dx, bar_y + 12 + dy);
                if px == Some(COLOR_YELLOW) {
                    found_yellow_god = true;
                }
                if px == Some(COLOR_RED) {
                    found_red_god = true;
                }
            }
        }
        assert!(
            found_yellow_god,
            "god mode health region must contain at least one COLOR_YELLOW pixel"
        );
        // The overlay overwrites the red health pixels; there should be no red
        // pixels remaining in the health number area when health=100.
        assert!(
            !found_red_god,
            "god mode health region must not contain COLOR_RED pixels after overlay"
        );

        // Normal bar should have red there, not yellow.
        let mut found_red_normal = false;
        for dy in 0..DIGIT_H as usize {
            for dx in 0..(3 * 8usize) {
                if fb_normal.get_pixel(52 + dx, bar_y + 12 + dy) == Some(COLOR_RED) {
                    found_red_normal = true;
                }
            }
        }
        assert!(
            found_red_normal,
            "normal bar health region must contain COLOR_RED pixels"
        );
    }

    // --- Test 16: god_mode=false leaves health as normal red ---
    #[test]
    fn god_mode_false_health_stays_red() {
        let mut fb = Framebuffer::new();
        let player = default_player();
        // draw_status_bar with god_mode=false.
        draw_status_bar(&mut fb, &player, false);

        // Health region should not contain COLOR_YELLOW in the number area
        // (player default health is 0, so the "0" digit renders; check both color
        // presence: yellow must be absent, background/red must appear as drawn).
        let bar_y = STATUS_BAR_Y;
        let mut found_yellow = false;
        for dy in 0..DIGIT_H as usize {
            for dx in 0..(3 * 8usize) {
                if fb.get_pixel(52 + dx, bar_y + 12 + dy) == Some(COLOR_YELLOW) {
                    found_yellow = true;
                }
            }
        }
        assert!(
            !found_yellow,
            "non-god mode must not show COLOR_YELLOW in the health number area"
        );
    }

    // --- Test 17: god_mode=true causes yellow face border ---
    #[test]
    fn god_mode_true_face_border_is_yellow() {
        let mut fb = Framebuffer::new();
        let data = StatusBarData {
            health: 100,
            ..StatusBarData::default()
        };
        draw_status_bar_data(&mut fb, &data);
        apply_god_mode_overlay(&mut fb, data.health);

        // The top border of the face rectangle (y=bar_y+2, x=144..181) must be yellow.
        let bar_y = STATUS_BAR_Y as i32;
        let face_x = 144i32;
        let face_y = bar_y + 2;
        let face_w = 38i32;

        let mut found_yellow = false;
        for dx in 0..face_w {
            if fb.get_pixel((face_x + dx) as usize, face_y as usize) == Some(COLOR_YELLOW) {
                found_yellow = true;
            }
        }
        assert!(
            found_yellow,
            "god mode face top border must contain COLOR_YELLOW pixels"
        );
    }
}
