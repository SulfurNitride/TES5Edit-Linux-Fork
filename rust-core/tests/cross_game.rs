//! Cross-game definition and configuration tests.
//!
//! These tests verify that game definitions load correctly for all supported
//! games and that basic GameId operations work as expected.
//! None of these require real game files.

use xedit_dom::GameId;
use xedit_games::DefinitionRegistry;

// ---------------------------------------------------------------------------
// GameId round-trip and basic properties
// ---------------------------------------------------------------------------

#[test]
fn game_id_short_name_round_trip() {
    let all_games = [
        (GameId::Morrowind, "TES3"),
        (GameId::Oblivion, "TES4"),
        (GameId::Fallout3, "FO3"),
        (GameId::FalloutNV, "FNV"),
        (GameId::SkyrimSE, "SSE"),
        (GameId::Fallout4, "FO4"),
        (GameId::Fallout76, "FO76"),
        (GameId::Starfield, "SF1"),
    ];

    for (game_id, expected_short_name) in &all_games {
        assert_eq!(
            game_id.short_name(),
            *expected_short_name,
            "GameId::{:?} should have short_name '{}'",
            game_id,
            expected_short_name
        );
    }
}

#[test]
fn game_id_header_signature_correct() {
    use xedit_dom::Signature;

    // Morrowind uses TES3, all others use TES4.
    assert_eq!(GameId::Morrowind.header_signature(), Signature::TES3);
    assert_eq!(GameId::Oblivion.header_signature(), Signature::TES4);
    assert_eq!(GameId::Fallout3.header_signature(), Signature::TES4);
    assert_eq!(GameId::FalloutNV.header_signature(), Signature::TES4);
    assert_eq!(GameId::SkyrimSE.header_signature(), Signature::TES4);
    assert_eq!(GameId::Fallout4.header_signature(), Signature::TES4);
    assert_eq!(GameId::Fallout76.header_signature(), Signature::TES4);
    assert_eq!(GameId::Starfield.header_signature(), Signature::TES4);
}

#[test]
fn game_id_dialect_family_correct() {
    use xedit_dom::dialect::DialectFamily;

    assert_eq!(GameId::Morrowind.dialect_family(), DialectFamily::TES3);
    assert_eq!(GameId::Oblivion.dialect_family(), DialectFamily::TES4Plus);
    assert_eq!(GameId::Fallout3.dialect_family(), DialectFamily::TES4Plus);
    assert_eq!(GameId::FalloutNV.dialect_family(), DialectFamily::TES4Plus);
    assert_eq!(GameId::SkyrimSE.dialect_family(), DialectFamily::TES4Plus);
    assert_eq!(GameId::Fallout4.dialect_family(), DialectFamily::TES4Plus);
    assert_eq!(GameId::Fallout76.dialect_family(), DialectFamily::TES4Plus);
    assert_eq!(GameId::Starfield.dialect_family(), DialectFamily::TES4Plus);
}

#[test]
fn game_id_equality_and_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(GameId::SkyrimSE);
    set.insert(GameId::Fallout4);
    set.insert(GameId::SkyrimSE); // duplicate

    assert_eq!(set.len(), 2, "HashSet should deduplicate GameId values");
    assert!(set.contains(&GameId::SkyrimSE));
    assert!(set.contains(&GameId::Fallout4));
    assert!(!set.contains(&GameId::Morrowind));
}

// ---------------------------------------------------------------------------
// Definition loading for each game
// ---------------------------------------------------------------------------

#[test]
fn sse_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_sse_definitions();
    let count = reg.signatures_for_game(GameId::SkyrimSE).len();
    assert!(
        count >= 100,
        "SSE should have >= 100 record definitions, got {}",
        count
    );
}

#[test]
fn fo4_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_fo4_definitions();
    let count = reg.signatures_for_game(GameId::Fallout4).len();
    assert!(
        count >= 50,
        "FO4 should have >= 50 record definitions, got {}",
        count
    );
}

#[test]
fn fo3_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_fo3_definitions();
    let count = reg.signatures_for_game(GameId::Fallout3).len();
    assert!(
        count >= 30,
        "FO3 should have >= 30 record definitions, got {}",
        count
    );
}

#[test]
fn fnv_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_fnv_definitions();
    let count = reg.signatures_for_game(GameId::FalloutNV).len();
    assert!(
        count >= 30,
        "FNV should have >= 30 record definitions, got {}",
        count
    );
}

#[test]
fn oblivion_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_tes4_definitions();
    let count = reg.signatures_for_game(GameId::Oblivion).len();
    assert!(
        count >= 30,
        "Oblivion should have >= 30 record definitions, got {}",
        count
    );
}

#[test]
fn morrowind_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_morrowind_definitions();
    let count = reg.signatures_for_game(GameId::Morrowind).len();
    assert!(
        count >= 10,
        "Morrowind should have >= 10 record definitions, got {}",
        count
    );
}

#[test]
fn fo76_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_fo76_definitions();
    let count = reg.signatures_for_game(GameId::Fallout76).len();
    assert!(
        count >= 50,
        "FO76 should have >= 50 record definitions, got {}",
        count
    );
}

#[test]
fn starfield_definitions_load_without_panic() {
    let mut reg = DefinitionRegistry::new();
    reg.load_starfield_definitions();
    let count = reg.signatures_for_game(GameId::Starfield).len();
    assert!(
        count >= 10,
        "Starfield should have >= 10 record definitions, got {}",
        count
    );
}

// ---------------------------------------------------------------------------
// Cross-game definition isolation
// ---------------------------------------------------------------------------

#[test]
fn definitions_for_different_games_are_independent() {
    let mut reg = DefinitionRegistry::new();
    reg.load_sse_definitions();
    reg.load_fo4_definitions();

    let sse_sigs = reg.signatures_for_game(GameId::SkyrimSE);
    let fo4_sigs = reg.signatures_for_game(GameId::Fallout4);

    // Both should have definitions, and they should not be identical sets
    // (although many overlap).
    assert!(!sse_sigs.is_empty());
    assert!(!fo4_sigs.is_empty());

    // The total in the registry should be the sum (no cross-contamination).
    assert_eq!(
        reg.len(),
        sse_sigs.len() + fo4_sigs.len(),
        "Registry should contain exactly the sum of SSE + FO4 defs"
    );
}

#[test]
fn all_games_have_nonempty_definitions() {
    let loaders: Vec<(&str, fn(&mut DefinitionRegistry), GameId)> = vec![
        ("SSE", DefinitionRegistry::load_sse_definitions, GameId::SkyrimSE),
        ("FO4", DefinitionRegistry::load_fo4_definitions, GameId::Fallout4),
        ("FO3", DefinitionRegistry::load_fo3_definitions, GameId::Fallout3),
        ("FNV", DefinitionRegistry::load_fnv_definitions, GameId::FalloutNV),
        ("TES4", DefinitionRegistry::load_tes4_definitions, GameId::Oblivion),
        ("TES3", DefinitionRegistry::load_morrowind_definitions, GameId::Morrowind),
        ("FO76", DefinitionRegistry::load_fo76_definitions, GameId::Fallout76),
        ("SF1", DefinitionRegistry::load_starfield_definitions, GameId::Starfield),
    ];

    for (label, loader, game_id) in loaders {
        let mut reg = DefinitionRegistry::new();
        loader(&mut reg);
        let count = reg.signatures_for_game(game_id).len();
        assert!(
            count > 0,
            "{} ({:?}) should have at least 1 definition, got 0",
            label,
            game_id
        );
    }
}
