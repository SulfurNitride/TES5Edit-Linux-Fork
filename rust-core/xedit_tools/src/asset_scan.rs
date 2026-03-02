//! Scan plugin records for asset references (texture paths, mesh paths, sound paths).
//!
//! Many record types embed file paths to external assets in subrecords such as
//! MODL (model path), MOD2/MOD3/MOD4 (alternative model paths), ICON/MICO
//! (inventory/menu icons), and various sound-related subrecords. This module
//! walks all records in a plugin and extracts those path strings.

use xedit_dom::{Plugin, Record, Signature};

/// Subrecord signatures known to contain asset file paths.
///
/// These are the most common path-bearing subrecords across TES4+ games.
const ASSET_SUBRECORD_SIGS: &[[u8; 4]] = &[
    *b"MODL", // Primary model path
    *b"MOD2", // Female 1st-person model (armor/NPC)
    *b"MOD3", // Male 1st-person model (armor)
    *b"MOD4", // Female 3rd-person model (armor)
    *b"MOD5", // Model path variant
    *b"ICON", // Inventory icon path
    *b"MICO", // Message icon / small icon path
    *b"ICO2", // Female icon path
    *b"MIC2", // Female message icon
    *b"NAM0", // Texture set path (some record types)
    *b"NAM1", // Texture set path (some record types)
    *b"DNAM", // Sound file in some records
    *b"ANAM", // Sound file / ambient path
    *b"BNAM", // Sound file in some records
    *b"CNAM", // Sound file in some records
    *b"ENAM", // Sound event path
    *b"FNAM", // Sound file reference
    *b"TX00", // Texture set diffuse
    *b"TX01", // Texture set normal
    *b"TX02", // Texture set glow/env mask
    *b"TX03", // Texture set height/parallax
    *b"TX04", // Texture set environment
    *b"TX05", // Texture set multilayer
    *b"TX06", // Texture set backlight
    *b"TX07", // Texture set specular
];

/// A discovered asset reference within a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetReference {
    /// The record type signature (e.g. WEAP, NPC_, ARMO).
    pub record_signature: Signature,
    /// The FormID of the record containing the reference.
    pub form_id: u32,
    /// The Editor ID of the record, if present.
    pub editor_id: Option<String>,
    /// The subrecord signature where the path was found (e.g. MODL, ICON).
    pub subrecord_signature: Signature,
    /// The asset path string extracted from the subrecord.
    pub asset_path: String,
}

/// Scan a single record for asset path references.
///
/// Returns all asset references found in asset-bearing subrecords.
fn scan_record(record: &Record) -> Vec<AssetReference> {
    let mut refs = Vec::new();
    let editor_id = record.editor_id().map(str::to_string);

    for subrecord in &record.subrecords {
        if is_asset_subrecord(&subrecord.signature) {
            if let Some(path) = extract_path_string(&subrecord.raw_data) {
                if !path.is_empty() {
                    refs.push(AssetReference {
                        record_signature: record.signature,
                        form_id: record.form_id.raw(),
                        editor_id: editor_id.clone(),
                        subrecord_signature: subrecord.signature,
                        asset_path: path,
                    });
                }
            }
        }
    }

    refs
}

/// Check if a subrecord signature is one of the known asset-path-bearing types.
fn is_asset_subrecord(sig: &Signature) -> bool {
    ASSET_SUBRECORD_SIGS
        .iter()
        .any(|s| sig.0 == *s)
}

/// Extract a null-terminated (or length-bounded) string from subrecord data.
///
/// Asset paths in Bethesda plugins are stored as null-terminated byte strings.
/// They are typically Windows-1252 encoded but usually ASCII for file paths.
fn extract_path_string(data: &[u8]) -> Option<String> {
    if data.is_empty() {
        return None;
    }

    // Find null terminator or use full length.
    let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    if len == 0 {
        return None;
    }

    // Best-effort UTF-8 conversion; paths are almost always ASCII.
    String::from_utf8(data[..len].to_vec()).ok().or_else(|| {
        // Fall back to lossy conversion for non-UTF8 paths.
        Some(String::from_utf8_lossy(&data[..len]).into_owned())
    })
}

/// Scan all records in a plugin for asset path references.
///
/// Walks every record (including those nested in sub-groups) and returns
/// a list of all discovered asset path references.
pub fn scan_plugin_assets(plugin: &Plugin) -> Vec<AssetReference> {
    let mut results = Vec::new();

    for record in plugin.all_records() {
        results.extend(scan_record(record));
    }

    results
}

