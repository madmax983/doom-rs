//! Multi-patch wall texture composition (TEXTURE1/TEXTURE2 + PNAMES).
//!
//! Doom wall textures are composites: each texture is built from one or more
//! rectangular patches placed at explicit offsets.  This module provides
//! WAD-independent parsing of PNAMES, TEXTURE1/TEXTURE2, and Doom
//! picture-format patches, plus a compositor that blits patches together into
//! column-major pixel buffers ready for the renderer.
//!
//! # Binary layout references
//! - **PNAMES**: `u32 count`, then `count x 8-byte` null-padded patch names
//! - **TEXTURE1/TEXTURE2**: `u32 num_textures`, `num_textures x u32` offsets,
//!   then texture descriptors (22-byte header + `patch_count x 10-byte` patch
//!   descriptors) at each offset.
//! - **Patch (picture format)**: `u16 width`, `u16 height`, `i16 left_offset`,
//!   `i16 top_offset`, `width x u32` column offsets, then column posts
//!   (topdelta, length, pad, pixels, pad; `0xFF` = end of column).

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A parsed texture definition from TEXTURE1/TEXTURE2.
#[derive(Debug, Clone)]
pub struct TextureDef {
    /// Texture name (uppercase, null-padded in WAD).
    pub name: [u8; 8],
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Patches composing this texture.
    pub patches: Vec<PatchDef>,
}

/// A single patch placement within a texture.
#[derive(Debug, Clone, Copy)]
pub struct PatchDef {
    /// X offset within the texture.
    pub origin_x: i16,
    /// Y offset within the texture.
    pub origin_y: i16,
    /// Index into the PNAMES array.
    pub pname_index: u16,
}

/// A decoded patch image (column-major picture format).
#[derive(Debug, Clone)]
pub struct PatchImage {
    /// Width of the patch in pixels.
    pub width: u16,
    /// Height of the patch in pixels.
    pub height: u16,
    /// Left offset (used by sprites, not texture composition).
    pub left_offset: i16,
    /// Top offset (used by sprites, not texture composition).
    pub top_offset: i16,
    /// Column data: for each x, a list of posts.
    pub columns: Vec<Vec<PatchPost>>,
}

/// A single post (run of opaque pixels) within a column.
#[derive(Debug, Clone)]
pub struct PatchPost {
    /// Y offset of this run within the column.
    pub y_offset: u8,
    /// Palette indices for each pixel in the run.
    pub pixels: Vec<u8>,
}

/// A fully composed texture -- column-major pixel data ready for rendering.
#[derive(Debug, Clone)]
pub struct ComposedTexture {
    /// Texture name (uppercase, null-padded).
    pub name: [u8; 8],
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
    /// Column-major pixel data. `columns[x]` contains `height` pixels.
    /// Unset pixels default to palette index 0.
    pub columns: Vec<Vec<u8>>,
}

// ---------------------------------------------------------------------------
// PNAMES parser
// ---------------------------------------------------------------------------

/// Parse the PNAMES lump into a list of 8-byte patch names.
///
/// Each name is stored as-is from the WAD (uppercase, null-padded).
pub fn parse_pnames(data: &[u8]) -> Vec<[u8; 8]> {
    if data.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    // Havoc 👺: Defend against OOM from fuzzed PNAMES lump count.
    // Max entries is based on remaining bytes.
    let max_names = data.len().saturating_sub(4) / 8;
    let safe_capacity = count.min(max_names);

    let mut names = Vec::with_capacity(safe_capacity);
    for i in 0..count {
        let off = 4 + i * 8;
        if off + 8 > data.len() {
            break;
        }
        let mut name = [0u8; 8];
        name.copy_from_slice(&data[off..off + 8]);
        names.push(name);
    }
    names
}

// ---------------------------------------------------------------------------
// TEXTURE1/TEXTURE2 parser
// ---------------------------------------------------------------------------

/// Parse a TEXTURE1 or TEXTURE2 lump into texture definitions.
///
/// Returns an empty `Vec` if the lump is too short or contains zero textures.
pub fn parse_texture_lump(data: &[u8]) -> Vec<TextureDef> {
    if data.len() < 4 {
        return Vec::new();
    }
    let num_textures = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

    // Havoc 👺: Defend against OOM and huge allocations from fuzzed lengths.
    // Doom's executable and standard WAD sizes are reasonable, so an allocation for
    // billions of textures is obviously malicious or corrupt data.
    // If the requested capacity is absurdly large (e.g. larger than the whole data slice length),
    // clamp or error. We'll clamp the initial capacity to avoid OOM while still parsing what we can.
    let safe_capacity = num_textures.min(data.len() / 4);
    let mut defs = Vec::with_capacity(safe_capacity);

    for i in 0..num_textures {
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

        // Texture header is 22 bytes: name(8) + masked(4) + width(2) + height(2)
        //   + columndirectory(4) + patchcount(2)
        if tex_offset + 22 > data.len() {
            continue;
        }

        let mut name = [0u8; 8];
        name.copy_from_slice(&data[tex_offset..tex_offset + 8]);

        // masked is u32 at offset +8 (skip)
        let width = u16::from_le_bytes([data[tex_offset + 12], data[tex_offset + 13]]);
        let height = u16::from_le_bytes([data[tex_offset + 14], data[tex_offset + 15]]);
        // columndirectory at +16..+20 (skip)
        let patch_count =
            u16::from_le_bytes([data[tex_offset + 20], data[tex_offset + 21]]) as usize;

        let patches_start = tex_offset + 22;

        // Havoc 👺: Defend against OOM and huge allocations from fuzzed lengths.
        // Each patch is 10 bytes. The patch_count could be corrupted to 65535.
        // If data.len() is small, this would allocate too much. Limit the capacity.
        let max_patches_in_data = data.len().saturating_sub(patches_start) / 10;
        let safe_patch_capacity = patch_count.min(max_patches_in_data);

        let mut patches = Vec::with_capacity(safe_patch_capacity);
        for p in 0..patch_count {
            let poff = patches_start + p * 10;
            if poff + 10 > data.len() {
                break;
            }
            patches.push(PatchDef {
                origin_x: i16::from_le_bytes([data[poff], data[poff + 1]]),
                origin_y: i16::from_le_bytes([data[poff + 2], data[poff + 3]]),
                pname_index: u16::from_le_bytes([data[poff + 4], data[poff + 5]]),
                // stepdir [poff+6..poff+8] and colormap [poff+8..poff+10] unused
            });
        }

        defs.push(TextureDef {
            name,
            width,
            height,
            patches,
        });
    }

    defs
}

