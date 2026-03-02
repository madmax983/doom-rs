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
        Self { frames: HashMap::new() }
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

    let width  = u16::from_le_bytes([data[0], data[1]]) as usize;
    let height = u16::from_le_bytes([data[2], data[3]]) as usize;
    let left   = i16::from_le_bytes([data[4], data[5]]);
    let top    = i16::from_le_bytes([data[6], data[7]]);

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
        let off = u32::from_le_bytes(
            data[table_pos..table_pos + 4].try_into().ok()?
        ) as usize;

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
        width:       width as u16,
        height:      height as u16,
        left_offset: left,
        top_offset:  top,
        pixels,
    })
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

    let width  = frame.width as i32;
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
        data[4..8].copy_from_slice(&0i32.to_le_bytes());  // numlumps = 0
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
        assert!(parse_picture(&[0u8; 7]).is_none(), "7-byte slice must return None");
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
        assert_eq!(frame.pixels[1], None,      "row 1 should be transparent (None)");
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
            width:       width as u16,
            height:      height as u16,
            left_offset: 0,
            top_offset:  0,
            pixels:      vec![Some(5); width * height],
        };

        let mut fb = Framebuffer::new();
        // Draw centered at x=315 — sprite_origin_x=315 (left_offset=0).
        // Sprite occupies columns 315..325; columns 320..325 are clipped.
        // Visible columns: 315..319 (inclusive).
        // sprite_origin_y = screen_y_bottom - height + 1 = 100 - 10 + 1 = 91.
        draw_sprite(&mut fb, &frame, 315, 100, &IDENTITY_COLORMAP);
        // Must not panic.  Visible area: x in [315,319], y in [91,100].
        assert_eq!(fb.get_pixel(315, 91), Some(5));  // top-left of visible portion
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
            width:       2,
            height:      2,
            left_offset: 1,
            top_offset:  1,
            pixels:      vec![Some(10), Some(20), Some(30), Some(40)],
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
        let wad_bytes = wad_with_lumps(&[
            ("S_START", &[]),
            ("PISGA0",  &pic),
            ("S_END",   &[]),
        ]);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("wad parse");
        let cache = SpriteCache::load(&wad);
        // The sprite lump should have been loaded.
        assert_eq!(cache.len(), 1, "one sprite lump expected");
        let frame = cache.get(b"PISGA0\0\0");
        assert!(frame.is_some(), "PISGA0 should be in cache");
        assert_eq!(frame.unwrap().pixels[0], Some(7));
    }

    // ------------------------------------------------------------------
    // Extra: draw_sprite correctly applies colormap
    // ------------------------------------------------------------------
    #[test]
    fn test_draw_sprite_applies_colormap() {
        let frame = SpriteFrame {
            width:       1,
            height:      1,
            left_offset: 0,
            top_offset:  0,
            pixels:      vec![Some(10)],
        };

        // Build a colormap that maps index 10 → 99.
        let mut colormap = IDENTITY_COLORMAP;
        colormap[10] = 99;

        let mut fb = Framebuffer::new();
        // Draw at center x=160, bottom y=100.
        // sprite_origin_x = 160 - 0 = 160, sprite_origin_y = 100 - 1 + 1 = 100
        draw_sprite(&mut fb, &frame, 160, 100, &colormap);
        assert_eq!(fb.get_pixel(160, 100), Some(99), "colormap mapping should be applied");
    }
}
