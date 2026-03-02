//! Game definition types - the schema for interpreting plugin records.
//!
//! These types mirror the wb* definition patterns from Delphi:
//! wbRecord, wbStruct, wbArray, wbInteger, wbFloat, wbString, etc.

use xedit_dom::Signature;

/// Definition for a complete record type (e.g., WEAP, ARMO, NPC_).
#[derive(Debug, Clone)]
pub struct RecordDef {
    /// Record signature (e.g., WEAP)
    pub signature: Signature,
    /// Human-readable name (e.g., "Weapon")
    pub name: String,
    /// Subrecord definitions in expected order
    pub members: Vec<SubrecordDef>,
}

/// Definition for a subrecord within a record.
#[derive(Debug, Clone)]
pub struct SubrecordDef {
    /// Subrecord signature (e.g., EDID, DATA)
    pub signature: Signature,
    /// Human-readable name
    pub name: String,
    /// Field layout within this subrecord
    pub fields: Vec<FieldDef>,
    /// Whether this subrecord is required
    pub required: bool,
}

/// Definition for a single field within a subrecord.
#[derive(Debug, Clone)]
pub struct FieldDef {
    /// Field name for display
    pub name: String,
    /// Data type and interpretation
    pub field_type: FieldType,
}

/// The type of a field, mirroring Delphi's wb* definition types.
#[derive(Debug, Clone)]
pub enum FieldType {
    /// wbInteger - integer of given size
    Integer {
        size: xedit_dom::element::IntegerSize,
        /// Optional enum interpretation
        enum_def: Option<EnumDef>,
        /// Optional flags interpretation
        flags_def: Option<FlagDef>,
    },
    /// wbFloat - 32-bit float
    Float,
    /// wbFloat - 16-bit half-precision float
    HalfFloat,
    /// wbString - null-terminated string
    String,
    /// wbLenString - length-prefixed string
    LenString,
    /// wbFormID / wbFormIDCk - FormID reference
    FormId {
        /// Valid reference signatures (e.g., [NPC_, RACE])
        valid_refs: Vec<Signature>,
    },
    /// wbByteArray - raw bytes
    ByteArray {
        /// Expected size (0 = variable)
        size: usize,
    },
    /// wbStruct - structured group of fields
    Struct {
        name: String,
        fields: Vec<FieldDef>,
    },
    /// wbArray - array of elements
    Array {
        name: String,
        element: Box<FieldDef>,
        /// Fixed count (0 = until end of data)
        count: usize,
    },
    /// wbUnion - conditional type based on context
    Union {
        /// Possible interpretations
        members: Vec<FieldDef>,
    },
    /// wbFlags - bit flags
    Flags(FlagDef),
    /// wbEnum - enumerated value
    Enum(EnumDef),
    /// Unknown/unrecognized - preserve raw bytes
    Unknown,
}

/// Named flag bits for a flags field.
#[derive(Debug, Clone)]
pub struct FlagDef {
    pub flags: Vec<(u64, String)>,
}

/// Named values for an enum field.
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub values: Vec<(i64, String)>,
}
