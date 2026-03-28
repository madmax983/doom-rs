//! UDMF (Universal Doom Map Format) TEXTMAP parsing and conversion.
//!
//! Phase 1 support is intentionally narrow:
//! - `namespace = "doom"` only
//! - geometry comes from `vertex`, `linedef`, `sidedef`, `sector`, and `thing` blocks
//! - BSP/collision lumps remain binary auxiliary lumps loaded by `Level::from_wad`
//! - unknown fields are ignored
//! - non-integral values for classic integer fields are rejected

use crate::lumps::{
    FLAG_BLOCKING, FLAG_BLOCKMONSTERS, FLAG_DONTPEGBOTTOM, FLAG_DONTPEGTOP, FLAG_TWO_SIDED,
    Linedef, Sector, Sidedef, Thing, Vertex,
};

const FLAG_SECRET: u16 = 0x0020;
const FLAG_SOUNDBLOCK: u16 = 0x0040;
const FLAG_DONTDRAW: u16 = 0x0080;
const FLAG_MAPPED: u16 = 0x0100;
const THING_FLAG_EASY: u16 = 0x0001;
const THING_FLAG_MEDIUM: u16 = 0x0002;
const THING_FLAG_HARD: u16 = 0x0004;
const THING_FLAG_AMBUSH: u16 = 0x0008;
const THING_FLAG_MULTIPLAYER: u16 = 0x0010;
const SIDEDEF_NONE: u16 = 0xFFFF;

/// A UDMF value: string, integer, float, or boolean.
///
/// Universal Doom Map Format allows variables to have these dynamic types.
/// The parser will decode numeric literals according to whether a decimal point
/// or exponent is present, otherwise they become integers.
#[derive(Debug, Clone, PartialEq)]
pub enum UdmfValue {
    /// Quoted string.
    Str(String),
    /// Integer literal.
    Int(i64),
    /// Floating-point literal.
    Float(f64),
    /// Boolean `true` or `false`.
    Bool(bool),
}

/// A single key = value assignment within a UDMF block.
///
/// e.g. `x = 10.0;`
#[derive(Debug, Clone)]
pub struct UdmfField {
    /// The property name.
    pub key: String,
    /// The assigned value.
    pub value: UdmfValue,
}

/// A named block (e.g. `vertex { x = 10.0; y = 20.0; }`).
///
/// UDMF blocks contain properties mapped to key-value pairs.
#[derive(Debug, Clone)]
pub struct UdmfBlock {
    /// The block name (e.g. "vertex", "sector").
    pub kind: String,
    /// The key-value fields contained in this block.
    pub fields: Vec<UdmfField>,
}

/// The fully parsed TEXTMAP lump.
///
/// This represents a raw, unvalidated AST of the `TEXTMAP` file.
/// To convert this into usable map geometry, call [`UdmfMap::into_level_data`].
#[derive(Debug, Clone)]
pub struct UdmfMap {
    /// The namespace declaration (first statement in the file).
    pub namespace: String,
    /// All top-level blocks in order.
    pub blocks: Vec<UdmfBlock>,
}

/// Geometry converted from UDMF into the engine's classic `Level`-shaped arrays.
///
/// This provides the canonical flat arrays of vertexes, sectors, linedefs, etc.
/// that the rest of the engine expects, identical in shape to what a classic
/// binary Doom format WAD would provide.
#[derive(Debug)]
pub struct UdmfLevelData {
    /// Level objects like monsters, items, and player starts.
    pub things: Vec<Thing>,
    /// Line segments connecting vertices to form geometry walls.
    pub linedefs: Vec<Linedef>,
    /// Visual definitions for walls on one or both sides of a linedef.
    pub sidedefs: Vec<Sidedef>,
    /// 2D points in space that linedefs connect.
    pub vertexes: Vec<Vertex>,
    /// Areas of the map with distinct floor/ceiling heights, textures, and lighting.
    pub sectors: Vec<Sector>,
}

