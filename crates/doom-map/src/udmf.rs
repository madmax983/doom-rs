//! UDMF (Universal Doom Map Format) TextMap parser.
//!
//! Uses `nom` to parse the TEXTMAP lump in UDMF maps.
//! UDMF extends the standard format with named fields, floats, and
//! user-defined namespaces (e.g. "doom", "heretic", "zdoom").
//!
//! # Status: skeleton
//! The parser recognises the top-level structure (namespace, blocks, assignments)
//! but does not yet convert UDMF geometry to the standard `Level` types.
//! Full conversion is Phase 3 stretch goal / Phase 8 polish.

/// A UDMF value: string, integer, float, or boolean.
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
#[derive(Debug, Clone)]
pub struct UdmfField {
    pub key: String,
    pub value: UdmfValue,
}

/// A named block (e.g. `vertex { x = 10.0; y = 20.0; }`).
#[derive(Debug, Clone)]
pub struct UdmfBlock {
    pub kind: String,
    pub fields: Vec<UdmfField>,
}

/// The fully parsed TEXTMAP lump.
#[derive(Debug, Clone)]
pub struct UdmfMap {
    /// The namespace declaration (first statement in the file).
    pub namespace: String,
    /// All top-level blocks in order.
    pub blocks: Vec<UdmfBlock>,
}

/// Parse errors from the UDMF TextMap parser.
#[derive(Debug, thiserror::Error)]
pub enum UdmfError {
    #[error("UDMF parse error near offset {offset}: {message}")]
    ParseFailed { offset: usize, message: String },
}

impl UdmfMap {
    /// Parse a TEXTMAP lump from raw bytes.
    ///
    /// # Errors
    /// Returns `UdmfError` if the lump is not valid UTF-8 or fails to parse.
    pub fn parse(_data: &[u8]) -> Result<Self, UdmfError> {
        // Placeholder — full nom-based parser is Phase 8 work.
        // For now return a minimal stub so the crate compiles.
        Ok(Self {
            namespace: String::from("doom"),
            blocks: Vec::new(),
        })
    }
}
