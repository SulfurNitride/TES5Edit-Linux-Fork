use crate::{Record, Signature};

/// Type of a GRUP (group) record in TES4+ plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupType {
    /// Top-level group containing all records of a given type
    Top(Signature),
    /// World children
    WorldChildren(u32),
    /// Interior cell block
    InteriorCellBlock(i32),
    /// Interior cell sub-block
    InteriorCellSubBlock(i32),
    /// Exterior cell block
    ExteriorCellBlock { x: i16, y: i16 },
    /// Exterior cell sub-block
    ExteriorCellSubBlock { x: i16, y: i16 },
    /// Cell children
    CellChildren(u32),
    /// Topic children
    TopicChildren(u32),
    /// Cell persistent children
    CellPersistentChildren(u32),
    /// Cell temporary children
    CellTemporaryChildren(u32),
    /// Cell visible distant children (Oblivion/Fallout-era)
    CellVisibleDistantChildren(u32),
}

/// A GRUP (group) container in a TES4+ plugin.
///
/// Groups form the hierarchical structure of a plugin file.
/// They contain records and/or nested groups.
#[derive(Debug, Clone)]
pub struct Group {
    /// What type of group this is
    pub group_type: GroupType,
    /// Timestamp/version from header
    pub stamp: u32,
    /// Unknown field from header
    pub unknown: u32,
    /// Contents: records and nested groups
    pub children: Vec<GroupChild>,
    /// Original raw header bytes (24 bytes) for lossless roundtrip
    pub raw_header: Option<Vec<u8>>,
    /// Original byte offset in the source file
    pub source_offset: Option<u64>,
}

/// A child item within a group: either a record or a nested group.
#[derive(Debug, Clone)]
pub enum GroupChild {
    Record(Record),
    Group(Group),
}
