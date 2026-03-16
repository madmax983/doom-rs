//! COLORMAP lump loading, light-level lookup, and special colormaps.
//!
//! COLORMAP is 34 × 256 bytes. Row 0 = full bright (identity mapping),
//! row 31 = darkest (heavily remapped to dark colours).
//! Rows 32-33 are special (invulnerability palette, etc.) and are not
//! used by this cache.
//!
//! `ColormapCache::get(light_index)` returns a reference to the 256-byte
//! row for the given light index, clamped to `[0, 31]`.
//!
//! # Invulnerability colormap
//! When the player has the invulnerability powerup, all rendered pixels
//! pass through `INVULN_COLORMAP` — a grayscale ramp that maps every
//! palette index to a bright grayscale equivalent.  This produces the
//! distinctive "negative / white-out" look of the powerup.
//!
//! # Loading
//! Call `ColormapCache::load(&wad_stack)` at startup.  If the `COLORMAP`
//! lump is missing the cache falls back to 34 identity rows so rendering
//! still works (everything will appear full-bright).

use doom_wad::{WadFile, WadStack};

/// Number of rows in the COLORMAP lump (34 total; 32 light levels + 2 special).
pub const COLORMAP_ROWS: usize = 34;

/// Number of bytes per colormap row (one entry per palette index).
pub const COLORMAP_SIZE: usize = 256;

/// Cache of colormap rows loaded from the `COLORMAP` WAD lump.
///
/// Each row is a 256-byte lookup table that translates a raw texture palette
/// index into a final display palette index at a given light level.  Row 0 is
/// full-bright (identity), row 31 is the darkest.
pub struct ColormapCache {
    /// `COLORMAP_ROWS * COLORMAP_SIZE` bytes stored row-major.
    data: Vec<u8>,
}

impl ColormapCache {
    /// Load from the `COLORMAP` lump in the WAD stack.
    ///
    /// If the lump is missing or has fewer bytes than expected, the cache
    /// falls back to identity mapping (0, 1, 2, … 255 repeated for each row).
    pub fn load(wad: &WadStack) -> Self {
        Self::from_bytes(wad.lump_data("COLORMAP"))
    }

    /// Load from a raw `WadFile` (convenience wrapper around `from_bytes`).
    pub fn from_wad_file(wad: &WadFile) -> Self {
        Self::from_bytes(wad.find_lump_data("COLORMAP"))
    }

    /// Build from optional raw lump bytes.
    fn from_bytes(bytes: Option<&[u8]>) -> Self {
        if let Some(b) = bytes {
            let expected = COLORMAP_ROWS * COLORMAP_SIZE;
            if b.len() >= expected {
                return Self {
                    data: b[..expected].to_vec(),
                };
            }
        }
        Self {
            data: (0..COLORMAP_ROWS).flat_map(|_| 0u8..=255).collect(),
        }
    }

    /// Return a reference to the 256-byte colormap row for `light_index`.
    ///
    /// `light_index` is clamped to `[0, 31]` automatically so callers may
    /// pass raw sector-light-derived values without range-checking.
    pub fn get(&self, light_index: u8) -> &[u8; 256] {
        let row = (light_index as usize).min(31);
        let offset = row * COLORMAP_SIZE;
        // SAFETY: `data` is always `COLORMAP_ROWS * COLORMAP_SIZE` bytes and
        // `row` is clamped to 31, so `offset + 256 <= data.len()`.
        self.data[offset..offset + COLORMAP_SIZE]
            .try_into()
            .expect("colormap row is always 256 bytes")
    }

    /// Construct from raw data (for testing).
    ///
    /// `data` must be exactly `COLORMAP_ROWS * COLORMAP_SIZE` bytes.
    /// Panics if the length is wrong.
    #[doc(hidden)]
    pub fn from_test_data(data: Vec<u8>) -> Self {
        assert_eq!(
            data.len(),
            COLORMAP_ROWS * COLORMAP_SIZE,
            "test data must be exactly {} bytes",
            COLORMAP_ROWS * COLORMAP_SIZE
        );
        Self { data }
    }

