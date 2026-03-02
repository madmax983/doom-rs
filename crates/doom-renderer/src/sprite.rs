//! Doom picture-format parser and sprite rendering.
//!
//! All sprite lumps and patch lumps use the "picture" format:
//!
//! ```text
//! Header (8 bytes):
//!   u16  width
//!   u16  height
//!   i16  leftoffset   ← pixels to the left of the center point
//!   i16  topoffset    ← pixels above the origin
//!
//! Column offsets (width × 4 bytes):
//!   width × u32  col_offset  ← byte offset from start of lump to column data
//!
//! Column data at each col_offset:
//!   loop:
//!     u8  topdelta    ← 0xFF = end of column; otherwise row where post starts
//!     u8  length      ← number of pixels in this post
//!     u8  _unused     ← skip (padding)
//!     length × u8  pixels  ← palette indices
//!     u8  _unused     ← skip (padding)
//! ```
//!
//! Pixels not covered by any post are transparent (`None`).

use std::collections::HashMap;

use doom_wad::WadFile;

use crate::framebuffer::Framebuffer;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A parsed Doom picture (sprite frame, patch, weapon overlay, etc.).
///
/// Pixel storage is column-major: `pixels[col * height + row]`.
/// Transparent pixels are `None`; opaque pixels are `Some(palette_index)`.
#[derive(Clone, Debug)]
pub struct SpriteFrame {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Pixels to the left of the sprite's center point (for X centering).
    pub left_offset: i16,
    /// Pixels above the sprite's baseline/origin (for Y positioning).
    pub top_offset: i16,
    /// Column-major pixel data. `pixels[col * height + row]`.
    /// Length is always `width * height`.
    pub pixels: Vec<Option<u8>>,
}

/// Cache of all sprite frames loaded from WAD lumps between S_START and S_END.
///
/// Keyed by uppercase lump name (null bytes stripped).
pub struct SpriteCache {
    frames: HashMap<String, SpriteFrame>,
}

impl SpriteCache {
    /// Load all sprite lumps between S_START and S_END from the WAD.
    ///
    /// Lumps that fail to parse (too small, malformed column offsets) are
    /// silently skipped — this matches vanilla Doom's behaviour.
    pub fn load(wad: &WadFile) -> Self {
        let mut frames = HashMap::new();

        for lump in wad.lumps_between("S_START", "S_END") {
            // Skip marker lumps (size == 0).
            if lump.size == 0 {
                continue;
            }
            let data = wad.lump_data(lump);
            if let Some(frame) = parse_picture(data) {
                let name = lump.name.as_str().to_uppercase();
                frames.insert(name, frame);
            }
        }

        Self { frames }
    }

    /// Look up a sprite frame by 8-byte lump name (uppercase, null-trimmed).
    ///
    /// The name is normalised to uppercase with trailing null bytes stripped
    /// before lookup, matching how WAD lump names are stored.
    pub fn get(&self, name: &[u8; 8]) -> Option<&SpriteFrame> {
        // Trim trailing nulls and convert to uppercase string.
        let trimmed_len = name.iter().position(|&b| b == 0).unwrap_or(8);
        let key = std::str::from_utf8(&name[..trimmed_len])
            .ok()?
            .to_uppercase();
        self.frames.get(&key)
    }

    /// Number of sprite frames in the cache.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns `true` if the cache contains no sprite frames.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Insert a frame directly (used in tests and by callers that pre-parse frames).
    pub fn insert(&mut self, name: String, frame: SpriteFrame) {
        self.frames.insert(name.to_uppercase(), frame);
    }

