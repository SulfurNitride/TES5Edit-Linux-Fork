//! Conflict detection between plugins.
//!
//! Provides multi-plugin conflict detection, ITM (Identical to Master) detection,
//! deleted reference detection, override chain tracking, and subrecord-level diffing.

use std::collections::HashMap;

use xedit_dom::{FormId, Record, Signature, Subrecord};

use crate::load_order::LoadOrder;

/// Severity of a conflict between records across plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictSeverity {
    /// Records differ in actual subrecord data (gameplay-affecting).
    Critical,
    /// Record exists in multiple plugins with different data.
    Override,
    /// Records differ only in non-gameplay header fields (vc_info, version, unknown).
    Benign,
    /// Record is identical to its master version (Identical to Master).
    ITM,
}

/// A conflict detected for a single FormID across multiple plugins.
#[derive(Debug, Clone)]
pub struct RecordConflict {
    /// The canonical FormID involved in the conflict.
    pub form_id: FormId,
    /// Record type signature (e.g. WEAP, NPC_).
    pub signature: Signature,
    /// Classified severity of this conflict.
    pub severity: ConflictSeverity,
    /// Entries from each plugin: (plugin_index, record's raw FormID in that plugin).
    /// Ordered by load order position.
    pub entries: Vec<(usize, FormId)>,
}

/// A difference in a single subrecord between two records.
#[derive(Debug, Clone)]
pub struct SubrecordDiff {
    /// The subrecord signature that differs.
    pub signature: Signature,
    /// Which plugins have differing data for this subrecord.
    /// Each entry is (plugin_index, raw data bytes).
    pub plugin_data: Vec<(usize, Vec<u8>)>,
}

/// Detects conflicts across plugins in a load order.
pub struct ConflictDetector<'a> {
    load_order: &'a LoadOrder,
}

impl<'a> ConflictDetector<'a> {
    /// Create a new conflict detector for the given load order.
    pub fn new(load_order: &'a LoadOrder) -> Self {
        Self { load_order }
    }

    /// Scan the entire load order for records that appear in multiple plugins.
    ///
    /// Returns a conflict entry for every FormID that has overrides, classified
    /// by severity.
    pub fn detect_all_conflicts(&self) -> Vec<RecordConflict> {
        // Build a map: canonical (target_plugin_index, local_id) -> Vec<(plugin_index, record)>
        let mut form_id_map: HashMap<(usize, u32), Vec<(usize, &Record)>> = HashMap::new();

        for (plugin_index, plugin) in self.load_order.plugins.iter().enumerate() {
            for record in plugin.all_records() {
                if let Some((target_plugin, target_local)) =
                    self.load_order.resolve_form_id(plugin_index, record.form_id)
                {
                    let key = (target_plugin, target_local.raw());
                    form_id_map
                        .entry(key)
                        .or_default()
                        .push((plugin_index, record));
                }
            }
        }

        let mut conflicts = Vec::new();
        for ((_target_plugin, _target_local), entries) in &form_id_map {
            if entries.len() < 2 {
                continue;
            }

            let first_record = entries[0].1;
            let severity = self.classify_conflict(entries);

            conflicts.push(RecordConflict {
                form_id: first_record.form_id,
                signature: first_record.signature,
                severity,
                entries: entries
                    .iter()
                    .map(|(idx, rec)| (*idx, rec.form_id))
                    .collect(),
            });
        }

        // Sort by form_id for deterministic output.
        conflicts.sort_by_key(|c| c.form_id.raw());
        conflicts
    }

    /// Find records in the given plugin that are identical to their master version.
    ///
    /// A record is ITM if ALL its subrecords are byte-identical to the master version
    /// and the record flags are the same.
    pub fn detect_itm(&self, plugin_index: usize) -> Vec<FormId> {
        let Some(plugin) = self.load_order.plugins.get(plugin_index) else {
            return Vec::new();
        };

        let mut itm_records = Vec::new();

        for record in plugin.all_records() {
            if let Some((master_plugin_index, master_local_id)) =
                self.load_order.resolve_form_id(plugin_index, record.form_id)
            {
                // Only check if this record refers to a different (master) plugin.
                if master_plugin_index == plugin_index {
                    continue;
                }

                // Find the master record.
                if let Some(master_record) =
                    self.find_record_in_plugin(master_plugin_index, master_local_id)
                {
                    if records_are_identical(record, master_record) {
                        itm_records.push(record.form_id);
                    }
                }
            }
        }

        itm_records
    }

