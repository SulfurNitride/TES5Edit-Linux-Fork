//! Regression tests: multi-plugin load order tests using real game files.
//!
//! Tests that require real game installations will auto-skip when the game
//! data directory is not found, so they run in CI without failures.

use std::path::Path;

use xedit_core::load_order::LoadOrder;
use xedit_dom::{GameId, Signature};
use xedit_io::PluginReader;

// ---------------------------------------------------------------------------
// Game data directories
// ---------------------------------------------------------------------------

const SSE_DATA_DIR: &str =
    "/home/luke/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data";
const FO4_DATA_DIR: &str =
    "/home/luke/.local/share/Steam/steamapps/common/Fallout 4/Data";
const FNV_DATA_DIR: &str =
    "/home/luke/.local/share/Steam/steamapps/common/Fallout New Vegas/Data";
const FO3_DATA_DIR: &str =
    "/home/luke/.local/share/Steam/steamapps/common/Fallout 3 goty/Data";
const OBLIVION_DATA_DIR: &str =
    "/home/luke/.local/share/Steam/steamapps/common/Oblivion/Data";
const MORROWIND_DATA_DIR: &str =
    "/home/luke/.local/share/Steam/steamapps/common/Morrowind/Data Files";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check that a data directory exists; if not, print a skip message and return false.
macro_rules! skip_if_missing {
    ($dir:expr) => {{
        let p = Path::new($dir);
        if !p.exists() {
            eprintln!("Skipping: {:?} not found", p);
            return;
        }
    }};
}

