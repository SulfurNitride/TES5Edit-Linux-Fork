/// Identifies which Bethesda game a plugin belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameId {
    /// TES3: Morrowind - completely different plugin structure, no FormIDs
    Morrowind,
    /// TES4: Oblivion
    Oblivion,
    /// FO3: Fallout 3
    Fallout3,
    /// FNV: Fallout New Vegas
    FalloutNV,
    /// TES5: Skyrim (original + Special Edition + Anniversary Edition)
    SkyrimSE,
    /// FO4: Fallout 4
    Fallout4,
    /// FO76: Fallout 76
    Fallout76,
    /// SF1: Starfield
    Starfield,
}

impl GameId {
    /// Returns which dialect family this game belongs to.
    pub fn dialect_family(&self) -> DialectFamily {
        match self {
            GameId::Morrowind => DialectFamily::TES3,
            _ => DialectFamily::TES4Plus,
        }
    }

    /// Returns the expected file header signature for this game's plugins.
    pub fn header_signature(&self) -> crate::Signature {
        match self {
            GameId::Morrowind => crate::Signature::TES3,
            _ => crate::Signature::TES4,
        }
    }

    /// Short display name for this game.
    pub fn short_name(&self) -> &'static str {
        match self {
            GameId::Morrowind => "TES3",
            GameId::Oblivion => "TES4",
            GameId::Fallout3 => "FO3",
            GameId::FalloutNV => "FNV",
            GameId::SkyrimSE => "SSE",
            GameId::Fallout4 => "FO4",
            GameId::Fallout76 => "FO76",
            GameId::Starfield => "SF1",
        }
    }
}

/// The two major dialect families for Bethesda plugin formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialectFamily {
    /// Morrowind: flat record list, no FormIDs, different subrecord model
    TES3,
    /// Oblivion onwards: GRUP hierarchy, FormIDs, standard subrecord model
    TES4Plus,
}

/// Trait for game-specific plugin handling behavior.
///
/// Implementations define how records are structured, how FormIDs work,
/// and other game-specific details.
pub trait GameDialect: Send + Sync {
    /// Which game this dialect handles.
    fn game_id(&self) -> GameId;

    /// Whether this game uses compressed records.
    fn supports_record_compression(&self) -> bool;

    /// Whether this game uses ESL (light master) flags.
    fn supports_esl(&self) -> bool;

    /// Whether this game uses localized strings.
    fn supports_localization(&self) -> bool;

    /// Maximum number of masters a plugin can reference.
    fn max_masters(&self) -> usize;
}
