/// A FormID identifying a record within a plugin or load order.
///
/// For TES4+ games: upper byte(s) = master index, lower 3 bytes = local ID.
/// For TES3: uses a different identification system (no FormIDs).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct FormId(pub u32);

impl FormId {
    pub const NULL: Self = Self(0);

    /// Create a new FormID from a raw u32 value.
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Get the master index (upper byte for standard plugins, upper 2 bytes for ESL).
    pub fn master_index(&self) -> u8 {
        (self.0 >> 24) as u8
    }

    /// Get the local form ID (lower 3 bytes).
    pub fn local_id(&self) -> u32 {
        self.0 & 0x00FF_FFFF
    }

    /// Get the raw u32 value.
    pub fn raw(&self) -> u32 {
        self.0
    }

    /// Check if this is the null/empty FormID.
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl std::fmt::Debug for FormId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FormId({:08X})", self.0)
    }
}

impl std::fmt::Display for FormId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08X}", self.0)
    }
}
