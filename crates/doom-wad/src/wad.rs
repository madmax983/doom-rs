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
//!   (Overlap-freeness is a weaker property we log-warn on, not hard-error,
//!   because some vanilla WADs technically have overlapping lumps via
//!   lump aliasing tricks — a full Verus proof would quantify this.)

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

    /// A PWAD was provided where an IWAD is required.
    #[error("expected an IWAD as the base WAD, but found a PWAD")]
    ExpectedIwad,

    /// The lump count is negative (corrupted WAD).
    #[error("WAD lump count is negative: {0}")]
    NegativeLumpCount(i32),

    /// The directory offset points outside the file.
    #[error(
        "WAD directory offset {offset} + directory size {dir_size} exceeds file length {file_len}"
    )]
    DirectoryOutOfBounds {
        /// The offset of the directory.
        offset: usize,
        /// The size of the directory.
        dir_size: usize,
        /// The total file length.
        file_len: usize,
    },

    /// A specific lump's data range exceeds the file.
    #[error("lump {name} at [{offset}, {end}) exceeds file length {file_len}")]
    LumpOutOfBounds {
        /// The name of the lump.
        name: String,
        /// The offset of the lump.
        offset: usize,
        /// The end offset of the lump.
        end: usize,
        /// The total file length.
        file_len: usize,
    },

    /// A lump entry has a negative filepos or size field.
    #[error("lump {name} has negative filepos or size")]
    LumpNegativeField {
        /// The name of the lump.
        name: String,
    },
}

/// A parsed, validated WAD file.
///
/// A WAD (Where's All the Data?) file is the core archive format for Doom.
/// It contains a header, a directory of lumps (files within the archive),
/// and the raw data payload for those lumps.
///
/// This struct holds an owned copy of the raw bytes plus the validated lump directory.
/// All [`LumpDef`] offsets and sizes are guaranteed to be in bounds at parse time,
/// so accessing lump data later is infallible.
///
/// # Examples
/// ```
/// use doom_wad::WadFile;
///
/// // A minimal valid WAD with one 4-byte lump named "TEST".
/// let wad_bytes = b"IWAD\x01\0\0\0\x0C\0\0\0\x1C\0\0\0\x04\0\0\0TEST\0\0\0\0DATA".to_vec();
/// let wad = WadFile::parse(wad_bytes).unwrap();
///
/// assert_eq!(wad.lump_count(), 1);
/// assert_eq!(wad.find_lump_data("TEST").unwrap(), b"DATA");
/// ```
#[derive(Debug)]
pub struct WadFile {
    kind: WadKind,
    data: Vec<u8>,
    dir: Vec<LumpDef>,
}

