//! Raw map lump types and byte-level parsers for all standard Doom map lumps.
//!
//! Every struct is parsed field-by-field from `&[u8]` slices using
//! `i16/u16::from_le_bytes` — no `bytemuck::cast_slice` to avoid
//! alignment panics on arbitrary byte slices from the WAD parser.
//!
//! # Lump sizes (from the Unofficial Doom Specs)
//! | Lump      | Entry bytes |
//! |-----------|-------------|
//! | THINGS    | 10          |
//! | LINEDEFS  | 14          |
//! | SIDEDEFS  | 30          |
//! | VERTEXES  | 4           |
//! | SEGS      | 12          |
//! | SSECTORS  | 4           |
//! | NODES     | 28          |
//! | SECTORS   | 26          |
//! | REJECT    | ceil(N²/8)  |
//! | BLOCKMAP  | variable    |

use doom_types::Fixed16_16;
use thiserror::Error;

/// Errors from map-lump parsing.
///
/// ## Examples
/// ```
/// use doom_map::lumps::LumpParseError;
///
/// let err = LumpParseError::BadLength { lump: "VERTEXES", entry_size: 10, actual: 5 };
/// ```
#[derive(Debug, Error)]
pub enum LumpParseError {
    /// Lump byte count is not divisible by the expected entry size.
    #[error("{lump}: expected size divisible by {entry_size}, got {actual}")]
    BadLength {
        /// The name of the lump.
        lump: &'static str,
        /// The expected entry size.
        entry_size: usize,
        /// The actual byte length of the lump.
        actual: usize,
    },

    /// The reject lump is the wrong size for the number of sectors.
    #[error("REJECT: expected {expected} bytes for {n_sectors} sectors, got {actual}")]
    BadRejectSize {
        /// The number of sectors in the map.
        n_sectors: usize,
        /// The expected byte length.
        expected: usize,
        /// The actual byte length.
        actual: usize,
    },

    /// Blockmap header is truncated.
    #[error("BLOCKMAP: lump too short for header ({0} bytes)")]
    BlockmapTooShort(usize),
}

// ---------------------------------------------------------------------------
// THINGS
// ---------------------------------------------------------------------------

/// Linedef flags — impassable to all actors (and projectiles).
pub const FLAG_BLOCKING: u16 = 0x0001;
/// Linedef flags — impassable to monsters only.
pub const FLAG_BLOCKMONSTERS: u16 = 0x0002;
/// Linedef flags bit for two-sided lines.
pub const FLAG_TWO_SIDED: u16 = 0x0004;
/// Linedef flags bit for upper-unpegged textures.
pub const FLAG_DONTPEGTOP: u16 = 0x0008;
/// Linedef flags bit for lower-unpegged / bottom-pegged textures.
pub const FLAG_DONTPEGBOTTOM: u16 = 0x0010;
/// Linedef flags bit for secret lines (`ML_SECRET`): shown as one-sided on the
/// automap and never openable by monsters via `P_UseSpecialLine`.
pub const FLAG_SECRET: u16 = 0x0020;

/// Sentinel value meaning "no sidedef assigned".
pub const SIDEDEF_NONE: u16 = 0xFFFF;

/// A map thing (monster, item, player start, etc.).
///
/// ## Examples
/// ```
/// use doom_map::lumps::Thing;
///
/// let thing = Thing {
///     x: 1056, y: -3616, angle: 90, kind: 3004, flags: 0,
/// };
/// assert_eq!(thing.kind, 3004);
/// ```
#[derive(Clone, Debug)]
pub struct Thing {
    /// X position in map units (i16 range).
    pub x: i16,
    /// Y position in map units.
    pub y: i16,
    /// Facing angle in degrees (0–360, NOT BAM).
    pub angle: u16,
    /// DoomEd thing type number.
    pub kind: u16,
    /// Bit flags (skill levels, deaf, etc.).
    pub flags: u16,
}

impl Thing {
    const BYTE_SIZE: usize = 10;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            x: i16::from_le_bytes([b[0], b[1]]),
            y: i16::from_le_bytes([b[2], b[3]]),
            angle: u16::from_le_bytes([b[4], b[5]]),
            kind: u16::from_le_bytes([b[6], b[7]]),
            flags: u16::from_le_bytes([b[8], b[9]]),
        }
    }

    /// Parse a THINGS lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "THINGS", Self::from_bytes)
    }
}