/// Load a plugin from the given data directory, returning the parsed plugin.
fn load_plugin(game_id: GameId, data_dir: &str, filename: &str) -> xedit_dom::Plugin {
    let path = Path::new(data_dir).join(filename);
    assert!(
        path.exists(),
        "Expected game file not found: {}",
        path.display()
    );
    let reader = PluginReader::new(game_id);
    reader
        .read_file(&path)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {e:#}", path.display()))
}

/// Build a load order from a list of (data_dir, filename) pairs.
fn build_load_order(game_id: GameId, files: &[(&str, &str)]) -> LoadOrder {
    let mut lo = LoadOrder::new(game_id);
    for &(data_dir, filename) in files {
        let plugin = load_plugin(game_id, data_dir, filename);
        lo.add_plugin(plugin);
    }
    lo.sort_load_order();
    lo
}

// ===========================================================================
// Skyrim Special Edition: multi-plugin load order
// ===========================================================================

#[test]
fn sse_multi_plugin_load_order_masters_and_records() {
    skip_if_missing!(SSE_DATA_DIR);

    let lo = build_load_order(
        GameId::SkyrimSE,
        &[
            (SSE_DATA_DIR, "Skyrim.esm"),
            (SSE_DATA_DIR, "Update.esm"),
            (SSE_DATA_DIR, "Dawnguard.esm"),
        ],
    );

    assert_eq!(lo.plugins.len(), 3, "should have 3 plugins loaded");

    // After sorting, all three are ESMs so they stay in insertion order.
    let skyrim = &lo.plugins[0];
    let update = &lo.plugins[1];
    let dawnguard = &lo.plugins[2];

    // Skyrim.esm: base master, no master list of its own.
    assert!(
        skyrim.masters.is_empty(),
        "Skyrim.esm should have no masters"
    );

    // Update.esm lists Skyrim.esm as its master.
    assert!(
        update.masters.iter().any(|m| m.eq_ignore_ascii_case("Skyrim.esm")),
        "Update.esm should list Skyrim.esm as a master, got: {:?}",
        update.masters
    );

    // Dawnguard.esm lists Skyrim.esm (and possibly Update.esm) as master(s).
    assert!(
        dawnguard.masters.iter().any(|m| m.eq_ignore_ascii_case("Skyrim.esm")),
        "Dawnguard.esm should list Skyrim.esm as a master, got: {:?}",
        dawnguard.masters
    );

    // Record count sanity: Skyrim.esm has tens of thousands of records.
    let skyrim_record_count = skyrim.all_records().len();
    assert!(
        skyrim_record_count > 10_000,
        "Skyrim.esm should have >10k records, got {}",
        skyrim_record_count
    );

    // Update.esm has meaningful record content (hundreds of overrides).
    let update_record_count = update.all_records().len();
    assert!(
        update_record_count > 100,
        "Update.esm should have >100 records, got {}",
        update_record_count
    );

    // Dawnguard.esm has thousands of records.
    let dawnguard_record_count = dawnguard.all_records().len();
    assert!(
        dawnguard_record_count > 1_000,
        "Dawnguard.esm should have >1k records, got {}",
        dawnguard_record_count
    );
}

#[test]
fn sse_update_esm_override_detection() {
    skip_if_missing!(SSE_DATA_DIR);

    let lo = build_load_order(
        GameId::SkyrimSE,
        &[
            (SSE_DATA_DIR, "Skyrim.esm"),
            (SSE_DATA_DIR, "Update.esm"),
        ],
    );

    // Find records in Update.esm whose FormIDs resolve back to Skyrim.esm.
    // These are overrides.
    let update_plugin = &lo.plugins[1];
    let mut override_count = 0usize;
    for record in update_plugin.all_records() {
        if let Some((target_plugin, _local)) =
            lo.resolve_form_id(1, record.form_id)
        {
            if target_plugin == 0 {
                override_count += 1;
            }
        }
    }

    assert!(
        override_count > 50,
        "Update.esm should override >50 records from Skyrim.esm, found {}",
        override_count
    );
}

#[test]
fn sse_master_resolution_consistency() {
    skip_if_missing!(SSE_DATA_DIR);

    let lo = build_load_order(
        GameId::SkyrimSE,
        &[
            (SSE_DATA_DIR, "Skyrim.esm"),
            (SSE_DATA_DIR, "Update.esm"),
            (SSE_DATA_DIR, "Dawnguard.esm"),
        ],
    );

    // Every record in Update.esm with master index 0 should resolve to Skyrim.esm (plugin 0).
    for record in lo.plugins[1].all_records() {
        if record.form_id.master_index() == 0 {
            let resolved = lo.resolve_form_id(1, record.form_id);
            assert!(
                resolved.is_some(),
                "Update.esm record {:08X} should resolve",
                record.form_id.raw()
            );
            let (target_plugin, _) = resolved.unwrap();
            assert_eq!(
                target_plugin, 0,
                "Update.esm record {:08X} with master_index=0 should resolve to plugin 0 (Skyrim.esm)",
                record.form_id.raw()
            );
        }
    }
}

// ===========================================================================
// Fallout 4: multi-plugin load order
// ===========================================================================

#[test]
fn fo4_multi_plugin_load_order() {
    skip_if_missing!(FO4_DATA_DIR);

    let lo = build_load_order(
        GameId::Fallout4,
        &[
            (FO4_DATA_DIR, "Fallout4.esm"),
            (FO4_DATA_DIR, "DLCRobot.esm"),
        ],
    );

    assert_eq!(lo.plugins.len(), 2);

    let fo4 = &lo.plugins[0];
    assert!(
        fo4.masters.is_empty(),
        "Fallout4.esm should have no masters"
    );

    let dlc = &lo.plugins[1];
    assert!(
        dlc.masters.iter().any(|m| m.eq_ignore_ascii_case("Fallout4.esm")),
        "DLCRobot.esm should list Fallout4.esm as master, got: {:?}",
        dlc.masters
    );

    let fo4_records = fo4.all_records().len();
    assert!(
        fo4_records > 10_000,
        "Fallout4.esm should have >10k records, got {}",
        fo4_records
    );
}

#[test]
fn fo4_override_detection() {
    skip_if_missing!(FO4_DATA_DIR);

    let lo = build_load_order(
        GameId::Fallout4,
        &[
            (FO4_DATA_DIR, "Fallout4.esm"),
            (FO4_DATA_DIR, "DLCRobot.esm"),
        ],
    );

    let mut override_count = 0usize;
    for record in lo.plugins[1].all_records() {
        if let Some((target_plugin, _)) = lo.resolve_form_id(1, record.form_id) {
            if target_plugin == 0 {
                override_count += 1;
            }
        }
    }

    // DLC plugins typically override at least some base game records.
    assert!(
        override_count > 0,
        "DLCRobot.esm should have at least some overrides of Fallout4.esm"
    );
}

// ===========================================================================
// Fallout New Vegas: multi-plugin load order
// ===========================================================================

#[test]
fn fnv_multi_plugin_load_order() {
    skip_if_missing!(FNV_DATA_DIR);

    let lo = build_load_order(
        GameId::FalloutNV,
        &[
            (FNV_DATA_DIR, "FalloutNV.esm"),
            (FNV_DATA_DIR, "DeadMoney.esm"),
        ],
    );

    assert_eq!(lo.plugins.len(), 2);

    let fnv = &lo.plugins[0];
    assert!(
        fnv.masters.is_empty(),
        "FalloutNV.esm should have no masters"
    );

    let dlc = &lo.plugins[1];
    assert!(
        dlc.masters.iter().any(|m| m.eq_ignore_ascii_case("FalloutNV.esm")),
        "DeadMoney.esm should list FalloutNV.esm as master, got: {:?}",
        dlc.masters
    );

    let fnv_records = fnv.all_records().len();
    assert!(
        fnv_records > 10_000,
        "FalloutNV.esm should have >10k records, got {}",
        fnv_records
    );
}

// ===========================================================================
// Fallout 3: multi-plugin load order
// ===========================================================================

#[test]
fn fo3_multi_plugin_load_order() {
    skip_if_missing!(FO3_DATA_DIR);

    let lo = build_load_order(
        GameId::Fallout3,
        &[
            (FO3_DATA_DIR, "Fallout3.esm"),
            (FO3_DATA_DIR, "Anchorage.esm"),
        ],
    );

    assert_eq!(lo.plugins.len(), 2);

    let fo3 = &lo.plugins[0];
    assert!(
        fo3.masters.is_empty(),
        "Fallout3.esm should have no masters"
    );

    let dlc = &lo.plugins[1];
    assert!(
        dlc.masters.iter().any(|m| m.eq_ignore_ascii_case("Fallout3.esm")),
        "Anchorage.esm should list Fallout3.esm as master, got: {:?}",
        dlc.masters
    );

    let fo3_records = fo3.all_records().len();
    assert!(
        fo3_records > 5_000,
        "Fallout3.esm should have >5k records, got {}",
        fo3_records
    );
}

// ===========================================================================
// Oblivion: multi-plugin load order
// ===========================================================================

#[test]
fn oblivion_multi_plugin_load_order() {
    skip_if_missing!(OBLIVION_DATA_DIR);

    // Oblivion DLCs are .esp files, but the master is Oblivion.esm.
    let reader = PluginReader::new(GameId::Oblivion);
    let oblivion = reader
        .read_file(Path::new(OBLIVION_DATA_DIR).join("Oblivion.esm").as_ref())
        .expect("should parse Oblivion.esm");

    assert!(
        oblivion.masters.is_empty(),
        "Oblivion.esm should have no masters"
    );

    let record_count = oblivion.all_records().len();
    assert!(
        record_count > 10_000,
        "Oblivion.esm should have >10k records, got {}",
        record_count
    );

    // Verify groups exist.
    assert!(
        oblivion.groups.len() > 10,
        "Oblivion.esm should have many top-level groups, got {}",
        oblivion.groups.len()
    );
}

// ===========================================================================
// Morrowind: load order (flat record model, no GRUPs)
// ===========================================================================

#[test]
fn morrowind_esm_load_sanity() {
    skip_if_missing!(MORROWIND_DATA_DIR);

    let reader = PluginReader::new(GameId::Morrowind);
    let morrowind = reader
        .read_file(Path::new(MORROWIND_DATA_DIR).join("Morrowind.esm").as_ref())
        .expect("should parse Morrowind.esm");

    assert_eq!(morrowind.header.signature, Signature::TES3);

    // Morrowind uses tes3_records (flat list), not groups.
    assert!(
        morrowind.groups.is_empty(),
        "Morrowind should have no TES4-style groups"
    );
    assert!(
        morrowind.tes3_records.len() > 1_000,
        "Morrowind.esm should have >1k records, got {}",
        morrowind.tes3_records.len()
    );
}

#[test]
fn morrowind_multi_plugin_load_order() {
    skip_if_missing!(MORROWIND_DATA_DIR);

    let lo = build_load_order(
        GameId::Morrowind,
        &[
            (MORROWIND_DATA_DIR, "Morrowind.esm"),
            (MORROWIND_DATA_DIR, "Tribunal.esm"),
        ],
    );

    assert_eq!(lo.plugins.len(), 2);

    let tribunal = &lo.plugins[1];
    assert!(
        tribunal.masters.iter().any(|m| m.eq_ignore_ascii_case("Morrowind.esm")),
        "Tribunal.esm should list Morrowind.esm as master, got: {:?}",
        tribunal.masters
    );
}

// ===========================================================================
// Cross-game: TES4 header signature validation
// ===========================================================================

#[test]
fn sse_header_is_tes4() {
    skip_if_missing!(SSE_DATA_DIR);

    let plugin = load_plugin(GameId::SkyrimSE, SSE_DATA_DIR, "Skyrim.esm");
    assert_eq!(plugin.header.signature, Signature::TES4);
}

#[test]
fn oblivion_header_is_tes4() {
    skip_if_missing!(OBLIVION_DATA_DIR);

    let plugin = load_plugin(GameId::Oblivion, OBLIVION_DATA_DIR, "Oblivion.esm");
    assert_eq!(plugin.header.signature, Signature::TES4);
}

#[test]
fn morrowind_header_is_tes3() {
    skip_if_missing!(MORROWIND_DATA_DIR);

    let plugin = load_plugin(GameId::Morrowind, MORROWIND_DATA_DIR, "Morrowind.esm");
    assert_eq!(plugin.header.signature, Signature::TES3);
}