/// Parse errors from the UDMF TEXTMAP parser and converter.
#[derive(Debug, thiserror::Error)]
pub enum UdmfError {
    /// The textmap is not valid UTF-8.
    #[error("UDMF TEXTMAP is not valid UTF-8")]
    InvalidUtf8(#[from] core::str::Utf8Error),

    /// A syntax error occurred during parsing.
    #[error("UDMF parse error near offset {offset}: {message}")]
    ParseFailed {
        /// Byte offset where the parse failed.
        offset: usize,
        /// Error description.
        message: String,
    },

    /// The textmap is missing the required namespace declaration.
    #[error("UDMF missing required namespace assignment")]
    MissingNamespace,

    /// The declared namespace is not supported by the parser.
    #[error("unsupported UDMF namespace '{0}'")]
    UnsupportedNamespace(String),

    /// A required field is missing from a block.
    #[error("UDMF {block}[{index}] missing required field '{field}'")]
    MissingField {
        /// The block type (e.g. "vertex").
        block: &'static str,
        /// The index of the block.
        index: usize,
        /// The missing field name.
        field: &'static str,
    },

    /// A field was provided with the wrong type.
    #[error("UDMF {block}[{index}] field '{field}' must be {expected}")]
    WrongType {
        /// The block type (e.g. "vertex").
        block: &'static str,
        /// The index of the block.
        index: usize,
        /// The field name.
        field: &'static str,
        /// A description of the expected type.
        expected: &'static str,
    },

    /// A fractional number was supplied where an integer is required.
    #[error("UDMF {block}[{index}] field '{field}' must be integral, got {value}")]
    NonIntegral {
        /// The block type (e.g. "vertex").
        block: &'static str,
        /// The index of the block.
        index: usize,
        /// The field name.
        field: &'static str,
        /// The offending fractional value.
        value: f64,
    },

    /// A value falls outside the valid range.
    #[error("UDMF {block}[{index}] field '{field}' value {value} is out of range")]
    OutOfRange {
        /// The block type (e.g. "vertex").
        block: &'static str,
        /// The index of the block.
        index: usize,
        /// The field name.
        field: &'static str,
        /// A string representation of the out-of-bounds value.
        value: String,
    },

    /// A lump name or texture name is longer than 8 characters.
    #[error("UDMF {block}[{index}] field '{field}' texture/name '{value}' exceeds 8 characters")]
    NameTooLong {
        /// The block type (e.g. "vertex").
        block: &'static str,
        /// The index of the block.
        index: usize,
        /// The field name.
        field: &'static str,
        /// The oversized name.
        value: String,
    },

    /// A lump name or texture name contains non-ASCII characters.
    #[error("UDMF {block}[{index}] field '{field}' texture/name '{value}' must be ASCII")]
    NameNotAscii {
        /// The block type (e.g. "vertex").
        block: &'static str,
        /// The index of the block.
        index: usize,
        /// The field name.
        field: &'static str,
        /// The offending name.
        value: String,
    },
}

impl UdmfMap {
    /// Parse a TEXTMAP lump from raw bytes.
    ///
    /// # Errors
    /// Returns `UdmfError` if the lump is not valid UTF-8 or fails to parse.
    ///
    /// # Examples
    /// ```
    /// use doom_map::udmf::UdmfMap;
    ///
    /// let textmap_data = br#"
    /// namespace = "doom";
    /// vertex { x = 10; y = 20; }
    /// "#;
    /// let map = UdmfMap::parse(textmap_data).unwrap();
    /// assert_eq!(map.namespace, "doom");
    /// assert_eq!(map.blocks.len(), 1);
    /// assert_eq!(map.blocks[0].kind, "vertex");
    /// ```
    pub fn parse(data: &[u8]) -> Result<Self, UdmfError> {
        let input = core::str::from_utf8(data)?;
        let mut parser = Parser::new(input);
        let mut namespace = None;
        let mut blocks = Vec::new();

        parser.skip_ws_and_comments()?;
        while !parser.is_eof() {
            let ident = parser.parse_ident()?;
            parser.skip_ws_and_comments()?;

            if parser.consume_char('=') {
                parser.skip_ws_and_comments()?;
                let value = parser.parse_value()?;
                parser.skip_ws_and_comments()?;
                parser.expect_char(';')?;
                if ident.eq_ignore_ascii_case("namespace") {
                    let UdmfValue::Str(value) = value else {
                        return Err(UdmfError::ParseFailed {
                            offset: parser.offset(),
                            message: "namespace must be a quoted string".to_string(),
                        });
                    };
                    namespace = Some(value.to_ascii_lowercase());
                }
            } else if parser.consume_char('{') {
                let fields = parser.parse_block_fields()?;
                blocks.push(UdmfBlock {
                    kind: ident.to_ascii_lowercase(),
                    fields,
                });
            } else {
                return Err(UdmfError::ParseFailed {
                    offset: parser.offset(),
                    message: "expected '=' or '{' after identifier".to_string(),
                });
            }

            parser.skip_ws_and_comments()?;
        }

        let namespace = namespace.ok_or(UdmfError::MissingNamespace)?;
        Ok(Self { namespace, blocks })
    }