// ---------------------------------------------------------------------------
// LINEDEFS
// ---------------------------------------------------------------------------

/// A map linedef: connects two vertices, has up to two sidedefs.
///
/// ## Examples
/// ```
/// use doom_map::lumps::Linedef;
///
/// let linedef = Linedef {
///     from_vertex: 0, to_vertex: 1, flags: 1, special: 0, tag: 0,
///     right_sidedef: 0, left_sidedef: 0xFFFF,
/// };
/// assert_eq!(linedef.from_vertex, 0);
/// ```
#[derive(Clone, Debug)]
pub struct Linedef {
    /// Index into VERTEXES for the start point.
    pub from_vertex: u16,
    /// Index into VERTEXES for the end point.
    pub to_vertex: u16,
    /// Bit flags (blocking, two-sided, secret, etc.).
    pub flags: u16,
    /// Special type (door trigger, teleport, etc.).
    pub special: u16,
    /// Sector tag for the special action.
    pub tag: u16,
    /// Right sidedef index (`SIDEDEF_NONE` if absent — always present per spec).
    pub right_sidedef: u16,
    /// Left sidedef index (`SIDEDEF_NONE` if no left side — single-sided line).
    pub left_sidedef: u16,
}

impl Linedef {
    const BYTE_SIZE: usize = 14;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            from_vertex: u16::from_le_bytes([b[0], b[1]]),
            to_vertex: u16::from_le_bytes([b[2], b[3]]),
            flags: u16::from_le_bytes([b[4], b[5]]),
            special: u16::from_le_bytes([b[6], b[7]]),
            tag: u16::from_le_bytes([b[8], b[9]]),
            right_sidedef: u16::from_le_bytes([b[10], b[11]]),
            left_sidedef: u16::from_le_bytes([b[12], b[13]]),
        }
    }

    /// Parse a LINEDEFS lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "LINEDEFS", Self::from_bytes)
    }

    /// Returns `true` if the two-sided flag is set.
    #[inline]
    pub fn is_two_sided(&self) -> bool {
        self.flags & FLAG_TWO_SIDED != 0
    }
}

// ---------------------------------------------------------------------------
// SIDEDEFS
// ---------------------------------------------------------------------------

/// A map sidedef: texture info + sector reference for one side of a linedef.
///
/// ## Examples
/// ```
/// use doom_map::lumps::Sidedef;
///
/// let sidedef = Sidedef {
///     x_offset: 0, y_offset: 0,
///     upper_texture: *b"-\x00\x00\x00\x00\x00\x00\x00",
///     lower_texture: *b"-\x00\x00\x00\x00\x00\x00\x00",
///     middle_texture: *b"BROWN1\x00\x00",
///     sector: 0,
/// };
/// assert_eq!(&sidedef.middle_texture[..6], b"BROWN1");
/// ```
#[derive(Clone, Debug)]
pub struct Sidedef {
    /// Horizontal texture offset.
    pub x_offset: i16,
    /// Vertical texture offset.
    pub y_offset: i16,
    /// Upper texture name (above the window on a two-sided line), null-padded.
    pub upper_texture: [u8; 8],
    /// Lower texture name (below the window on a two-sided line).
    pub lower_texture: [u8; 8],
    /// Middle texture name (solid wall on a one-sided line).
    pub middle_texture: [u8; 8],
    /// Index into SECTORS that this sidedef faces.
    pub sector: u16,
}

impl Sidedef {
    const BYTE_SIZE: usize = 30;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            x_offset: i16::from_le_bytes([b[0], b[1]]),
            y_offset: i16::from_le_bytes([b[2], b[3]]),
            upper_texture: b[4..12].try_into().unwrap(),
            lower_texture: b[12..20].try_into().unwrap(),
            middle_texture: b[20..28].try_into().unwrap(),
            sector: u16::from_le_bytes([b[28], b[29]]),
        }
    }

    /// Parse a SIDEDEFS lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "SIDEDEFS", Self::from_bytes)
    }
}

// ---------------------------------------------------------------------------
// VERTEXES
// ---------------------------------------------------------------------------