    /// Find records in the given plugin that have the DELETED flag set.
    pub fn detect_deleted_references(&self, plugin_index: usize) -> Vec<FormId> {
        let Some(plugin) = self.load_order.plugins.get(plugin_index) else {
            return Vec::new();
        };

        plugin
            .all_records()
            .into_iter()
            .filter(|record| record.flags.is_deleted())
            .map(|record| record.form_id)
            .collect()
    }

    /// Classify the severity of a conflict given a set of (plugin_index, record) entries.
    ///
    /// - If fewer than 2 entries, there is no real conflict (Benign).
    /// - If all override entries are byte-identical to the first (master), it is ITM.
    /// - If entries differ only in header fields (vc_info, version, unknown), it is Benign.
    /// - If entries differ in subrecord data, it is Critical.
    /// - Otherwise it is Override.
    pub fn classify_conflict(&self, entries: &[(usize, &Record)]) -> ConflictSeverity {
        if entries.len() < 2 {
            return ConflictSeverity::Benign;
        }

        let master_record = entries[0].1;

        // Check if all overrides are identical to the master.
        let all_identical = entries[1..]
            .iter()
            .all(|(_, record)| records_are_identical(record, master_record));

        if all_identical {
            return ConflictSeverity::ITM;
        }

        // Check if differences are only in non-gameplay header fields
        // (vc_info, version, unknown). Subrecords and flags must still match.
        let only_benign_diffs = entries[1..]
            .iter()
            .all(|(_, record)| records_are_gameplay_identical(record, master_record));

        if only_benign_diffs {
            return ConflictSeverity::Benign;
        }

        // Records differ in subrecord data or flags -> Critical.
        let has_subrecord_diffs = entries[1..].iter().any(|(_, record)| {
            !subrecords_are_identical(&record.subrecords, &master_record.subrecords)
        });

        if has_subrecord_diffs {
            return ConflictSeverity::Critical;
        }

        ConflictSeverity::Override
    }

    /// Produce a detailed field-level diff between two records.
    ///
    /// Returns a SubrecordDiff for each subrecord signature where the two records
    /// have different data (or where a subrecord exists in one but not the other).
    pub fn diff_subrecords(
        &self,
        plugin_a_index: usize,
        record_a: &Record,
        plugin_b_index: usize,
        record_b: &Record,
    ) -> Vec<SubrecordDiff> {
        diff_subrecords_impl(plugin_a_index, record_a, plugin_b_index, record_b)
    }

    /// Find a record with the given local FormID in the specified plugin.
    fn find_record_in_plugin(
        &self,
        plugin_index: usize,
        local_form_id: FormId,
    ) -> Option<&'a Record> {
        let plugin = self.load_order.plugins.get(plugin_index)?;
        plugin
            .all_records()
            .into_iter()
            .find(|record| record.form_id.local_id() == local_form_id.raw())
    }
}

/// Check if two records are fully identical (flags, subrecords, and header metadata).
///
/// This is the strict check used for ITM detection: vc_info, version, and unknown
/// must also match.
fn records_are_identical(a: &Record, b: &Record) -> bool {
    a.flags == b.flags
        && a.vc_info == b.vc_info
        && a.version == b.version
        && a.unknown == b.unknown
        && subrecords_are_identical(&a.subrecords, &b.subrecords)
}

/// Check if two records are identical in gameplay-relevant fields only.
///
/// Compares flags and subrecords but ignores vc_info, version, and unknown header fields.
fn records_are_gameplay_identical(a: &Record, b: &Record) -> bool {
    a.flags == b.flags && subrecords_are_identical(&a.subrecords, &b.subrecords)
}