    /// Convert parsed UDMF blocks into the engine's classic geometry arrays.
    ///
    /// # Errors
    /// Returns `UdmfError` when required fields are missing, malformed, or
    /// outside the classic Doom value ranges.
    ///
    /// # Examples
    /// ```
    /// use doom_map::udmf::UdmfMap;
    ///
    /// let map = UdmfMap::parse(br#"
    /// namespace = "doom";
    /// vertex { x = 0; y = 0; }
    /// "#).unwrap();
    /// let level_data = map.into_level_data().unwrap();
    /// assert_eq!(level_data.vertexes.len(), 1);
    /// assert_eq!(level_data.vertexes[0].x, 0);
    /// ```
    pub fn into_level_data(self) -> Result<UdmfLevelData, UdmfError> {
        if self.namespace != "doom" {
            return Err(UdmfError::UnsupportedNamespace(self.namespace));
        }

        let mut things = Vec::new();
        let mut linedefs = Vec::new();
        let mut sidedefs = Vec::new();
        let mut vertexes = Vec::new();
        let mut sectors = Vec::new();

        for (index, block) in self.blocks.iter().enumerate() {
            match block.kind.as_str() {
                "vertex" => vertexes.push(Vertex {
                    x: required_i16(block, index, "vertex", "x")?,
                    y: required_i16(block, index, "vertex", "y")?,
                }),
                "sector" => sectors.push(Sector {
                    floor_height: required_i16(block, index, "sector", "heightfloor")?,
                    ceil_height: required_i16(block, index, "sector", "heightceiling")?,
                    floor_flat: required_name(block, index, "sector", "texturefloor")?,
                    ceil_flat: required_name(block, index, "sector", "textureceiling")?,
                    light_level: optional_i16(block, index, "sector", "lightlevel", 160)?,
                    special: optional_u16(block, index, "sector", "special", 0)?,
                    tag: first_present_u16(block, index, "sector", &["tag", "id"], 0)?,
                }),
                "sidedef" => sidedefs.push(Sidedef {
                    x_offset: optional_i16(block, index, "sidedef", "offsetx", 0)?,
                    y_offset: optional_i16(block, index, "sidedef", "offsety", 0)?,
                    upper_texture: optional_name(block, index, "sidedef", "texturetop")?,
                    lower_texture: optional_name(block, index, "sidedef", "texturebottom")?,
                    middle_texture: optional_name(block, index, "sidedef", "texturemiddle")?,
                    sector: required_u16(block, index, "sidedef", "sector")?,
                }),
                "linedef" => {
                    let left_sidedef = optional_sidedef_index(block, index, "linedef", "sideback")?;
                    let mut flags = 0;
                    if optional_bool(block, index, "linedef", "blocking")? {
                        flags |= FLAG_BLOCKING;
                    }
                    if optional_bool(block, index, "linedef", "blockmonsters")? {
                        flags |= FLAG_BLOCKMONSTERS;
                    }
                    if optional_bool(block, index, "linedef", "dontpegtop")? {
                        flags |= FLAG_DONTPEGTOP;
                    }
                    if optional_bool(block, index, "linedef", "dontpegbottom")? {
                        flags |= FLAG_DONTPEGBOTTOM;
                    }
                    if optional_bool(block, index, "linedef", "secret")? {
                        flags |= FLAG_SECRET;
                    }
                    if optional_bool(block, index, "linedef", "blocksound")? {
                        flags |= FLAG_SOUNDBLOCK;
                    }
                    if optional_bool(block, index, "linedef", "dontdraw")? {
                        flags |= FLAG_DONTDRAW;
                    }
                    if optional_bool(block, index, "linedef", "mapped")? {
                        flags |= FLAG_MAPPED;
                    }
                    if left_sidedef != SIDEDEF_NONE
                        || optional_bool(block, index, "linedef", "twosided")?
                    {
                        flags |= FLAG_TWO_SIDED;
                    }

                    linedefs.push(Linedef {
                        from_vertex: required_u16(block, index, "linedef", "v1")?,
                        to_vertex: required_u16(block, index, "linedef", "v2")?,
                        flags,
                        special: optional_u16(block, index, "linedef", "special", 0)?,
                        tag: first_present_u16(block, index, "linedef", &["tag", "arg0", "id"], 0)?,
                        right_sidedef: required_u16(block, index, "linedef", "sidefront")?,
                        left_sidedef,
                    });
                }
                "thing" => things.push(Thing {
                    x: required_i16(block, index, "thing", "x")?,
                    y: required_i16(block, index, "thing", "y")?,
                    angle: optional_u16(block, index, "thing", "angle", 0)?,
                    kind: required_first_present_u16(block, index, "thing", &["type", "kind"])?,
                    flags: thing_flags(block, index)?,
                }),
                _ => {}
            }
        }

        Ok(UdmfLevelData {
            things,
            linedefs,
            sidedefs,
            vertexes,
            sectors,
        })
    }
}

struct Parser<'a> {
    input: &'a str,
    offset: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, offset: 0 }
    }

    fn is_eof(&self) -> bool {
        self.offset >= self.input.len()
    }

    fn offset(&self) -> usize {
        self.offset
    }

    fn rest(&self) -> &'a str {
        &self.input[self.offset..]
    }

    fn peek_char(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }

    fn consume_char(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.bump_char();
            true
        } else {
            false
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<(), UdmfError> {
        if self.consume_char(expected) {
            Ok(())
        } else {
            Err(UdmfError::ParseFailed {
                offset: self.offset,
                message: format!("expected '{expected}'"),
            })
        }
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), UdmfError> {
        loop {
            while matches!(self.peek_char(), Some(ch) if ch.is_whitespace()) {
                self.bump_char();
            }

            let rest = self.rest();
            if rest.starts_with("//") {
                while let Some(ch) = self.bump_char() {
                    if ch == '\n' {
                        break;
                    }
                }
                continue;
            }

            if rest.starts_with("/*") {
                self.offset += 2;
                if let Some(end_idx) = self.rest().find("*/") {
                    self.offset += end_idx + 2;
                    continue;
                }
                return Err(UdmfError::ParseFailed {
                    offset: self.offset,
                    message: "unterminated block comment".to_string(),
                });
            }

            return Ok(());
        }
    }

    fn parse_ident(&mut self) -> Result<String, UdmfError> {
        let start = self.offset;
        let Some(first) = self.peek_char() else {
            return Err(UdmfError::ParseFailed {
                offset: self.offset,
                message: "expected identifier, found end of input".to_string(),
            });
        };
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return Err(UdmfError::ParseFailed {
                offset: self.offset,
                message: "expected identifier".to_string(),
            });
        }

        self.bump_char();
        while matches!(self.peek_char(), Some(ch) if ch == '_' || ch.is_ascii_alphanumeric()) {
            self.bump_char();
        }

        Ok(self.input[start..self.offset].to_ascii_lowercase())
    }

    fn parse_value(&mut self) -> Result<UdmfValue, UdmfError> {
        match self.peek_char() {
            Some('"') => self.parse_string().map(UdmfValue::Str),
            Some('+') | Some('-') | Some('0'..='9') => self.parse_number(),
            Some(ch) if ch == '_' || ch.is_ascii_alphabetic() => {
                let ident = self.parse_ident()?;
                match ident.as_str() {
                    "true" => Ok(UdmfValue::Bool(true)),
                    "false" => Ok(UdmfValue::Bool(false)),
                    _ => Err(UdmfError::ParseFailed {
                        offset: self.offset,
                        message: format!("unexpected bare identifier '{ident}'"),
                    }),
                }
            }
            _ => Err(UdmfError::ParseFailed {
                offset: self.offset,
                message: "expected value".to_string(),
            }),
        }
    }

    fn parse_string(&mut self) -> Result<String, UdmfError> {
        self.expect_char('"')?;
        let mut out = String::new();

        loop {
            let Some(ch) = self.bump_char() else {
                return Err(UdmfError::ParseFailed {
                    offset: self.offset,
                    message: "unterminated string literal".to_string(),
                });
            };

            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let Some(escaped) = self.bump_char() else {
                        return Err(UdmfError::ParseFailed {
                            offset: self.offset,
                            message: "unterminated string escape".to_string(),
                        });
                    };
                    let decoded = match escaped {
                        '"' => '"',
                        '\\' => '\\',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        other => other,
                    };
                    out.push(decoded);
                }
                other => out.push(other),
            }
        }
    }

    fn parse_number(&mut self) -> Result<UdmfValue, UdmfError> {
        let start = self.offset;

        if matches!(self.peek_char(), Some('+') | Some('-')) {
            self.bump_char();
        }

        let mut saw_digit = false;
        while matches!(self.peek_char(), Some('0'..='9')) {
            saw_digit = true;
            self.bump_char();
        }

        let mut is_float = false;
        if self.consume_char('.') {
            is_float = true;
            while matches!(self.peek_char(), Some('0'..='9')) {
                saw_digit = true;
                self.bump_char();
            }
        }

        if matches!(self.peek_char(), Some('e') | Some('E')) {
            is_float = true;
            self.bump_char();
            if matches!(self.peek_char(), Some('+') | Some('-')) {
                self.bump_char();
            }
            let mut exponent_digits = 0usize;
            while matches!(self.peek_char(), Some('0'..='9')) {
                exponent_digits += 1;
                self.bump_char();
            }
            if exponent_digits == 0 {
                return Err(UdmfError::ParseFailed {
                    offset: self.offset,
                    message: "malformed numeric exponent".to_string(),
                });
            }
        }

        if !saw_digit {
            return Err(UdmfError::ParseFailed {
                offset: start,
                message: "expected numeric literal".to_string(),
            });
        }

        let token = &self.input[start..self.offset];
        if is_float {
            token
                .parse::<f64>()
                .map(UdmfValue::Float)
                .map_err(|_| UdmfError::ParseFailed {
                    offset: start,
                    message: format!("invalid float literal '{token}'"),
                })
        } else {
            token
                .parse::<i64>()
                .map(UdmfValue::Int)
                .map_err(|_| UdmfError::ParseFailed {
                    offset: start,
                    message: format!("invalid integer literal '{token}'"),
                })
        }
    }

    fn parse_block_fields(&mut self) -> Result<Vec<UdmfField>, UdmfError> {
        let mut fields = Vec::new();
        self.skip_ws_and_comments()?;

        while !self.consume_char('}') {
            let key = self.parse_ident()?;
            self.skip_ws_and_comments()?;
            self.expect_char('=')?;
            self.skip_ws_and_comments()?;
            let value = self.parse_value()?;
            self.skip_ws_and_comments()?;
            self.expect_char(';')?;
            fields.push(UdmfField { key, value });
            self.skip_ws_and_comments()?;
        }

        Ok(fields)
    }
}