/// A map vertex: raw (x, y) in i16 map-unit coordinates.
///
/// ## Examples
/// ```
/// use doom_map::lumps::Vertex;
///
/// let v = Vertex { x: 1088, y: -3680 };
/// assert_eq!(v.x, 1088);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vertex {
    /// The X position in the map's coordinate space.
    pub x: i16,
    /// The Y position in the map's coordinate space.
    pub y: i16,
}

impl Vertex {
    const BYTE_SIZE: usize = 4;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            x: i16::from_le_bytes([b[0], b[1]]),
            y: i16::from_le_bytes([b[2], b[3]]),
        }
    }

    /// Parse a VERTEXES lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "VERTEXES", Self::from_bytes)
    }
}

// ---------------------------------------------------------------------------
// SEGS
// ---------------------------------------------------------------------------

/// A BSP seg: a portion of a linedef used during rendering.
///
/// ## Examples
/// ```
/// use doom_map::lumps::Seg;
///
/// let seg = Seg {
///     from_vertex: 0, to_vertex: 1, angle: 16384, linedef: 0, direction: 0, offset: 0,
/// };
/// assert_eq!(seg.linedef, 0);
/// ```
#[derive(Clone, Debug)]
pub struct Seg {
    /// Index into VERTEXES for the seg start.
    pub from_vertex: u16,
    /// Index into VERTEXES for the seg end.
    pub to_vertex: u16,
    /// Facing angle (16-bit BAM: `bam16 << 16` to get a full 32-bit Bam).
    pub angle: u16,
    /// Linedef this seg is part of.
    pub linedef: u16,
    /// 0 = same direction as linedef, 1 = opposite.
    pub direction: u16,
    /// Distance along the linedef to the seg start (in fixed-point map units >> 16).
    pub offset: u16,
}

impl Seg {
    const BYTE_SIZE: usize = 12;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            from_vertex: u16::from_le_bytes([b[0], b[1]]),
            to_vertex: u16::from_le_bytes([b[2], b[3]]),
            angle: u16::from_le_bytes([b[4], b[5]]),
            linedef: u16::from_le_bytes([b[6], b[7]]),
            direction: u16::from_le_bytes([b[8], b[9]]),
            offset: u16::from_le_bytes([b[10], b[11]]),
        }
    }

    /// Parse a SEGS lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "SEGS", Self::from_bytes)
    }
}

// ---------------------------------------------------------------------------
// SSECTORS (subsectors)
// ---------------------------------------------------------------------------

/// A BSP leaf: a convex sub-region of the map covered by a run of segs.
///
/// ## Examples
/// ```
/// use doom_map::lumps::Ssector;
///
/// let ssector = Ssector {
///     seg_count: 5, first_seg: 0,
/// };
/// assert_eq!(ssector.seg_count, 5);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct Ssector {
    /// Number of segs in this subsector.
    pub seg_count: u16,
    /// Index of the first seg in SEGS.
    pub first_seg: u16,
}

impl Ssector {
    const BYTE_SIZE: usize = 4;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            seg_count: u16::from_le_bytes([b[0], b[1]]),
            first_seg: u16::from_le_bytes([b[2], b[3]]),
        }
    }

    /// Parse a SSECTORS lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "SSECTORS", Self::from_bytes)
    }

    /// Exclusive end index into SEGS: `first_seg + seg_count`.
    #[inline]
    pub fn seg_end(self) -> usize {
        self.first_seg as usize + self.seg_count as usize
    }
}

// ---------------------------------------------------------------------------
// NODES
// ---------------------------------------------------------------------------

/// The bit flag in a child pointer that signals "this child is a subsector".
pub const NODE_SUBSECTOR_BIT: u16 = 0x8000;

/// The mask to extract the actual index from a child pointer.
pub const NODE_INDEX_MASK: u16 = 0x7FFF;

/// An axis-aligned bounding box in node format (ymax, ymin, xmin, xmax).
///
/// ## Examples
/// ```
/// use doom_map::lumps::NodeBBox;
///
/// let bbox = NodeBBox {
///     ymax: 100, ymin: 0, xmin: 0, xmax: 100,
/// };
/// assert_eq!(bbox.ymax, 100);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct NodeBBox {
    /// The upper edge (North) of the bounding box.
    pub ymax: i16,
    /// The lower edge (South) of the bounding box.
    pub ymin: i16,
    /// The left edge (West) of the bounding box.
    pub xmin: i16,
    /// The right edge (East) of the bounding box.
    pub xmax: i16,
}