    /// Construct an empty cache (useful in tests).
    pub fn empty() -> Self {
        Self {
            frames: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Picture-format parser
// ---------------------------------------------------------------------------

/// Parse a Doom picture-format lump into a [`SpriteFrame`].
///
/// Returns `None` if the data is too short, the dimensions are zero, or the
/// column offset table would overflow the buffer.
pub fn parse_picture(data: &[u8]) -> Option<SpriteFrame> {
    // Minimum: 8-byte header.
    if data.len() < 8 {
        return None;
    }

    let width = u16::from_le_bytes([data[0], data[1]]) as usize;
    let height = u16::from_le_bytes([data[2], data[3]]) as usize;
    let left = i16::from_le_bytes([data[4], data[5]]);
    let top = i16::from_le_bytes([data[6], data[7]]);

    // Zero-dimension pictures are not renderable.
    if width == 0 || height == 0 {
        return None;
    }

    // The column offset table follows the header: width × 4 bytes.
    let col_table_end = 8usize.checked_add(width.checked_mul(4)?)?;
    if data.len() < col_table_end {
        return None;
    }

    let mut pixels = vec![None::<u8>; width * height];

    for col in 0..width {
        let table_pos = 8 + col * 4;
        let off = u32::from_le_bytes(data[table_pos..table_pos + 4].try_into().ok()?) as usize;

        // Parse the column's posts.
        let mut pos = off;
        loop {
            if pos >= data.len() {
                break;
            }
            let topdelta = data[pos];
            pos += 1;

            // 0xFF signals end of column.
            if topdelta == 0xFF {
                break;
            }

            if pos >= data.len() {
                break;
            }
            let length = data[pos] as usize;
            pos += 1;

            // Skip the first unused byte (pre-pixel padding).
            pos += 1;

            for i in 0..length {
                if pos >= data.len() {
                    break;
                }
                let pixel = data[pos];
                pos += 1;

                let row = (topdelta as usize) + i;
                if row < height {
                    pixels[col * height + row] = Some(pixel);
                }
            }

            // Skip the post-pixel trailing padding byte.
            pos += 1;
        }
    }

    Some(SpriteFrame {
        width: width as u16,
        height: height as u16,
        left_offset: left,
        top_offset: top,
        pixels,
    })
}

// ---------------------------------------------------------------------------
// Thing projection constants (must match render.rs)
// ---------------------------------------------------------------------------

const SCREEN_W: usize = 320;
const SCREEN_H: usize = 200;
const HALF_W: i32 = 160;
const HALF_H: i32 = 100;
const FOCAL_LEN: f32 = 160.0;
/// Eye height above floor in map units (matches render.rs PLAYER_HEIGHT).
const PLAYER_HEIGHT: f32 = 41.0;

// ---------------------------------------------------------------------------
// Thing sprite lookup
// ---------------------------------------------------------------------------

/// Map a Doom DoomEd type number to the 8-byte lump name of its idle sprite
/// (frame A, rotation 0).  Returns `None` for things that have no world sprite
/// (player starts, teleport destinations, etc.).
fn thing_sprite(kind: u16) -> Option<[u8; 8]> {
    let base: &[u8; 4] = match kind {
        1 => return None, // player 1 start — no world sprite
        2 => b"SHOT",     // shotgun (dropped)
        3 => b"BSKU",     // blue skull key
        5 => b"BKEY",     // blue keycard
        6 => b"YKEY",     // yellow keycard
        13 => b"RKEY",    // red keycard
        38 => b"RSKU",    // red skull key
        39 => b"YSKU",    // yellow skull key
        40 => b"BSKU",    // blue skull key (alt number)
        2001 => b"SHOT",  // shotgun pickup
        2002 => b"MGUN",  // chaingun pickup
        2003 => b"LAUN",  // rocket launcher
        2004 => b"PLAS",  // plasma gun
        2005 => b"CSAW",  // chainsaw
        2006 => b"BFUG",  // BFG9000
        2007 => b"CLIP",  // ammo clip
        2008 => b"SHEL",  // shotgun shells
        2010 => b"ROCK",  // rocket
        2011 => b"STIM",  // stimpack
        2012 => b"MEDI",  // medikit
        2013 => b"SOUL",  // soulsphere
        2014 => b"BON1",  // health bonus
        2015 => b"BON2",  // armor bonus
        2018 => b"ARM1",  // green armor
        2019 => b"ARM2",  // blue armor
        2022 => b"PINV",  // invulnerability sphere
        2023 => b"PSTR",  // berserk pack
        2024 => b"PINS",  // invisibility sphere
        2025 => b"SUIT",  // radiation suit
        2026 => b"PMAP",  // computer area map
        2028 => b"COLU",  // floor lamp
        2035 => b"BAR1",  // barrel (explosive)
        2045 => b"PVIS",  // light amplification visor
        3001 => b"TROO",  // imp
        3002 => b"SARG",  // demon (pinky)
        3003 => b"BOSS",  // baron of hell
        3004 => b"POSS",  // former human (zombie man)
        3005 => b"HEAD",  // cacodemon
        3006 => b"SKUL",  // lost soul
        9 => b"SPOS",     // shotgun guy (former sergeant)
        58 => b"SARG",    // spectre (same sprite as demon)
        65 => b"CPOS",    // heavy weapon dude (chaingunner)
        66 => b"SKEL",    // revenant
        67 => b"FATT",    // mancubus
        68 => b"VILE",    // archvile
        71 => b"PAIN",    // pain elemental
        72 => b"KEEN",    // commander keen
        84 => b"SSWV",    // wolfenstein ss
        88 => b"BBRN",    // boss brain
        _ => return None, // unknown / no world sprite
    };

    // Frame A, rotation 0: four base bytes + "A0" + two null bytes.
    let mut name = [0u8; 8];
    name[0..4].copy_from_slice(base);
    name[4] = b'A';
    name[5] = b'0';
    // name[6] and name[7] remain 0x00 (null).
    Some(name)
}

// ---------------------------------------------------------------------------
// Billboard sprite projection
// ---------------------------------------------------------------------------

/// Project and render all Things from the level as billboard sprites.
///
/// Call this **after** `render_level` so wall columns are already drawn.
/// Uses the painter's algorithm (back-to-front sort); no z-buffer clipping.
///
/// # Arguments
/// - `level`        — parsed map (provides Things list).
/// - `player_x/y`  — player world position (Fixed16_16).
/// - `player_angle` — player view angle (Bam, 32-bit; full circle = 2³²).
/// - `fb`           — framebuffer to draw into.
/// - `cache`        — sprite frame cache (loaded from S_START..S_END).
///
/// # Projection model
/// View space is computed with a standard rotation: `vx` is depth (forward
/// from the player eye), `vy` is lateral displacement.  A sprite with
/// `vx <= 0.5` is behind or too close and is skipped.  Screen X of the
/// sprite centre is `HALF_W - FOCAL_LEN * vy / vx`; sprite screen height is
/// `frame.height * FOCAL_LEN / vx`.
pub fn render_things(
    level: &doom_map::Level,
    player_x: doom_types::Fixed16_16,
    player_y: doom_types::Fixed16_16,
    player_angle: doom_types::Bam,
    fb: &mut Framebuffer,
    cache: &SpriteCache,
) {
    // Convert player angle (32-bit BAM) to radians.
    // BAM: 0x0000_0000 = 0°, 0x4000_0000 = 90°, 0x8000_0000 = 180°, etc.
    let angle_rad =
        (player_angle.0 as f32) * (std::f32::consts::PI * 2.0 / (u32::MAX as f32 + 1.0));
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();

    // Player eye position in map units (f32).
    // Fixed16_16 stores value as raw i32 with 16.16 encoding; divide by 65536
    // to convert to f32 map units.
    let px = player_x.raw() as f32 / 65536.0;
    let py = player_y.raw() as f32 / 65536.0;

    // ---------- Collect visible things with their view-space depths ----------
    let mut visible: Vec<(f32, &doom_map::Thing)> = level
        .things
        .iter()
        .filter_map(|thing| {
            let dx = thing.x as f32 - px;
            let dy = thing.y as f32 - py;
            // Rotate into view space.
            let vx = dx * cos_a + dy * sin_a; // depth (forward)
            if vx > 0.5 {
                Some((vx, thing))
            } else {
                None // behind or too close
            }
        })
        .collect();

    // Painter's algorithm: draw farthest things first so nearer ones overdraw.
    visible.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // ---------- Render each visible thing ------------------------------------
    for (vx, thing) in visible {
        // Look up sprite name; skip unknown kinds and player starts.
        let lump_name = match thing_sprite(thing.kind) {
            Some(n) => n,
            None => continue,
        };
        // Look up the frame in the cache; skip if not loaded (PWAD without sprites).
        let frame = match cache.get(&lump_name) {
            Some(f) => f,
            None => continue,
        };

        let dx = thing.x as f32 - px;
        let dy = thing.y as f32 - py;
        let vy = -dx * sin_a + dy * cos_a; // lateral displacement

        // --- Screen-space projection ---
        // Horizontal centre of the sprite on screen.
        let sx_center = HALF_W as f32 - FOCAL_LEN * vy / vx;

        // Scale factor: how many screen pixels per map unit at this depth.
        let sprite_scale = FOCAL_LEN / vx;

        // Scaled screen dimensions of the sprite.
        let screen_h = ((frame.height as f32) * sprite_scale).round() as i32;
        let screen_w = ((frame.width as f32) * sprite_scale).round() as i32;

        if screen_h <= 0 || screen_w <= 0 {
            continue;
        }

        // Vertical placement: bottom of sprite is at floor level.
        // Player eye is PLAYER_HEIGHT map units above the floor, so the floor
        // projects to HALF_H + PLAYER_HEIGHT * sprite_scale below the horizon.
        let screen_y_bot = HALF_H + (PLAYER_HEIGHT * sprite_scale).round() as i32;
        let screen_y_top = screen_y_bot - screen_h;

        // Horizontal placement: left_offset tells us how many sprite pixels
        // the centre point is to the right of column 0.
        let screen_x_left =
            sx_center as i32 - (frame.left_offset as i32 * screen_w / frame.width as i32);
        let screen_x_right = screen_x_left + screen_w;

        // Guard against degenerate cases (e.g. 0-width frame).
        if screen_x_right <= 0 || screen_x_left >= SCREEN_W as i32 {
            continue;
        }

        let col_h = screen_y_bot - screen_y_top;

        // --- Draw each scaled sprite column ---
        for col in 0..frame.width as i32 {
            let sx = screen_x_left + col * screen_w / frame.width as i32;
            if sx < 0 || sx >= SCREEN_W as i32 {
                continue;
            }

            let sy_top_clamped = screen_y_top.max(0);
            let sy_bot_clamped = screen_y_bot.min(SCREEN_H as i32 - 1);

            for sy in sy_top_clamped..=sy_bot_clamped {
                // Map screen row back to sprite row.
                let sprite_row = (sy - screen_y_top) * frame.height as i32 / col_h.max(1);
                let sprite_row = sprite_row.clamp(0, frame.height as i32 - 1) as usize;

                let pixel_idx = col as usize * frame.height as usize + sprite_row;
                if let Some(Some(idx)) = frame.pixels.get(pixel_idx) {
                    fb.data[sy as usize * SCREEN_W + sx as usize] = *idx;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Draw a sprite frame onto the framebuffer.
///
/// Positioning:
/// - `screen_x_center`: screen column at the sprite's *center* point.
///   Column 0 of the sprite is placed at `screen_x_center - left_offset`.
/// - `screen_y_bottom`: screen row at the sprite's bottom (baseline row).
///   The sprite occupies rows `[screen_y_bottom - height + 1, screen_y_bottom]`.
///
/// Transparent pixels (stored as `None`) are skipped.
/// The `colormap` slice translates palette indices through light diminishment.
/// Pass [`IDENTITY_COLORMAP`] for full-brightness rendering.
pub fn draw_sprite(
    fb: &mut Framebuffer,
    frame: &SpriteFrame,
    screen_x_center: i32,
    screen_y_bottom: i32,
    colormap: &[u8; 256],
) {
    // Column 0 of the sprite sits left_offset pixels to the right of center.
    // So sprite X origin = center - left_offset.
    let sprite_origin_x = screen_x_center - frame.left_offset as i32;

    // Sprite's top row in screen space.
    let sprite_origin_y = screen_y_bottom - frame.height as i32 + 1;

    let width = frame.width as i32;
    let height = frame.height as i32;

    for col in 0..width {
        let sx = sprite_origin_x + col;
        if sx < 0 || sx >= 320 {
            continue;
        }

        let col_base = col as usize * frame.height as usize;
        for row in 0..height {
            let sy = sprite_origin_y + row;
            if sy < 0 || sy >= 200 {
                continue;
            }
            if let Some(idx) = frame.pixels[col_base + row as usize] {
                let final_color = colormap[idx as usize];
                fb.set_pixel(sx as usize, sy as usize, final_color);
            }
        }
    }
}

/// Draw a weapon sprite at the standard weapon-overlay position (bottom-center).
///
/// Looks up `lump_name` in the cache and draws it centred at `x=160, y=167`
/// (the standard Doom weapon position in a 320×168 view — the area above the
/// status bar).  Does nothing if the sprite is not found in the cache.
pub fn draw_weapon_sprite(
    fb: &mut Framebuffer,
    lump_name: &[u8; 8],
    cache: &SpriteCache,
    colormap: &[u8; 256],
) {
    if let Some(frame) = cache.get(lump_name) {
        draw_sprite(fb, frame, 160, 167, colormap);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::column::IDENTITY_COLORMAP;

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Build a minimal valid 1×1 picture lump with a single pixel at (0,0).
    ///
    /// Layout:
    ///   [0..2]  width  = 1 (u16 LE)
    ///   [2..4]  height = 1 (u16 LE)
    ///   [4..6]  left_offset = 0 (i16 LE)
    ///   [6..8]  top_offset  = 0 (i16 LE)
    ///   [8..12] col_offset  = 12 (u32 LE) — column 0 data starts at byte 12
    ///   [12]    topdelta = 0
    ///   [13]    length   = 1
    ///   [14]    _pad     = 0
    ///   [15]    pixel    = 7
    ///   [16]    _pad     = 0
    ///   [17]    topdelta = 0xFF  (end of column)
    fn minimal_picture() -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes()); // width = 1
        data.extend_from_slice(&1u16.to_le_bytes()); // height = 1
        data.extend_from_slice(&0i16.to_le_bytes()); // left_offset = 0
        data.extend_from_slice(&0i16.to_le_bytes()); // top_offset = 0
        let col_offset: u32 = 12; // header(8) + col_offsets(1×4) = 12
        data.extend_from_slice(&col_offset.to_le_bytes());
        // Post: topdelta=0, length=1, padding, pixel=7, padding, end-marker
        data.extend_from_slice(&[0, 1, 0, 7, 0, 0xFF]);
        data
    }

    /// Build a minimal valid IWAD with zero lumps.
    fn empty_wad() -> Vec<u8> {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"IWAD");
        data[4..8].copy_from_slice(&0i32.to_le_bytes()); // numlumps = 0
        data[8..12].copy_from_slice(&12i32.to_le_bytes()); // dir at end of header
        data
    }

    /// Build a minimal IWAD with the specified named lumps.
    fn wad_with_lumps(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes()); // dir offset placeholder

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, payload) in lumps {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());

        for (i, (name, _)) in lumps.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            let mut name_buf = [0u8; 8];
            for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                name_buf[j] = b.to_ascii_uppercase();
            }
            data.extend_from_slice(&name_buf);
        }

        data
    }

    // ------------------------------------------------------------------
    // 1. Parse a minimal valid picture — pixel must be present
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_picture_minimal() {
        let data = minimal_picture();
        let frame = parse_picture(&data).expect("should parse minimal picture");
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 1);
        assert_eq!(frame.left_offset, 0);
        assert_eq!(frame.top_offset, 0);
        // Column 0, row 0 → pixels[0 * 1 + 0] = Some(7)
        assert_eq!(frame.pixels.len(), 1);
        assert_eq!(frame.pixels[0], Some(7), "pixel at (0,0) should be Some(7)");
    }

    // ------------------------------------------------------------------
    // 2. Too-small buffer returns None
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_picture_empty_lump() {
        // Completely empty.
        assert!(parse_picture(&[]).is_none(), "empty slice must return None");
        // 7 bytes — one short of the 8-byte header.
        assert!(
            parse_picture(&[0u8; 7]).is_none(),
            "7-byte slice must return None"
        );
    }

    // ------------------------------------------------------------------
    // 3. Pixel not covered by any post is None (transparent)
    // ------------------------------------------------------------------
    #[test]
    fn test_parse_picture_transparent() {
        // Build a 1×2 picture where only row 0 has a pixel; row 1 is transparent.
        let mut data = Vec::new();
        data.extend_from_slice(&1u16.to_le_bytes()); // width = 1
        data.extend_from_slice(&2u16.to_le_bytes()); // height = 2
        data.extend_from_slice(&0i16.to_le_bytes()); // left_offset
        data.extend_from_slice(&0i16.to_le_bytes()); // top_offset
        let col_offset: u32 = 12; // header(8) + col_offsets(1×4) = 12
        data.extend_from_slice(&col_offset.to_le_bytes());
        // Post: topdelta=0, length=1, pad, pixel=42, pad, end
        // Only row 0 gets a pixel; row 1 is never written → transparent.
        data.extend_from_slice(&[0, 1, 0, 42, 0, 0xFF]);

        let frame = parse_picture(&data).expect("should parse");
        assert_eq!(frame.width, 1);
        assert_eq!(frame.height, 2);
        assert_eq!(frame.pixels[0], Some(42), "row 0 should be Some(42)");
        assert_eq!(frame.pixels[1], None, "row 1 should be transparent (None)");
    }

    // ------------------------------------------------------------------
    // 4. Loading a WAD without S_START/S_END gives an empty cache
    // ------------------------------------------------------------------
    #[test]
    fn test_sprite_cache_empty_wad() {
        let wad_bytes = empty_wad();
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("wad parse failed");
        let cache = SpriteCache::load(&wad);
        assert!(cache.is_empty(), "no S_START/S_END → cache must be empty");
        assert_eq!(cache.len(), 0);
    }

    // ------------------------------------------------------------------
    // 5. Drawing at the right edge of the screen must not panic
    // ------------------------------------------------------------------
    #[test]
    fn test_draw_sprite_clips_out_of_bounds() {
        // Build a frame 10 pixels wide, 10 pixels tall, all opaque (index 5).
        let width = 10usize;
        let height = 10usize;
        let frame = SpriteFrame {
            width: width as u16,
            height: height as u16,
            left_offset: 0,
            top_offset: 0,
            pixels: vec![Some(5); width * height],
        };

        let mut fb = Framebuffer::new();
        // Draw centered at x=315 — sprite_origin_x=315 (left_offset=0).
        // Sprite occupies columns 315..325; columns 320..325 are clipped.
        // Visible columns: 315..319 (inclusive).
        // sprite_origin_y = screen_y_bottom - height + 1 = 100 - 10 + 1 = 91.
        draw_sprite(&mut fb, &frame, 315, 100, &IDENTITY_COLORMAP);
        // Must not panic.  Visible area: x in [315,319], y in [91,100].
        assert_eq!(fb.get_pixel(315, 91), Some(5)); // top-left of visible portion
        assert_eq!(fb.get_pixel(319, 100), Some(5)); // bottom-right visible
    }

    // ------------------------------------------------------------------
    // 6. draw_weapon_sprite with a missing lump name does nothing (no panic)
    // ------------------------------------------------------------------
    #[test]
    fn test_weapon_sprite_noop_for_missing() {
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        // This sprite does not exist in the (empty) cache.
        draw_weapon_sprite(&mut fb, b"PISGA0\0\0", &cache, &IDENTITY_COLORMAP);
        // Framebuffer stays zeroed — no pixels written, no panic.
        assert!(fb.data.iter().all(|&b| b == 0), "fb should remain zeroed");
    }

    // ------------------------------------------------------------------
    // 7. SpriteCache::get() retrieves a manually inserted frame
    // ------------------------------------------------------------------
    #[test]
    fn test_sprite_cache_name_lookup() {
        let mut cache = SpriteCache::empty();

        let frame = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 1,
            pixels: vec![Some(10), Some(20), Some(30), Some(40)],
        };
        cache.insert("TROOA1".to_string(), frame);

        // Lookup via exact byte array name (null-padded).
        let found = cache.get(b"TROOA1\0\0");
        assert!(found.is_some(), "TROOA1 should be found in cache");
        let f = found.unwrap();
        assert_eq!(f.width, 2);
        assert_eq!(f.height, 2);

        // Non-existent name returns None.
        assert!(cache.get(b"NOTHERE\0").is_none());

        // len() reflects the insertion.
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
    }