fn block_field<'a>(block: &'a UdmfBlock, key: &str) -> Option<&'a UdmfValue> {
    block
        .fields
        .iter()
        .rev()
        .find(|field| field.key == key)
        .map(|field| &field.value)
}

fn has_block_field(block: &UdmfBlock, key: &str) -> bool {
    block_field(block, key).is_some()
}

fn optional_bool(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
) -> Result<bool, UdmfError> {
    match block_field(block, field) {
        None => Ok(false),
        Some(UdmfValue::Bool(value)) => Ok(*value),
        Some(_) => Err(UdmfError::WrongType {
            block: block_name,
            index,
            field,
            expected: "a boolean",
        }),
    }
}

fn required_i16(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
) -> Result<i16, UdmfError> {
    let value = required_i64(block, index, block_name, field)?;
    i16::try_from(value).map_err(|_| UdmfError::OutOfRange {
        block: block_name,
        index,
        field,
        value: value.to_string(),
    })
}

fn optional_i16(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
    default: i16,
) -> Result<i16, UdmfError> {
    match block_field(block, field) {
        None => Ok(default),
        Some(value) => {
            let value = integral_value(value, block_name, index, field)?;
            i16::try_from(value).map_err(|_| UdmfError::OutOfRange {
                block: block_name,
                index,
                field,
                value: value.to_string(),
            })
        }
    }
}