impl NodeBBox {
    /// Returns `true` if the invariant holds: ymax ≥ ymin ∧ xmax ≥ xmin.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.ymax >= self.ymin && self.xmax >= self.xmin
    }

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            ymax: i16::from_le_bytes([b[0], b[1]]),
            ymin: i16::from_le_bytes([b[2], b[3]]),
            xmin: i16::from_le_bytes([b[4], b[5]]),
            xmax: i16::from_le_bytes([b[6], b[7]]),
        }
    }
}

/// A BSP node: a partition line and two child pointers.
///
/// ## Examples
/// ```
/// use doom_map::lumps::{Node, NodeBBox};
///
/// let node = Node {
///     x: 0, y: 0, dx: 64, dy: 64,
///     right_bbox: NodeBBox { ymax: 64, ymin: 0, xmin: 0, xmax: 64 },
///     left_bbox: NodeBBox { ymax: 64, ymin: 0, xmin: 0, xmax: 64 },
///     right_child: 0,
///     left_child: 1,
/// };
/// assert_eq!(node.dx, 64);
/// ```
#[derive(Clone, Debug)]
pub struct Node {
    /// X coordinate of the partition line's starting point.
    pub x: i16,
    /// Y coordinate of the partition line's starting point.
    pub y: i16,
    /// Delta X of the partition line direction.
    pub dx: i16,
    /// Delta Y of the partition line direction.
    pub dy: i16,
    /// Bounding box of the right (front) child's space.
    pub right_bbox: NodeBBox,
    /// Bounding box of the left (back) child's space.
    pub left_bbox: NodeBBox,
    /// Right child pointer (bit 15 = subsector flag, bits 14-0 = index).
    pub right_child: u16,
    /// Left child pointer.
    pub left_child: u16,
}

impl Node {
    const BYTE_SIZE: usize = 28;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            x: i16::from_le_bytes([b[0], b[1]]),
            y: i16::from_le_bytes([b[2], b[3]]),
            dx: i16::from_le_bytes([b[4], b[5]]),
            dy: i16::from_le_bytes([b[6], b[7]]),
            right_bbox: NodeBBox::from_bytes(&b[8..16]),
            left_bbox: NodeBBox::from_bytes(&b[16..24]),
            right_child: u16::from_le_bytes([b[24], b[25]]),
            left_child: u16::from_le_bytes([b[26], b[27]]),
        }
    }

    /// Parse a NODES lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "NODES", Self::from_bytes)
    }
}

// ---------------------------------------------------------------------------
// SECTORS
// ---------------------------------------------------------------------------

/// A map sector: floor/ceiling heights, textures, light level, specials.
///
/// ## Examples
/// ```
/// use doom_map::lumps::Sector;
/// use doom_types::Fixed16_16;
///
/// let sector = Sector {
///     floor_height: Fixed16_16::from_int(0), ceil_height: Fixed16_16::from_int(128),
///     floor_flat: *b"FLOOR4_8",
///     ceil_flat: *b"CEIL3_5\x00",
///     light_level: 144, special: 0, tag: 0,
/// };
/// assert_eq!(sector.floor_height, Fixed16_16::from_int(0));
/// ```
#[derive(Clone, Debug)]
pub struct Sector {
    /// Floor height in fixed-point map units (`Fixed16_16`; loaded as
    /// `map_unit << FRACBITS`, always an integer multiple of `FIXED_ONE`).
    pub floor_height: Fixed16_16,
    /// Ceiling height in fixed-point map units (`Fixed16_16`).
    pub ceil_height: Fixed16_16,
    /// Floor flat texture name.
    pub floor_flat: [u8; 8],
    /// Ceiling flat texture name.
    pub ceil_flat: [u8; 8],
    /// Light level (0–255 in practice, though stored as i16).
    pub light_level: i16,
    /// Sector special type (0 = normal, 1–16 = various effects).
    pub special: u16,
    /// Sector tag (for linedef trigger targeting).
    pub tag: u16,
}

impl Sector {
    const BYTE_SIZE: usize = 26;

    fn from_bytes(b: &[u8]) -> Self {
        Self {
            floor_height: Fixed16_16::from_int(i16::from_le_bytes([b[0], b[1]]) as i32),
            ceil_height: Fixed16_16::from_int(i16::from_le_bytes([b[2], b[3]]) as i32),
            floor_flat: b[4..12].try_into().unwrap(),
            ceil_flat: b[12..20].try_into().unwrap(),
            light_level: i16::from_le_bytes([b[20], b[21]]),
            special: u16::from_le_bytes([b[22], b[23]]),
            tag: u16::from_le_bytes([b[24], b[25]]),
        }
    }

