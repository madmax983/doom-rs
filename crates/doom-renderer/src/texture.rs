//! Wall texture cache — loads and composes TEXTURE1/TEXTURE2 textures from a WAD.
//!
//! Doom wall textures are not stored as single images.  Instead, each texture is
//! defined in the TEXTURE1/TEXTURE2 lumps as a grid of rectangular patches drawn
//! from raw picture-format lumps named in PNAMES.
//!
//! This module parses those lumps, blits the patches together, and stores the
//! resulting textures in column-major order for O(1) column lookup at render time.
//!
//! # Binary layout references
//! - PNAMES:   `u32 count`, then `count × 8-byte` null-padded patch names
//! - TEXTURE1: `u32 num_textures`, `num_textures × u32 offsets`, then texture
//!   descriptors at each offset
//! - Patch:    `u16 width/height`, `i16 leftoffset/topoffset`, `width × u32 col_offsets`,
//!   then column posts (`topdelta`, `length`, pad, pixels, pad; 0xFF = end)

use doom_wad::{WadFile, WadStack};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A composed wall texture, stored column-major for O(1) column lookup.
///
/// `data[col * height_pow2 + row]` gives the palette index for that texel.
/// `height` is always the next power of two >= the real texture height, so
/// the column renderer can use `& (height - 1)` for fast wrapping.
pub struct WallTexture {
    /// Texture width in texels.
    pub width: u32,
    /// Logical texture height from TEXTURE1/TEXTURE2.
    pub logical_height: u32,
    /// Texture height in texels (padded to the next power of 2).
    pub height: u32,
    /// Column-major texel data: `data[col * height + row]`.
    pub data: Vec<u8>,
}

use doom_wad::lump::LumpName;

/// Cache of all wall textures composed from TEXTURE1/TEXTURE2 + PNAMES.
///
/// Look up textures by their 8-byte, null-padded WAD name.
///
/// ## Examples
/// ```
/// use doom_wad::WadFile;
/// use doom_renderer::texture::TextureCache;
///
/// let wad_bytes = b"IWAD\x00\0\0\0\x0C\0\0\0".to_vec();
/// let wad = WadFile::parse(wad_bytes).unwrap();
/// let textures = TextureCache::load(&wad);
/// ```
pub struct TextureCache {
    textures: HashMap<LumpName, WallTexture>,
}

impl TextureCache {
    /// Load and compose all wall textures from the WAD.
    ///
    /// Reads PNAMES, then TEXTURE1 (and TEXTURE2 if present), blits all
    /// constituent patches together, and stores the result column-major with
    /// height padded to the next power of 2.
    pub fn load(wad: &WadFile) -> Self {
        let pnames = match wad.find_lump_data("PNAMES") {
            Some(d) => parse_pnames(d),
            None => {
                return TextureCache {
                    textures: HashMap::new(),
                };
            }
        };

        let mut textures = HashMap::new();

        // Process TEXTURE1 (always present in valid IWADs).
        if let Some(data) = wad.find_lump_data("TEXTURE1") {
            parse_texture_lump(
                data,
                &pnames,
                |patch_name| wad.find_lump_data(patch_name),
                &mut textures,
            );
        }

        // TEXTURE2 is optional (only in registered Doom/Doom 2).
        if let Some(data) = wad.find_lump_data("TEXTURE2") {
            parse_texture_lump(
                data,
                &pnames,
                |patch_name| wad.find_lump_data(patch_name),
                &mut textures,
            );
        }

        TextureCache { textures }
    }

    /// Load and compose all wall textures from a stacked IWAD/PWAD view.
    pub fn load_from_stack(wad: &WadStack) -> Self {
        let pnames = match wad.lump_data("PNAMES") {
            Some(d) => parse_pnames(d),
            None => {
                return TextureCache {
                    textures: HashMap::new(),
                };
            }
        };

        let mut textures = HashMap::new();

        if let Some(data) = wad.lump_data("TEXTURE1") {
            parse_texture_lump(
                data,
                &pnames,
                |patch_name| wad.lump_data(patch_name),
                &mut textures,
            );
        }

        if let Some(data) = wad.lump_data("TEXTURE2") {
            parse_texture_lump(
                data,
                &pnames,
                |patch_name| wad.lump_data(patch_name),
                &mut textures,
            );
        }

        TextureCache { textures }
    }

