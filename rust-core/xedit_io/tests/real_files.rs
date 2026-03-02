use std::fs;
use std::path::{Path, PathBuf};

use xedit_dom::{GameId, Signature};
use xedit_io::{PluginReader, PluginWriter};

const SSE_DATA_DIR: &str = "/home/luke/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data";
const OBLIVION_DATA_DIR: &str = "/home/luke/.local/share/Steam/steamapps/common/Oblivion/Data";
const FNV_DATA_DIR: &str = "/home/luke/.local/share/Steam/steamapps/common/Fallout New Vegas/Data";
const FO3_DATA_DIR: &str = "/home/luke/.local/share/Steam/steamapps/common/Fallout 3 goty/Data";
const FO4_DATA_DIR: &str = "/home/luke/.local/share/Steam/steamapps/common/Fallout 4/Data";
const MORROWIND_DATA_DIR: &str = "/home/luke/.local/share/Steam/steamapps/common/Morrowind/Data Files";

/// Check that a data directory exists; if not, print a skip message and return.
macro_rules! skip_if_missing {
    ($dir:expr) => {{
        let p = Path::new($dir);
        if !p.exists() {
            eprintln!("Skipping: {:?} not found", p);
            return;
        }
    }};
}

/// Check that a file path exists; if not, print a skip message and return.
macro_rules! skip_if_file_missing {
    ($path:expr) => {{
        if !$path.exists() {
            eprintln!("Skipping: {:?} not found", $path);
            return;
        }
    }};
}

fn find_first_file(dir: &Path, predicate: impl Fn(&str) -> bool) -> PathBuf {
    let mut matches: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", dir.display()))
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(&predicate)
                .unwrap_or(false)
        })
        .collect();

    matches.sort();
    matches
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no matching file found in {}", dir.display()))
}

fn first_diff_offset(lhs: &[u8], rhs: &[u8]) -> Option<usize> {
    let min_len = lhs.len().min(rhs.len());
    for i in 0..min_len {
        if lhs[i] != rhs[i] {
            return Some(i);
        }
    }
    if lhs.len() != rhs.len() {
        Some(min_len)
    } else {
        None
    }
}

fn hex_slice_context(bytes: &[u8], center: usize, radius: usize) -> String {
    let start = center.saturating_sub(radius);
    let end = (center + radius + 1).min(bytes.len());
    let mut out = String::new();
    for (i, b) in bytes[start..end].iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{b:02X}"));
    }
    out
}

fn assert_roundtrip_identical(path: &Path, game_id: GameId) {
    let original =
        fs::read(path).unwrap_or_else(|e| panic!("failed reading {}: {e}", path.display()));
    let reader = PluginReader::new(game_id);
    let plugin = reader
        .read_file(path)
        .unwrap_or_else(|e| panic!("failed parsing {} as {:?}: {e:#}", path.display(), game_id));
    let written = PluginWriter::write_bytes(&plugin)
        .unwrap_or_else(|e| panic!("failed writing {}: {e:#}", path.display()));

    if original != written {
        let offset = first_diff_offset(&original, &written)
            .expect("mismatch reported but diff offset was not found");
        let left = hex_slice_context(&original, offset, 16);
        let right = hex_slice_context(&written, offset, 16);
        panic!(
            "roundtrip mismatch for {} at offset 0x{:X} ({})\noriginal[{}] = {:#04X}\nwritten[{}]  = {:#04X}\noriginal context: {}\nwritten context : {}\norig len: {} written len: {}",
            path.display(),
            offset,
            offset,
            offset,
            original.get(offset).copied().unwrap_or(0),
            offset,
            written.get(offset).copied().unwrap_or(0),
            left,
            right,
            original.len(),
            written.len(),
        );
    }
}

#[test]
fn real_sse_small_esl_roundtrip_byte_identical() {
    skip_if_missing!(SSE_DATA_DIR);
    let sse_dir = Path::new(SSE_DATA_DIR);
    let esl = find_first_file(sse_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("cc") && lower.ends_with(".esl")
    });
    assert_roundtrip_identical(&esl, GameId::SkyrimSE);
}

#[test]
fn real_sse_hearthfires_roundtrip_byte_identical() {
    let path = Path::new(SSE_DATA_DIR).join("HearthFires.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_identical(&path, GameId::SkyrimSE);
}

#[test]
fn real_oblivion_dlc_esp_roundtrip_byte_identical() {
    skip_if_missing!(OBLIVION_DATA_DIR);
    let data_dir = Path::new(OBLIVION_DATA_DIR);
    let esp = find_first_file(data_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("dlc") && lower.ends_with(".esp")
    });
    assert_roundtrip_identical(&esp, GameId::Oblivion);
}

#[test]
fn real_morrowind_esm_roundtrip_byte_identical() {
    let path = Path::new(MORROWIND_DATA_DIR).join("Morrowind.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_identical(&path, GameId::Morrowind);
}

#[test]
fn real_fnv_dlc_esm_roundtrip_byte_identical() {
    skip_if_missing!(FNV_DATA_DIR);
    let data_dir = Path::new(FNV_DATA_DIR);
    let esm = find_first_file(data_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".esm") && lower != "falloutnv.esm"
    });
    assert_roundtrip_identical(&esm, GameId::FalloutNV);
}

#[test]
fn real_fo3_dlc_esm_roundtrip_byte_identical() {
    skip_if_missing!(FO3_DATA_DIR);
    let data_dir = Path::new(FO3_DATA_DIR);
    let esm = find_first_file(data_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".esm") && lower != "fallout3.esm"
    });
    assert_roundtrip_identical(&esm, GameId::Fallout3);
}