// ---------------------------------------------------------------------------
// Patch (picture format) parser
// ---------------------------------------------------------------------------

/// Parse a Doom picture-format patch lump into a `PatchImage`.
///
/// Returns `None` if the data is too short or the column offset table is
/// truncated.
pub fn parse_patch(data: &[u8]) -> Option<PatchImage> {
    if data.len() < 8 {
        return None;
    }

    let width = u16::from_le_bytes([data[0], data[1]]);
    let height = u16::from_le_bytes([data[2], data[3]]);
    let left_offset = i16::from_le_bytes([data[4], data[5]]);
    let top_offset = i16::from_le_bytes([data[6], data[7]]);

    let col_offsets_end = 8 + width as usize * 4;
    if col_offsets_end > data.len() {
        return None;
    }

    let mut columns = Vec::with_capacity(width as usize);

    for col in 0..width as usize {
        let off_idx = 8 + col * 4;
        let col_offset = u32::from_le_bytes([
            data[off_idx],
            data[off_idx + 1],
            data[off_idx + 2],
            data[off_idx + 3],
        ]) as usize;

        let mut posts = Vec::new();
        let mut pos = col_offset;

        loop {
            if pos >= data.len() {
                break;
            }
            let topdelta = data[pos];
            if topdelta == 0xFF {
                break;
            }
            pos += 1;

            if pos >= data.len() {
                break;
            }
            let length = data[pos] as usize;
            pos += 1;

            // Skip pre-pixel padding byte.
            pos += 1;

            let mut pixels = Vec::with_capacity(length);
            for _ in 0..length {
                if pos >= data.len() {
                    break;
                }
                pixels.push(data[pos]);
                pos += 1;
            }

            // Skip post-pixel padding byte.
            pos += 1;

            posts.push(PatchPost {
                y_offset: topdelta,
                pixels,
            });
        }

        columns.push(posts);
    }

    Some(PatchImage {
        width,
        height,
        left_offset,
        top_offset,
        columns,
    })
}

// ---------------------------------------------------------------------------
// Texture composition (R_GenerateComposite)
// ---------------------------------------------------------------------------

/// Compose a texture from its constituent patches.
///
/// For each patch in the texture definition:
/// 1. Look up the patch name from PNAMES via `pname_index`
/// 2. Call `get_patch_data` to retrieve the raw patch lump bytes
/// 3. Parse the patch lump data into a `PatchImage`
/// 4. Blit the patch pixels onto the texture canvas at `(origin_x, origin_y)`
///
/// Patches are applied in order; later patches overdraw earlier ones.
pub fn compose_texture(
    tex_def: &TextureDef,
    pnames: &[[u8; 8]],
    get_patch_data: &dyn Fn(&[u8; 8]) -> Option<Vec<u8>>,
) -> ComposedTexture {
    let w = tex_def.width as usize;
    let h = tex_def.height as usize;

    // Initialize all columns to palette index 0 (transparent/black).
    let mut columns: Vec<Vec<u8>> = (0..w).map(|_| vec![0u8; h]).collect();

    for patch_def in &tex_def.patches {
        let idx = patch_def.pname_index as usize;
        if idx >= pnames.len() {
            continue;
        }
        let pname = &pnames[idx];
        if let Some(patch_data) = get_patch_data(pname) {
            if let Some(patch_img) = parse_patch(&patch_data) {
                blit_patch(
                    &mut columns,
                    w,
                    h,
                    &patch_img,
                    patch_def.origin_x,
                    patch_def.origin_y,
                );
            }
        }
    }

    ComposedTexture {
        name: tex_def.name,
        width: tex_def.width,
        height: tex_def.height,
        columns,
    }
}