    /// Look up a texture by its 8-byte WAD name (null-padded, uppercase).
    ///
    /// Returns `None` for the special `-` no-texture marker and for any name
    /// not present in TEXTURE1/TEXTURE2.
    pub fn get(&self, name: &[u8; 8]) -> Option<&WallTexture> {
        // The first byte being b'-' (and rest null) is the "no texture" sentinel.
        if name[0] == b'-' {
            return None;
        }

        // Trim trailing NUL/space padding so LumpName interprets correctly.
        let mut clean_name = *name;
        for i in (0..8).rev() {
            if clean_name[i] == b' ' || clean_name[i] == 0 {
                clean_name[i] = 0;
            } else {
                break;
            }
        }

        let key = LumpName::from_raw(clean_name);
        self.textures.get(&key)
    }

    /// Number of textures in the cache.
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }
}

// ---------------------------------------------------------------------------
// PNAMES parser
// ---------------------------------------------------------------------------

/// Parse the PNAMES lump into a list of patch name strings (uppercase).
fn parse_pnames(data: &[u8]) -> Vec<String> {
    if data.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    // Havoc 👺: Defend against OOM from fuzzed PNAMES lump count.
    let max_names = data.len().saturating_sub(4) / 8;
    let safe_capacity = count.min(max_names);

    let mut names = Vec::with_capacity(safe_capacity);

    for i in 0..count {
        let off = 4 + i * 8;
        if off + 8 > data.len() {
            break;
        }
        let raw = &data[off..off + 8];
        let len = raw.iter().position(|&b| b == 0).unwrap_or(8);
        let name = String::from_utf8_lossy(&raw[..len]).to_uppercase();
        names.push(name.to_owned());
    }

    names
}

// ---------------------------------------------------------------------------
// TEXTURE1/TEXTURE2 parser
// ---------------------------------------------------------------------------

/// A patch reference inside a texture definition (10 bytes in the WAD).
#[derive(Clone, Copy)]
struct MapPatch {
    origin_x: i16,
    origin_y: i16,
    patch: u16, // index into pnames
                // stepdir and colormap are unused
}

