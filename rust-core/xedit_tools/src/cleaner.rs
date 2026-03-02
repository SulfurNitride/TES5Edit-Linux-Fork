//! Plugin cleaning: remove ITM records and undelete deleted references.
//!
//! This module provides automated cleaning operations commonly used to prepare
//! plugins for distribution. It uses the conflict detection from `xedit_core`
//! to identify:
//!
//! - **ITM (Identical to Master) records**: Records that are byte-identical to
//!   their master version and serve no purpose. These are removed entirely.
//!
//! - **Deleted references**: Records with the DELETED flag that can cause
//!   crashes when the game engine tries to access them. These are "undeleted"
//!   by clearing the DELETED flag and replacing their subrecords with a
//!   minimal disabled reference (setting the initially-disabled flag and
//!   moving the reference to a safe position).

use std::collections::HashSet;

use xedit_dom::group::GroupChild;
use xedit_dom::record::RecordFlags;
use xedit_dom::{FormId, Group, Plugin, Record, Signature, Subrecord};

/// The result of a cleaning operation on a plugin.
#[derive(Debug, Clone, Default)]
pub struct CleanResult {
    /// Number of ITM records removed.
    pub itm_removed: usize,
    /// FormIDs of the ITM records that were removed.
    pub itm_form_ids: Vec<FormId>,
    /// Number of deleted references that were undeleted.
    pub deleted_cleaned: usize,
    /// FormIDs of the deleted references that were cleaned.
    pub deleted_form_ids: Vec<FormId>,
}

/// Remove ITM (Identical to Master) records from a plugin.
///
/// Given a set of FormIDs identified as ITM by the conflict detector,
/// this function removes those records from the plugin's group tree.
/// Returns the number of records actually removed.
pub fn remove_itm_records(plugin: &mut Plugin, itm_form_ids: &[FormId]) -> usize {
    if itm_form_ids.is_empty() {
        return 0;
    }

    let itm_set: HashSet<u32> = itm_form_ids.iter().map(|fid| fid.raw()).collect();
    let mut removed = 0;

    for group in &mut plugin.groups {
        removed += remove_itm_from_group(group, &itm_set);
    }

    if removed > 0 {
        plugin.modified = true;
    }

    removed
}

/// Recursively remove ITM records from a group and its sub-groups.
fn remove_itm_from_group(group: &mut Group, itm_set: &HashSet<u32>) -> usize {
    let mut removed = 0;
    let original_len = group.children.len();

    group.children.retain_mut(|child| match child {
        GroupChild::Record(record) => {
            if itm_set.contains(&record.form_id.raw()) {
                removed += 1;
                false // remove this record
            } else {
                true // keep it
            }
        }
        GroupChild::Group(sub_group) => {
            removed += remove_itm_from_group(sub_group, itm_set);
            true // always keep groups (even if emptied)
        }
    });

    // Adjust removed count based on actual removals at this level.
    let actual_removed_here = original_len - group.children.len();
    // removed already counts sub-group removals; add this level's count.
    removed = removed - actual_removed_here + actual_removed_here;

    removed
}

/// Record flags for the initially-disabled state.
const INITIALLY_DISABLED: u32 = 0x0000_0800;

/// "Undelete" deleted references in a plugin.
///
/// For each record with the DELETED flag, this function:
/// 1. Clears the DELETED flag.
/// 2. Sets the initially-disabled flag so the reference does not appear in-game.
/// 3. Replaces the record's subrecords with a minimal DATA subrecord that
///    positions the reference at (0, 0, -30000) to move it safely underground.
///
/// This prevents the CTDs (crashes to desktop) that deleted references can cause
/// when the game engine tries to access them.
///
/// Returns the number of references cleaned.
pub fn undelete_references(plugin: &mut Plugin, deleted_form_ids: &[FormId]) -> usize {
    if deleted_form_ids.is_empty() {
        return 0;
    }

    let deleted_set: HashSet<u32> = deleted_form_ids.iter().map(|fid| fid.raw()).collect();
    let mut cleaned = 0;

    for group in &mut plugin.groups {
        cleaned += undelete_in_group(group, &deleted_set);
    }

    if cleaned > 0 {
        plugin.modified = true;
    }

    cleaned
}

/// Recursively process deleted references in a group.
fn undelete_in_group(group: &mut Group, deleted_set: &HashSet<u32>) -> usize {
    let mut cleaned = 0;

    for child in &mut group.children {
        match child {
            GroupChild::Record(record) => {
                if deleted_set.contains(&record.form_id.raw())
                    && record.flags.is_deleted()
                    && is_reference_record(&record.signature)
                {
                    undelete_record(record);
                    cleaned += 1;
                }
            }
            GroupChild::Group(sub_group) => {
                cleaned += undelete_in_group(sub_group, deleted_set);
            }
        }
    }

    cleaned
}

/// Check if a record signature is a reference type that can be meaningfully
/// undeleted (REFR, ACHR, ACRE, PGRE, PMIS, PHZD, PARW, PBAR, PBEA, PCON, PFLA).
fn is_reference_record(sig: &Signature) -> bool {
    matches!(
        &sig.0,
        b"REFR" | b"ACHR" | b"ACRE" | b"PGRE" | b"PMIS" | b"PHZD"
            | b"PARW" | b"PBAR" | b"PBEA" | b"PCON" | b"PFLA"
    )
}