fn required_u16(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
) -> Result<u16, UdmfError> {
    let value = required_i64(block, index, block_name, field)?;
    u16::try_from(value).map_err(|_| UdmfError::OutOfRange {
        block: block_name,
        index,
        field,
        value: value.to_string(),
    })
}

fn optional_u16(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
    default: u16,
) -> Result<u16, UdmfError> {
    match block_field(block, field) {
        None => Ok(default),
        Some(value) => {
            let value = integral_value(value, block_name, index, field)?;
            u16::try_from(value).map_err(|_| UdmfError::OutOfRange {
                block: block_name,
                index,
                field,
                value: value.to_string(),
            })
        }
    }
}

fn required_first_present_u16(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    fields: &[&'static str],
) -> Result<u16, UdmfError> {
    for field in fields {
        if has_block_field(block, field) {
            return required_u16(block, index, block_name, field);
        }
    }
    Err(UdmfError::MissingField {
        block: block_name,
        index,
        field: fields[0],
    })
}

fn first_present_u16(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    fields: &[&'static str],
    default: u16,
) -> Result<u16, UdmfError> {
    for field in fields {
        if has_block_field(block, field) {
            return optional_u16(block, index, block_name, field, default);
        }
    }
    Ok(default)
}

fn optional_sidedef_index(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
) -> Result<u16, UdmfError> {
    match block_field(block, field) {
        None => Ok(SIDEDEF_NONE),
        Some(value) => {
            let value = integral_value(value, block_name, index, field)?;
            if value < 0 {
                Ok(SIDEDEF_NONE)
            } else {
                u16::try_from(value).map_err(|_| UdmfError::OutOfRange {
                    block: block_name,
                    index,
                    field,
                    value: value.to_string(),
                })
            }
        }
    }
}

fn required_name(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
) -> Result<[u8; 8], UdmfError> {
    let value = block_field(block, field).ok_or(UdmfError::MissingField {
        block: block_name,
        index,
        field,
    })?;
    name_value(value, block_name, index, field)
}

fn optional_name(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
) -> Result<[u8; 8], UdmfError> {
    match block_field(block, field) {
        None => Ok([0; 8]),
        Some(value) => name_value(value, block_name, index, field),
    }
}

fn name_value(
    value: &UdmfValue,
    block: &'static str,
    index: usize,
    field: &'static str,
) -> Result<[u8; 8], UdmfError> {
    let UdmfValue::Str(value) = value else {
        return Err(UdmfError::WrongType {
            block,
            index,
            field,
            expected: "a string",
        });
    };
    encode_name(value, block, index, field)
}

fn encode_name(
    value: &str,
    block: &'static str,
    index: usize,
    field: &'static str,
) -> Result<[u8; 8], UdmfError> {
    if !value.is_ascii() {
        return Err(UdmfError::NameNotAscii {
            block,
            index,
            field,
            value: value.to_string(),
        });
    }
    if value.len() > 8 {
        return Err(UdmfError::NameTooLong {
            block,
            index,
            field,
            value: value.to_string(),
        });
    }

    let mut buf = [0u8; 8];
    for (i, byte) in value.bytes().enumerate() {
        buf[i] = byte.to_ascii_uppercase();
    }
    Ok(buf)
}

fn required_i64(
    block: &UdmfBlock,
    index: usize,
    block_name: &'static str,
    field: &'static str,
) -> Result<i64, UdmfError> {
    let value = block_field(block, field).ok_or(UdmfError::MissingField {
        block: block_name,
        index,
        field,
    })?;
    integral_value(value, block_name, index, field)
}

fn integral_value(
    value: &UdmfValue,
    block: &'static str,
    index: usize,
    field: &'static str,
) -> Result<i64, UdmfError> {
    match value {
        UdmfValue::Int(value) => Ok(*value),
        UdmfValue::Float(value) => {
            if value.fract() == 0.0 {
                if *value < i64::MIN as f64 || *value > i64::MAX as f64 {
                    Err(UdmfError::OutOfRange {
                        block,
                        index,
                        field,
                        value: value.to_string(),
                    })
                } else {
                    Ok(*value as i64)
                }
            } else {
                Err(UdmfError::NonIntegral {
                    block,
                    index,
                    field,
                    value: *value,
                })
            }
        }
        _ => Err(UdmfError::WrongType {
            block,
            index,
            field,
            expected: "an integer",
        }),
    }
}

fn thing_flags(block: &UdmfBlock, index: usize) -> Result<u16, UdmfError> {
    if has_block_field(block, "flags") {
        return required_u16(block, index, "thing", "flags");
    }

    let skill_fields_present = ["skill1", "skill2", "skill3", "skill4", "skill5"]
        .into_iter()
        .any(|field| has_block_field(block, field));
    let single_present = has_block_field(block, "single");

    let easy = if skill_fields_present {
        optional_bool(block, index, "thing", "skill1")?
            || optional_bool(block, index, "thing", "skill2")?
    } else {
        true
    };
    let medium = if skill_fields_present {
        optional_bool(block, index, "thing", "skill3")?
    } else {
        true
    };
    let hard = if skill_fields_present {
        optional_bool(block, index, "thing", "skill4")?
            || optional_bool(block, index, "thing", "skill5")?
    } else {
        true
    };

    let mut flags = 0;
    if easy {
        flags |= THING_FLAG_EASY;
    }
    if medium {
        flags |= THING_FLAG_MEDIUM;
    }
    if hard {
        flags |= THING_FLAG_HARD;
    }
    if optional_bool(block, index, "thing", "ambush")? {
        flags |= THING_FLAG_AMBUSH;
    }

    if single_present && !optional_bool(block, index, "thing", "single")? {
        flags |= THING_FLAG_MULTIPLAYER;
    }

    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_textmap_smoke() {
        let map = UdmfMap::parse(
            br#"
            namespace = "doom";
            vertex { x = 0; y = 64; }
            "#,
        )
        .expect("parse");

        assert_eq!(map.namespace, "doom");
        assert_eq!(map.blocks.len(), 1);
        assert_eq!(map.blocks[0].kind, "vertex");
    }

    #[test]
    fn conversion_rejects_non_integral_vertex() {
        let map = UdmfMap::parse(
            br#"
            namespace = "doom";
            vertex { x = 1.5; y = 0; }
            "#,
        )
        .expect("parse");

        assert!(matches!(
            map.into_level_data(),
            Err(UdmfError::NonIntegral {
                block: "vertex",
                field: "x",
                ..
            })
        ));
    }

    #[test]
    fn conversion_rejects_non_doom_namespace() {
        let map = UdmfMap::parse(
            br#"
            namespace = "zdoom";
            vertex { x = 0; y = 0; }
            "#,
        )
        .expect("parse");

        assert!(matches!(
            map.into_level_data(),
            Err(UdmfError::UnsupportedNamespace(namespace)) if namespace == "zdoom"
        ));
    }
}
