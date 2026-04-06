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
use doom_types::CompatibilityProfile;
use doom_wad::{LumpDef, WadFile, WadStack};
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
            Self::insert_flat(&mut flats, wad, lump);
        }

        Self {
            flats,
            default_flat: Box::new([0u8; FLAT_SIZE]),
        }
    }

    /// Load all flat textures from a WAD stack using last-loaded override semantics.
    pub fn load_from_stack(wad_stack: &WadStack) -> Self {
        Self::load_from_stack_extended(wad_stack)
    }

    /// Load flats with extended stacked-marker support (`FF_START`/`FF_END`).
    pub fn load_from_stack_extended(wad_stack: &WadStack) -> Self {
        Self::load_from_stack_impl(wad_stack, true)
    }

    /// Load flats using only vanilla-style `F_START`/`F_END` sections.
    pub fn load_from_stack_strict(wad_stack: &WadStack) -> Self {
        Self::load_from_stack_impl(wad_stack, false)
    }

    /// Load all flat textures from a WAD stack with an explicit compatibility
    /// profile.
    pub fn load_from_stack_with_profile(
        wad_stack: &WadStack,
        compat: CompatibilityProfile,
    ) -> Self {
        match compat {
            CompatibilityProfile::Extended => Self::load_from_stack_extended(wad_stack),
            CompatibilityProfile::VanillaStrict => Self::load_from_stack_strict(wad_stack),
        }
    }

    fn load_from_stack_impl(wad_stack: &WadStack, allow_ff_markers: bool) -> Self {
        let mut flats: HashMap<String, Box<[u8; FLAT_SIZE]>> = HashMap::new();
        let mut in_flat_section = false;

        for (wad, lump) in wad_stack.all_lumps() {
            match lump.name.as_str() {
                "F_START" => {
                    in_flat_section = true;
                    continue;
                }
                "F_END" => {
                    in_flat_section = false;
                    continue;
                }
                "FF_START" if allow_ff_markers => {
                    in_flat_section = true;
                    continue;
                }
                "FF_END" if allow_ff_markers => {
                    in_flat_section = false;
                    continue;
                }
                _ => {}
            }

            if !in_flat_section {
                continue;
            }

            Self::insert_flat(&mut flats, wad, lump);
        }

        Self {
            flats,
            default_flat: Box::new([0u8; FLAT_SIZE]),
        }
    }

    fn insert_flat(
        flats: &mut HashMap<String, Box<[u8; FLAT_SIZE]>>,
        wad: &WadFile,
        lump: &LumpDef,
    ) {
        if lump.size != FLAT_SIZE {
            return;
        }

        let name = lump.name.as_str().to_uppercase();
        let data = wad.lump_data(lump);

        let mut texels = Box::new([0u8; FLAT_SIZE]);
        texels.copy_from_slice(data);
        flats.insert(name, texels);
    }

    /// Get the flat texture data for a given 8-byte lump name.
    ///
    /// The name is interpreted as a null-padded ASCII string (matching the WAD
    /// format). Lookup is case-insensitive. Returns the default (zeroed) flat
    /// if the name is not in the cache.
    pub fn get(&self, name: &[u8; 8]) -> &[u8; FLAT_SIZE] {
        // Trim trailing NUL/space padding and uppercase for lookup.
        let len = name
            .iter()
            .rposition(|&b| b != 0 && b != b' ')
            .map_or(0, |i| i + 1);
        let key = String::from_utf8_lossy(&name[..len]).to_uppercase();
        self.flats
            .get(&key)
            .map(|b| b.as_ref())
            .unwrap_or(&self.default_flat)
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

    fn make_wad(kind: &[u8; 4], lumps: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(kind);
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

    /// Build a minimal IWAD in memory with the given lumps.
    fn make_iwad(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        make_wad(b"IWAD", lumps)
    }

    /// A freshly constructed `FlatCache` with no WAD data has a 4096-byte
    /// fallback flat that is all zeroes.
    #[test]
    fn test_flat_cache_default_flat_length() {
        let empty_wad_bytes = make_iwad(&[]);
        let wad = WadFile::parse(empty_wad_bytes).expect("parse empty WAD");
        let cache = FlatCache::load(&wad);
        let default = cache.get(b"-\0\0\0\0\0\0\0");
        assert_eq!(
            default.len(),
            FLAT_SIZE,
            "default flat must be {FLAT_SIZE} bytes"
        );
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
        let lumps: Vec<(&str, &[u8])> =
            vec![("F_START", b""), ("FLOOR4_8", &flat_data), ("F_END", b"")];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let cache = FlatCache::load(&wad);

        // Exact match with null padding.
        let a = cache.get(b"FLOOR4_8");
        assert_eq!(a[0], 42, "first texel should be 42");

        // Same name as uppercase bytes with null terminator.
        let b = cache.get(b"FLOOR4_8");
        assert_eq!(
            a as *const _, b as *const _,
            "same name should return same pointer"
        );
    }

    #[test]
    fn test_flat_cache_space_padded_lookup() {
        let flat_data = vec![99u8; FLAT_SIZE];
        let lumps: Vec<(&str, &[u8])> =
            vec![("F_START", b""), ("LAVA", &flat_data), ("F_END", b"")];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let cache = FlatCache::load(&wad);

        let texels = cache.get(b"LAVA    ");
        assert_eq!(texels[0], 99, "space-padded name should resolve");
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

        let lumps: Vec<(&str, &[u8])> = vec![("F_START", b""), ("NUKAGE1", &flat), ("F_END", b"")];
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

    #[test]
    fn test_flat_cache_stack_prefers_pwad_flat() {
        let iwad_bytes = make_iwad(&[
            ("F_START", b""),
            ("NUKAGE1", &vec![1u8; FLAT_SIZE]),
            ("F_END", b""),
        ]);
        let pwad_bytes = make_wad(
            b"PWAD",
            &[
                ("F_START", b""),
                ("NUKAGE1", &vec![9u8; FLAT_SIZE]),
                ("F_END", b""),
            ],
        );

        let mut stack = WadStack::new();
        stack.push_iwad(iwad_bytes).expect("IWAD push must succeed");
        stack.push_pwad(pwad_bytes).expect("PWAD push must succeed");

        let cache = FlatCache::load_from_stack(&stack);
        assert_eq!(cache.get(b"NUKAGE1\0")[0], 9);
    }

    #[test]
    fn test_flat_cache_stack_extended_honors_ff_markers() {
        let iwad_bytes = make_iwad(&[
            ("F_START", b""),
            ("NUKAGE1", &vec![1u8; FLAT_SIZE]),
            ("F_END", b""),
        ]);
        let pwad_bytes = make_wad(
            b"PWAD",
            &[
                ("FF_START", b""),
                ("NUKAGE1", &vec![9u8; FLAT_SIZE]),
                ("FF_END", b""),
            ],
        );

        let mut stack = WadStack::new();
        stack.push_iwad(iwad_bytes).expect("IWAD push must succeed");
        stack.push_pwad(pwad_bytes).expect("PWAD push must succeed");

        let cache = FlatCache::load_from_stack_extended(&stack);
        assert_eq!(cache.get(b"NUKAGE1\0")[0], 9);
    }

    #[test]
    fn test_flat_cache_stack_strict_ignores_ff_markers() {
        let iwad_bytes = make_iwad(&[
            ("F_START", b""),
            ("NUKAGE1", &vec![1u8; FLAT_SIZE]),
            ("F_END", b""),
        ]);
        let pwad_bytes = make_wad(
            b"PWAD",
            &[
                ("FF_START", b""),
                ("NUKAGE1", &vec![9u8; FLAT_SIZE]),
                ("FF_END", b""),
            ],
        );

        let mut stack = WadStack::new();
        stack.push_iwad(iwad_bytes).expect("IWAD push must succeed");
        stack.push_pwad(pwad_bytes).expect("PWAD push must succeed");

        let cache = FlatCache::load_from_stack_strict(&stack);
        assert_eq!(cache.get(b"NUKAGE1\0")[0], 1);
    }
}