/// Transform a deleted record into an undeleted, initially-disabled reference.
fn undelete_record(record: &mut Record) {
    // Clear DELETED flag, set initially-disabled.
    record.flags = RecordFlags(
        (record.flags.0 & !RecordFlags::DELETED) | INITIALLY_DISABLED,
    );

    // Build a minimal DATA subrecord: position (0, 0, -30000) with zero rotation.
    // DATA for REFR is 24 bytes: 3 floats position + 3 floats rotation.
    let mut data = Vec::with_capacity(24);
    data.extend_from_slice(&0.0f32.to_le_bytes()); // X
    data.extend_from_slice(&0.0f32.to_le_bytes()); // Y
    data.extend_from_slice(&(-30000.0f32).to_le_bytes()); // Z (underground)
    data.extend_from_slice(&0.0f32.to_le_bytes()); // Rotation X
    data.extend_from_slice(&0.0f32.to_le_bytes()); // Rotation Y
    data.extend_from_slice(&0.0f32.to_le_bytes()); // Rotation Z

    // Keep the NAME subrecord (base form reference) if present, replace DATA.
    let name_sub = record
        .subrecords
        .iter()
        .find(|sr| sr.signature == Signature::NAME)
        .cloned();

    record.subrecords.clear();
    if let Some(name) = name_sub {
        record.subrecords.push(name);
    }
    record.subrecords.push(Subrecord::new(Signature::DATA, data));

    record.modified = true;
    // Invalidate raw data caches since we changed the record.
    record.raw_header = None;
    record.raw_compressed_data = None;
    record.raw_data = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use xedit_dom::group::{Group, GroupChild, GroupType};
    use xedit_dom::record::RecordFlags;
    use xedit_dom::{FormId, GameId};

    fn make_plugin_with_records(records: Vec<Record>) -> Plugin {
        let group = Group {
            group_type: GroupType::Top(Signature::REFR),
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
    fn test_remove_itm_records() {
        let records = vec![
            Record {
                signature: Signature::WEAP,
                flags: RecordFlags::NONE,
                form_id: FormId::new(0x00001234),
                vc_info: 0,
                version: 0,
                unknown: 0,
                subrecords: vec![Subrecord::new(Signature::EDID, b"ITMRecord\0".to_vec())],
                raw_header: None,
                raw_compressed_data: None,
                raw_data: None,
                source_offset: None,
                modified: false,
            },
            Record {
                signature: Signature::WEAP,
                flags: RecordFlags::NONE,
                form_id: FormId::new(0x00005678),
                vc_info: 0,
                version: 0,
                unknown: 0,
                subrecords: vec![Subrecord::new(Signature::EDID, b"KeepMe\0".to_vec())],
                raw_header: None,
                raw_compressed_data: None,
                raw_data: None,
                source_offset: None,
                modified: false,
            },
        ];

        let mut plugin = make_plugin_with_records(records);
        let itm_ids = vec![FormId::new(0x00001234)];
        let removed = remove_itm_records(&mut plugin, &itm_ids);

        assert_eq!(removed, 1);
        assert!(plugin.modified);

        // Only one record should remain.
        let remaining: Vec<&Record> = plugin.all_records();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].form_id.raw(), 0x00005678);
    }

    #[test]
    fn test_undelete_references() {
        let records = vec![Record {
            signature: Signature::REFR,
            flags: RecordFlags(RecordFlags::DELETED),
            form_id: FormId::new(0x00009ABC),
            vc_info: 0,
            version: 0,
            unknown: 0,
            subrecords: vec![
                Subrecord::new(Signature::NAME, vec![0x34, 0x12, 0x00, 0x00]),
                Subrecord::new(Signature::DATA, vec![0; 24]),
            ],
            raw_header: None,
            raw_compressed_data: None,
            raw_data: None,
            source_offset: None,
            modified: false,
        }];

        let mut plugin = make_plugin_with_records(records);
        let deleted_ids = vec![FormId::new(0x00009ABC)];
        let cleaned = undelete_references(&mut plugin, &deleted_ids);

        assert_eq!(cleaned, 1);
        assert!(plugin.modified);

        let all = plugin.all_records();
        assert_eq!(all.len(), 1);

        let record = all[0];
        // DELETED flag should be cleared.
        assert!(!record.flags.is_deleted());
        // Initially-disabled flag should be set.
        assert_ne!(record.flags.0 & INITIALLY_DISABLED, 0);
        // NAME subrecord should be preserved.
        assert!(record.subrecords.iter().any(|sr| sr.signature == Signature::NAME));
        // DATA subrecord should be present with underground position.
        let data_sub = record
            .subrecords
            .iter()
            .find(|sr| sr.signature == Signature::DATA)
            .expect("DATA subrecord should exist");
        assert_eq!(data_sub.raw_data.len(), 24);

        // Check Z position is -30000.
        let z_bytes: [u8; 4] = data_sub.raw_data[8..12].try_into().unwrap();
        let z = f32::from_le_bytes(z_bytes);
        assert!((z - (-30000.0)).abs() < 0.01);
    }

    #[test]
    fn test_remove_itm_empty_list() {
        let records = vec![Record {
            signature: Signature::WEAP,
            flags: RecordFlags::NONE,
            form_id: FormId::new(0x00001111),
            vc_info: 0,
            version: 0,
            unknown: 0,
            subrecords: vec![],
            raw_header: None,
            raw_compressed_data: None,
            raw_data: None,
            source_offset: None,
            modified: false,
        }];

        let mut plugin = make_plugin_with_records(records);
        let removed = remove_itm_records(&mut plugin, &[]);

        assert_eq!(removed, 0);
        assert!(!plugin.modified);
        assert_eq!(plugin.all_records().len(), 1);
    }
}