#[test]
fn real_fo4_dlc_esm_roundtrip_byte_identical() {
    skip_if_missing!(FO4_DATA_DIR);
    let data_dir = Path::new(FO4_DATA_DIR);
    let esm = find_first_file(data_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".esm") && lower != "fallout4.esm" && lower.starts_with("dlc")
    });
    assert_roundtrip_identical(&esm, GameId::Fallout4);
}

#[test]
fn real_fo4_small_esl_roundtrip_byte_identical() {
    skip_if_missing!(FO4_DATA_DIR);
    let data_dir = Path::new(FO4_DATA_DIR);
    let esl = find_first_file(data_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("cc") && lower.ends_with(".esl")
    });
    assert_roundtrip_identical(&esl, GameId::Fallout4);
}

#[test]
fn real_sse_skyrim_esm_parse_sanity() {
    let path = Path::new(SSE_DATA_DIR).join("Skyrim.esm");
    skip_if_file_missing!(path);
    let reader = PluginReader::new(GameId::SkyrimSE);
    let plugin = reader
        .read_file(&path)
        .unwrap_or_else(|e| panic!("failed parsing {}: {e:#}", path.display()));

    assert_eq!(plugin.header.signature, Signature::TES4);
    // Skyrim.esm is a base game master - it has no MAST entries (no masters).
    assert!(plugin.masters.is_empty(), "expected Skyrim.esm to have no masters (it is a base master)");
    assert!(
        plugin.all_records().len() > 0,
        "expected Skyrim.esm to contain records"
    );
}

// ===========================================================================
// Multi-plugin roundtrip tests: load -> save to bytes -> reload -> compare
// ===========================================================================

/// Load a plugin, serialize to bytes, reload from those bytes, and compare
/// byte-for-byte with the original file.
fn assert_roundtrip_via_memory(path: &Path, game_id: GameId) {
    let original =
        fs::read(path).unwrap_or_else(|e| panic!("failed reading {}: {e}", path.display()));
    let reader = PluginReader::new(game_id);
    let plugin = reader
        .read_file(path)
        .unwrap_or_else(|e| panic!("failed parsing {}: {e:#}", path.display()));

    let written = PluginWriter::write_bytes(&plugin)
        .unwrap_or_else(|e| panic!("failed writing {}: {e:#}", path.display()));

    // Reload from written bytes.
    let reloaded = reader
        .read_bytes(&written, None)
        .unwrap_or_else(|e| panic!("failed re-parsing written bytes for {}: {e:#}", path.display()));

    // Compare original bytes to written bytes.
    if original != written {
        let offset = first_diff_offset(&original, &written)
            .expect("mismatch reported but diff offset was not found");
        panic!(
            "roundtrip mismatch for {} at offset 0x{:X}\norig len: {} written len: {}",
            path.display(),
            offset,
            original.len(),
            written.len(),
        );
    }

    // Verify structural equivalence after reload.
    assert_eq!(
        plugin.masters, reloaded.masters,
        "masters should match after roundtrip for {}",
        path.display()
    );
    assert_eq!(
        plugin.all_records().len(),
        reloaded.all_records().len(),
        "record count should match after roundtrip for {}",
        path.display()
    );

    // Verify the re-serialized output is also identical.
    let rewritten = PluginWriter::write_bytes(&reloaded)
        .unwrap_or_else(|e| panic!("failed re-writing {}: {e:#}", path.display()));
    assert_eq!(
        written, rewritten,
        "double roundtrip should be byte-identical for {}",
        path.display()
    );
}

#[test]
fn real_sse_update_esm_roundtrip() {
    let path = Path::new(SSE_DATA_DIR).join("Update.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_via_memory(&path, GameId::SkyrimSE);
}

#[test]
fn real_sse_dawnguard_roundtrip() {
    let path = Path::new(SSE_DATA_DIR).join("Dawnguard.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_via_memory(&path, GameId::SkyrimSE);
}

#[test]
fn real_fo4_dlc_roundtrip() {
    skip_if_missing!(FO4_DATA_DIR);
    let data_dir = Path::new(FO4_DATA_DIR);
    let esm = find_first_file(data_dir, |name| {
        let lower = name.to_ascii_lowercase();
        lower.ends_with(".esm") && lower != "fallout4.esm" && lower.starts_with("dlc")
    });
    assert_roundtrip_via_memory(&esm, GameId::Fallout4);
}

#[test]
fn real_fnv_base_esm_roundtrip() {
    let path = Path::new(FNV_DATA_DIR).join("FalloutNV.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_via_memory(&path, GameId::FalloutNV);
}

#[test]
fn real_fo3_base_esm_roundtrip() {
    let path = Path::new(FO3_DATA_DIR).join("Fallout3.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_via_memory(&path, GameId::Fallout3);
}

#[test]
fn real_oblivion_esm_roundtrip() {
    let path = Path::new(OBLIVION_DATA_DIR).join("Oblivion.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_via_memory(&path, GameId::Oblivion);
}

#[test]
fn real_morrowind_esm_roundtrip() {
    let path = Path::new(MORROWIND_DATA_DIR).join("Morrowind.esm");
    skip_if_file_missing!(path);
    assert_roundtrip_via_memory(&path, GameId::Morrowind);
}

// ===========================================================================
// Multi-plugin roundtrip: load multiple plugins, save each, verify each
// ===========================================================================

#[test]
fn real_sse_multi_plugin_roundtrip() {
    skip_if_missing!(SSE_DATA_DIR);
    let files = ["Skyrim.esm", "Update.esm", "Dawnguard.esm", "HearthFires.esm", "Dragonborn.esm"];
    let data_dir = Path::new(SSE_DATA_DIR);
    for filename in &files {
        let path = data_dir.join(filename);
        if path.exists() {
            assert_roundtrip_identical(&path, GameId::SkyrimSE);
        }
    }
}