    /// Parse a SECTORS lump.
    pub fn parse_lump(data: &[u8]) -> Result<Vec<Self>, LumpParseError> {
        parse_fixed_records(data, Self::BYTE_SIZE, "SECTORS", Self::from_bytes)
    }
}

// ---------------------------------------------------------------------------
// REJECT
// ---------------------------------------------------------------------------

/// The sector visibility reject table.
///
/// Bit `sector_i * n_sectors + sector_j` being set means sector `i` cannot
/// see sector `j` — the engine can skip LOS checks between them.
///
/// Size: `ceil(n_sectors² / 8)` bytes.
///
/// ## Examples
/// ```
/// use doom_map::lumps::Reject;
///
/// let data = vec![0b00000000];
/// let reject = Reject::parse_lump(&data, 2).unwrap();
/// assert!(reject.visible(0, 1));
/// ```
#[derive(Clone, Debug)]
pub struct Reject {
    n_sectors: usize,
    data: Vec<u8>,
}

impl Reject {
    /// Parse a REJECT lump given the number of sectors.
    ///
    /// # Errors
    /// Returns `LumpParseError::BadRejectSize` if the lump size doesn't match,
    /// or if the required sector size overflows.
    pub fn parse_lump(data: &[u8], n_sectors: usize) -> Result<Self, LumpParseError> {
        let expected = n_sectors
            .checked_mul(n_sectors)
            .map(|sq| sq.div_ceil(8))
            .ok_or(LumpParseError::BadRejectSize {
                n_sectors,
                expected: usize::MAX,
                actual: data.len(),
            })?;
        if data.len() != expected {
            return Err(LumpParseError::BadRejectSize {
                n_sectors,
                expected,
                actual: data.len(),
            });
        }
        Ok(Self {
            n_sectors,
            data: data.to_vec(),
        })
    }

    /// Returns `true` if sectors `a` and `b` might be mutually visible
    /// (i.e., the reject bit is NOT set).
    pub fn visible(&self, a: usize, b: usize) -> bool {
        if a >= self.n_sectors || b >= self.n_sectors {
            return false;
        }
        let bit_idx = a * self.n_sectors + b;
        let byte = bit_idx / 8;
        let bit = bit_idx % 8;
        (self.data[byte] >> bit) & 1 == 0
    }

    /// Number of sectors this reject was built for.
    pub fn n_sectors(&self) -> usize {
        self.n_sectors
    }
}

// ---------------------------------------------------------------------------
// BLOCKMAP
// ---------------------------------------------------------------------------

/// The blockmap header and offset table.
///
/// The blockmap divides the map into 128×128 unit cells.
/// Each cell has a list of linedefs that cross it (used for collision detection).
///
/// ## Examples
/// ```
/// use doom_map::lumps::Blockmap;
///
/// let mut data = vec![0u8; 14];
/// // Setup minimal blockmap: origin (0, 0), columns=1, rows=1
/// data[4..6].copy_from_slice(&1u16.to_le_bytes());
/// data[6..8].copy_from_slice(&1u16.to_le_bytes());
/// data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset
/// data[10..12].copy_from_slice(&0u16.to_le_bytes()); // list start
/// data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // list end
///
/// let blockmap = Blockmap::parse_lump(&data).unwrap();
/// let mut it = blockmap.block_linedefs(0, 0);
/// // Vanilla treats the leading 0 word as linedef 0 (present in every cell).
/// assert_eq!(it.next(), Some(0));
/// assert_eq!(it.next(), None);
/// ```
#[derive(Clone, Debug)]
pub struct Blockmap {
    /// X origin of the grid.
    pub x_origin: i16,
    /// Y origin of the grid.
    pub y_origin: i16,
    /// Number of columns.
    pub x_count: u16,
    /// Number of rows.
    pub y_count: u16,
    /// Offsets (in 16-bit words from lump start) to each block's linedef list.
    pub offsets: Vec<u16>,
    /// Raw lump bytes (for linedef list lookup).
    raw: Vec<u8>,
}