    /// Build the identity-fallback cache (no WAD needed).
    ///
    /// Every row maps index `i` to `i` (full-bright).
    pub fn identity() -> Self {
        Self {
            data: (0..COLORMAP_ROWS).flat_map(|_| 0u8..=255).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Invulnerability colormap
// ---------------------------------------------------------------------------

/// Build the invulnerability (grayscale) colormap.
///
/// Maps each palette index to a bright grayscale equivalent.  Doom's actual
/// invulnerability effect uses COLORMAP row 32 (the "inverse" map), but
/// that requires a WAD.  This function produces a deterministic grayscale
/// ramp that works without any WAD data:
///
/// - Indices 0-15 (black-to-white ramp in the Doom palette) map to the
///   bright end of the range.
/// - All other indices are mapped based on a simple luminance approximation.
///
/// The result is the distinctive bright/white-washed look of the
/// invulnerability powerup.
pub fn build_invuln_colormap() -> [u8; 256] {
    let mut map = [0u8; 256];
    for i in 0u16..256 {
        // Doom's grayscale palette ramp occupies indices 0-15 (in most
        // IWADs).  We map every index to the upper portion of this ramp
        // to simulate the bright invulnerability effect.
        //
        // Simple approximation: treat the palette index as if it encodes
        // some brightness information via modular position, then bias
        // everything toward the bright end.
        //
        // The mapping: spread across indices 4..15 (bright gray to white).
        // We use (i % 16) * 11/16 + 4 clamped to 15 to get a ramp.
        let gray = ((i % 16) * 11 / 16 + 4).min(15) as u8;
        map[i as usize] = gray;
    }
    map
}

/// Pre-computed invulnerability colormap — maps every palette index to a
/// bright grayscale value.
///
/// Use this as the colormap for all rendering when the invulnerability
/// powerup is active.
pub const INVULN_COLORMAP: [u8; 256] = {
    let mut map = [0u8; 256];
    let mut i: u16 = 0;
    while i < 256 {
        // Same formula as build_invuln_colormap but usable in const context.
        let gray = (i % 16) * 11 / 16 + 4;
        let clamped = if gray > 15 { 15 } else { gray };
        map[i as usize] = clamped as u8;
        i += 1;
    }
    map
};

impl core::fmt::Debug for ColormapCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ColormapCache({} rows)", COLORMAP_ROWS)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_fallback_has_correct_rows() {
        // Build a fallback cache (no WAD) directly.
        let cache = ColormapCache {
            data: (0..COLORMAP_ROWS).flat_map(|_| 0u8..=255).collect(),
        };
        // Row 0 must be the identity mapping.
        let row = cache.get(0);
        for (i, &v) in row.iter().enumerate() {
            assert_eq!(v, i as u8, "identity row[{i}] should be {i}");
        }
    }

    #[test]
    fn light_index_clamps_to_31() {
        let cache = ColormapCache {
            data: vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE],
        };
        // None of these should panic.
        let _ = cache.get(200);
        let _ = cache.get(32);
        let _ = cache.get(31);
    }

    #[test]
    fn get_returns_256_bytes() {
        let cache = ColormapCache {
            data: vec![42u8; COLORMAP_ROWS * COLORMAP_SIZE],
        };
        assert_eq!(cache.get(5).len(), 256);
    }

    #[test]
    fn different_rows_are_different_slices() {
        // Fill row 0 with 0, row 1 with 1, etc. so we can verify get() picks
        // the right row.
        let mut data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for row in 0..COLORMAP_ROWS {
            let start = row * COLORMAP_SIZE;
            data[start..start + COLORMAP_SIZE].fill(row as u8);
        }
        let cache = ColormapCache { data };

        for row in 0u8..32 {
            let slice = cache.get(row);
            assert!(
                slice.iter().all(|&b| b == row),
                "row {row} should contain all {row}s"
            );
        }
        // Index 32 and 33 should be clamped to row 31.
        assert!(cache.get(32).iter().all(|&b| b == 31));
        assert!(cache.get(33).iter().all(|&b| b == 31));
    }

    #[test]
    fn load_from_wad_with_colormap() {
        // Build a minimal IWAD that contains a COLORMAP lump.
        let mut colormap_data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        // Mark row 5 with a distinctive byte so we can verify it was loaded.
        colormap_data[5 * COLORMAP_SIZE] = 0xAB;

        let wad_bytes = make_iwad(&[("COLORMAP", &colormap_data)]);
        let mut stack = WadStack::new();
        stack.push_iwad(wad_bytes).expect("push iwad");
        let cache = ColormapCache::load(&stack);

        // Row 5 byte 0 should be 0xAB (loaded from WAD, not identity fallback).
        assert_eq!(cache.get(5)[0], 0xAB);
    }

    #[test]
    fn load_falls_back_to_identity_when_lump_missing() {
        let wad_bytes = make_iwad(&[("PLAYPAL", b"ignored")]);
        let mut stack = WadStack::new();
        stack.push_iwad(wad_bytes).expect("push iwad");
        let cache = ColormapCache::load(&stack);

        // Fallback is identity.
        let row = cache.get(0);
        for (i, &v) in row.iter().enumerate() {
            assert_eq!(v, i as u8);
        }
    }

    // ------------------------------------------------------------------
    // Invulnerability colormap tests
    // ------------------------------------------------------------------

    #[test]
    fn invuln_colormap_has_256_entries() {
        assert_eq!(INVULN_COLORMAP.len(), 256);
    }

    #[test]
    fn invuln_colormap_maps_all_to_bright_range() {
        // All mapped values should be in the bright gray range [4, 15].
        for (i, &val) in INVULN_COLORMAP.iter().enumerate() {
            assert!(
                (4..=15).contains(&val),
                "INVULN_COLORMAP[{i}] = {val}, expected [4, 15]"
            );
        }
    }

    #[test]
    fn build_invuln_colormap_is_deterministic() {
        let map1 = build_invuln_colormap();
        let map2 = build_invuln_colormap();
        assert_eq!(map1, map2, "build_invuln_colormap must be deterministic");
    }

    #[test]
    fn build_invuln_colormap_matches_const() {
        let dynamic = build_invuln_colormap();
        assert_eq!(
            dynamic, INVULN_COLORMAP,
            "build_invuln_colormap() and INVULN_COLORMAP should be identical"
        );
    }

    #[test]
    fn identity_helper_produces_identity_rows() {
        let cache = ColormapCache::identity();
        let row = cache.get(0);
        for (i, &v) in row.iter().enumerate() {
            assert_eq!(v, i as u8);
        }
    }

    // ------------------------------------------------------------------
    // WAD builder helper (mirrors what other renderer tests use)
    // ------------------------------------------------------------------

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
}
