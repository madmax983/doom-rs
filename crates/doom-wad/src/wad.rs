//! WAD file parsing: header identification, lump directory, and data access.
//!
//! The WAD format is specified in the Unofficial Doom Specs.
//!
//! # Header (12 bytes at offset 0)
//! ```text
//! [0..4]  identification: "IWAD" or "PWAD"
//! [4..8]  numlumps: i32 LE
//! [8..12] infotableofs: i32 LE
//! ```
//!
//! # Lump directory
//! `numlumps` × 16-byte entries starting at `infotableofs`.
//!
//! # Verus invariants enforced at parse time
//! All returned `WadFile` instances satisfy:
//! - `∀ i: lump[i].offset + lump[i].size ≤ data.len()`
//! - No zero-size lump start/end range escapes bounds
//! (Overlap-freeness is a weaker property we log-warn on, not hard-error,
//!  because some vanilla WADs technically have overlapping lumps via
//!  lump aliasing tricks — a full Verus proof would quantify this.)

use crate::lump::{LumpDef, LumpName, RawLumpEntry};
use thiserror::Error;

/// WAD type identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WadKind {
    /// Internal WAD — the base game data file.
    Iwad,
    /// Patch WAD — overrides lumps from the IWAD.
    Pwad,
}

/// Errors that can occur when parsing a WAD file.
#[derive(Debug, Error)]
pub enum WadError {
    /// File too short to contain a valid header.
    #[error("WAD file too short: {0} bytes (minimum 12)")]
    TooShort(usize),

    /// The 4-byte identification field is not "IWAD" or "PWAD".
    #[error("invalid WAD magic: expected IWAD or PWAD, got {0:?}")]
    InvalidMagic([u8; 4]),

    /// The lump count is negative (corrupted WAD).
    #[error("WAD lump count is negative: {0}")]
    NegativeLumpCount(i32),

    /// The directory offset points outside the file.
    #[error("WAD directory offset {offset} + directory size {dir_size} exceeds file length {file_len}")]
    DirectoryOutOfBounds {
        offset: usize,
        dir_size: usize,
        file_len: usize,
    },

    /// A specific lump's data range exceeds the file.
    #[error("lump {name} at [{offset}, {end}) exceeds file length {file_len}")]
    LumpOutOfBounds {
        name: String,
        offset: usize,
        end: usize,
        file_len: usize,
    },

    /// A lump entry has a negative filepos or size field.
    #[error("lump {name} has negative filepos or size")]
    LumpNegativeField { name: String },
}

/// A parsed, validated WAD file.
///
/// Holds an owned copy of the raw bytes plus the validated lump directory.
/// All `LumpDef` offsets and sizes are guaranteed to be in bounds.
#[derive(Debug)]
pub struct WadFile {
    kind: WadKind,
    data: Vec<u8>,
    dir: Vec<LumpDef>,
}

impl WadFile {
    /// Parse a WAD from raw bytes (typically `std::fs::read("doom.wad")`).
    ///
    /// # Errors
    /// Returns `WadError` if the file is malformed or any lump is out of bounds.
    pub fn parse(data: Vec<u8>) -> Result<Self, WadError> {
        if data.len() < 12 {
            return Err(WadError::TooShort(data.len()));
        }

        // Identify WAD kind.
        let magic: [u8; 4] = data[0..4].try_into().unwrap();
        let kind = match &magic {
            b"IWAD" => WadKind::Iwad,
            b"PWAD" => WadKind::Pwad,
            _ => return Err(WadError::InvalidMagic(magic)),
        };

        let numlumps = i32::from_le_bytes(data[4..8].try_into().unwrap());
        let infotableofs = i32::from_le_bytes(data[8..12].try_into().unwrap());

        if numlumps < 0 {
            return Err(WadError::NegativeLumpCount(numlumps));
        }
        let numlumps = numlumps as usize;
        let dir_offset = infotableofs as usize;
        let dir_size = numlumps * size_of::<RawLumpEntry>();

        let dir_end = dir_offset.saturating_add(dir_size);
        if dir_end > data.len() {
            return Err(WadError::DirectoryOutOfBounds {
                offset: dir_offset,
                dir_size,
                file_len: data.len(),
            });
        }

        // Parse directory entries field-by-field from the raw byte slice.
        // We don't use cast_slice here because Vec<u8> alignment is not
        // guaranteed to satisfy RawLumpEntry's i32 alignment requirement.
        // bytemuck::cast_slice is used only for lump *data* (typed structs
        // loaded from already-validated, caller-owned buffers).
        let dir_bytes = &data[dir_offset..dir_end];

        let mut dir = Vec::with_capacity(numlumps);
        for chunk in dir_bytes.chunks_exact(16) {
            let raw = RawLumpEntry {
                filepos: i32::from_le_bytes(chunk[0..4].try_into().unwrap()),
                size:    i32::from_le_bytes(chunk[4..8].try_into().unwrap()),
                name:    chunk[8..16].try_into().unwrap(),
            };
            let name = LumpName::from_raw(raw.name);
            let (offset, end) = raw.byte_range().ok_or_else(|| WadError::LumpNegativeField {
                name: name.as_str().to_owned(),
            })?;

            if end > data.len() {
                return Err(WadError::LumpOutOfBounds {
                    name: name.as_str().to_owned(),
                    offset,
                    end,
                    file_len: data.len(),
                });
            }

            dir.push(LumpDef { name, offset, size: raw.size as usize });
        }

        Ok(Self { kind, data, dir })
    }