impl Blockmap {
    const HEADER_BYTES: usize = 8;

    /// Parse a BLOCKMAP lump.
    pub fn parse_lump(data: &[u8]) -> Result<Self, LumpParseError> {
        if data.len() < Self::HEADER_BYTES {
            return Err(LumpParseError::BlockmapTooShort(data.len()));
        }
        let x_origin = i16::from_le_bytes([data[0], data[1]]);
        let y_origin = i16::from_le_bytes([data[2], data[3]]);
        let x_count = u16::from_le_bytes([data[4], data[5]]);
        let y_count = u16::from_le_bytes([data[6], data[7]]);

        let n_blocks = x_count as usize * y_count as usize;

        // Prevent OOM from large x_count/y_count values by clamping to physical size
        let max_possible = data.len().saturating_sub(Self::HEADER_BYTES) / 2;
        let n_offsets = n_blocks.min(max_possible);

        let mut offsets = Vec::with_capacity(n_offsets);

        let offsets_end_actual = Self::HEADER_BYTES + n_offsets * 2;
        let offset_bytes = &data[Self::HEADER_BYTES..offsets_end_actual];

        for chunk in offset_bytes.chunks_exact(2) {
            offsets.push(u16::from_le_bytes([chunk[0], chunk[1]]));
        }

        Ok(Self {
            x_origin,
            y_origin,
            x_count,
            y_count,
            offsets,
            raw: data.to_vec(),
        })
    }

    /// Iterate over the linedef indices in the block at column `col`, row `row`.
    ///
    /// Returns an empty iterator if the block index is out of range or the
    /// offset points past the lump.
    pub fn block_linedefs(&self, col: usize, row: usize) -> impl Iterator<Item = u16> + '_ {
        let idx = row * self.x_count as usize + col;
        let data = &self.raw;

        let mut pos = match self.offsets.get(idx).copied() {
            Some(o) => (o as usize) * 2,
            None => data.len(), // out of bounds, start at EOF
        };

        // Vanilla `P_BlockLinesIterator` reads the block list starting at
        // `blockmaplump + offset` with no adjustment, iterating linedef indices
        // until the 0xFFFF terminator (`for (list = blockmaplump+offset;
        // *list != -1; list++)`, p_maputl.c).  The standard Doom BLOCKMAP format
        // places a 0 word at the start of every block list, so vanilla treats
        // that leading word as *linedef 0* and processes linedef 0 in every
        // cell.  We must NOT skip it: omitting linedef 0 causes blockmap-driven
        // traces (e.g. P_SlideMove's PTR_SlideTraverse, which side-tests the
        // infinite line and has no bbox guard) to miss line 0 and slide along a
        // different wall than vanilla, desyncing demos.

        std::iter::from_fn(move || {
            if pos + 1 >= data.len() {
                return None;
            }
            let val = u16::from_le_bytes([data[pos], data[pos + 1]]);
            pos += 2;
            if val == 0xFFFF { None } else { Some(val) }
        })
    }
}

// ---------------------------------------------------------------------------
// Shared helper
// ---------------------------------------------------------------------------

fn parse_fixed_records<T, F>(
    data: &[u8],
    entry_size: usize,
    lump_name: &'static str,
    f: F,
) -> Result<Vec<T>, LumpParseError>
where
    F: Fn(&[u8]) -> T,
{
    if !data.len().is_multiple_of(entry_size) {
        return Err(LumpParseError::BadLength {
            lump: lump_name,
            entry_size,
            actual: data.len(),
        });
    }
    Ok(data.chunks_exact(entry_size).map(f).collect())
}

