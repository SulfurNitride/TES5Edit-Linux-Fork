/// A 4-byte record/subrecord type signature (e.g., TES4, GRUP, WEAP, NPC_, EDID).
///
/// Stored as raw bytes to preserve exact binary representation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature(pub [u8; 4]);

impl Signature {
    pub const TES3: Self = Self(*b"TES3");
    pub const TES4: Self = Self(*b"TES4");
    pub const GRUP: Self = Self(*b"GRUP");

    // Common record types
    pub const AACT: Self = Self(*b"AACT");
    pub const ACHR: Self = Self(*b"ACHR");
    pub const ARMO: Self = Self(*b"ARMO");
    pub const BOOK: Self = Self(*b"BOOK");
    pub const CELL: Self = Self(*b"CELL");
    pub const DIAL: Self = Self(*b"DIAL");
    pub const FACT: Self = Self(*b"FACT");
    pub const FLST: Self = Self(*b"FLST");
    pub const GLOB: Self = Self(*b"GLOB");
    pub const GMST: Self = Self(*b"GMST");
    pub const IDLE: Self = Self(*b"IDLE");
    pub const INFO: Self = Self(*b"INFO");
    pub const KYWD: Self = Self(*b"KYWD");
    pub const LCRT: Self = Self(*b"LCRT");
    pub const MGEF: Self = Self(*b"MGEF");
    pub const MISC: Self = Self(*b"MISC");
    pub const NPC_: Self = Self(*b"NPC_");
    pub const PERK: Self = Self(*b"PERK");
    pub const QUST: Self = Self(*b"QUST");
    pub const RACE: Self = Self(*b"RACE");
    pub const REFR: Self = Self(*b"REFR");
    pub const SPEL: Self = Self(*b"SPEL");
    pub const STAT: Self = Self(*b"STAT");
    pub const WEAP: Self = Self(*b"WEAP");
    pub const WRLD: Self = Self(*b"WRLD");

    // Common subrecord types
    pub const EDID: Self = Self(*b"EDID");
    pub const FULL: Self = Self(*b"FULL");
    pub const DATA: Self = Self(*b"DATA");
    pub const NAME: Self = Self(*b"NAME");
    pub const MODL: Self = Self(*b"MODL");
    pub const OBND: Self = Self(*b"OBND");
    pub const VMAD: Self = Self(*b"VMAD");

    /// Create a signature from a 4-byte slice.
    pub fn from_bytes(bytes: &[u8; 4]) -> Self {
        Self(*bytes)
    }

    /// Get the raw bytes.
    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }

    /// Display as ASCII string (replacing non-printable with '?').
    pub fn as_str(&self) -> String {
        self.0
            .iter()
            .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' })
            .collect()
    }
}

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sig({})", self.as_str())
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
