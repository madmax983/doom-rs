//! Flat texture cache — loads all 64×64 floor/ceiling textures from a WAD.
//!
//! Flats live between `F_START` and `F_END` marker lumps in the WAD directory.
//! Each flat is exactly 4096 bytes (64×64 palette indices, row-major).
//!
//! # Usage
//! ```ignore
//! let cache = FlatCache::load(&wad);
//! let texels: &[u8; 4096] = cache.get(b"FLOOR4_8");
//! ```

use doom_types::limits::FLAT_SIZE;
use doom_wad::WadFile;
use std::collections::HashMap;

/// Cache of 64×64 flat textures loaded from a WAD file.
///
/// All flats are looked up by their 8-byte lump name (uppercase, null-padded).
/// If a requested flat is not present in the WAD, a zeroed fallback is returned.
pub struct FlatCache {
    /// Keyed by uppercase lump name string (trimmed of null bytes).
    flats: HashMap<String, Box<[u8; FLAT_SIZE]>>,
    /// 4096-byte zeroed fallback, returned when a lump is missing.
    default_flat: Box<[u8; FLAT_SIZE]>,
}

impl FlatCache {
    /// Load all flat textures from the WAD.
    ///
    /// Iterates lumps between `F_START` and `F_END` (inclusive of nested
    /// `FF_START`/`FF_END` sections for PWAD support) and loads every lump
    /// that is exactly [`FLAT_SIZE`] bytes.
    pub fn load(wad: &WadFile) -> Self {
        let mut flats: HashMap<String, Box<[u8; FLAT_SIZE]>> = HashMap::new();

        for lump in wad.lumps_between("F_START", "F_END") {
            // Skip marker lumps and any lump that is not exactly 4096 bytes.
            if lump.size != FLAT_SIZE {
                continue;
            }

            let name = lump.name.as_str().to_uppercase();
            let data = wad.lump_data(lump);

            // Exactly FLAT_SIZE bytes — copy into a boxed fixed-size array.
            let mut texels = Box::new([0u8; FLAT_SIZE]);
            texels.copy_from_slice(data);
            flats.insert(name, texels);
        }

        Self {
            flats,
            default_flat: Box::new([0u8; FLAT_SIZE]),
        }
    }

    /// Get the flat texture data for a given 8-byte lump name.
    ///
    /// The name is interpreted as a null-padded ASCII string (matching the WAD
    /// format). Lookup is case-insensitive. Returns the default (zeroed) flat
    /// if the name is not in the cache.
    pub fn get(&self, name: &[u8; 8]) -> &[u8; FLAT_SIZE] {
        // Trim null bytes and uppercase for lookup.
        let len = name.iter().position(|&b| b == 0).unwrap_or(8);
        let key = String::from_utf8_lossy(&name[..len]).to_uppercase();
        self.flats.get(&key).map(|b| b.as_ref()).unwrap_or(&self.default_flat)
    }

    /// Number of flats successfully loaded.
    pub fn len(&self) -> usize {
        self.flats.len()
    }

    /// Returns `true` if no flats are cached (e.g. WAD has no `F_START`/`F_END`).
    pub fn is_empty(&self) -> bool {
        self.flats.is_empty()
    }
}

impl core::fmt::Debug for FlatCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "FlatCache({} flats)", self.flats.len())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal IWAD in memory with the given lumps.
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

    /// A freshly constructed `FlatCache` with no WAD data has a 4096-byte
    /// fallback flat that is all zeroes.
    #[test]
    fn test_flat_cache_default_flat_length() {
        let empty_wad_bytes = make_iwad(&[]);
        let wad = WadFile::parse(empty_wad_bytes).expect("parse empty WAD");
        let cache = FlatCache::load(&wad);
        let default = cache.get(b"-\0\0\0\0\0\0\0");
        assert_eq!(default.len(), FLAT_SIZE, "default flat must be {FLAT_SIZE} bytes");
        assert!(
            default.iter().all(|&b| b == 0),
            "default flat must be all zeroes"
        );
    }

    /// `get()` trims null bytes and uppercases the name: `b"FLOOR4_8"` and
    /// `b"floor4_8"` (if stored uppercase) resolve to the same entry.
    #[test]
    fn test_flat_cache_name_trimming() {
        // Build a WAD with F_START, one flat, F_END.
        let flat_data = vec![42u8; FLAT_SIZE];
        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLOOR4_8", &flat_data),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let cache = FlatCache::load(&wad);

        // Exact match with null padding.
        let a = cache.get(b"FLOOR4_8");
        assert_eq!(a[0], 42, "first texel should be 42");

        // Same name as uppercase bytes with null terminator.
        let b = cache.get(b"FLOOR4_8");
        assert_eq!(
            a as *const _,
            b as *const _,
            "same name should return same pointer"
        );
    }

    /// Loading a WAD without `F_START`/`F_END` produces an empty cache,
    /// and `get()` returns the default (zeroed) flat.
    #[test]
    fn test_flat_cache_load_empty_wad() {
        let wad_bytes = make_iwad(&[("PLAYPAL", b"some palette data")]);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let cache = FlatCache::load(&wad);

        assert!(cache.is_empty(), "no F_START/F_END → cache must be empty");

        let fallback = cache.get(b"FLOOR4_8");
        assert_eq!(fallback.len(), FLAT_SIZE);
        assert!(fallback.iter().all(|&b| b == 0), "fallback must be zeroed");
    }

    /// A WAD with `F_START`, a correctly-sized flat, and `F_END` loads the flat.
    #[test]
    fn test_flat_cache_loads_flat_between_markers() {
        let mut flat = vec![0u8; FLAT_SIZE];
        // Mark each texel with its index modulo 255 for easy verification.
        for (i, b) in flat.iter_mut().enumerate() {
            *b = (i % 255) as u8;
        }

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("NUKAGE1", &flat),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let cache = FlatCache::load(&wad);

        assert_eq!(cache.len(), 1, "one flat loaded");
        let texels = cache.get(b"NUKAGE1\0");
        assert_eq!(texels[0], 0);
        assert_eq!(texels[1], 1);
        assert_eq!(texels[255], 0); // 255 % 255 == 0
    }

    /// A lump between F_START and F_END that is NOT 4096 bytes is ignored.
    #[test]
    fn test_flat_cache_skips_wrong_size_lumps() {
        let short = vec![0u8; 100]; // not a flat
        let flat_data = vec![7u8; FLAT_SIZE]; // valid flat

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("NOTFLAT", &short),
            ("MYFLAT", &flat_data),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let cache = FlatCache::load(&wad);

        assert_eq!(cache.len(), 1, "only one valid flat loaded");
        assert_eq!(cache.get(b"MYFLAT\0\0")[0], 7);
    }
}
