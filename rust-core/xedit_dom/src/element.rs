use crate::Signature;

/// A parsed element within a subrecord, interpreted according to game definitions.
///
/// Elements form a tree structure matching the game definition hierarchy
/// (wbStruct, wbArray, wbInteger, etc.). Unknown/unrecognized data is
/// preserved as raw bytes.
#[derive(Debug, Clone)]
pub enum Element {
    /// A structured group of named fields
    Struct {
        name: String,
        children: Vec<NamedElement>,
    },
    /// An array of elements
    Array {
        name: String,
        items: Vec<Element>,
    },
    /// An integer value (various sizes)
    Integer {
        name: String,
        value: i64,
        size: IntegerSize,
    },
    /// A floating-point value
    Float {
        name: String,
        value: f64,
        half_precision: bool,
    },
    /// A string value
    String {
        name: String,
        value: String,
    },
    /// A FormID reference
    FormId {
        name: String,
        value: crate::FormId,
        /// Which record types this FormID can reference (e.g., [NPC_, RACE])
        valid_refs: Vec<Signature>,
    },
    /// A byte array (raw binary data)
    Bytes {
        name: String,
        data: Vec<u8>,
    },
    /// Flags field (integer interpreted as bit flags)
    Flags {
        name: String,
        value: u64,
        flag_names: Vec<(u64, String)>,
    },
    /// An enum value (integer with named values)
    Enum {
        name: String,
        value: i64,
        options: Vec<(i64, String)>,
    },
    /// Unknown/unrecognized data preserved as raw bytes for lossless roundtrip
    Unknown {
        /// Subrecord signature this data belongs to
        signature: Signature,
        /// Raw bytes preserved exactly as loaded
        raw: Vec<u8>,
        /// Byte offset within the subrecord
        offset: usize,
    },
}

/// A named element within a struct.
#[derive(Debug, Clone)]
pub struct NamedElement {
    pub name: String,
    pub element: Element,
}

/// Integer sizes supported by the format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerSize {
    U8,
    S8,
    U16,
    S16,
    U32,
    S32,
    U64,
    S64,
}

impl IntegerSize {
    /// Size in bytes.
    pub fn byte_size(&self) -> usize {
        match self {
            IntegerSize::U8 | IntegerSize::S8 => 1,
            IntegerSize::U16 | IntegerSize::S16 => 2,
            IntegerSize::U32 | IntegerSize::S32 => 4,
            IntegerSize::U64 | IntegerSize::S64 => 8,
        }
    }
}