/// Scan a plugin and return only the unique asset path strings (deduplicated, lowercase).
///
/// This is a convenience function for generating a flat list of referenced assets
/// without duplicates. Paths are normalized to lowercase with forward slashes.
pub fn scan_unique_asset_paths(plugin: &Plugin) -> Vec<String> {
    let refs = scan_plugin_assets(plugin);
    let mut paths: Vec<String> = refs
        .into_iter()
        .map(|r| r.asset_path.to_lowercase().replace('\\', "/"))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use xedit_dom::group::{Group, GroupChild, GroupType};
    use xedit_dom::record::RecordFlags;
    use xedit_dom::{FormId, GameId, Subrecord};

    fn make_plugin_with_records(records: Vec<Record>) -> Plugin {
        let group = Group {
            group_type: GroupType::Top(Signature::WEAP),
            stamp: 0,
            unknown: 0,
            children: records.into_iter().map(GroupChild::Record).collect(),
            raw_header: None,
            source_offset: None,
        };

        Plugin {
            game_id: GameId::SkyrimSE,
            file_path: Some(PathBuf::from("Test.esp")),
            header: Record {
                signature: Signature::TES4,
                flags: RecordFlags::NONE,
                form_id: FormId::NULL,
                vc_info: 0,
                version: 0,
                unknown: 0,
                subrecords: vec![],
                raw_header: None,
                raw_compressed_data: None,
                raw_data: None,
                source_offset: None,
                modified: false,
            },
            groups: vec![group],
            tes3_records: Vec::new(),
            masters: vec![],
            description: None,
            author: None,
            modified: false,
        }
    }

    #[test]
    fn test_scan_finds_model_path() {
        let mut model_data = b"meshes/weapons/iron/ironsword.nif".to_vec();
        model_data.push(0); // null terminator

        let record = Record {
            signature: Signature::WEAP,
            flags: RecordFlags::NONE,
            form_id: FormId::new(0x00012345),
            vc_info: 0,
            version: 0,
            unknown: 0,
            subrecords: vec![
                Subrecord::new(Signature::from_bytes(b"EDID"), b"IronSword\0".to_vec()),
                Subrecord::new(Signature::MODL, model_data),
            ],
            raw_header: None,
            raw_compressed_data: None,
            raw_data: None,
            source_offset: None,
            modified: false,
        };

        let plugin = make_plugin_with_records(vec![record]);
        let refs = scan_plugin_assets(&plugin);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].asset_path, "meshes/weapons/iron/ironsword.nif");
        assert_eq!(refs[0].subrecord_signature, Signature::MODL);
        assert_eq!(refs[0].editor_id.as_deref(), Some("IronSword"));
    }

    #[test]
    fn test_scan_finds_multiple_asset_types() {
        let record = Record {
            signature: Signature::WEAP,
            flags: RecordFlags::NONE,
            form_id: FormId::new(0x00012345),
            vc_info: 0,
            version: 0,
            unknown: 0,
            subrecords: vec![
                Subrecord::new(
                    Signature::MODL,
                    b"meshes/weapons/sword.nif\0".to_vec(),
                ),
                Subrecord::new(
                    Signature::from_bytes(b"ICON"),
                    b"textures/weapons/sword_icon.dds\0".to_vec(),
                ),
                Subrecord::new(
                    Signature::from_bytes(b"MICO"),
                    b"textures/weapons/sword_micon.dds\0".to_vec(),
                ),
            ],
            raw_header: None,
            raw_compressed_data: None,
            raw_data: None,
            source_offset: None,
            modified: false,
        };

        let plugin = make_plugin_with_records(vec![record]);
        let refs = scan_plugin_assets(&plugin);

        assert_eq!(refs.len(), 3);

        let paths: Vec<&str> = refs.iter().map(|r| r.asset_path.as_str()).collect();
        assert!(paths.contains(&"meshes/weapons/sword.nif"));
        assert!(paths.contains(&"textures/weapons/sword_icon.dds"));
        assert!(paths.contains(&"textures/weapons/sword_micon.dds"));
    }

    #[test]
    fn test_scan_unique_deduplicates() {
        // Two records referencing the same texture.
        let records = vec![
            Record {
                signature: Signature::WEAP,
                flags: RecordFlags::NONE,
                form_id: FormId::new(0x00000001),
                vc_info: 0,
                version: 0,
                unknown: 0,
                subrecords: vec![Subrecord::new(
                    Signature::MODL,
                    b"Meshes\\Weapons\\Sword.nif\0".to_vec(),
                )],
                raw_header: None,
                raw_compressed_data: None,
                raw_data: None,
                source_offset: None,
                modified: false,
            },
            Record {
                signature: Signature::WEAP,
                flags: RecordFlags::NONE,
                form_id: FormId::new(0x00000002),
                vc_info: 0,
                version: 0,
                unknown: 0,
                subrecords: vec![Subrecord::new(
                    Signature::MODL,
                    b"meshes/weapons/sword.nif\0".to_vec(),
                )],
                raw_header: None,
                raw_compressed_data: None,
                raw_data: None,
                source_offset: None,
                modified: false,
            },
        ];

        let plugin = make_plugin_with_records(records);
        let paths = scan_unique_asset_paths(&plugin);

        // Both should normalize to the same path.
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], "meshes/weapons/sword.nif");
    }
}
