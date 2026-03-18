//! Lump descriptor types for the WAD directory.
//!
//! A lump is a named, contiguous region of bytes within the WAD file.
//! The directory entry is exactly 16 bytes — laid out as:
//!
//! ```text
//! offset: i32 LE  (4 bytes)
//! size:   i32 LE  (4 bytes)
//! name:   [u8; 8] (8 bytes, null-padded)
//! ```
//!
//! # Verus invariant (runtime-checked here, formally verified in `proofs/`)
//! - `∀ i: lump[i].offset + lump[i].size ≤ file.len()`
//! - `∀ i≠j: disjoint(lump_range(i), lump_range(j))`

use bytemuck::{Pod, Zeroable};

/// Raw 16-byte directory entry as it appears on disk (little-endian).
///
/// `Pod` allows zero-copy casting from `&[u8]` → `&[RawLumpEntry]`.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct RawLumpEntry {
    /// Byte offset of lump data from start of WAD file.
    pub filepos: i32,
    /// Size of lump data in bytes (0 is valid — marker lump).
    pub size: i32,
    /// Lump name, null-padded to 8 bytes.  Not necessarily UTF-8.
    pub name: [u8; 8],
}

impl RawLumpEntry {
    /// Byte range `[filepos, filepos + size)` as `usize` pair.
    ///
    /// Returns `None` if either field is negative (malformed WAD).
    pub fn byte_range(self) -> Option<(usize, usize)> {
        if self.filepos < 0 || self.size < 0 {
            return None;
        }
        let start = self.filepos as usize;
        let end = start.checked_add(self.size as usize)?;
        Some((start, end))
    }
}

/// An 8-character lump name, uppercase, null-padded.
///
/// Names are case-insensitive in the original engine; we store them
/// normalized to uppercase for consistent key comparisons.
///
/// ## Examples
///
/// ```
/// use doom_wad::lump::LumpName;
///
/// let name = LumpName::from_str("PlayPal");
/// assert_eq!(name.as_str(), "PLAYPAL");
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct LumpName([u8; 8]);

impl LumpName {
    /// Construct from a raw name field, uppercasing ASCII letters.
    ///
    /// The input must be an 8-byte array. Any bytes after the first null byte
    /// are ignored and zero-filled.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_wad::lump::LumpName;
    ///
    /// let raw = *b"playpal\0";
    /// let name = LumpName::from_raw(raw);
    /// assert_eq!(name.as_str(), "PLAYPAL");
    /// ```
    pub fn from_raw(raw: [u8; 8]) -> Self {
        let mut buf = raw;
        let mut seen_null = false;
        for byte in &mut buf {
            if *byte == 0 {
                seen_null = true;
            }
            if seen_null {
                *byte = 0; // zero-fill after first null
            } else {
                *byte = byte.to_ascii_uppercase();
            }
        }
        Self(buf)
    }

    /// Construct from a string slice.
    ///
    /// The input string is truncated to 8 bytes. All characters are uppercased.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_wad::lump::LumpName;
    ///
    /// let name = LumpName::from_str("Demo1");
    /// assert_eq!(name.as_str(), "DEMO1");
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let mut buf = [0u8; 8];
        for (i, ch) in s.bytes().take(8).enumerate() {
            buf[i] = ch.to_ascii_uppercase();
        }
        Self(buf)
    }

    /// Returns the name as a string slice (`&str`), trimming trailing null bytes.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_wad::lump::LumpName;
    ///
    /// let name = LumpName::from_str("E1M1");
    /// assert_eq!(name.as_str(), "E1M1");
    /// ```
    pub fn as_str(&self) -> &str {
        let len = self.0.iter().position(|&b| b == 0).unwrap_or(8);
        // SAFETY: we only store ASCII uppercase; valid UTF-8.
        core::str::from_utf8(&self.0[..len]).unwrap_or("")
    }

    /// Raw bytes.
    pub fn raw(&self) -> &[u8; 8] {
        &self.0
    }
}

impl core::fmt::Debug for LumpName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "LumpName({:?})", self.as_str())
    }
}

impl core::fmt::Display for LumpName {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A validated, resolved lump descriptor.
///
/// Created by `WadFile::parse()` after verifying bounds invariants.
#[derive(Clone, Debug)]
pub struct LumpDef {
    /// Lump name (normalized uppercase).
    pub name: LumpName,
    /// Byte offset from start of WAD file.
    pub offset: usize,
    /// Byte count of lump data.  0 for marker lumps.
    pub size: usize,
}

impl LumpDef {
    /// Exclusive end byte: `offset + size`.
    #[inline]
    pub fn end(&self) -> usize {
        self.offset + self.size
    }

    /// Returns `true` if this lump is a marker (zero-size).
    #[inline]
    pub fn is_marker(&self) -> bool {
        self.size == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lump_name_normalizes_case() {
        let mut raw = [0u8; 8];
        raw[..8].copy_from_slice(b"playpal\0");
        let name = LumpName::from_raw(raw);
        assert_eq!(name.as_str(), "PLAYPAL");
    }

    #[test]
    fn lump_name_from_str() {
        assert_eq!(LumpName::from_str("E1M1").as_str(), "E1M1");
    }

    #[test]
    fn lump_name_truncates_at_8() {
        let name = LumpName::from_str("TOOLONGNAME");
        assert_eq!(name.as_str().len(), 8);
    }

    #[test]
    fn raw_entry_byte_range() {
        let entry = RawLumpEntry {
            filepos: 100,
            size: 50,
            name: [0; 8],
        };
        assert_eq!(entry.byte_range(), Some((100, 150)));
    }

    #[test]
    fn raw_entry_negative_filepos_returns_none() {
        let entry = RawLumpEntry {
            filepos: -1,
            size: 10,
            name: [0; 8],
        };
        assert!(entry.byte_range().is_none());
    }

    #[test]
    fn raw_entry_zero_size_valid() {
        let entry = RawLumpEntry {
            filepos: 200,
            size: 0,
            name: [0; 8],
        };
        assert_eq!(entry.byte_range(), Some((200, 200)));
    }
}