/// Parse one TEXTURE1 or TEXTURE2 lump and insert composed textures into `out`.
fn parse_texture_lump<'a, F>(
    data: &[u8],
    pnames: &[String],
    mut find_patch: F,
    out: &mut HashMap<LumpName, WallTexture>,
) where
    F: FnMut(&str) -> Option<&'a [u8]>,
{
    if data.len() < 4 {
        return;
    }

    let num_textures = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    for i in 0..num_textures {
        // Each offset entry is 4 bytes, starting at byte 4.
        let off_idx = 4 + i * 4;
        if off_idx + 4 > data.len() {
            break;
        }
        let tex_offset = u32::from_le_bytes([
            data[off_idx],
            data[off_idx + 1],
            data[off_idx + 2],
            data[off_idx + 3],
        ]) as usize;

        if tex_offset + 22 > data.len() {
            continue;
        }

        // Parse texture header at tex_offset.
        // Layout (offsets relative to tex_offset):
        //   +0   u8[8]  name
        //   +8   u32    masked  (skip)
        //   +12  u16    width
        //   +14  u16    height
        //   +16  u32    columndir (unused in Doom, skip)
        //   +20  u16    patch_count
        //   +22  patch_count × MapPatch (10 bytes each)
        let mut name_raw = [0u8; 8];
        name_raw.copy_from_slice(&data[tex_offset..tex_offset + 8]);
        let name = LumpName::from_raw(name_raw);

        let width = u16::from_le_bytes([data[tex_offset + 12], data[tex_offset + 13]]) as u32;
        let height = u16::from_le_bytes([data[tex_offset + 14], data[tex_offset + 15]]) as u32;
        // columndir at [tex_offset+16..tex_offset+20] — skipped
        let patch_count =
            u16::from_le_bytes([data[tex_offset + 20], data[tex_offset + 21]]) as usize;

        if width == 0 || height == 0 {
            continue;
        }

        // Parse MapPatch entries (10 bytes each), starting at tex_offset + 22.
        let patches_start = tex_offset + 22;

        // Havoc 👺: Defend against OOM and huge allocations from fuzzed lengths.
        let max_patches_in_data = data.len().saturating_sub(patches_start) / 10;
        let safe_patch_capacity = patch_count.min(max_patches_in_data);

        let mut patches = Vec::with_capacity(safe_patch_capacity);
        for p in 0..patch_count {
            let poff = patches_start + p * 10;
            if poff + 10 > data.len() {
                break;
            }
            patches.push(MapPatch {
                origin_x: i16::from_le_bytes([data[poff], data[poff + 1]]),
                origin_y: i16::from_le_bytes([data[poff + 2], data[poff + 3]]),
                patch: u16::from_le_bytes([data[poff + 4], data[poff + 5]]),
                // stepdir: [poff+6..poff+8], colormap: [poff+8..poff+10] — both unused
            });
        }

        // Pad height to next power of 2.
        let height_pow2 = next_pow2(height.max(1));

        // Allocate the column-major texture buffer (all palette index 0 = transparent).
        let mut texdata = vec![0u8; width as usize * height_pow2 as usize];

        // Blit each patch into the texture.
        for mp in &patches {
            let patch_name = match pnames.get(mp.patch as usize) {
                Some(n) => n.clone(),
                None => continue,
            };
            let patch_data = match find_patch(&patch_name) {
                Some(d) => d,
                None => continue,
            };
            blit_patch(
                patch_data,
                mp.origin_x,
                mp.origin_y,
                width,
                height_pow2,
                &mut texdata,
            );
        }

        // Pad the extra rows (height..height_pow2) by cycling the real rows.
        // This ensures clean wrapping when frac overflows at the bottom of the texture.
        if height < height_pow2 {
            for col in 0..width as usize {
                let col_off = col * height_pow2 as usize;
                for row in height as usize..height_pow2 as usize {
                    let src = texdata[col_off + (row % height as usize)];
                    texdata[col_off + row] = src;
                }
            }
        }

        out.insert(
            name,
            WallTexture {
                width,
                logical_height: height,
                height: height_pow2,
                data: texdata,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Patch blitter
// ---------------------------------------------------------------------------

/// Blit a Doom picture-format patch into a column-major texture buffer.
///
/// `tex_w` and `tex_h` are the destination texture dimensions (height is
/// already power-of-2 padded).  The origin coordinates can be negative
/// (patch overhangs the texture edge) — those pixels are simply skipped.
fn blit_patch(patch: &[u8], origin_x: i16, origin_y: i16, tex_w: u32, tex_h: u32, dest: &mut [u8]) {
    if patch.len() < 8 {
        return;
    }

    let patch_w = u16::from_le_bytes([patch[0], patch[1]]) as i32;
    let patch_h = u16::from_le_bytes([patch[2], patch[3]]) as i32;
    // leftoffset / topoffset at [4..8] — not used here (already factored into
    // the origin in the texture definition)
    let _ = patch_h; // suppress unused warning — used implicitly via topdelta

    // Column offset table: 4 bytes per column, starting at byte 8.
    let col_offsets_end = 8 + patch_w as usize * 4;
    if col_offsets_end > patch.len() {
        return;
    }

    let ox = origin_x as i32;
    let oy = origin_y as i32;

    for col in 0..patch_w {
        let dest_x = ox + col;
        if dest_x < 0 || dest_x >= tex_w as i32 {
            continue;
        }

        let col_off_idx = 8 + col as usize * 4;
        let col_off = u32::from_le_bytes([
            patch[col_off_idx],
            patch[col_off_idx + 1],
            patch[col_off_idx + 2],
            patch[col_off_idx + 3],
        ]) as usize;

        // Walk the column posts.
        let mut pos = col_off;
        loop {
            if pos >= patch.len() {
                break;
            }
            let topdelta = patch[pos];
            if topdelta == 0xFF {
                break; // end of column
            }
            pos += 1;

            if pos >= patch.len() {
                break;
            }
            let length = patch[pos] as i32;
            pos += 1;

            // Skip one padding byte before pixels.
            pos += 1;

            let dest_col_base = dest_x as usize * tex_h as usize;

            for i in 0..length {
                if pos >= patch.len() {
                    break;
                }
                let pixel = patch[pos];
                pos += 1;

                let dest_y = oy + topdelta as i32 + i;
                if dest_y >= 0 && dest_y < tex_h as i32 {
                    dest[dest_col_base + dest_y as usize] = pixel;
                }
            }

            // Skip one trailing padding byte after pixels.
            pos += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: next power of 2
// ---------------------------------------------------------------------------

/// Return the smallest power of 2 that is >= `n`.
fn next_pow2(n: u32) -> u32 {
    if n == 0 {
        return 1;
    }
    if n.is_power_of_two() {
        n
    } else {
        n.next_power_of_two()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Shared WAD builder (mirrors flat_cache tests pattern)
    // -----------------------------------------------------------------------

    fn make_iwad(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes()); // dir offset placeholder

        let mut offsets: Vec<(usize, usize)> = Vec::new();
        for (_, lump_bytes) in lumps {
            let pos = data.len();
            data.extend_from_slice(lump_bytes);
            offsets.push((pos, lump_bytes.len()));
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

    // -----------------------------------------------------------------------
    // Build a minimal in-memory PNAMES lump with one entry.
    // -----------------------------------------------------------------------
    fn make_pnames(names: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(names.len() as u32).to_le_bytes());
        for name in names {
            let mut buf = [0u8; 8];
            for (i, &b) in name.as_bytes().iter().take(8).enumerate() {
                buf[i] = b.to_ascii_uppercase();
            }
            out.extend_from_slice(&buf);
        }
        out
    }

    // -----------------------------------------------------------------------
    // Build a minimal TEXTURE1 lump with one texture and one patch.
    // -----------------------------------------------------------------------
    fn make_texture1(
        tex_name: &str,
        width: u16,
        height: u16,
        patch_idx: u16,
        origin_x: i16,
        origin_y: i16,
    ) -> Vec<u8> {
        // num_textures = 1
        // offset[0] = 8  (points past the 4-byte count + 4-byte offset table)
        let tex_offset: u32 = 8; // 4 (count) + 4 (one offset entry)
        let mut out = Vec::new();

        // Header: num_textures
        out.extend_from_slice(&1u32.to_le_bytes());
        // Offset table: one offset
        out.extend_from_slice(&tex_offset.to_le_bytes());

        // Texture descriptor (starts at tex_offset = 8):
        // name (8 bytes)
        let mut name_buf = [0u8; 8];
        for (i, &b) in tex_name.as_bytes().iter().take(8).enumerate() {
            name_buf[i] = b.to_ascii_uppercase();
        }
        out.extend_from_slice(&name_buf);
        // masked (u32)
        out.extend_from_slice(&0u32.to_le_bytes());
        // width, height (u16 each)
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        // columndir (u32, unused)
        out.extend_from_slice(&0u32.to_le_bytes());
        // patch_count = 1
        out.extend_from_slice(&1u16.to_le_bytes());

        // MapPatch (10 bytes):
        out.extend_from_slice(&origin_x.to_le_bytes());
        out.extend_from_slice(&origin_y.to_le_bytes());
        out.extend_from_slice(&patch_idx.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // stepdir
        out.extend_from_slice(&0u16.to_le_bytes()); // colormap

        out
    }

    // -----------------------------------------------------------------------
    // Build a minimal picture-format patch (width × height, all one colour).
    // -----------------------------------------------------------------------
    fn make_patch(width: u16, height: u16, colour: u8) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;

        // Picture header: width(2) + height(2) + leftoffset(2) + topoffset(2) = 8 bytes
        // Column offset table: w * 4 bytes
        // Each column: one post (topdelta=0, length=h, pad, h bytes, pad) + 0xFF end

        let col_data_len = 1 + 1 + 1 + h + 1 + 1; // topdelta + len + pad + pixels + pad + 0xFF
        let header_size = 8;
        let offsets_size = w * 4;
        let first_col_offset = header_size + offsets_size;

        let mut out = Vec::new();
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes()); // leftoffset
        out.extend_from_slice(&0i16.to_le_bytes()); // topoffset

        // Column offsets: all columns are identical post layout, laid out sequentially.
        for c in 0..w {
            let col_off = (first_col_offset + c * col_data_len) as u32;
            out.extend_from_slice(&col_off.to_le_bytes());
        }

        // Column data: one post per column.
        for _ in 0..w {
            out.push(0); // topdelta = 0
            out.push(h as u8); // length
            out.push(0); // padding before pixels
            for _ in 0..h {
                out.push(colour); // pixel data
            }
            out.push(0); // padding after pixels
            out.push(0xFF); // end of column
        }

        out
    }

    // -----------------------------------------------------------------------
    // 1. test_pnames_parse
    // -----------------------------------------------------------------------
    #[test]
    fn test_pnames_parse() {
        let pnames_data = make_pnames(&["WALL01"]);
        let names = parse_pnames(&pnames_data);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "WALL01");
    }

    // -----------------------------------------------------------------------
    // 2. test_texture1_parse_single
    // -----------------------------------------------------------------------
    #[test]
    fn test_texture1_parse_single() {
        // Build a complete WAD: PNAMES + WALL01 patch + TEXTURE1
        let patch_data = make_patch(8, 8, 42);
        let pnames_data = make_pnames(&["WALL01"]);
        let tex1_data = make_texture1("BRICK1", 8, 8, 0, 0, 0);

        let wad_bytes = make_iwad(&[
            ("PNAMES", &pnames_data),
            ("WALL01", &patch_data),
            ("TEXTURE1", &tex1_data),
        ]);
        let wad = WadFile::parse(wad_bytes).expect("WAD parse");
        let cache = TextureCache::load(&wad);

        let tex = cache.get(b"BRICK1\0\0").expect("texture should be present");
        assert_eq!(tex.width, 8, "texture width must match");
        // Height is padded to next pow2: 8 is already a power of 2.
        assert_eq!(
            tex.height, 8,
            "texture height must match (no padding needed)"
        );
    }

    // -----------------------------------------------------------------------
    // 3. test_texture_get_none_for_dash
    // -----------------------------------------------------------------------
    #[test]
    fn test_texture_get_none_for_dash() {
        let cache = TextureCache {
            textures: HashMap::new(),
        };
        let result = cache.get(b"-\0\0\0\0\0\0\0");
        assert!(
            result.is_none(),
            "'-' must return None (no-texture sentinel)"
        );
    }

    // -----------------------------------------------------------------------
    // 4. test_texture_get_returns_correct_dims
    // -----------------------------------------------------------------------
    #[test]
    fn test_texture_get_returns_correct_dims() {
        let patch_data = make_patch(16, 64, 77);
        let pnames_data = make_pnames(&["MYWALL"]);
        let tex1_data = make_texture1("MYWALL", 16, 64, 0, 0, 0);

        let wad_bytes = make_iwad(&[
            ("PNAMES", &pnames_data),
            ("MYWALL", &patch_data),
            ("TEXTURE1", &tex1_data),
        ]);
        let wad = WadFile::parse(wad_bytes).expect("WAD parse");
        let cache = TextureCache::load(&wad);

        let tex = cache.get(b"MYWALL\0\0").expect("texture must be present");
        assert_eq!(tex.width, 16);
        // 64 is already pow2
        assert_eq!(tex.height, 64);
    }

    // -----------------------------------------------------------------------
    // 5. test_texture_height_power_of_2
    // -----------------------------------------------------------------------
    #[test]
    fn test_texture_height_power_of_2() {
        // Use height 72 (not a power of 2); expect padded to 128.
        let patch_data = make_patch(8, 72, 1);
        let pnames_data = make_pnames(&["PATCH0"]);
        let tex1_data = make_texture1("TEX72", 8, 72, 0, 0, 0);

        let wad_bytes = make_iwad(&[
            ("PNAMES", &pnames_data),
            ("PATCH0", &patch_data),
            ("TEXTURE1", &tex1_data),
        ]);
        let wad = WadFile::parse(wad_bytes).expect("WAD parse");
        let cache = TextureCache::load(&wad);

        let tex = cache.get(b"TEX72\0\0\0").expect("texture must be present");
        assert!(
            tex.height.is_power_of_two(),
            "height {} must be a power of 2",
            tex.height
        );
        assert!(
            tex.height >= 72,
            "padded height {} must be >= real height 72",
            tex.height
        );
        assert_eq!(tex.height, 128, "72 should pad to 128");
    }

    // -----------------------------------------------------------------------
    // 6. test_texture_column_major_layout
    // -----------------------------------------------------------------------
    #[test]
    fn test_texture_column_major_layout() {
        // Patch: 4 wide, 4 tall, all pixels = 99.
        let patch_data = make_patch(4, 4, 99);
        let pnames_data = make_pnames(&["COL0"]);
        let tex1_data = make_texture1("COLTEST", 4, 4, 0, 0, 0);

        let wad_bytes = make_iwad(&[
            ("PNAMES", &pnames_data),
            ("COL0", &patch_data),
            ("TEXTURE1", &tex1_data),
        ]);
        let wad = WadFile::parse(wad_bytes).expect("WAD parse");
        let cache = TextureCache::load(&wad);

        let tex = cache.get(b"COLTEST\0").expect("texture must be present");
        // Column 0, rows 0..4 should all be 99.
        for row in 0..4 {
            assert_eq!(tex.data[row], 99, "column 0 row {row} should be 99");
        }
    }
}
