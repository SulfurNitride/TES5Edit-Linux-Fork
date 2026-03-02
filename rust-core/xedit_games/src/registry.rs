//! Registry for game-specific record definitions.

use std::collections::HashMap;

use xedit_dom::{GameId, Signature};

use crate::RecordDef;

/// Central registry mapping game + signature to record definitions.
pub struct DefinitionRegistry {
    defs: HashMap<(GameId, Signature), RecordDef>,
}

impl DefinitionRegistry {
    pub fn new() -> Self {
        Self {
            defs: HashMap::new(),
        }
    }

    /// Register a record definition for a specific game.
    pub fn register(&mut self, game: GameId, def: RecordDef) {
        self.defs.insert((game, def.signature), def);
    }

    /// Look up a record definition by game and signature.
    pub fn get(&self, game: GameId, sig: Signature) -> Option<&RecordDef> {
        self.defs.get(&(game, sig))
    }

    /// Get all record signatures defined for a game.
    pub fn signatures_for_game(&self, game: GameId) -> Vec<Signature> {
        self.defs
            .keys()
            .filter(|(g, _)| *g == game)
            .map(|(_, s)| *s)
            .collect()
    }

    /// Load all Skyrim SE (TES5) record definitions.
    pub fn load_sse_definitions(&mut self) {
        for def in crate::sse_defs::register_all() {
            self.register(GameId::SkyrimSE, def);
        }
    }

    /// Load all Morrowind (TES3) record definitions.
    pub fn load_morrowind_definitions(&mut self) {
        for def in crate::morrowind_defs::register_all() {
            self.register(GameId::Morrowind, def);
        }
    }

    /// Load all Oblivion (TES4) record definitions.
    pub fn load_tes4_definitions(&mut self) {
        for def in crate::tes4_defs::register_all() {
            self.register(GameId::Oblivion, def);
        }
    }

    /// Load all Fallout 3 record definitions.
    pub fn load_fo3_definitions(&mut self) {
        for def in crate::fo3_defs::register_all() {
            self.register(GameId::Fallout3, def);
        }
    }

    /// Load all Fallout New Vegas record definitions.
    pub fn load_fnv_definitions(&mut self) {
        for def in crate::fnv_defs::register_all() {
            self.register(GameId::FalloutNV, def);
        }
    }

    /// Load all Fallout 4 record definitions.
    pub fn load_fo4_definitions(&mut self) {
        for def in crate::fo4_defs::register_all() {
            self.register(GameId::Fallout4, def);
        }
    }

    /// Load all Fallout 76 record definitions.
    pub fn load_fo76_definitions(&mut self) {
        for def in crate::fo76_defs::register_all() {
            self.register(GameId::Fallout76, def);
        }
    }

    /// Load all Starfield record definitions.
    pub fn load_starfield_definitions(&mut self) {
        for def in crate::starfield_defs::register_all() {
            self.register(GameId::Starfield, def);
        }
    }

    /// Number of definitions currently registered.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

impl Default for DefinitionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_sse_definitions() {
        let mut registry = DefinitionRegistry::new();
        registry.load_sse_definitions();

        // At least 100 record definitions loaded
        let sigs = registry.signatures_for_game(GameId::SkyrimSE);
        assert!(
            sigs.len() >= 100,
            "Expected at least 100 SSE record defs, got {}",
            sigs.len()
        );
    }

    #[test]
    fn test_sse_weap_record_exists() {
        let mut registry = DefinitionRegistry::new();
        registry.load_sse_definitions();

        let weap = registry
            .get(GameId::SkyrimSE, Signature(*b"WEAP"))
            .expect("WEAP record should exist");
        assert_eq!(weap.signature, Signature(*b"WEAP"));
        assert_eq!(weap.name, "Weapon");
    }

    #[test]
    fn test_sse_npc_record_exists() {
        let mut registry = DefinitionRegistry::new();
        registry.load_sse_definitions();

        let npc = registry
            .get(GameId::SkyrimSE, Signature(*b"NPC_"))
            .expect("NPC_ record should exist");
        assert_eq!(npc.signature, Signature(*b"NPC_"));
    }

    #[test]
    fn test_sse_weap_dnam_fields() {
        let mut registry = DefinitionRegistry::new();
        registry.load_sse_definitions();

        let weap = registry
            .get(GameId::SkyrimSE, Signature(*b"WEAP"))
            .expect("WEAP record should exist");

        // Find DNAM subrecord named "Data"
        let dnam = weap
            .members
            .iter()
            .find(|s| s.signature == Signature(*b"DNAM") && s.name == "Data")
            .expect("WEAP should have a DNAM 'Data' subrecord");

        // DNAM has 29 fields (Animation Type, Speed, Reach, Flags, etc.)
        assert_eq!(
            dnam.fields.len(),
            29,
            "WEAP DNAM should have 29 fields, got {}",
            dnam.fields.len()
        );
    }

    #[test]
    fn test_load_fo4_definitions() {
        let mut registry = DefinitionRegistry::new();
        registry.load_fo4_definitions();

        let sigs = registry.signatures_for_game(GameId::Fallout4);
        assert!(
            sigs.len() >= 50,
            "Expected at least 50 FO4 record defs, got {}",
            sigs.len()
        );
    }
}