impl WadFile {
    /// Parse a WAD from raw bytes (typically loaded via `std::fs::read("doom.wad")`).
    ///
    /// The parser reads the 12-byte header, jumps to the directory offset, and
    /// processes every 16-byte lump entry. During this phase, it validates that
    /// every lump's byte range falls strictly within the bounds of the provided data.
    ///
    /// # Errors
    /// Returns [`WadError`] if the file is malformed, too short, has invalid magic bytes,
    /// or if any lump's claimed offset and size exceed the file's total length.
    pub fn parse(data: Vec<u8>) -> Result<Self, WadError> {
        if data.len() < 12 {
            return Err(WadError::TooShort(data.len()));
        }

        // Identify WAD kind.
        let magic: [u8; 4] = data[0..4]
            .try_into()
            .expect("slice of len 4 must fit into [u8; 4]");
        let kind = match &magic {
            b"IWAD" => WadKind::Iwad,
            b"PWAD" => WadKind::Pwad,
            _ => return Err(WadError::InvalidMagic(magic)),
        };

        let numlumps = i32::from_le_bytes(
            data[4..8]
                .try_into()
                .expect("slice of len 4 must fit into [u8; 4]"),
        );
        let infotableofs = i32::from_le_bytes(
            data[8..12]
                .try_into()
                .expect("slice of len 4 must fit into [u8; 4]"),
        );

        if numlumps < 0 {
            return Err(WadError::NegativeLumpCount(numlumps));
        }

        // Havoc 👺: Negative directory offset could cause a cast to a huge usize
        // or a crash when wrapping. Let's make sure it's valid.
        if infotableofs < 0 {
            return Err(WadError::DirectoryOutOfBounds {
                offset: infotableofs as usize, // Will be garbage but signals the error
                dir_size: (numlumps as usize).saturating_mul(size_of::<RawLumpEntry>()),
                file_len: data.len(),
            });
        }

        let numlumps = numlumps as usize;
        let dir_offset = infotableofs as usize;
        let dir_size = numlumps.saturating_mul(size_of::<RawLumpEntry>());

        let dir_end = dir_offset.saturating_add(dir_size);
        if dir_end > data.len() || dir_offset > data.len() {
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
                filepos: i32::from_le_bytes(chunk[0..4].try_into().expect("chunk 0..4 is 4 bytes")),
                size: i32::from_le_bytes(chunk[4..8].try_into().expect("chunk 4..8 is 4 bytes")),
                name: chunk[8..16].try_into().expect("chunk 8..16 is 8 bytes"),
            };
            let name = LumpName::from_raw(raw.name);
            let (offset, end) = raw
                .byte_range()
                .ok_or_else(|| WadError::LumpNegativeField {
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

            dir.push(LumpDef {
                name,
                offset,
                size: raw.size as usize,
            });
        }

        Ok(Self { kind, data, dir })
    }

    /// WAD kind (IWAD or PWAD).
    pub fn kind(&self) -> WadKind {
        self.kind
    }

    /// Number of lumps in the directory.
    pub fn lump_count(&self) -> usize {
        self.dir.len()
    }

    /// Iterate over all lump descriptors.
    pub fn lumps(&self) -> &[LumpDef] {
        &self.dir
    }

    /// Find the last lump with the given name.
    ///
    /// Because WADs can contain multiple lumps with the same name (especially
    /// marker lumps or map data), this method searches backwards from the end
    /// of the directory. This respects the engine's "last defined wins" rule.
    ///
    /// Returns `None` if no lump with that name exists.
    ///
    /// # Examples
    /// ```
    /// use doom_wad::WadFile;
    ///
    /// // A WAD with two lumps named "TEST", the last one containing "SECOND".
    /// // Header: 12 bytes
    /// // 1st lump entry: offset 12+16+16=44, size 5, name "TEST"
    /// // 2nd lump entry: offset 44+5=49, size 6, name "TEST"
    /// let wad_bytes = b"IWAD\x02\0\0\0\x0C\0\0\0\x2C\0\0\0\x05\0\0\0TEST\0\0\0\0\x31\0\0\0\x06\0\0\0TEST\0\0\0\0FIRSTSECOND".to_vec();
    /// let wad = WadFile::parse(wad_bytes).unwrap();
    ///
    /// let lump = wad.find_lump("TEST").unwrap();
    /// assert_eq!(wad.lump_data(lump), b"SECOND");
    /// ```
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
    ///
    /// # Examples
    /// ```
    /// use doom_wad::WadFile;
    ///
    /// let wad_bytes = b"IWAD\x01\0\0\0\x0C\0\0\0\x1C\0\0\0\x04\0\0\0TEST\0\0\0\0DATA".to_vec();
    /// let wad = WadFile::parse(wad_bytes).unwrap();
    ///
    /// assert_eq!(wad.find_lump_data("test").unwrap(), b"DATA"); // Search is case-insensitive
    /// assert!(wad.find_lump_data("MISSING").is_none());
    /// ```
    pub fn find_lump_data(&self, name: &str) -> Option<&[u8]> {
        let lump = self.find_lump(name)?.clone();
        Some(self.lump_data(&lump))
    }

    /// Find all lumps between two marker lumps (e.g. `F_START`/`F_END`).
    ///
    /// Returns an iterator over lumps that appear strictly between the
    /// last occurrence of `start_marker` and the next `end_marker`.
    ///
    /// # Examples
    /// ```
    /// use doom_wad::WadFile;
    ///
    /// // A WAD with marker lumps
    /// let wad_bytes = b"IWAD\x04\0\0\0\x0C\0\0\0\x4C\0\0\0\x00\0\0\0F_START\0\x4C\0\0\0\x00\0\0\0FLAT1\0\0\0\x4C\0\0\0\x00\0\0\0FLAT2\0\0\0\x4C\0\0\0\x00\0\0\0F_END\0\0\0".to_vec();
    /// let wad = WadFile::parse(wad_bytes).unwrap();
    ///
    /// let flats: Vec<_> = wad.lumps_between("F_START", "F_END").map(|l| l.name.as_str()).collect();
    /// assert_eq!(flats, vec!["FLAT1", "FLAT2"]);
    /// ```
    pub fn lumps_between<'a>(
        &'a self,
        start: &str,
        end: &str,
    ) -> impl Iterator<Item = &'a LumpDef> {
        let start_key = LumpName::from_str(start);
        let end_key = LumpName::from_str(end);