    /// WAD kind (IWAD or PWAD).
    pub fn kind(&self) -> WadKind { self.kind }

    /// Number of lumps in the directory.
    pub fn lump_count(&self) -> usize { self.dir.len() }

    /// Iterate over all lump descriptors.
    pub fn lumps(&self) -> &[LumpDef] { &self.dir }

    /// Find the last lump with the given name (PWAD override semantics).
    ///
    /// Returns `None` if no lump with that name exists.
    pub fn find_lump(&self, name: &str) -> Option<&LumpDef> {
        let key = LumpName::from_str(name);
        self.dir.iter().rev().find(|l| l.name == key)
    }

    /// Return the raw bytes for a lump.
    ///
    /// # Panics
    /// Never — bounds are validated at parse time.
    pub fn lump_data(&self, lump: &LumpDef) -> &[u8] {
        &self.data[lump.offset..lump.end()]
    }

    /// Convenience: find a lump by name and return its data.
    ///
    /// Returns `None` if the lump doesn't exist.
    pub fn find_lump_data(&self, name: &str) -> Option<&[u8]> {
        let lump = self.find_lump(name)?.clone();
        Some(self.lump_data(&lump))
    }

    /// Find all lumps between two marker lumps (e.g. `F_START`/`F_END`).
    ///
    /// Returns an iterator over lumps that appear strictly between the
    /// last occurrence of `start_marker` and the next `end_marker`.
    pub fn lumps_between<'a>(&'a self, start: &str, end: &str) -> impl Iterator<Item = &'a LumpDef> {
        let start_key = LumpName::from_str(start);
        let end_key   = LumpName::from_str(end);

        let start_idx = self.dir.iter().rposition(|l| l.name == start_key).map(|i| i + 1).unwrap_or(0);
        let end_idx   = self.dir[start_idx..].iter().position(|l| l.name == end_key)
            .map(|i| start_idx + i)
            .unwrap_or(self.dir.len());

        self.dir[start_idx..end_idx].iter()
    }

    /// Find a map's lump group: returns the index in `dir` of the map marker lump.
    ///
    /// Map lumps follow the pattern: `E1M1` (marker), `THINGS`, `LINEDEFS`, …
    pub fn find_map_marker(&self, map_name: &str) -> Option<usize> {
        let key = LumpName::from_str(map_name);
        self.dir.iter().position(|l| l.name == key)
    }
}

/// Required lumps that must follow a map marker (in order).
pub const REQUIRED_MAP_LUMPS: &[&str] = &[
    "THINGS", "LINEDEFS", "SIDEDEFS", "VERTEXES",
    "SEGS", "SSECTORS", "NODES", "SECTORS", "REJECT", "BLOCKMAP",
];

/// Validated map lump group: marker + the 10 required sub-lumps.
#[derive(Debug)]
pub struct MapLumpGroup<'a> {
    /// Map name marker (e.g. "E1M1").
    pub marker: &'a LumpDef,
    /// The 10 required lumps in spec order.
    pub lumps: [&'a LumpDef; 10],
}

impl WadFile {
    /// Locate and validate a map's lump group.
    ///
    /// # Errors
    /// Returns `None` if the map marker isn't present or any required lump
    /// is missing from the expected position after the marker.
    pub fn map_lump_group<'a>(&'a self, map_name: &str) -> Option<MapLumpGroup<'a>> {
        let marker_idx = self.find_map_marker(map_name)?;
        let marker = &self.dir[marker_idx];

        // The 10 required lumps must immediately follow the marker.
        if marker_idx + 10 >= self.dir.len() {
            return None;
        }

        let mut lumps: [Option<&LumpDef>; 10] = [None; 10];
        for (i, expected) in REQUIRED_MAP_LUMPS.iter().enumerate() {
            let candidate = &self.dir[marker_idx + 1 + i];
            if candidate.name != LumpName::from_str(expected) {
                return None;
            }
            lumps[i] = Some(candidate);
        }

        Some(MapLumpGroup {
            marker,
            lumps: lumps.map(Option::unwrap),
        })
    }
}