/// Check if two subrecord lists are byte-identical.
fn subrecords_are_identical(a: &[Subrecord], b: &[Subrecord]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(sa, sb)| sa.signature == sb.signature && sa.raw_data == sb.raw_data)
}

/// Build subrecord-level diffs between two records.
fn diff_subrecords_impl(
    plugin_a_index: usize,
    record_a: &Record,
    plugin_b_index: usize,
    record_b: &Record,
) -> Vec<SubrecordDiff> {
    let subs_a = collect_subrecords_by_sig(record_a);
    let subs_b = collect_subrecords_by_sig(record_b);

    let mut diffs = Vec::new();

    // Gather all unique signatures in order of first appearance.
    let mut all_sigs = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for sr in &record_a.subrecords {
        if seen.insert(sr.signature) {
            all_sigs.push(sr.signature);
        }
    }
    for sr in &record_b.subrecords {
        if seen.insert(sr.signature) {
            all_sigs.push(sr.signature);
        }
    }

    for sig in all_sigs {
        let data_a = subs_a.get(&sig);
        let data_b = subs_b.get(&sig);

        match (data_a, data_b) {
            (Some(a_list), Some(b_list)) => {
                if a_list != b_list {
                    let concat_a: Vec<u8> = a_list.iter().flat_map(|d| d.iter().copied()).collect();
                    let concat_b: Vec<u8> = b_list.iter().flat_map(|d| d.iter().copied()).collect();
                    diffs.push(SubrecordDiff {
                        signature: sig,
                        plugin_data: vec![(plugin_a_index, concat_a), (plugin_b_index, concat_b)],
                    });
                }
            }
            (Some(a_list), None) => {
                let concat_a: Vec<u8> = a_list.iter().flat_map(|d| d.iter().copied()).collect();
                diffs.push(SubrecordDiff {
                    signature: sig,
                    plugin_data: vec![
                        (plugin_a_index, concat_a),
                        (plugin_b_index, Vec::new()),
                    ],
                });
            }
            (None, Some(b_list)) => {
                let concat_b: Vec<u8> = b_list.iter().flat_map(|d| d.iter().copied()).collect();
                diffs.push(SubrecordDiff {
                    signature: sig,
                    plugin_data: vec![
                        (plugin_a_index, Vec::new()),
                        (plugin_b_index, concat_b),
                    ],
                });
            }
            (None, None) => unreachable!(),
        }
    }

    diffs
}