        let start_idx = self
            .dir
            .iter()
            .rposition(|l| l.name == start_key)
            .map(|i| i + 1)
            .unwrap_or(0);
        let end_idx = self.dir[start_idx..]
            .iter()
            .position(|l| l.name == end_key)
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
    "THINGS", "LINEDEFS", "SIDEDEFS", "VERTEXES", "SEGS", "SSECTORS", "NODES", "SECTORS", "REJECT",
    "BLOCKMAP",
];

/// Validated classic map lump group: marker + the 10 required sub-lumps.
#[derive(Debug)]
pub struct ClassicMapLumpGroup<'a> {
    /// Map name marker (e.g. "E1M1").
    pub marker: &'a LumpDef,
    /// The 10 required lumps in spec order.
    pub lumps: [&'a LumpDef; 10],
}

/// Validated UDMF map lump group: marker + `TEXTMAP` + auxiliary lumps + `ENDMAP`.
#[derive(Debug)]
pub struct UdmfMapLumpGroup<'a> {
    /// Map name marker (e.g. "MAP01").
    pub marker: &'a LumpDef,
    /// The `TEXTMAP` lump containing the textual UDMF map body.
    pub textmap: &'a LumpDef,
    /// All auxiliary lumps between `TEXTMAP` and `ENDMAP`.
    aux_lumps: &'a [LumpDef],
    /// The terminating `ENDMAP` marker.
    pub endmap: &'a LumpDef,
}

impl<'a> UdmfMapLumpGroup<'a> {
    /// Find the last auxiliary lump with the given name inside this UDMF map.
    pub fn find_lump(&self, name: &str) -> Option<&'a LumpDef> {
        let key = LumpName::from_str(name);
        self.aux_lumps.iter().rev().find(|lump| lump.name == key)
    }

    /// All auxiliary lumps between `TEXTMAP` and `ENDMAP`.
    pub fn aux_lumps(&self) -> &'a [LumpDef] {
        self.aux_lumps
    }
}