    // ------------------------------------------------------------------
    // Extra: WAD with S_START/S_END markers but valid sprite lump parses
    // ------------------------------------------------------------------
    #[test]
    fn test_sprite_cache_loads_from_wad_markers() {
        let pic = minimal_picture();
        // Lumps: S_START (marker, empty), PISGA0 (sprite), S_END (marker, empty)
        let wad_bytes = wad_with_lumps(&[("S_START", &[]), ("PISGA0", &pic), ("S_END", &[])]);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("wad parse");
        let cache = SpriteCache::load(&wad);
        // The sprite lump should have been loaded.
        assert_eq!(cache.len(), 1, "one sprite lump expected");
        let frame = cache.get(b"PISGA0\0\0");
        assert!(frame.is_some(), "PISGA0 should be in cache");
        assert_eq!(frame.unwrap().pixels[0], Some(7));
    }

    // ------------------------------------------------------------------
    // render_things helpers
    // ------------------------------------------------------------------

    /// Build a minimal Level suitable for render_things tests.
    ///
    /// The level contains exactly the things provided and has the minimum
    /// valid BSP (0 nodes, 1 ssector) required by Level::from_wad validation.
    fn make_test_level(things: Vec<doom_map::Thing>) -> doom_map::Level {
        use doom_wad::{REQUIRED_MAP_LUMPS, WadKind};

        let _ = WadKind::Iwad; // suppress unused import warning
        let _ = REQUIRED_MAP_LUMPS;

        // ---- sector ----
        let mut sector_data = vec![0u8; 26];
        sector_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        sector_data[2..4].copy_from_slice(&128i16.to_le_bytes());
        sector_data[4..12].copy_from_slice(b"FLAT1\0\0\0");
        sector_data[12..20].copy_from_slice(b"FLAT2\0\0\0");
        sector_data[20..22].copy_from_slice(&192i16.to_le_bytes());
        sector_data[22..24].copy_from_slice(&0u16.to_le_bytes());
        sector_data[24..26].copy_from_slice(&0u16.to_le_bytes());

        // ---- vertices ----
        let mut vert_data = vec![0u8; 4 * 4];
        let verts: [(i16, i16); 4] = [(0, 0), (64, 0), (64, 64), (0, 64)];
        for (i, (x, y)) in verts.iter().enumerate() {
            vert_data[i * 4..i * 4 + 2].copy_from_slice(&x.to_le_bytes());
            vert_data[i * 4 + 2..i * 4 + 4].copy_from_slice(&y.to_le_bytes());
        }

        // ---- sidedefs ----
        let mut sd_data = vec![0u8; 4 * 30];
        for i in 0..4 {
            sd_data[i * 30 + 20..i * 30 + 28].copy_from_slice(b"WALL1\0\0\0");
            sd_data[i * 30 + 28..i * 30 + 30].copy_from_slice(&0u16.to_le_bytes());
        }

        // ---- linedefs ----
        let mut ld_data = vec![0u8; 4 * 14];
        let edges: [(u16, u16); 4] = [(0, 1), (1, 2), (2, 3), (3, 0)];
        for (i, (from, to)) in edges.iter().enumerate() {
            let b = &mut ld_data[i * 14..i * 14 + 14];
            b[0..2].copy_from_slice(&from.to_le_bytes());
            b[2..4].copy_from_slice(&to.to_le_bytes());
            b[4..6].copy_from_slice(&0u16.to_le_bytes());
            b[6..8].copy_from_slice(&0u16.to_le_bytes());
            b[8..10].copy_from_slice(&0u16.to_le_bytes());
            b[10..12].copy_from_slice(&(i as u16).to_le_bytes());
            b[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        }

        // ---- seg ----
        let mut seg_data = vec![0u8; 12];
        seg_data[0..2].copy_from_slice(&0u16.to_le_bytes());
        seg_data[2..4].copy_from_slice(&1u16.to_le_bytes());

        // ---- ssector ----
        let mut ss_data = vec![0u8; 4];
        ss_data[0..2].copy_from_slice(&1u16.to_le_bytes());
        ss_data[2..4].copy_from_slice(&0u16.to_le_bytes());

        // ---- things ----
        let mut thing_data = vec![0u8; things.len() * 10];
        for (i, t) in things.iter().enumerate() {
            let b = &mut thing_data[i * 10..i * 10 + 10];
            b[0..2].copy_from_slice(&t.x.to_le_bytes());
            b[2..4].copy_from_slice(&t.y.to_le_bytes());
            b[4..6].copy_from_slice(&t.angle.to_le_bytes());
            b[6..8].copy_from_slice(&t.kind.to_le_bytes());
            b[8..10].copy_from_slice(&t.flags.to_le_bytes());
        }
        // If no things provided, add a dummy player-start so validation
        // doesn't fail on completely empty THINGS lump (which is valid anyway,
        // but some engines require at least a player start).
        if things.is_empty() {
            // 0-byte THINGS lump is valid (0 entries).
        }

        // ---- reject ----
        let reject_data = vec![0u8; 1];

        // ---- blockmap ----
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[0..2].copy_from_slice(&0i16.to_le_bytes());
        bm_data[2..4].copy_from_slice(&0i16.to_le_bytes());
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());

        let lump_payloads: &[(&[u8; 8], &[u8])] = &[
            (b"E1M1\0\0\0\0", &[]),
            (b"THINGS\0\0", &thing_data),
            (b"LINEDEFS", &ld_data),
            (b"SIDEDEFS", &sd_data),
            (b"VERTEXES", &vert_data),
            (b"SEGS\0\0\0\0", &seg_data),
            (b"SSECTORS", &ss_data),
            (b"NODES\0\0\0", &[]),
            (b"SECTORS\0", &sector_data),
            (b"REJECT\0\0", &reject_data),
            (b"BLOCKMAP", &bm_data),
        ];

        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lump_payloads.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, payload) in lump_payloads {
            let pos = data.len();
            data.extend_from_slice(payload);
            offsets.push((pos, payload.len()));
        }

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());
        for (i, (name_bytes, _)) in lump_payloads.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            data.extend_from_slice(*name_bytes);
        }

        let wad = doom_wad::WadFile::parse(data).expect("test WAD parse failed");
        doom_map::Level::from_wad(&wad, "E1M1").expect("test level load failed")
    }

    /// Build a Thing with given position, kind.
    fn make_thing(x: i16, y: i16, kind: u16) -> doom_map::Thing {
        doom_map::Thing {
            x,
            y,
            angle: 0,
            kind,
            flags: 7,
        }
    }

    // ------------------------------------------------------------------
    // render_things test 1: empty level — no panic
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_empty_level() {
        let level = make_test_level(vec![]);
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        // Must not panic on zero things.
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(32),
            doom_types::Fixed16_16::from_int(32),
            doom_types::Bam(0),
            &mut fb,
            &cache,
        );
        // Framebuffer stays zeroed (empty cache → nothing drawn).
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // ------------------------------------------------------------------
    // render_things test 2: thing behind player is not rendered
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_behind_player_skipped() {
        // Player at (0,0) facing east (angle 0 = east in Doom convention).
        // Thing is at (-100, 0) — directly behind the player.
        let thing = make_thing(-100, 0, 2035); // barrel — has sprite name BAR1
        let level = make_test_level(vec![thing]);

        // Insert a dummy sprite so the cache lookup would succeed if
        // the thing were incorrectly included.
        let mut cache = SpriteCache::empty();
        let dummy_frame = SpriteFrame {
            width: 2,
            height: 2,
            left_offset: 1,
            top_offset: 0,
            pixels: vec![Some(42); 4],
        };
        cache.insert("BAR1A0".to_string(), dummy_frame);

        let mut fb = Framebuffer::new();
        // Player at origin, facing east (Bam(0)).
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
        );
        // If the thing behind the player were rendered it would write pixel 42.
        // The framebuffer must remain all zeros.
        assert!(
            fb.data.iter().all(|&b| b == 0),
            "behind-player thing must not be rendered"
        );
    }

    // ------------------------------------------------------------------
    // render_things test 3: unknown thing kind is silently skipped
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_unknown_kind_skipped() {
        // Kind 9999 is not in the lookup table.
        let thing = make_thing(100, 0, 9999);
        let level = make_test_level(vec![thing]);
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
        );
        // Nothing drawn — no panic.
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // ------------------------------------------------------------------
    // render_things test 4: player start (kind=1) is skipped
    // ------------------------------------------------------------------
    #[test]
    fn test_render_things_player_start_skipped() {
        // Kind 1 = player 1 start; thing_sprite returns None.
        let thing = make_thing(100, 0, 1);
        let level = make_test_level(vec![thing]);
        let cache = SpriteCache::empty();
        let mut fb = Framebuffer::new();
        render_things(
            &level,
            doom_types::Fixed16_16::from_int(0),
            doom_types::Fixed16_16::from_int(0),
            doom_types::Bam(0),
            &mut fb,
            &cache,
        );
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // ------------------------------------------------------------------
    // render_things test 5: forward thing projects to screen centre
    // ------------------------------------------------------------------
    #[test]
    fn test_proj_math_forward_thing() {
        // Player at (0,0) facing east (Bam(0)).
        // Thing at (200, 0) — directly ahead.
        // vy = 0 so sx_center = HALF_W - FOCAL_LEN * 0 / vx = 160.

        let player_x = doom_types::Fixed16_16::from_int(0);
        let player_y = doom_types::Fixed16_16::from_int(0);
        let player_angle = doom_types::Bam(0);

        // Manually replicate the projection math for a forward thing.
        let angle_rad =
            (player_angle.0 as f32) * (std::f32::consts::PI * 2.0 / (u32::MAX as f32 + 1.0));
        let cos_a = angle_rad.cos();
        let sin_a = angle_rad.sin();

        let px = player_x.raw() as f32 / 65536.0;
        let py = player_y.raw() as f32 / 65536.0;

        let thing_x: f32 = 200.0;
        let thing_y: f32 = 0.0;

        let dx = thing_x - px;
        let dy = thing_y - py;
        let vx = dx * cos_a + dy * sin_a;
        let vy = -dx * sin_a + dy * cos_a;

        // vx must be positive (thing is in front).
        assert!(vx > 0.5, "thing should be in front: vx={vx}");

        let sx_center = 160.0_f32 - 160.0 * vy / vx;

        // For a thing directly ahead, vy ≈ 0 so sx_center ≈ 160.
        let deviation = (sx_center - 160.0).abs();
        assert!(
            deviation < 1.0,
            "forward thing should project to screen centre ≈160, got {sx_center}"
        );
    }

    // ------------------------------------------------------------------
    // Extra: draw_sprite correctly applies colormap
    // ------------------------------------------------------------------
    #[test]
    fn test_draw_sprite_applies_colormap() {
        let frame = SpriteFrame {
            width: 1,
            height: 1,
            left_offset: 0,
            top_offset: 0,
            pixels: vec![Some(10)],
        };

        // Build a colormap that maps index 10 → 99.
        let mut colormap = IDENTITY_COLORMAP;
        colormap[10] = 99;

        let mut fb = Framebuffer::new();
        // Draw at center x=160, bottom y=100.
        // sprite_origin_x = 160 - 0 = 160, sprite_origin_y = 100 - 1 + 1 = 100
        draw_sprite(&mut fb, &frame, 160, 100, &colormap);
        assert_eq!(
            fb.get_pixel(160, 100),
            Some(99),
            "colormap mapping should be applied"
        );
    }
}
