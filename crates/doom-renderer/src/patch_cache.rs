//! Cache for Doom picture-format patches loaded from the WAD.
//!
//! Patches are stored by uppercase lump name and parsed lazily on first access.
//! Used by the menu renderer and status bar to draw WAD graphics.

use std::collections::HashMap;

use doom_wad::WadStack;

use crate::texture_compose::{PatchImage, parse_patch};

/// Lazy-loading cache for picture-format patches.
pub struct PatchCache {
    patches: HashMap<doom_wad::lump::LumpName, PatchImage>,
}

impl PatchCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            patches: HashMap::new(),
        }
    }

    /// Retrieves a patch graphic by lump name, dynamically loading and caching it from the active WAD file on first request.
    ///
    /// Returns `None` if the lump doesn't exist or isn't a valid patch.
    ///
    /// ⚡ Bolt Optimization:
    /// Avoids an extra `.clone()` on the string key and eliminates double hash lookups
    /// by using the `Entry` API during the cache miss path.
    pub fn get<'a>(&'a mut self, name: &str, wad: &WadStack) -> Option<&'a PatchImage> {
        let mut key_bytes = [0u8; 8];
        let bytes = name.as_bytes();
        for i in 0..8.min(bytes.len()) {
            key_bytes[i] = bytes[i].to_ascii_uppercase();
        }
        let key = doom_wad::lump::LumpName::from_raw(key_bytes);

        use std::collections::hash_map::Entry;
        match self.patches.entry(key) {
            Entry::Occupied(o) => Some(o.into_mut()),
            Entry::Vacant(v) => {
                let data = wad.lump_data(v.key().as_str())?;
                let patch = parse_patch(data)?;
                Some(v.insert(patch))
            }
        }
    }

    /// Preload all menu patches so first frame has no stutter.
    ///
    /// Missing lumps are silently skipped (e.g. Doom 2 has no episode patches).
    pub fn preload_menu_patches(&mut self, wad: &WadStack) {
        let names = [
            "M_DOOM", "M_NEWG", "M_NGAME", "M_OPTION", "M_OPTTTL", "M_LOADG", "M_SAVEG", "M_QUITG",
            "M_EPISOD", "M_EPI1", "M_EPI2", "M_EPI3", "M_EPI4", "M_JKILL", "M_ROUGH", "M_HURT",
            "M_ULTRA", "M_NMARE", "M_SKULL1", "M_SKULL2", "M_MESSG", "M_DETAIL", "M_SCRNSZ",
            "M_MSENS", "M_SVOL", "M_THERML", "M_THERMM", "M_THERMR", "M_THERMO", "M_PAUSE",
            "TITLEPIC", "CREDIT", "HELP", "HELP1", "HELP2",
        ];
        for name in &names {
            self.get(name, wad);
        }
    }

    /// Preload all status bar patches.
    pub fn preload_statusbar_patches(&mut self, wad: &WadStack) {
        // Tall red digits for ammo/health/armor.
        for i in 0..=9 {
            self.get(&format!("STTNUM{i}"), wad);
        }
        self.get("STTMINUS", wad);
        self.get("STTPRCNT", wad);

        // Short yellow digits for ammo tally.
        for i in 0..=9 {
            self.get(&format!("STYSNUM{i}"), wad);
        }

        // Arms / weapon numbers.
        self.get("STARMS", wad);
        for i in 2..=7 {
            self.get(&format!("STGNUM{i}"), wad);
        }

        // Keys.
        for i in 0..=5 {
            self.get(&format!("STKEYS{i}"), wad);
        }

        // Face backgrounds.
        self.get("STFB0", wad);
        self.get("STFB1", wad);
        self.get("STBAR", wad);

        // Face mugshots — all health tiers and directions.
        let dirs = ["ST", "TR", "TL", "OUC", "EVL", "KLL", "GOD", "DEA", "XDE"];
        let suffixes = ["STFST", "STFTL", "STFTR", "STFOUCH", "STFEVL", "STFKLL"];
        for tier in 0..=4 {
            for prefix in &suffixes {
                self.get(&format!("{prefix}{tier}0"), wad);
            }
        }
        self.get("STFGOD0", wad);
        self.get("STFDEAD0", wad);
        self.get("STFXDTH1", wad);
        let _ = dirs; // suppress unused warning
    }
}

impl Default for PatchCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache_is_empty() {
        let cache = PatchCache::new();
        assert!(cache.patches.is_empty());
    }

    #[test]
    fn get_missing_lump_returns_none() {
        let mut cache = PatchCache::new();
        let wad = WadStack::new();
        assert!(cache.get("NOTEXIST", &wad).is_none());
    }
}
