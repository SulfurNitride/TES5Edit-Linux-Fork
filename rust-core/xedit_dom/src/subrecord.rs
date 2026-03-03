use crate::Signature;

/// A subrecord within a plugin record.
///
/// Subrecords are the leaf-level data containers in a plugin file.
/// They store raw data that is interpreted according to game-specific
/// record definitions.
///
/// The raw bytes are always preserved for lossless roundtrip.
#[derive(Debug, Clone)]
pub struct Subrecord {
    /// 4-byte type signature (e.g., EDID, DATA, NAME)
    pub signature: Signature,
    /// Raw data bytes - always preserved for lossless roundtrip.
    /// Interpretation depends on game definitions.
    pub raw_data: Vec<u8>,
    /// Cached data size in bytes. Preserved after offloading raw_data to disk.
    pub size: u32,
    /// Original byte offset in the source file (for surgical patching)
    pub source_offset: Option<u64>,
    /// Whether this subrecord has been modified since loading
    pub modified: bool,
}

impl Subrecord {
    /// Create a new subrecord with raw data.
    pub fn new(signature: Signature, raw_data: Vec<u8>) -> Self {
        let size = raw_data.len() as u32;
        Self {
            signature,
            raw_data,
            size,
            source_offset: None,
            modified: false,
        }
    }

    /// Size of the subrecord data (not including header).
    pub fn data_size(&self) -> usize {
        self.raw_data.len()
    }

    /// Total size on disk: 6 bytes header (4 sig + 2 size) + data.
    /// Note: Some games use extended size subrecords (XXXX prefix).
    pub fn total_size(&self) -> usize {
        6 + self.raw_data.len()
    }
}
