use crate::{FormId, Signature, Subrecord};

/// Flags on a record header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordFlags(pub u32);

impl RecordFlags {
    pub const NONE: Self = Self(0);

    // Common flags shared across TES4+ games
    pub const ESM: u32 = 0x0000_0001;
    pub const DELETED: u32 = 0x0000_0020;
    pub const LOCALIZED: u32 = 0x0000_0080;
    pub const COMPRESSED: u32 = 0x0004_0000;
    pub const ESL: u32 = 0x0000_0200;

    pub fn is_compressed(&self) -> bool {
        self.0 & Self::COMPRESSED != 0
    }

    pub fn is_deleted(&self) -> bool {
        self.0 & Self::DELETED != 0
    }

    pub fn is_esm(&self) -> bool {
        self.0 & Self::ESM != 0
    }

    pub fn is_esl(&self) -> bool {
        self.0 & Self::ESL != 0
    }

    pub fn is_localized(&self) -> bool {
        self.0 & Self::LOCALIZED != 0
    }
}

/// A record in a plugin file (e.g., a WEAP, NPC_, CELL record).
///
/// Records contain subrecords and can be nested within GRUPs.
/// The raw header bytes and unmodified subrecords are preserved
/// for lossless roundtrip.
#[derive(Debug, Clone)]
pub struct Record {
    /// 4-byte type signature
    pub signature: Signature,
    /// Record flags
    pub flags: RecordFlags,
    /// Form ID (0 for TES3 game, meaningful for TES4+)
    pub form_id: FormId,
    /// Version control info (game-specific)
    pub vc_info: u32,
    /// Internal version (game-specific)
    pub version: u16,
    /// Unknown field from header
    pub unknown: u16,
    /// Subrecords belonging to this record
    pub subrecords: Vec<Subrecord>,
    /// Original raw header bytes (24 bytes for TES4+) for lossless roundtrip
    pub raw_header: Option<Vec<u8>>,
    /// If the record was compressed, the original compressed bytes.
    /// Used for lossless roundtrip when no subrecords are modified.
    pub raw_compressed_data: Option<Vec<u8>>,
    /// Original raw record payload bytes exactly as stored on disk.
    /// For compressed records this includes the decompressed-size prefix
    /// and compressed zlib bytes.
    pub raw_data: Option<Vec<u8>>,
    /// Original byte offset in the source file
    pub source_offset: Option<u64>,
    /// Whether any subrecord has been modified since loading
    pub modified: bool,
}

impl Record {
    /// Get the Editor ID (EDID subrecord) if present.
    pub fn editor_id(&self) -> Option<&str> {
        self.subrecords
            .iter()
            .find(|sr| sr.signature == Signature::EDID)
            .and_then(|sr| {
                // EDID is a null-terminated string
                let data = &sr.raw_data;
                let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                std::str::from_utf8(&data[..len]).ok()
            })
    }

    /// Find all subrecords with a given signature.
    pub fn subrecords_by_sig(&self, sig: Signature) -> impl Iterator<Item = &Subrecord> {
        self.subrecords.iter().filter(move |sr| sr.signature == sig)
    }

    /// Check if any subrecord in this record has been modified.
    pub fn is_modified(&self) -> bool {
        self.modified || self.subrecords.iter().any(|sr| sr.modified)
    }
}