/// Collect subrecord data grouped by signature, preserving order of instances.
fn collect_subrecords_by_sig(record: &Record) -> HashMap<Signature, Vec<Vec<u8>>> {
    let mut map: HashMap<Signature, Vec<Vec<u8>>> = HashMap::new();
    for sr in &record.subrecords {
        map.entry(sr.signature)
            .or_default()
            .push(sr.raw_data.clone());
    }
    map
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use xedit_dom::group::{GroupChild, GroupType};
    use xedit_dom::record::RecordFlags;
    use xedit_dom::{FormId, GameId, Group, Plugin, Record, Signature, Subrecord};

    use crate::load_order::LoadOrder;

    use super::*;

    /// Helper to build a record with specific subrecords.
    fn make_record_with_subrecords(
        signature: Signature,
        form_id: u32,
        flags: u32,
        subrecords: Vec<Subrecord>,
    ) -> Record {
        Record {
            signature,
            flags: RecordFlags(flags),
            form_id: FormId::new(form_id),
            vc_info: 0,
            version: 0,
            unknown: 0,
            subrecords,
            raw_header: None,
            raw_compressed_data: None,
            raw_data: None,
            source_offset: None,
            modified: false,
        }
    }

    /// Helper to build a record with an EDID and optional DATA subrecord.
    fn make_record(
        signature: Signature,
        form_id: u32,
        flags: u32,
        edid: Option<&str>,
        data: Option<Vec<u8>>,
    ) -> Record {
        let mut subrecords = Vec::new();
        if let Some(edid) = edid {
            let mut edid_data = edid.as_bytes().to_vec();
            edid_data.push(0);
            subrecords.push(Subrecord::new(Signature::EDID, edid_data));
        }
        if let Some(data) = data {
            subrecords.push(Subrecord::new(Signature::DATA, data));
        }
        make_record_with_subrecords(signature, form_id, flags, subrecords)
    }

    /// Helper to build a plugin with given records in a top-level WEAP group.
    fn make_plugin(
        name: &str,
        header_flags: u32,
        masters: Vec<&str>,
        records: Vec<Record>,
    ) -> Plugin {
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
            file_path: Some(PathBuf::from(name)),
            header: make_record_with_subrecords(Signature::TES4, 0, header_flags, vec![]),
            groups: vec![group],
            tes3_records: Vec::new(),
            masters: masters.into_iter().map(str::to_string).collect(),
            description: None,
            author: None,
            modified: false,
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: Two plugins overriding same record with different data -> Critical
    // -----------------------------------------------------------------------
    #[test]
    fn override_different_data_is_critical() {
        let mut lo = LoadOrder::new(GameId::SkyrimSE);
        lo.add_plugin(make_plugin(
            "Master.esm",
            RecordFlags::ESM,
            vec![],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("IronSword"),
                Some(vec![10, 0, 0, 0]),
            )],
        ));
        lo.add_plugin(make_plugin(
            "Mod.esp",
            0,
            vec!["Master.esm"],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("IronSword"),
                Some(vec![20, 0, 0, 0]), // Different DATA
            )],
        ));
        lo.sort_load_order();

        let detector = ConflictDetector::new(&lo);
        let conflicts = detector.detect_all_conflicts();

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].severity, ConflictSeverity::Critical);
        assert_eq!(conflicts[0].entries.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Test 2: Plugin with record identical to master -> ITM detection
    // -----------------------------------------------------------------------
    #[test]
    fn identical_to_master_detection() {
        let mut lo = LoadOrder::new(GameId::SkyrimSE);
        lo.add_plugin(make_plugin(
            "Master.esm",
            RecordFlags::ESM,
            vec![],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("IronSword"),
                Some(vec![10, 0, 0, 0]),
            )],
        ));
        // Child has identical subrecords and flags.
        lo.add_plugin(make_plugin(
            "Clean.esp",
            0,
            vec!["Master.esm"],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("IronSword"),
                Some(vec![10, 0, 0, 0]),
            )],
        ));
        lo.sort_load_order();

        let detector = ConflictDetector::new(&lo);
        // Plugin index 1 is the child (after sort, ESM comes first).
        let itm = detector.detect_itm(1);

        assert_eq!(itm.len(), 1);
        assert_eq!(itm[0].raw(), 0x0000_1234);
    }

    // -----------------------------------------------------------------------
    // Test 3: Plugin with deleted record -> deleted reference detection
    // -----------------------------------------------------------------------
    #[test]
    fn deleted_reference_detection() {
        let mut lo = LoadOrder::new(GameId::SkyrimSE);
        lo.add_plugin(make_plugin(
            "Master.esm",
            RecordFlags::ESM,
            vec![],
            vec![make_record(
                Signature::REFR,
                0x0000_5678,
                0,
                Some("SomeRef"),
                None,
            )],
        ));
        lo.add_plugin(make_plugin(
            "Deleter.esp",
            0,
            vec!["Master.esm"],
            vec![make_record(
                Signature::REFR,
                0x0000_5678,
                RecordFlags::DELETED,
                None,
                None,
            )],
        ));
        lo.sort_load_order();

        let detector = ConflictDetector::new(&lo);
        let deleted = detector.detect_deleted_references(1);

        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].raw(), 0x0000_5678);
    }

    // -----------------------------------------------------------------------
    // Test 4: Subrecord-level diff between two records
    // -----------------------------------------------------------------------
    #[test]
    fn subrecord_diff_identifies_differences() {
        let record_a = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![10, 0, 0, 0]),
        );
        let record_b = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![20, 0, 0, 0]), // Different DATA
        );

        let lo = LoadOrder::new(GameId::SkyrimSE);
        let detector = ConflictDetector::new(&lo);
        let diffs = detector.diff_subrecords(0, &record_a, 1, &record_b);

        // EDID is the same, only DATA differs.
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].signature, Signature::DATA);
        assert_eq!(diffs[0].plugin_data.len(), 2);
        assert_eq!(diffs[0].plugin_data[0].1, vec![10, 0, 0, 0]);
        assert_eq!(diffs[0].plugin_data[1].1, vec![20, 0, 0, 0]);
    }

    // -----------------------------------------------------------------------
    // Test 5: Multiple plugins with same FormID -> conflict classification
    // -----------------------------------------------------------------------
    #[test]
    fn classify_conflict_multiple_plugins() {
        let mut lo = LoadOrder::new(GameId::SkyrimSE);

        let master_rec = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![10, 0, 0, 0]),
        );
        let override_a = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![20, 0, 0, 0]),
        );
        let override_b = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![10, 0, 0, 0]),
        );

        lo.add_plugin(make_plugin(
            "Master.esm",
            RecordFlags::ESM,
            vec![],
            vec![master_rec.clone()],
        ));
        lo.add_plugin(make_plugin(
            "ModA.esp",
            0,
            vec!["Master.esm"],
            vec![override_a.clone()],
        ));
        lo.add_plugin(make_plugin(
            "ModB.esp",
            0,
            vec!["Master.esm"],
            vec![override_b.clone()],
        ));
        lo.sort_load_order();

        let detector = ConflictDetector::new(&lo);

        // Master + different override -> Critical
        let entries_critical: Vec<(usize, &Record)> = vec![(0, &master_rec), (1, &override_a)];
        assert_eq!(
            detector.classify_conflict(&entries_critical),
            ConflictSeverity::Critical
        );

        // Master + identical override -> ITM
        let entries_itm: Vec<(usize, &Record)> = vec![(0, &master_rec), (2, &override_b)];
        assert_eq!(
            detector.classify_conflict(&entries_itm),
            ConflictSeverity::ITM
        );

        // Master + different + identical -> Critical (not all identical)
        let entries_mixed: Vec<(usize, &Record)> =
            vec![(0, &master_rec), (1, &override_a), (2, &override_b)];
        assert_eq!(
            detector.classify_conflict(&entries_mixed),
            ConflictSeverity::Critical
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: Benign conflict (header-only differences)
    // -----------------------------------------------------------------------
    #[test]
    fn benign_conflict_header_only_diff() {
        let mut record_a = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![10, 0, 0, 0]),
        );
        record_a.vc_info = 0;
        record_a.version = 40;
        record_a.unknown = 0;

        let mut record_b = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![10, 0, 0, 0]),
        );
        // Only header fields differ.
        record_b.vc_info = 12345;
        record_b.version = 44;
        record_b.unknown = 7;

        let lo = LoadOrder::new(GameId::SkyrimSE);
        let detector = ConflictDetector::new(&lo);

        let entries: Vec<(usize, &Record)> = vec![(0, &record_a), (1, &record_b)];
        assert_eq!(
            detector.classify_conflict(&entries),
            ConflictSeverity::Benign
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: Subrecord diff with missing subrecord in one record
    // -----------------------------------------------------------------------
    #[test]
    fn subrecord_diff_missing_subrecord() {
        let record_a = make_record(
            Signature::WEAP,
            0x0000_1234,
            0,
            Some("IronSword"),
            Some(vec![10, 0, 0, 0]),
        );
        let record_b = make_record(Signature::WEAP, 0x0000_1234, 0, Some("IronSword"), None);

        let lo = LoadOrder::new(GameId::SkyrimSE);
        let detector = ConflictDetector::new(&lo);
        let diffs = detector.diff_subrecords(0, &record_a, 1, &record_b);

        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].signature, Signature::DATA);
        assert_eq!(diffs[0].plugin_data[0].1, vec![10, 0, 0, 0]);
        assert!(diffs[0].plugin_data[1].1.is_empty());
    }
}