// ---------------------------------------------------------------------------
// Proptest property tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// For an all-zero reject table (everything visible), `visible(a, b)`
        /// must equal `visible(b, a)` — the relation is symmetric.
        ///
        /// Note: symmetry is a property of the *data*, not structurally
        /// guaranteed by the format.  Doom's BSP builder always writes
        /// symmetric tables.  We verify this for the all-zero (all-visible)
        /// case which maps cleanly to a property test.
        #[test]
        fn reject_visible_symmetric_all_zero(
            n_sectors in 1usize..=8,
            a in 0usize..8,
            b in 0usize..8,
        ) {
            // Clamp a and b to valid sector indices for this test.
            let a = a % n_sectors;
            let b = b % n_sectors;
            // All-zero reject: every bit is 0 → all pairs visible.
            let size = (n_sectors * n_sectors).div_ceil(8);
            let data = vec![0u8; size];
            let reject = Reject::parse_lump(&data, n_sectors).expect("value must exist in test");
            let vis_ab = reject.visible(a, b);
            let vis_ba = reject.visible(b, a);
            prop_assert!(
                vis_ab == vis_ba,
                "visible({},{}) = {} != visible({},{}) = {} for n_sectors={}",
                a, b, vis_ab, b, a, vis_ba, n_sectors
            );
        }

        /// For an all-zero reject table, `visible(a, a)` must be `true` —
        /// a sector is always visible from itself.
        #[test]
        fn reject_sector_always_visible_from_itself_all_zero(
            n_sectors in 1usize..=16,
            a in 0usize..16,
        ) {
            let a = a % n_sectors;
            let size = (n_sectors * n_sectors).div_ceil(8);
            let data = vec![0u8; size];
            let reject = Reject::parse_lump(&data, n_sectors).expect("value must exist in test");
            prop_assert!(
                reject.visible(a, a),
                "sector {} should be visible from itself", a
            );
        }

        /// `visible` with out-of-range indices must return `false`, not panic.
        #[test]
        fn reject_out_of_range_returns_false(
            n_sectors in 1usize..=4,
            a in 100usize..=200,
            b in 100usize..=200,
        ) {
            let size = (n_sectors * n_sectors).div_ceil(8);
            let data = vec![0u8; size];
            let reject = Reject::parse_lump(&data, n_sectors).expect("value must exist in test");
            // a and b are deliberately far out of range.
            prop_assert!(!reject.visible(a, b));
        }

        /// All-ones reject (nothing visible): `visible(a, b)` is always `false`
        /// for valid in-range indices.
        #[test]
        fn reject_all_ones_never_visible(
            n_sectors in 1usize..=8,
            a in 0usize..8,
            b in 0usize..8,
        ) {
            let a = a % n_sectors;
            let b = b % n_sectors;
            let size = (n_sectors * n_sectors).div_ceil(8);
            let data = vec![0xFFu8; size];
            let reject = Reject::parse_lump(&data, n_sectors).expect("value must exist in test");
            prop_assert!(
                !reject.visible(a, b),
                "all-ones reject must report not-visible for ({},{})", a, b
            );
        }

        /// `Reject::parse_lump` must accept any data buffer of exactly the
        /// expected size `ceil(n² / 8)`.
        #[test]
        fn reject_parse_accepts_correctly_sized_buffer(
            n_sectors in 1usize..=10,
            fill_byte in 0u8..=255u8,
        ) {
            let size = (n_sectors * n_sectors).div_ceil(8);
            let data = vec![fill_byte; size];
            prop_assert!(
                Reject::parse_lump(&data, n_sectors).is_ok(),
                "correctly-sized buffer should parse successfully"
            );
        }

        /// `Reject::parse_lump` must reject any buffer whose size does not
        /// match `ceil(n² / 8)`.
        #[test]
        fn reject_parse_rejects_wrong_size(
            n_sectors in 2usize..=8,
            // Offset the size by at least 1 in either direction.
            offset in 1usize..=4,
            add_not_sub in any::<bool>(),
        ) {
            let expected: usize = (n_sectors * n_sectors).div_ceil(8);
            let wrong_size = if add_not_sub {
                expected + offset
            } else {
                expected.saturating_sub(offset)
            };
            if wrong_size == expected {
                return Ok(()); // saturating_sub hit 0 == expected; skip
            }
            let data = vec![0u8; wrong_size];
            prop_assert!(
                Reject::parse_lump(&data, n_sectors).is_err(),
                "wrong-sized buffer (expected={}, got={}) should fail", expected, wrong_size
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_thing() {
        // x=100, y=200, angle=90, kind=1, flags=7
        let mut data = [0u8; 10];
        data[0..2].copy_from_slice(&100i16.to_le_bytes());
        data[2..4].copy_from_slice(&200i16.to_le_bytes());
        data[4..6].copy_from_slice(&90u16.to_le_bytes());
        data[6..8].copy_from_slice(&1u16.to_le_bytes());
        data[8..10].copy_from_slice(&7u16.to_le_bytes());
        let things = Thing::parse_lump(&data).expect("value must exist in test");
        assert_eq!(things.len(), 1);
        assert_eq!(things[0].x, 100);
        assert_eq!(things[0].y, 200);
    }

    /// Regression: vanilla `P_BlockLinesIterator` reads the block list directly
    /// from `blockmaplump + offset` and treats the leading 0 word (present in
    /// every standard Doom BLOCKMAP block list) as *linedef 0*. Every cell must
    /// therefore yield linedef 0 first. Skipping it (as we used to) makes
    /// blockmap traces such as `P_SlideMove` miss line 0 and desync demos.
    #[test]
    fn block_linedefs_includes_leading_linedef_zero() {
        // 2 blocks. Block 0 list: [0, 0xFFFF]. Block 1 list: [0, 7, 0xFFFF].
        // header(8) + offsets(2*2=4) => list data starts at word index 6.
        // Block0 list at word 6 (bytes 12), Block1 list at word 8 (bytes 16).
        let mut data = vec![0u8; 8 + 4 + 2 * 2 + 3 * 2];
        data[4..6].copy_from_slice(&2u16.to_le_bytes()); // columns = 2
        data[6..8].copy_from_slice(&1u16.to_le_bytes()); // rows = 1
        data[8..10].copy_from_slice(&6u16.to_le_bytes()); // block0 offset (words)
        data[10..12].copy_from_slice(&8u16.to_le_bytes()); // block1 offset (words)
        // block0 list @ bytes 12: 0x0000, 0xFFFF
        data[12..14].copy_from_slice(&0x0000u16.to_le_bytes());
        data[14..16].copy_from_slice(&0xFFFFu16.to_le_bytes());
        // block1 list @ bytes 16: 0x0000, 7, 0xFFFF
        data[16..18].copy_from_slice(&0x0000u16.to_le_bytes());
        data[18..20].copy_from_slice(&7u16.to_le_bytes());
        data[20..22].copy_from_slice(&0xFFFFu16.to_le_bytes());

        let bm = Blockmap::parse_lump(&data).expect("blockmap parse");
        // Cell (0,0): linedef 0 only (the leading word), NOT skipped.
        assert_eq!(bm.block_linedefs(0, 0).collect::<Vec<_>>(), vec![0]);
        // Cell (1,0): leading linedef 0 followed by the genuine member 7.
        assert_eq!(bm.block_linedefs(1, 0).collect::<Vec<_>>(), vec![0, 7]);
    }

    #[test]
    fn parse_vertex_pair() {
        let mut data = [0u8; 8];
        data[0..2].copy_from_slice(&(-10i16).to_le_bytes());
        data[2..4].copy_from_slice(&20i16.to_le_bytes());
        data[4..6].copy_from_slice(&30i16.to_le_bytes());
        data[6..8].copy_from_slice(&(-40i16).to_le_bytes());
        let verts = Vertex::parse_lump(&data).expect("value must exist in test");
        assert_eq!(verts.len(), 2);
        assert_eq!(verts[0], Vertex { x: -10, y: 20 });
        assert_eq!(verts[1], Vertex { x: 30, y: -40 });
    }

    #[test]
    fn ssector_seg_end() {
        let ss = Ssector {
            seg_count: 5,
            first_seg: 3,
        };
        assert_eq!(ss.seg_end(), 8);
    }

    #[test]
    fn node_subsector_bit_decode() {
        let child = NODE_SUBSECTOR_BIT | 7;
        assert_ne!(child & NODE_SUBSECTOR_BIT, 0);
        assert_eq!(child & NODE_INDEX_MASK, 7);
    }

    #[test]
    fn reject_visible_symmetry() {
        // All-zero reject → everything visible.
        let data = vec![0u8; (4usize * 4).div_ceil(8)]; // 4 sectors
        let reject = Reject::parse_lump(&data, 4).expect("value must exist in test");
        for i in 0..4 {
            for j in 0..4 {
                assert!(reject.visible(i, j));
                assert_eq!(reject.visible(i, j), reject.visible(j, i));
            }
        }
    }

    #[test]
    fn reject_bad_size_errors() {
        let data = vec![0u8; 5];
        assert!(Reject::parse_lump(&data, 4).is_err()); // expects 2 bytes
    }

    #[test]
    fn bad_lump_length_errors() {
        assert!(Thing::parse_lump(&[0u8; 7]).is_err()); // 7 not divisible by 10
    }
}