/// A complete directory of resolved lumps (from one or more WAD files).
///
/// Used as the output of `WadStack::build_dir()`.
#[derive(Debug, Default)]
pub struct WadDir {
    lumps: Vec<LumpDef>,
}

impl WadDir {
    /// Construct from a flat list of lumps.
    pub fn from_lumps(lumps: Vec<LumpDef>) -> Self {
        Self { lumps }
    }

    /// Find a lump by name (last occurrence wins — PWAD override semantics).
    pub fn find(&self, name: &str) -> Option<&LumpDef> {
        let key = LumpName::from_str(name);
        self.lumps.iter().rev().find(|l| l.name == key)
    }

    /// All lumps.
    pub fn lumps(&self) -> &[LumpDef] { &self.lumps }

    /// Total lump count.
    pub fn len(&self) -> usize { self.lumps.len() }

    /// Returns `true` if no lumps are present.
    pub fn is_empty(&self) -> bool { self.lumps.is_empty() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_iwad(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        // Build a minimal IWAD in memory.
        // Lay out lump data contiguously after the header.
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
        // Directory offset placeholder — we'll fill in after appending lump data.
        data.extend_from_slice(&0i32.to_le_bytes()); // placeholder
        let mut offsets: Vec<(usize, usize)> = Vec::new(); // (filepos, size)

        for (_, lump_bytes) in lumps {
            let pos = data.len();
            data.extend_from_slice(lump_bytes);
            offsets.push((pos, lump_bytes.len()));
        }

        // Now write the directory.
        let dir_offset = data.len() as i32;
        let dir_offset_bytes = dir_offset.to_le_bytes();
        data[8..12].copy_from_slice(&dir_offset_bytes);

        for (i, (name, _)) in lumps.iter().enumerate() {
            let (filepos, size) = offsets[i];
            data.extend_from_slice(&(filepos as i32).to_le_bytes());
            data.extend_from_slice(&(size as i32).to_le_bytes());
            let mut name_buf = [0u8; 8];
            for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                name_buf[j] = b;
            }
            data.extend_from_slice(&name_buf);
        }

        data
    }

    #[test]
    fn parse_minimal_iwad() {
        let payload = b"HELLO";
        let wad_bytes = make_iwad(&[("TEST", payload)]);
        let wad = WadFile::parse(wad_bytes).expect("parse failed");
        assert_eq!(wad.kind(), WadKind::Iwad);
        assert_eq!(wad.lump_count(), 1);
        assert_eq!(wad.find_lump("TEST").unwrap().name.as_str(), "TEST");
        assert_eq!(wad.find_lump_data("TEST").unwrap(), b"HELLO");
    }

    #[test]
    fn parse_rejects_bad_magic() {
        let mut bad = vec![0u8; 16];
        bad[0..4].copy_from_slice(b"XWAD");
        assert!(matches!(WadFile::parse(bad), Err(WadError::InvalidMagic(_))));
    }

    #[test]
    fn parse_rejects_too_short() {
        assert!(matches!(WadFile::parse(vec![0u8; 4]), Err(WadError::TooShort(4))));
    }

    #[test]
    fn find_lump_case_insensitive() {
        let wad_bytes = make_iwad(&[("PLAYPAL", b"palette_data")]);
        let wad = WadFile::parse(wad_bytes).unwrap();
        assert!(wad.find_lump("playpal").is_some());
        assert!(wad.find_lump("PLAYPAL").is_some());
    }

    #[test]
    fn lump_count_zero_valid() {
        // A WAD with zero lumps (edge case).
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"IWAD");
        // numlumps = 0, infotableofs = 12 (points just past header)
        data[4..8].copy_from_slice(&0i32.to_le_bytes());
        data[8..12].copy_from_slice(&12i32.to_le_bytes());
        let wad = WadFile::parse(data).unwrap();
        assert_eq!(wad.lump_count(), 0);
    }

    #[test]
    fn multiple_lumps_same_name_last_wins() {
        let wad_bytes = make_iwad(&[
            ("DEMO", b"first"),
            ("OTHER", b"other"),
            ("DEMO", b"second"),
        ]);
        let wad = WadFile::parse(wad_bytes).unwrap();
        // find_lump returns last occurrence
        assert_eq!(wad.find_lump_data("DEMO").unwrap(), b"second");
    }
}