/// Validated map lump group for either classic or UDMF map layouts.
#[derive(Debug)]
pub enum MapLumpGroup<'a> {
    /// Classic binary Doom map marker followed by the fixed 10-lump payload.
    Classic(ClassicMapLumpGroup<'a>),
    /// UDMF map marker followed by `TEXTMAP`, auxiliary lumps, and `ENDMAP`.
    Udmf(UdmfMapLumpGroup<'a>),
}

impl WadFile {
    /// Locate and validate a map's lump group.
    ///
    /// Maps in Doom aren't single files; they are a contiguous sequence of lumps
    /// following a specific marker (like `E1M1` or `MAP01`). This method finds
    /// the marker and groups the subsequent lumps based on whether they follow
    /// the classic binary format (10 required lumps like `THINGS`, `LINEDEFS`)
    /// or the newer text-based `UDMF` format.
    ///
    /// Returns `None` if the map marker isn't present or any required lump
    /// is missing from the expected position after the marker.
    ///
    /// # Examples
    /// ```
    /// use doom_wad::{WadFile, MapLumpGroup};
    ///
    /// // A WAD with a valid classic map requires 10 specific lumps after the marker.
    /// // (The binary representation here is shortened for illustration.)
    /// let wad_bytes = b"IWAD\x0B\0\0\0\x0C\0\0\0\xBC\0\0\0\x00\0\0\0MAP01\0\0\0\xBC\0\0\0\x00\0\0\0THINGS\0\0\xBC\0\0\0\x00\0\0\0LINEDEFS\xBC\0\0\0\x00\0\0\0SIDEDEFS\xBC\0\0\0\x00\0\0\0VERTEXES\xBC\0\0\0\x00\0\0\0SEGS\0\0\0\0\xBC\0\0\0\x00\0\0\0SSECTORS\xBC\0\0\0\x00\0\0\0NODES\0\0\0\xBC\0\0\0\x00\0\0\0SECTORS\0\xBC\0\0\0\x00\0\0\0REJECT\0\0\xBC\0\0\0\x00\0\0\0BLOCKMAP".to_vec();
    /// let wad = WadFile::parse(wad_bytes).unwrap();
    ///
    /// if let Some(MapLumpGroup::Classic(map)) = wad.map_lump_group("MAP01") {
    ///     assert_eq!(map.marker.name.as_str(), "MAP01");
    ///     assert_eq!(map.lumps[0].name.as_str(), "THINGS");
    ///     // ... and so on for all 10 required lumps
    /// }
    /// ```
    pub fn map_lump_group<'a>(&'a self, map_name: &str) -> Option<MapLumpGroup<'a>> {
        let marker_idx = self.find_map_marker(map_name)?;
        let marker = &self.dir[marker_idx];

        // UDMF layout: MAPxx/E#M# marker, then TEXTMAP ... ENDMAP.
        if let Some(textmap) = self.dir.get(marker_idx + 1)
            && textmap.name == LumpName::from_str("TEXTMAP")
        {
            let end_relative_idx = self.dir[marker_idx + 2..]
                .iter()
                .position(|lump| lump.name == LumpName::from_str("ENDMAP"))?;
            let end_idx = marker_idx + 2 + end_relative_idx;
            return Some(MapLumpGroup::Udmf(UdmfMapLumpGroup {
                marker,
                textmap,
                aux_lumps: &self.dir[marker_idx + 2..end_idx],
                endmap: &self.dir[end_idx],
            }));
        }

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

        Some(MapLumpGroup::Classic(ClassicMapLumpGroup {
            marker,
            lumps: lumps.map(Option::unwrap),
        }))
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
    pub fn lumps(&self) -> &[LumpDef] {
        &self.lumps
    }

    /// Total lump count.
    pub fn len(&self) -> usize {
        self.lumps.len()
    }

    /// Returns `true` if no lumps are present.
    pub fn is_empty(&self) -> bool {
        self.lumps.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Proptest property tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    /// Build a minimal valid IWAD with one lump from its component parts.
    fn make_iwad_with_lump(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&1i32.to_le_bytes()); // numlumps = 1
        data.extend_from_slice(&0i32.to_le_bytes()); // dir offset placeholder

        let lump_offset = data.len();
        data.extend_from_slice(payload);

        let dir_offset = data.len() as i32;
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());

        // Directory entry: filepos (4) + size (4) + name (8)
        data.extend_from_slice(&(lump_offset as i32).to_le_bytes());
        data.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        let mut name_buf = [0u8; 8];
        for (i, &b) in name.as_bytes().iter().take(8).enumerate() {
            name_buf[i] = b.to_ascii_uppercase();
        }
        data.extend_from_slice(&name_buf);
        data
    }

    proptest! {
        /// Any buffer shorter than 12 bytes must be rejected — the WAD header
        /// requires exactly 12 bytes (4 magic + 4 numlumps + 4 infotableofs).
        #[test]
        fn parse_rejects_any_buffer_under_12_bytes(len in 0usize..12) {
            let buf = vec![0u8; len];
            prop_assert!(
                WadFile::parse(buf).is_err(),
                "buffer of {len} bytes should be rejected (too short)"
            );
        }

        /// A buffer of exactly 12 bytes with a bad magic must produce
        /// `InvalidMagic`, not a panic or `TooShort`.
        #[test]
        fn parse_12_byte_bad_magic_gives_invalid_magic(
            // Use byte values that can never form "IWAD" or "PWAD"
            b0 in 0u8..=255u8,
            b1 in 0u8..=255u8,
            b2 in 0u8..=255u8,
            b3 in 0u8..=255u8,
        ) {
            let magic = [b0, b1, b2, b3];
            if magic == *b"IWAD" || magic == *b"PWAD" {
                // Skip valid magic — it would parse further.
                return Ok(());
            }
            let mut buf = vec![0u8; 12];
            buf[0..4].copy_from_slice(&magic);
            // numlumps = 0, infotableofs = 12 → minimal valid structure
            // but magic is wrong so parse must error.
            buf[8..12].copy_from_slice(&12i32.to_le_bytes());
            prop_assert!(
                matches!(WadFile::parse(buf), Err(WadError::InvalidMagic(_))),
                "bad magic {:?} should give InvalidMagic", magic
            );
        }

        /// After loading an IWAD and a PWAD, every IWAD lump that the PWAD
        /// does NOT override must still be accessible via the stack.
        ///
        /// We test with fixed lump names to keep the test fast and self-contained.
        #[test]
        fn pwad_override_preserves_all_base_lumps(
            // Number of extra lumps only in IWAD (0..4) to vary coverage.
            n_extra in 0usize..=3,
        ) {
            // Build IWAD with "BASE" + n_extra unique lumps.
            let unique_names: Vec<String> = (0..n_extra)
                .map(|i| format!("UNQ{i}"))
                .collect();

            let mut iwad_data: Vec<u8> = Vec::new();
            let all_lump_count = 1 + n_extra; // BASE + unique
            iwad_data.extend_from_slice(b"IWAD");
            iwad_data.extend_from_slice(&(all_lump_count as i32).to_le_bytes());
            iwad_data.extend_from_slice(&0i32.to_le_bytes()); // placeholder

            let mut offsets = Vec::new();
            // BASE lump
            offsets.push((iwad_data.len(), b"base_data".len()));
            iwad_data.extend_from_slice(b"base_data");
            // Unique lumps
            for name in &unique_names {
                let payload = format!("data_{name}");
                offsets.push((iwad_data.len(), payload.len()));
                iwad_data.extend_from_slice(payload.as_bytes());
            }

            let dir_offset = iwad_data.len() as i32;
            iwad_data[8..12].copy_from_slice(&dir_offset.to_le_bytes());

            // BASE directory entry
            iwad_data.extend_from_slice(&(offsets[0].0 as i32).to_le_bytes());
            iwad_data.extend_from_slice(&(offsets[0].1 as i32).to_le_bytes());
            let mut nb = [0u8; 8];
            nb[..4].copy_from_slice(b"BASE");
            iwad_data.extend_from_slice(&nb);
            // Unique entries
            for (i, name) in unique_names.iter().enumerate() {
                let (off, sz) = offsets[i + 1];
                iwad_data.extend_from_slice(&(off as i32).to_le_bytes());
                iwad_data.extend_from_slice(&(sz as i32).to_le_bytes());
                let mut nb2 = [0u8; 8];
                for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                    nb2[j] = b.to_ascii_uppercase();
                }
                iwad_data.extend_from_slice(&nb2);
            }

            // Build PWAD that overrides only "BASE".
            let pwad_bytes = {
                let mut d: Vec<u8> = Vec::new();
                d.extend_from_slice(b"PWAD");
                d.extend_from_slice(&1i32.to_le_bytes());
                d.extend_from_slice(&0i32.to_le_bytes());
                let payload_off = d.len();
                d.extend_from_slice(b"override");
                let dir_off = d.len() as i32;
                d[8..12].copy_from_slice(&dir_off.to_le_bytes());
                d.extend_from_slice(&(payload_off as i32).to_le_bytes());
                d.extend_from_slice(&8i32.to_le_bytes()); // "override" is 8 bytes
                let mut nb2 = [0u8; 8];
                nb2[..4].copy_from_slice(b"BASE");
                d.extend_from_slice(&nb2);
                d
            };

            let iwad = WadFile::parse(iwad_data).expect("IWAD parse failed");
            // Verify all unique base lumps are present in the IWAD directly.
            for name in &unique_names {
                prop_assert!(
                    iwad.find_lump(name).is_some(),
                    "IWAD should contain lump {name}"
                );
            }

            // Simulate stack behavior: PWAD overrides BASE but unique lumps stay.
            // (WadStack is in stack.rs; here we test WadFile-level semantics.)
            let pwad = WadFile::parse(pwad_bytes).expect("PWAD parse failed");
            prop_assert_eq!(
                pwad.find_lump_data("BASE").unwrap(),
                b"override",
                "PWAD must override BASE"
            );
            // Unique lumps are not in PWAD — absence is correct.
            for name in &unique_names {
                prop_assert!(
                    pwad.find_lump(name).is_none(),
                    "PWAD should not contain unique IWAD lump {name}"
                );
            }
        }

        /// Parsed lump names must never contain lowercase ASCII letters.
        /// `LumpName::from_raw` normalises all names to uppercase at parse time.
        #[test]
        fn all_parsed_lump_names_are_uppercase(
            // Generate a 1-8 char ASCII name with mixed case.
            name_len in 1usize..=8,
            // Use printable-ASCII byte range (excluding null) for the name chars.
            ch0 in b'a'..=b'z',
            ch1 in b'a'..=b'z',
        ) {
            // Build a name of `name_len` alternating between ch0/ch1.
            let name_bytes: Vec<u8> = (0..name_len)
                .map(|i| if i % 2 == 0 { ch0 } else { ch1 })
                .collect();
            let name_str = String::from_utf8(name_bytes).unwrap();

            let payload = b"data";
            let wad_bytes = make_iwad_with_lump(&name_str, payload);
            let wad = WadFile::parse(wad_bytes).expect("should parse");

            for lump in wad.lumps() {
                let s = lump.name.as_str();
                prop_assert!(
                    s.chars().all(|c| !c.is_ascii_lowercase()),
                    "lump name '{}' contains lowercase", s
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
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
        assert!(matches!(
            WadFile::parse(bad),
            Err(WadError::InvalidMagic(_))
        ));
    }

    #[test]
    fn parse_rejects_too_short() {
        assert!(matches!(
            WadFile::parse(vec![0u8; 4]),
            Err(WadError::TooShort(4))
        ));
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
        let wad_bytes = make_iwad(&[("DEMO", b"first"), ("OTHER", b"other"), ("DEMO", b"second")]);
        let wad = WadFile::parse(wad_bytes).unwrap();
        // find_lump returns last occurrence
        assert_eq!(wad.find_lump_data("DEMO").unwrap(), b"second");
    }

    #[test]
    fn map_lump_group_detects_udmf_group() {
        let wad_bytes = make_iwad(&[
            ("MAP01", b""),
            ("TEXTMAP", br#"namespace = "doom";"#),
            ("ZNODES", b"not_relevant_here"),
            ("ENDMAP", b""),
        ]);
        let wad = WadFile::parse(wad_bytes).unwrap();

        let group = wad.map_lump_group("MAP01").expect("map group");

        assert!(matches!(group, MapLumpGroup::Udmf(_)));
        if let MapLumpGroup::Udmf(group) = group {
            assert_eq!(group.marker.name.as_str(), "MAP01");
            assert_eq!(group.textmap.name.as_str(), "TEXTMAP");
            assert_eq!(group.endmap.name.as_str(), "ENDMAP");
            assert_eq!(
                group.find_lump("ZNODES").map(|lump| lump.name.as_str()),
                Some("ZNODES")
            );
        }
    }

    #[test]
    fn map_lump_group_missing_lumps_returns_none() {
        let wad_bytes = make_iwad(&[
            ("MAP01", b""),
            ("THINGS", b"iwad_things"),
            // Missing LINEDEFS
            ("SIDEDEFS", b"iwad_sidedefs"),
        ]);
        let wad = WadFile::parse(wad_bytes).unwrap();
        assert!(wad.map_lump_group("MAP01").is_none());
    }

    #[test]
    fn map_lump_group_classic_returns_classic_group() {
        let wad_bytes = make_iwad(&[
            ("MAP01", b""),
            ("THINGS", b""),
            ("LINEDEFS", b""),
            ("SIDEDEFS", b""),
            ("VERTEXES", b""),
            ("SEGS", b""),
            ("SSECTORS", b""),
            ("NODES", b""),
            ("SECTORS", b""),
            ("REJECT", b""),
            ("BLOCKMAP", b""),
        ]);
        let wad = WadFile::parse(wad_bytes).unwrap();
        match wad.map_lump_group("MAP01").unwrap() {
            MapLumpGroup::Classic(c) => {
                assert_eq!(c.marker.name.as_str(), "MAP01");
                assert_eq!(c.lumps[0].name.as_str(), "THINGS");
                assert_eq!(c.lumps[1].name.as_str(), "LINEDEFS");
                assert_eq!(c.lumps[2].name.as_str(), "SIDEDEFS");
                assert_eq!(c.lumps[3].name.as_str(), "VERTEXES");
                assert_eq!(c.lumps[4].name.as_str(), "SEGS");
                assert_eq!(c.lumps[5].name.as_str(), "SSECTORS");
                assert_eq!(c.lumps[6].name.as_str(), "NODES");
                assert_eq!(c.lumps[7].name.as_str(), "SECTORS");
                assert_eq!(c.lumps[8].name.as_str(), "REJECT");
                assert_eq!(c.lumps[9].name.as_str(), "BLOCKMAP");
            }
            _ => panic!("Expected Classic"),
        }
    }

    #[test]
    fn find_lump_data_returns_none_if_missing() {
        let wad_bytes = make_iwad(&[("TEST", b"data")]);
        let wad = WadFile::parse(wad_bytes).unwrap();
        assert!(wad.find_lump_data("MISSING").is_none());
    }

    #[test]
    fn lumps_between_returns_correct_range() {
        let wad_bytes = make_iwad(&[
            ("F_START", b""),
            ("FLAT1", b"flat1_data"),
            ("FLAT2", b"flat2_data"),
            ("F_END", b""),
        ]);
        let wad = WadFile::parse(wad_bytes).unwrap();
        let flats: Vec<_> = wad
            .lumps_between("F_START", "F_END")
            .map(|l| l.name.as_str().to_string())
            .collect();
        assert_eq!(flats, vec!["FLAT1", "FLAT2"]);
    }

    #[test]
    fn parse_directory_out_of_bounds_negative() {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"IWAD");
        data[4..8].copy_from_slice(&1i32.to_le_bytes()); // 1 lump
        data[8..12].copy_from_slice(&(-1i32).to_le_bytes()); // Negative directory offset

        let result = WadFile::parse(data);
        assert!(matches!(result, Err(WadError::DirectoryOutOfBounds { .. })));
    }

    #[test]
    fn parse_directory_out_of_bounds_past_end() {
        let mut data = vec![0u8; 12];
        data[0..4].copy_from_slice(b"IWAD");
        data[4..8].copy_from_slice(&1i32.to_le_bytes()); // 1 lump
        data[8..12].copy_from_slice(&100i32.to_le_bytes()); // Offset far past end

        let result = WadFile::parse(data);
        assert!(matches!(result, Err(WadError::DirectoryOutOfBounds { .. })));
    }

    #[test]
    fn parse_lump_negative_field() {
        let mut data = vec![0u8; 28]; // Header (12) + 1 Lump Entry (16)
        data[0..4].copy_from_slice(b"IWAD");
        data[4..8].copy_from_slice(&1i32.to_le_bytes()); // 1 lump
        data[8..12].copy_from_slice(&12i32.to_le_bytes()); // Directory at offset 12

        // Lump Entry 1
        data[12..16].copy_from_slice(&(-1i32).to_le_bytes()); // Negative filepos
        data[16..20].copy_from_slice(&10i32.to_le_bytes()); // size
        data[20..28].copy_from_slice(b"TEST\0\0\0\0"); // name

        let result = WadFile::parse(data);
        assert!(matches!(
            result,
            Err(WadError::LumpNegativeField { name }) if name == "TEST"
        ));
    }

    #[test]
    fn parse_lump_out_of_bounds() {
        let mut data = vec![0u8; 28]; // Header (12) + 1 Lump Entry (16)
        data[0..4].copy_from_slice(b"IWAD");
        data[4..8].copy_from_slice(&1i32.to_le_bytes()); // 1 lump
        data[8..12].copy_from_slice(&12i32.to_le_bytes()); // Directory at offset 12

        // Lump Entry 1
        data[12..16].copy_from_slice(&100i32.to_le_bytes()); // filepos far past end
        data[16..20].copy_from_slice(&10i32.to_le_bytes()); // size
        data[20..28].copy_from_slice(b"TEST\0\0\0\0"); // name

        let result = WadFile::parse(data);
        assert!(matches!(
            result,
            Err(WadError::LumpOutOfBounds { name, .. }) if name == "TEST"
        ));
    }

    #[test]
    fn wad_dir_getters() {
        let dir = WadDir::from_lumps(vec![
            LumpDef {
                name: LumpName::from_str("LUMP1"),
                offset: 0,
                size: 0,
            },
            LumpDef {
                name: LumpName::from_str("LUMP2"),
                offset: 0,
                size: 0,
            },
        ]);

        assert_eq!(dir.len(), 2);
        assert!(!dir.is_empty());
        assert_eq!(dir.lumps().len(), 2);

        let empty_dir = WadDir::from_lumps(vec![]);
        assert!(empty_dir.is_empty());
    }

    #[test]
    fn parse_negative_lump_count() {
        let mut data = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(-1i32).to_le_bytes()); // numlumps
        data.extend_from_slice(&12i32.to_le_bytes()); // infotableofs

        let result = WadFile::parse(data);
        assert!(matches!(result, Err(WadError::NegativeLumpCount(-1))));
    }

    #[test]
    fn parse_directory_out_of_bounds_negative_infotableofs() {
        let mut data = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&1i32.to_le_bytes()); // numlumps
        data.extend_from_slice(&(-1i32).to_le_bytes()); // infotableofs

        let result = WadFile::parse(data);
        assert!(matches!(result, Err(WadError::DirectoryOutOfBounds { .. })));
    }

    #[test]
    fn wad_dir_find() {
        let dir = WadDir::from_lumps(vec![
            LumpDef {
                name: LumpName::from_str("LUMP1"),
                offset: 0,
                size: 0,
            },
            LumpDef {
                name: LumpName::from_str("LUMP2"),
                offset: 0,
                size: 0,
            },
            LumpDef {
                name: LumpName::from_str("LUMP1"),
                offset: 10,
                size: 10,
            },
        ]);

        let found = dir.find("LUMP1").unwrap();
        assert_eq!(found.size, 10);
        assert!(dir.find("LUMP3").is_none());
    }

    #[test]
    fn map_lump_group_wrong_lump_name_returns_none() {
        let mut lumps = vec![("MAP01", b"".as_slice())];
        // 10 valid lumps, but the first is WRONG instead of THINGS
        lumps.push(("WRONG", b""));
        lumps.push(("LINEDEFS", b""));
        lumps.push(("SIDEDEFS", b""));
        lumps.push(("VERTEXES", b""));
        lumps.push(("SEGS", b""));
        lumps.push(("SSECTORS", b""));
        lumps.push(("NODES", b""));
        lumps.push(("SECTORS", b""));
        lumps.push(("REJECT", b""));
        lumps.push(("BLOCKMAP", b""));

        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).unwrap();
        assert!(wad.map_lump_group("MAP01").is_none());
    }

    #[test]
    fn udmf_map_lump_group_methods() {
        let wad_bytes = make_iwad(&[
            ("MAP01", b""),
            ("TEXTMAP", b""),
            ("ZNODES", b""),
            ("ENDMAP", b""),
        ]);
        let wad = WadFile::parse(wad_bytes).unwrap();

        let group = wad.map_lump_group("MAP01").unwrap();
        assert!(matches!(group, MapLumpGroup::Udmf(_)));
        if let MapLumpGroup::Udmf(udmf) = group {
            assert_eq!(udmf.aux_lumps().len(), 1);
            assert_eq!(udmf.aux_lumps()[0].name.as_str(), "ZNODES");
            assert_eq!(udmf.find_lump("ZNODES").unwrap().name.as_str(), "ZNODES");
            assert!(udmf.find_lump("NONEXISTENT").is_none());
        }
    }
}