/// Blit a patch image onto the texture canvas at the given origin.
///
/// Pixels that fall outside the texture bounds are silently clipped.
fn blit_patch(
    columns: &mut [Vec<u8>],
    tex_width: usize,
    tex_height: usize,
    patch: &PatchImage,
    origin_x: i16,
    origin_y: i16,
) {
    for (col_idx, col_posts) in patch.columns.iter().enumerate() {
        let dest_x = origin_x as i32 + col_idx as i32;
        if dest_x < 0 || dest_x >= tex_width as i32 {
            continue;
        }

        for post in col_posts {
            for (i, &pixel) in post.pixels.iter().enumerate() {
                let dest_y = origin_y as i32 + post.y_offset as i32 + i as i32;
                if dest_y >= 0 && (dest_y as usize) < tex_height {
                    columns[dest_x as usize][dest_y as usize] = pixel;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TextureDirectory -- lookup by name
// ---------------------------------------------------------------------------

/// Directory of all wall textures, parsed from TEXTURE1/TEXTURE2 + PNAMES.
///
/// Provides O(1) lookup by 8-byte texture name and iteration over all
/// definitions.  Does **not** compose textures itself; call [`compose_texture`]
/// with a texture definition retrieved from this directory.
pub struct TextureDirectory {
    /// All texture definitions from TEXTURE1 and (optionally) TEXTURE2.
    textures: Vec<TextureDef>,
    /// Patch names from PNAMES.
    pnames: Vec<[u8; 8]>,
    /// Name -> index lookup.
    name_index: HashMap<[u8; 8], usize>,
}

impl TextureDirectory {
    /// Build the directory from raw TEXTURE1, optional TEXTURE2, and PNAMES
    /// lump data.
    pub fn new(texture1_data: &[u8], texture2_data: Option<&[u8]>, pnames_data: &[u8]) -> Self {
        let pnames = parse_pnames(pnames_data);
        let mut textures = parse_texture_lump(texture1_data);
        if let Some(t2) = texture2_data {
            textures.extend(parse_texture_lump(t2));
        }
        let name_index = textures
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name, i))
            .collect();
        Self {
            textures,
            pnames,
            name_index,
        }
    }

    /// Look up a texture definition by name.
    pub fn get(&self, name: &[u8; 8]) -> Option<&TextureDef> {
        self.name_index.get(name).map(|&i| &self.textures[i])
    }

    /// Number of textures in the directory.
    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// Returns `true` if the directory contains no textures.
    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// Provides access to the PNAMES array, which maps integer patch IDs to string WAD lump names for texture composition.
    pub fn pnames(&self) -> &[[u8; 8]] {
        &self.pnames
    }

    /// Iterate all texture definitions.
    pub fn iter(&self) -> impl Iterator<Item = &TextureDef> {
        self.textures.iter()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // =======================================================================
    // Helpers -- build binary lump data for testing
    // =======================================================================

    /// Build a PNAMES lump from a list of name strings.
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

    /// Build a TEXTURE1/TEXTURE2 lump from a list of texture specs.
    ///
    /// Each spec: `(name, width, height, &[(origin_x, origin_y, pname_index)])`.
    type PatchPlacement = (i16, i16, u16);
    type TextureSpec<'a> = (&'a str, u16, u16, &'a [PatchPlacement]);

    fn make_texture_lump(specs: &[TextureSpec<'_>]) -> Vec<u8> {
        let num = specs.len() as u32;
        // We'll build the offset table and texture data separately,
        // then concatenate.
        let offset_table_size = 4 + specs.len() * 4; // count + offsets
        let mut tex_data = Vec::new();
        let mut offsets = Vec::new();

        for &(name_str, width, height, patches) in specs {
            let offset = offset_table_size + tex_data.len();
            offsets.push(offset as u32);

            // name (8 bytes)
            let mut name_buf = [0u8; 8];
            for (i, &b) in name_str.as_bytes().iter().take(8).enumerate() {
                name_buf[i] = b.to_ascii_uppercase();
            }
            tex_data.extend_from_slice(&name_buf);
            // masked (u32)
            tex_data.extend_from_slice(&0u32.to_le_bytes());
            // width (u16)
            tex_data.extend_from_slice(&width.to_le_bytes());
            // height (u16)
            tex_data.extend_from_slice(&height.to_le_bytes());
            // columndirectory (u32)
            tex_data.extend_from_slice(&0u32.to_le_bytes());
            // patchcount (u16)
            tex_data.extend_from_slice(&(patches.len() as u16).to_le_bytes());

            for &(ox, oy, pidx) in patches {
                tex_data.extend_from_slice(&ox.to_le_bytes());
                tex_data.extend_from_slice(&oy.to_le_bytes());
                tex_data.extend_from_slice(&pidx.to_le_bytes());
                tex_data.extend_from_slice(&1u16.to_le_bytes()); // stepdir
                tex_data.extend_from_slice(&0u16.to_le_bytes()); // colormap
            }
        }

        let mut out = Vec::new();
        out.extend_from_slice(&num.to_le_bytes());
        for off in &offsets {
            out.extend_from_slice(&off.to_le_bytes());
        }
        out.extend_from_slice(&tex_data);
        out
    }

    /// Build a minimal picture-format patch: `width x height`, all pixels set
    /// to `colour`, single post per column starting at y=0.
    fn make_patch_data(width: u16, height: u16, colour: u8) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;

        let header_size = 8;
        let offsets_size = w * 4;
        let col_data_len = 1 + 1 + 1 + h + 1 + 1; // topdelta+len+pad+pixels+pad+0xFF
        let first_col_offset = header_size + offsets_size;

        let mut out = Vec::new();
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes()); // left_offset
        out.extend_from_slice(&0i16.to_le_bytes()); // top_offset

        for c in 0..w {
            let col_off = (first_col_offset + c * col_data_len) as u32;
            out.extend_from_slice(&col_off.to_le_bytes());
        }

        for _ in 0..w {
            out.push(0); // topdelta = 0
            out.push(h as u8); // length
            out.push(0); // pre-pixel padding
            for _ in 0..h {
                out.push(colour);
            }
            out.push(0); // post-pixel padding
            out.push(0xFF); // end of column
        }

        out
    }

    /// Build a patch with custom left/top offsets.
    fn make_patch_data_with_offsets(
        width: u16,
        height: u16,
        colour: u8,
        left_offset: i16,
        top_offset: i16,
    ) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;

        let header_size = 8;
        let offsets_size = w * 4;
        let col_data_len = 1 + 1 + 1 + h + 1 + 1;
        let first_col_offset = header_size + offsets_size;

        let mut out = Vec::new();
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&left_offset.to_le_bytes());
        out.extend_from_slice(&top_offset.to_le_bytes());

        for c in 0..w {
            let col_off = (first_col_offset + c * col_data_len) as u32;
            out.extend_from_slice(&col_off.to_le_bytes());
        }

        for _ in 0..w {
            out.push(0);
            out.push(h as u8);
            out.push(0);
            for _ in 0..h {
                out.push(colour);
            }
            out.push(0);
            out.push(0xFF);
        }

        out
    }

    /// Build a patch with multiple posts per column (transparent gaps).
    ///
    /// `posts_spec`: for each column, a list of `(y_offset, pixels)`.
    fn make_patch_with_posts(
        width: u16,
        height: u16,
        posts_spec: &[Vec<(u8, Vec<u8>)>],
    ) -> Vec<u8> {
        let w = width as usize;
        assert_eq!(posts_spec.len(), w);

        let header_size = 8usize;
        let offsets_size = w * 4;

        // First pass: compute column data bytes to get offsets.
        let mut col_blobs: Vec<Vec<u8>> = Vec::new();
        for col_posts in posts_spec {
            let mut blob = Vec::new();
            for (y_off, pixels) in col_posts {
                blob.push(*y_off);
                blob.push(pixels.len() as u8);
                blob.push(0); // pre-pad
                blob.extend_from_slice(pixels);
                blob.push(0); // post-pad
            }
            blob.push(0xFF); // end of column
            col_blobs.push(blob);
        }

        let mut out = Vec::new();
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes()); // left_offset
        out.extend_from_slice(&0i16.to_le_bytes()); // top_offset

        // Column offset table.
        let mut running_offset = header_size + offsets_size;
        for blob in &col_blobs {
            out.extend_from_slice(&(running_offset as u32).to_le_bytes());
            running_offset += blob.len();
        }

        // Column data.
        for blob in &col_blobs {
            out.extend_from_slice(blob);
        }

        out
    }

    /// Build a patch where each column has unique pixel values (column index
    /// as the pixel value for the first pixel, column index + 1 for the
    /// second, etc.).  This lets us verify per-pixel placement.
    fn make_patch_with_gradient(width: u16, height: u16) -> Vec<u8> {
        let w = width as usize;
        let h = height as usize;

        let header_size = 8;
        let offsets_size = w * 4;
        let col_data_len = 1 + 1 + 1 + h + 1 + 1;
        let first_col_offset = header_size + offsets_size;

        let mut out = Vec::new();
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());

        for c in 0..w {
            let col_off = (first_col_offset + c * col_data_len) as u32;
            out.extend_from_slice(&col_off.to_le_bytes());
        }

        for c in 0..w {
            out.push(0); // topdelta
            out.push(h as u8); // length
            out.push(0); // pre-pad
            for row in 0..h {
                out.push(((c * h + row) & 0xFF) as u8);
            }
            out.push(0); // post-pad
            out.push(0xFF);
        }

        out
    }

    /// Helper: name string to `[u8; 8]`.
    fn name8(s: &str) -> [u8; 8] {
        let mut buf = [0u8; 8];
        for (i, &b) in s.as_bytes().iter().take(8).enumerate() {
            buf[i] = b.to_ascii_uppercase();
        }
        buf
    }

    // =======================================================================
    // parse_pnames tests
    // =======================================================================

    #[test]
    fn pnames_parses_correct_count() {
        let data = make_pnames(&["WALL01", "WALL02", "WALL03"]);
        let names = parse_pnames(&data);
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn pnames_returns_correct_names() {
        let data = make_pnames(&["WALL01", "FLOOR4"]);
        let names = parse_pnames(&data);
        assert_eq!(names[0], name8("WALL01"));
        assert_eq!(names[1], name8("FLOOR4"));
    }

    #[test]
    fn pnames_empty_lump_returns_empty() {
        let names = parse_pnames(&[]);
        assert!(names.is_empty());
    }

    #[test]
    fn pnames_single_entry() {
        let data = make_pnames(&["DOOR2_1"]);
        let names = parse_pnames(&data);
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], name8("DOOR2_1"));
    }

    #[test]
    fn pnames_short_data_does_not_panic() {
        // Count says 5 entries but data only has 1.
        let mut data = Vec::new();
        data.extend_from_slice(&5u32.to_le_bytes());
        data.extend_from_slice(&[b'A'; 8]);
        let names = parse_pnames(&data);
        assert_eq!(names.len(), 1);
    }

    // =======================================================================
    // parse_texture_lump tests
    // =======================================================================

    #[test]
    fn texture_lump_single_one_patch() {
        let lump = make_texture_lump(&[("BRICK1", 64, 128, &[(0, 0, 0)])]);
        let defs = parse_texture_lump(&lump);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].name, name8("BRICK1"));
        assert_eq!(defs[0].width, 64);
        assert_eq!(defs[0].height, 128);
        assert_eq!(defs[0].patches.len(), 1);
        assert_eq!(defs[0].patches[0].origin_x, 0);
        assert_eq!(defs[0].patches[0].origin_y, 0);
        assert_eq!(defs[0].patches[0].pname_index, 0);
    }

    #[test]
    fn texture_lump_single_two_patches() {
        let lump = make_texture_lump(&[("BIGWALL", 128, 64, &[(0, 0, 0), (64, 0, 1)])]);
        let defs = parse_texture_lump(&lump);
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].patches.len(), 2);
        assert_eq!(defs[0].patches[1].origin_x, 64);
        assert_eq!(defs[0].patches[1].pname_index, 1);
    }

    #[test]
    fn texture_lump_multiple_textures() {
        let lump = make_texture_lump(&[
            ("TEX_A", 32, 32, &[(0, 0, 0)]),
            ("TEX_B", 64, 64, &[(0, 0, 1)]),
            ("TEX_C", 128, 128, &[(0, 0, 2), (64, 0, 3)]),
        ]);
        let defs = parse_texture_lump(&lump);
        assert_eq!(defs.len(), 3);
        assert_eq!(defs[0].name, name8("TEX_A"));
        assert_eq!(defs[1].name, name8("TEX_B"));
        assert_eq!(defs[2].name, name8("TEX_C"));
        assert_eq!(defs[2].patches.len(), 2);
    }

    #[test]
    fn texture_lump_empty_returns_empty() {
        let defs = parse_texture_lump(&[]);
        assert!(defs.is_empty());
    }

    #[test]
    fn texture_lump_correct_width_height() {
        let lump = make_texture_lump(&[("WIDE", 256, 72, &[(0, 0, 0)])]);
        let defs = parse_texture_lump(&lump);
        assert_eq!(defs[0].width, 256);
        assert_eq!(defs[0].height, 72);
    }

    #[test]
    fn texture_lump_correct_patch_origins() {
        let lump = make_texture_lump(&[("OFFSET", 64, 64, &[(-16, 8, 0)])]);
        let defs = parse_texture_lump(&lump);
        assert_eq!(defs[0].patches[0].origin_x, -16);
        assert_eq!(defs[0].patches[0].origin_y, 8);
    }

    // =======================================================================
    // parse_patch tests
    // =======================================================================

    #[test]
    fn patch_parses_single_column() {
        let data = make_patch_data(1, 4, 42);
        let img = parse_patch(&data).expect("should parse");
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 4);
        assert_eq!(img.columns.len(), 1);
        assert_eq!(img.columns[0].len(), 1); // one post
        assert_eq!(img.columns[0][0].pixels.len(), 4);
        assert!(img.columns[0][0].pixels.iter().all(|&p| p == 42));
    }

    #[test]
    fn patch_parses_multi_column() {
        let data = make_patch_data(4, 8, 77);
        let img = parse_patch(&data).expect("should parse");
        assert_eq!(img.width, 4);
        assert_eq!(img.columns.len(), 4);
        for col in &img.columns {
            assert_eq!(col[0].pixels.len(), 8);
            assert!(col[0].pixels.iter().all(|&p| p == 77));
        }
    }

    #[test]
    fn patch_handles_transparent_posts() {
        // Column 0: two posts with a gap between them.
        let posts = vec![vec![
            (0u8, vec![10, 11]),     // y=0..2
            (5u8, vec![20, 21, 22]), // y=5..8
        ]];
        let data = make_patch_with_posts(1, 8, &posts);
        let img = parse_patch(&data).expect("should parse");
        assert_eq!(img.columns[0].len(), 2);
        assert_eq!(img.columns[0][0].y_offset, 0);
        assert_eq!(img.columns[0][0].pixels, vec![10, 11]);
        assert_eq!(img.columns[0][1].y_offset, 5);
        assert_eq!(img.columns[0][1].pixels, vec![20, 21, 22]);
    }

    #[test]
    fn patch_handles_empty_column() {
        // Column with no posts (just 0xFF terminator).
        let posts = vec![vec![]]; // empty column
        let data = make_patch_with_posts(1, 8, &posts);
        let img = parse_patch(&data).expect("should parse");
        assert_eq!(img.columns[0].len(), 0);
    }

    #[test]
    fn patch_returns_none_on_short_data() {
        assert!(parse_patch(&[0, 0, 0]).is_none());
        assert!(parse_patch(&[]).is_none());
    }

    #[test]
    fn patch_correct_offsets() {
        let data = make_patch_data_with_offsets(4, 4, 1, -3, 7);
        let img = parse_patch(&data).expect("should parse");
        assert_eq!(img.left_offset, -3);
        assert_eq!(img.top_offset, 7);
    }

    #[test]
    fn patch_post_pixels_correct_indices() {
        let data = make_patch_with_gradient(3, 2);
        let img = parse_patch(&data).expect("should parse");
        // Column 0: pixels [0, 1], column 1: [2, 3], column 2: [4, 5]
        assert_eq!(img.columns[0][0].pixels, vec![0, 1]);
        assert_eq!(img.columns[1][0].pixels, vec![2, 3]);
        assert_eq!(img.columns[2][0].pixels, vec![4, 5]);
    }

    #[test]
    fn patch_returns_none_when_col_offsets_truncated() {
        // Width says 100 columns, but data is only 12 bytes.
        let mut data = Vec::new();
        data.extend_from_slice(&100u16.to_le_bytes()); // width
        data.extend_from_slice(&8u16.to_le_bytes()); // height
        data.extend_from_slice(&0i16.to_le_bytes()); // left
        data.extend_from_slice(&0i16.to_le_bytes()); // top
        assert!(parse_patch(&data).is_none());
    }

    // =======================================================================
    // compose_texture tests
    // =======================================================================

    #[test]
    fn compose_single_patch_matches_directly() {
        let pnames = vec![name8("PATCH0")];
        let patch_data = make_patch_data(4, 4, 55);
        let tex_def = TextureDef {
            name: name8("TEX1"),
            width: 4,
            height: 4,
            patches: vec![PatchDef {
                origin_x: 0,
                origin_y: 0,
                pname_index: 0,
            }],
        };

        let composed = compose_texture(&tex_def, &pnames, &|name| {
            if *name == name8("PATCH0") {
                Some(patch_data.clone())
            } else {
                None
            }
        });

        assert_eq!(composed.width, 4);
        assert_eq!(composed.height, 4);
        for col in &composed.columns {
            assert!(col.iter().all(|&p| p == 55));
        }
    }

    #[test]
    fn compose_multi_patch_later_overdraws() {
        let pnames = vec![name8("BASE"), name8("OVER")];
        let base_data = make_patch_data(4, 4, 10);
        let over_data = make_patch_data(4, 4, 20);

        let tex_def = TextureDef {
            name: name8("MULTI"),
            width: 4,
            height: 4,
            patches: vec![
                PatchDef {
                    origin_x: 0,
                    origin_y: 0,
                    pname_index: 0,
                },
                PatchDef {
                    origin_x: 0,
                    origin_y: 0,
                    pname_index: 1,
                },
            ],
        };

        let composed = compose_texture(&tex_def, &pnames, &|name| {
            if *name == name8("BASE") {
                Some(base_data.clone())
            } else if *name == name8("OVER") {
                Some(over_data.clone())
            } else {
                None
            }
        });

        // All pixels should be 20 (overwritten by second patch).
        for col in &composed.columns {
            assert!(col.iter().all(|&p| p == 20));
        }
    }

    #[test]
    fn compose_patch_offset_positions_correctly() {
        let pnames = vec![name8("SMALL")];
        // 2x2 patch placed at (2, 1) in a 4x4 texture.
        let patch_data = make_patch_data(2, 2, 99);
        let tex_def = TextureDef {
            name: name8("OFFSET"),
            width: 4,
            height: 4,
            patches: vec![PatchDef {
                origin_x: 2,
                origin_y: 1,
                pname_index: 0,
            }],
        };

        let composed = compose_texture(&tex_def, &pnames, &|_| Some(patch_data.clone()));

        // Columns 0,1 should be all zeros.
        assert!(composed.columns[0].iter().all(|&p| p == 0));
        assert!(composed.columns[1].iter().all(|&p| p == 0));
        // Column 2: row 0 = 0, rows 1-2 = 99, row 3 = 0.
        assert_eq!(composed.columns[2][0], 0);
        assert_eq!(composed.columns[2][1], 99);
        assert_eq!(composed.columns[2][2], 99);
        assert_eq!(composed.columns[2][3], 0);
        // Column 3: same pattern.
        assert_eq!(composed.columns[3][1], 99);
        assert_eq!(composed.columns[3][2], 99);
    }

    #[test]
    fn compose_negative_origin_clips_correctly() {
        let pnames = vec![name8("BIG")];
        // 4x4 patch at origin (-2, -1) in a 4x4 texture.
        let patch_data = make_patch_data(4, 4, 88);
        let tex_def = TextureDef {
            name: name8("NEGOFF"),
            width: 4,
            height: 4,
            patches: vec![PatchDef {
                origin_x: -2,
                origin_y: -1,
                pname_index: 0,
            }],
        };

        let composed = compose_texture(&tex_def, &pnames, &|_| Some(patch_data.clone()));

        // Columns 0,1 get patch columns 2,3 (since origin_x = -2).
        // Rows 0..3 get patch rows 1..4 (since origin_y = -1).
        assert_eq!(composed.columns[0][0], 88);
        assert_eq!(composed.columns[0][1], 88);
        assert_eq!(composed.columns[0][2], 88);
        assert_eq!(composed.columns[1][0], 88);
        // Columns 2,3 should be all zeros (patch ends at x=2).
        assert!(composed.columns[2].iter().all(|&p| p == 0));
        assert!(composed.columns[3].iter().all(|&p| p == 0));
    }

    #[test]
    fn compose_patch_extends_beyond_bounds_clips() {
        let pnames = vec![name8("WIDE")];
        // 4x4 patch at origin (2, 2) in a 4x4 texture -- right/bottom overflow.
        let patch_data = make_patch_data(4, 4, 66);
        let tex_def = TextureDef {
            name: name8("CLIP"),
            width: 4,
            height: 4,
            patches: vec![PatchDef {
                origin_x: 2,
                origin_y: 2,
                pname_index: 0,
            }],
        };

        let composed = compose_texture(&tex_def, &pnames, &|_| Some(patch_data.clone()));

        // Only columns 2,3 and rows 2,3 should have pixels.
        assert!(composed.columns[0].iter().all(|&p| p == 0));
        assert!(composed.columns[1].iter().all(|&p| p == 0));
        assert_eq!(composed.columns[2][0], 0);
        assert_eq!(composed.columns[2][1], 0);
        assert_eq!(composed.columns[2][2], 66);
        assert_eq!(composed.columns[2][3], 66);
        assert_eq!(composed.columns[3][2], 66);
        assert_eq!(composed.columns[3][3], 66);
    }

    #[test]
    fn compose_empty_texture_def_zero_filled() {
        let pnames = vec![name8("UNUSED")];
        let tex_def = TextureDef {
            name: name8("EMPTY"),
            width: 4,
            height: 4,
            patches: vec![],
        };

        let composed = compose_texture(&tex_def, &pnames, &|_| None);

        for col in &composed.columns {
            assert!(col.iter().all(|&p| p == 0));
        }
    }

    #[test]
    fn compose_column_data_correct_height() {
        let pnames = vec![name8("P")];
        let patch_data = make_patch_data(2, 16, 1);
        let tex_def = TextureDef {
            name: name8("TALL"),
            width: 2,
            height: 16,
            patches: vec![PatchDef {
                origin_x: 0,
                origin_y: 0,
                pname_index: 0,
            }],
        };

        let composed = compose_texture(&tex_def, &pnames, &|_| Some(patch_data.clone()));

        for col in &composed.columns {
            assert_eq!(col.len(), 16);
        }
    }

    // =======================================================================
    // blit_patch tests
    // =======================================================================

    #[test]
    fn blit_at_origin_copies_directly() {
        let data = make_patch_data(2, 2, 44);
        let img = parse_patch(&data).unwrap();
        let mut columns: Vec<Vec<u8>> = vec![vec![0; 2]; 2];
        blit_patch(&mut columns, 2, 2, &img, 0, 0);
        assert!(columns[0].iter().all(|&p| p == 44));
        assert!(columns[1].iter().all(|&p| p == 44));
    }

    #[test]
    fn blit_positive_offset_shifts_pixels() {
        let data = make_patch_data(1, 1, 77);
        let img = parse_patch(&data).unwrap();
        let mut columns: Vec<Vec<u8>> = vec![vec![0; 3]; 3];
        blit_patch(&mut columns, 3, 3, &img, 2, 1);
        assert_eq!(columns[2][1], 77);
        // Everything else is 0.
        assert_eq!(columns[0][0], 0);
        assert_eq!(columns[1][0], 0);
        assert_eq!(columns[2][0], 0);
        assert_eq!(columns[2][2], 0);
    }

    #[test]
    fn blit_negative_offset_clips_left_top() {
        let data = make_patch_data(3, 3, 55);
        let img = parse_patch(&data).unwrap();
        let mut columns: Vec<Vec<u8>> = vec![vec![0; 3]; 3];
        blit_patch(&mut columns, 3, 3, &img, -1, -1);
        // Patch columns 1,2 map to dest columns 0,1; rows 1,2 map to rows 0,1.
        assert_eq!(columns[0][0], 55);
        assert_eq!(columns[0][1], 55);
        assert_eq!(columns[1][0], 55);
        assert_eq!(columns[1][1], 55);
        // Column 2 untouched.
        assert!(columns[2].iter().all(|&p| p == 0));
    }

    #[test]
    fn blit_multiple_posts_transparent_gaps() {
        let posts = vec![vec![(0u8, vec![10, 11]), (5u8, vec![50, 51])]];
        let data = make_patch_with_posts(1, 8, &posts);
        let img = parse_patch(&data).unwrap();
        let mut columns: Vec<Vec<u8>> = vec![vec![0; 8]];
        blit_patch(&mut columns, 1, 8, &img, 0, 0);
        assert_eq!(columns[0][0], 10);
        assert_eq!(columns[0][1], 11);
        assert_eq!(columns[0][2], 0); // gap
        assert_eq!(columns[0][3], 0);
        assert_eq!(columns[0][4], 0);
        assert_eq!(columns[0][5], 50);
        assert_eq!(columns[0][6], 51);
        assert_eq!(columns[0][7], 0);
    }

    #[test]
    fn blit_patch_wider_than_texture_clips_right() {
        let data = make_patch_data(5, 2, 33);
        let img = parse_patch(&data).unwrap();
        let mut columns: Vec<Vec<u8>> = vec![vec![0; 2]; 3]; // texture is only 3 wide
        blit_patch(&mut columns, 3, 2, &img, 0, 0);
        // Only first 3 columns should be filled.
        assert!(columns[0].iter().all(|&p| p == 33));
        assert!(columns[1].iter().all(|&p| p == 33));
        assert!(columns[2].iter().all(|&p| p == 33));
    }

    // =======================================================================
    // TextureDirectory tests
    // =======================================================================

    #[test]
    fn directory_new_single_texture1() {
        let pnames_data = make_pnames(&["P1"]);
        let tex1_data = make_texture_lump(&[("WALL1", 64, 64, &[(0, 0, 0)])]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        assert_eq!(dir.len(), 1);
    }

    #[test]
    fn directory_new_texture1_plus_texture2() {
        let pnames_data = make_pnames(&["P1", "P2"]);
        let tex1_data = make_texture_lump(&[("WALL1", 64, 64, &[(0, 0, 0)])]);
        let tex2_data = make_texture_lump(&[("WALL2", 32, 128, &[(0, 0, 1)])]);
        let dir = TextureDirectory::new(&tex1_data, Some(&tex2_data), &pnames_data);
        assert_eq!(dir.len(), 2);
    }

    #[test]
    fn directory_get_finds_by_name() {
        let pnames_data = make_pnames(&["P1"]);
        let tex1_data = make_texture_lump(&[("BRICK1", 64, 128, &[(0, 0, 0)])]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        let tex = dir.get(&name8("BRICK1"));
        assert!(tex.is_some());
        assert_eq!(tex.unwrap().width, 64);
        assert_eq!(tex.unwrap().height, 128);
    }

    #[test]
    fn directory_get_returns_none_for_unknown() {
        let pnames_data = make_pnames(&["P1"]);
        let tex1_data = make_texture_lump(&[("KNOWN", 8, 8, &[(0, 0, 0)])]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        assert!(dir.get(&name8("UNKNOWN")).is_none());
    }

    #[test]
    fn directory_len_correct() {
        let pnames_data = make_pnames(&["P1", "P2"]);
        let tex1_data = make_texture_lump(&[
            ("A", 8, 8, &[(0, 0, 0)]),
            ("B", 8, 8, &[(0, 0, 1)]),
            ("C", 8, 8, &[(0, 0, 0)]),
        ]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        assert_eq!(dir.len(), 3);
    }

    #[test]
    fn directory_iter_yields_all() {
        let pnames_data = make_pnames(&["P1"]);
        let tex1_data =
            make_texture_lump(&[("X", 8, 8, &[(0, 0, 0)]), ("Y", 16, 16, &[(0, 0, 0)])]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        let collected: Vec<_> = dir.iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn directory_is_empty_on_empty() {
        let pnames_data = make_pnames(&[]);
        let tex1_data = make_texture_lump(&[]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        assert!(dir.is_empty());
        assert_eq!(dir.len(), 0);
    }

    #[test]
    fn directory_name_preserves_case() {
        let pnames_data = make_pnames(&["P1"]);
        // Name stored uppercase in WAD by our helper.
        let tex1_data = make_texture_lump(&[("MYWALL", 8, 8, &[(0, 0, 0)])]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        // Should find with the same uppercase representation.
        assert!(dir.get(&name8("MYWALL")).is_some());
    }

    #[test]
    fn directory_pnames_accessor() {
        let pnames_data = make_pnames(&["WALL01", "WALL02"]);
        let tex1_data = make_texture_lump(&[]);
        let dir = TextureDirectory::new(&tex1_data, None, &pnames_data);
        assert_eq!(dir.pnames().len(), 2);
        assert_eq!(dir.pnames()[0], name8("WALL01"));
    }

    // =======================================================================
    // ComposedTexture structural tests
    // =======================================================================

    #[test]
    fn composed_columns_len_equals_width() {
        let pnames = vec![name8("P")];
        let tex_def = TextureDef {
            name: name8("T"),
            width: 7,
            height: 3,
            patches: vec![],
        };
        let composed = compose_texture(&tex_def, &pnames, &|_| None);
        assert_eq!(composed.columns.len(), 7);
    }

    #[test]
    fn composed_each_column_len_equals_height() {
        let pnames = vec![name8("P")];
        let tex_def = TextureDef {
            name: name8("T"),
            width: 5,
            height: 13,
            patches: vec![],
        };
        let composed = compose_texture(&tex_def, &pnames, &|_| None);
        for col in &composed.columns {
            assert_eq!(col.len(), 13);
        }
    }

    #[test]
    fn composed_column_access_gives_palette_index() {
        let pnames = vec![name8("GRAD")];
        let patch_data = make_patch_with_gradient(3, 2);
        let tex_def = TextureDef {
            name: name8("GRADIENT"),
            width: 3,
            height: 2,
            patches: vec![PatchDef {
                origin_x: 0,
                origin_y: 0,
                pname_index: 0,
            }],
        };
        let composed = compose_texture(&tex_def, &pnames, &|_| Some(patch_data.clone()));
        // Verify individual palette indices.
        assert_eq!(composed.columns[0][0], 0);
        assert_eq!(composed.columns[0][1], 1);
        assert_eq!(composed.columns[1][0], 2);
        assert_eq!(composed.columns[1][1], 3);
        assert_eq!(composed.columns[2][0], 4);
        assert_eq!(composed.columns[2][1], 5);
    }

    // =======================================================================
    // Edge cases and integration
    // =======================================================================

    #[test]
    fn compose_missing_patch_data_leaves_zeros() {
        let pnames = vec![name8("MISSING")];
        let tex_def = TextureDef {
            name: name8("HOLES"),
            width: 2,
            height: 2,
            patches: vec![PatchDef {
                origin_x: 0,
                origin_y: 0,
                pname_index: 0,
            }],
        };
        // get_patch_data always returns None.
        let composed = compose_texture(&tex_def, &pnames, &|_| None);
        for col in &composed.columns {
            assert!(col.iter().all(|&p| p == 0));
        }
    }

    #[test]
    fn compose_out_of_range_pname_index_skipped() {
        let pnames = vec![name8("ONLY")];
        let tex_def = TextureDef {
            name: name8("BAD"),
            width: 2,
            height: 2,
            patches: vec![PatchDef {
                origin_x: 0,
                origin_y: 0,
                pname_index: 99, // out of range
            }],
        };
        // Should not panic.
        let composed = compose_texture(&tex_def, &pnames, &|_| None);
        assert_eq!(composed.columns.len(), 2);
    }

    #[test]
    fn compose_two_patches_side_by_side() {
        let pnames = vec![name8("LEFT"), name8("RIGHT")];
        let left_data = make_patch_data(2, 4, 11);
        let right_data = make_patch_data(2, 4, 22);
        let tex_def = TextureDef {
            name: name8("SIDE"),
            width: 4,
            height: 4,
            patches: vec![
                PatchDef {
                    origin_x: 0,
                    origin_y: 0,
                    pname_index: 0,
                },
                PatchDef {
                    origin_x: 2,
                    origin_y: 0,
                    pname_index: 1,
                },
            ],
        };

        let composed = compose_texture(&tex_def, &pnames, &|name| {
            if *name == name8("LEFT") {
                Some(left_data.clone())
            } else if *name == name8("RIGHT") {
                Some(right_data.clone())
            } else {
                None
            }
        });

        assert!(composed.columns[0].iter().all(|&p| p == 11));
        assert!(composed.columns[1].iter().all(|&p| p == 11));
        assert!(composed.columns[2].iter().all(|&p| p == 22));
        assert!(composed.columns[3].iter().all(|&p| p == 22));
    }

    #[test]
    fn compose_partial_overlap_second_wins() {
        let pnames = vec![name8("A"), name8("B")];
        let a_data = make_patch_data(4, 4, 10);
        let b_data = make_patch_data(4, 4, 20);
        let tex_def = TextureDef {
            name: name8("OVERLAP"),
            width: 6,
            height: 4,
            patches: vec![
                PatchDef {
                    origin_x: 0,
                    origin_y: 0,
                    pname_index: 0,
                },
                PatchDef {
                    origin_x: 2,
                    origin_y: 0,
                    pname_index: 1,
                },
            ],
        };

        let composed = compose_texture(&tex_def, &pnames, &|name| {
            if *name == name8("A") {
                Some(a_data.clone())
            } else if *name == name8("B") {
                Some(b_data.clone())
            } else {
                None
            }
        });

        // Columns 0,1: only A = 10
        assert!(composed.columns[0].iter().all(|&p| p == 10));
        assert!(composed.columns[1].iter().all(|&p| p == 10));
        // Columns 2,3: overlap zone, B overwrites A = 20
        assert!(composed.columns[2].iter().all(|&p| p == 20));
        assert!(composed.columns[3].iter().all(|&p| p == 20));
        // Columns 4,5: only B = 20
        assert!(composed.columns[4].iter().all(|&p| p == 20));
        assert!(composed.columns[5].iter().all(|&p| p == 20));
    }

    #[test]
    fn directory_texture2_overrides_texture1_on_duplicate_name() {
        let pnames_data = make_pnames(&["P1"]);
        // Both lumps define "DUP" but with different sizes.
        let tex1_data = make_texture_lump(&[("DUP", 32, 32, &[(0, 0, 0)])]);
        let tex2_data = make_texture_lump(&[("DUP", 64, 64, &[(0, 0, 0)])]);
        let dir = TextureDirectory::new(&tex1_data, Some(&tex2_data), &pnames_data);
        // TEXTURE2 entry comes second, so it overwrites in the HashMap.
        let tex = dir.get(&name8("DUP")).unwrap();
        assert_eq!(tex.width, 64);
    }

    #[test]
    fn parse_pnames_three_byte_lump_returns_empty() {
        // 3 bytes is too short even for the count field.
        let names = parse_pnames(&[0, 0, 0]);
        assert!(names.is_empty());
    }

    #[test]
    fn texture_lump_zero_count() {
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_le_bytes());
        let defs = parse_texture_lump(&data);
        assert!(defs.is_empty());
    }
}
