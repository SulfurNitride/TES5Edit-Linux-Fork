use std::io::Write;

use byteorder::{LittleEndian, WriteBytesExt};
use xedit_dom::{group::GroupChild, group::GroupType, GameId, Signature};

use crate::{PluginReader, PluginWriter};

fn write_subrecord(buf: &mut Vec<u8>, sig: [u8; 4], data: &[u8]) {
    buf.extend_from_slice(&sig);
    buf.write_u16::<LittleEndian>(data.len() as u16).unwrap();
    buf.extend_from_slice(data);
}

fn write_record(
    buf: &mut Vec<u8>,
    sig: [u8; 4],
    flags: u32,
    form_id: u32,
    vc_info: u32,
    version: u16,
    unknown: u16,
    body: &[u8],
) {
    buf.extend_from_slice(&sig);
    buf.write_u32::<LittleEndian>(body.len() as u32).unwrap();
    buf.write_u32::<LittleEndian>(flags).unwrap();
    buf.write_u32::<LittleEndian>(form_id).unwrap();
    buf.write_u32::<LittleEndian>(vc_info).unwrap();
    buf.write_u16::<LittleEndian>(version).unwrap();
    buf.write_u16::<LittleEndian>(unknown).unwrap();
    buf.extend_from_slice(body);
}

fn write_group(
    buf: &mut Vec<u8>,
    label: [u8; 4],
    group_type: u32,
    stamp: u32,
    unknown: u32,
    children: &[u8],
) {
    buf.extend_from_slice(b"GRUP");
    buf.write_u32::<LittleEndian>((24 + children.len()) as u32)
        .unwrap();
    buf.extend_from_slice(&label);
    buf.write_u32::<LittleEndian>(group_type).unwrap();
    buf.write_u32::<LittleEndian>(stamp).unwrap();
    buf.write_u32::<LittleEndian>(unknown).unwrap();
    buf.extend_from_slice(children);
}

fn build_minimal_sse_plugin() -> Vec<u8> {
    let mut tes4_body = Vec::new();

    let mut hedr_data = Vec::new();
    hedr_data.write_f32::<LittleEndian>(1.7).unwrap();
    hedr_data.write_i32::<LittleEndian>(1).unwrap();
    hedr_data.write_u32::<LittleEndian>(0x0000_0800).unwrap();
    write_subrecord(&mut tes4_body, *b"HEDR", &hedr_data);

    write_subrecord(&mut tes4_body, *b"MAST", b"Skyrim.esm\0");

    let mut mast_data = Vec::new();
    mast_data.write_u64::<LittleEndian>(0).unwrap();
    write_subrecord(&mut tes4_body, *b"DATA", &mast_data);

    let mut gmst_body = Vec::new();
    write_subrecord(&mut gmst_body, *b"EDID", b"sTestSetting\0");

    let mut gmst_data = Vec::new();
    gmst_data.write_f32::<LittleEndian>(1.0).unwrap();
    write_subrecord(&mut gmst_body, *b"DATA", &gmst_data);

    let mut gmst_record = Vec::new();
    write_record(
        &mut gmst_record,
        *b"GMST",
        0,
        0x0000_0800,
        0,
        0,
        0,
        &gmst_body,
    );

    let mut top_group = Vec::new();
    write_group(&mut top_group, *b"GMST", 0, 0, 0, &gmst_record);

    let mut out = Vec::new();
    write_record(&mut out, *b"TES4", 0, 0, 0, 0, 0, &tes4_body);
    out.write_all(&top_group).unwrap();
    out
}

fn find_unique_subslice(haystack: &[u8], needle: &[u8]) -> usize {
    let mut matches = haystack
        .windows(needle.len())
        .enumerate()
        .filter_map(|(idx, window)| (window == needle).then_some(idx));
    let first = matches.next().expect("needle should exist");
    assert!(
        matches.next().is_none(),
        "needle should have exactly one occurrence"
    );
    first
}

#[test]
fn minimal_sse_plugin_roundtrip_is_byte_identical() {
    let input = build_minimal_sse_plugin();
    let reader = PluginReader::new(GameId::SkyrimSE);
    let plugin = reader.read_bytes(&input, None).expect("plugin should parse");

    assert_eq!(plugin.header.signature, Signature::TES4);
    assert_eq!(plugin.groups.len(), 1);
    assert_eq!(plugin.masters, vec!["Skyrim.esm".to_string()]);
    assert_eq!(plugin.all_records().len(), 1);

    let hedr = plugin
        .header
        .subrecords_by_sig(Signature::from_bytes(b"HEDR"))
        .next()
        .expect("TES4 should contain HEDR");
    assert_eq!(hedr.raw_data.len(), 12);
    let num_records =
        i32::from_le_bytes([hedr.raw_data[4], hedr.raw_data[5], hedr.raw_data[6], hedr.raw_data[7]]);
    assert_eq!(num_records, 1);

    let group = &plugin.groups[0];
    assert_eq!(group.group_type, GroupType::Top(Signature::GMST));
    let gmst = match &group.children[0] {
        GroupChild::Record(record) => record,
        GroupChild::Group(_) => panic!("expected GMST record"),
    };
    assert_eq!(gmst.signature, Signature::GMST);

    let output = PluginWriter::write_bytes(&plugin).expect("plugin should serialize");
    assert_eq!(output, input);
}

#[test]
fn modifying_a_subrecord_changes_only_payload_bytes() {
    let input = build_minimal_sse_plugin();
    let old_payload = [0x00, 0x00, 0x80, 0x3F];
    let new_payload = [0x00, 0x00, 0x00, 0x40];
    let old_offset = find_unique_subslice(&input, &old_payload);

    let reader = PluginReader::new(GameId::SkyrimSE);
    let mut plugin = reader.read_bytes(&input, None).expect("plugin should parse");

    let group = plugin.groups.get_mut(0).expect("top group should exist");
    let record = match group.children.get_mut(0).expect("GMST child should exist") {
        GroupChild::Record(record) => record,
        GroupChild::Group(_) => panic!("expected GMST record"),
    };
    let data_subrecord = record
        .subrecords
        .iter_mut()
        .find(|sr| sr.signature == Signature::DATA)
        .expect("GMST should contain DATA");
    data_subrecord.raw_data = new_payload.to_vec();
    data_subrecord.modified = true;

    let output = PluginWriter::write_bytes(&plugin).expect("plugin should serialize");
    assert_eq!(output.len(), input.len());

    let diffs: Vec<usize> = input
        .iter()
        .zip(output.iter())
        .enumerate()
        .filter_map(|(idx, (lhs, rhs))| (lhs != rhs).then_some(idx))
        .collect();
    let expected: Vec<usize> = old_payload
        .iter()
        .zip(new_payload.iter())
        .enumerate()
        .filter_map(|(idx, (old, new))| (old != new).then_some(old_offset + idx))
        .collect();
    assert_eq!(diffs, expected);
    assert_eq!(&output[old_offset..old_offset + 4], &new_payload);
}

// ---------------------------------------------------------------------------
// Helpers for multi-record synthetic plugins
// ---------------------------------------------------------------------------

/// Build a plugin with multiple GMST records inside one top-level GMST group.
/// Each record has a unique EDID and a DATA float value.
/// Returns (raw_bytes, form_ids) where form_ids[i] is the FormID of the i-th GMST.
fn build_multi_record_sse_plugin(count: usize) -> (Vec<u8>, Vec<u32>) {
    // TES4 header body
    let mut tes4_body = Vec::new();
    let mut hedr_data = Vec::new();
    hedr_data.write_f32::<LittleEndian>(1.7).unwrap();
    hedr_data.write_i32::<LittleEndian>(count as i32).unwrap();
    hedr_data.write_u32::<LittleEndian>(0x0000_0800).unwrap();
    write_subrecord(&mut tes4_body, *b"HEDR", &hedr_data);
    write_subrecord(&mut tes4_body, *b"MAST", b"Skyrim.esm\0");
    let mut mast_data = Vec::new();
    mast_data.write_u64::<LittleEndian>(0).unwrap();
    write_subrecord(&mut tes4_body, *b"DATA", &mast_data);

    let mut all_records = Vec::new();
    let mut form_ids = Vec::new();
    for i in 0..count {
        let form_id = 0x0000_0800 + i as u32;
        form_ids.push(form_id);

        let edid = format!("sTestSetting{}\0", i);
        let mut rec_body = Vec::new();
        write_subrecord(&mut rec_body, *b"EDID", edid.as_bytes());
        let mut data_val = Vec::new();
        data_val
            .write_f32::<LittleEndian>((i + 1) as f32)
            .unwrap();
        write_subrecord(&mut rec_body, *b"DATA", &data_val);

        write_record(
            &mut all_records,
            *b"GMST",
            0,
            form_id,
            0,
            0,
            0,
            &rec_body,
        );
    }

    let mut top_group = Vec::new();
    write_group(&mut top_group, *b"GMST", 0, 0, 0, &all_records);

    let mut out = Vec::new();
    write_record(&mut out, *b"TES4", 0, 0, 0, 0, 0, &tes4_body);
    out.write_all(&top_group).unwrap();
    (out, form_ids)
}

/// Serialize a single record to bytes (for byte-level comparison).
fn serialize_record_bytes(record: &xedit_dom::Record) -> Vec<u8> {
    // Build a throwaway single-record plugin and extract just the record bytes.
    // Simpler: just use the writer internals by writing through a plugin wrapper.
    // Actually, the simplest approach: re-read the record from the writer output
    // by noting offsets. Instead, let's manually serialize.
    let mut buf = Vec::new();
    if let Some(ref raw_header) = record.raw_header {
        buf.extend_from_slice(raw_header);
        if let Some(ref raw_data) = record.raw_data {
            buf.extend_from_slice(raw_data);
        }
    }
    buf
}

/// Extract byte ranges for each record child in a group from the raw plugin bytes.
/// Returns Vec<(start, end)> offsets into `plugin_bytes` for each record in the group.
fn extract_record_byte_ranges(
    _plugin_bytes: &[u8],
    plugin: &xedit_dom::Plugin,
    group_idx: usize,
) -> Vec<(usize, usize)> {
    let group = &plugin.groups[group_idx];
    let mut ranges = Vec::new();
    for child in &group.children {
        if let GroupChild::Record(r) = child {
            if let Some(offset) = r.source_offset {
                let header_len = r.raw_header.as_ref().map_or(24, |h| h.len());
                let data_len = r.raw_data.as_ref().map_or(0, |d| d.len());
                ranges.push((offset as usize, offset as usize + header_len + data_len));
            }
        }
    }
    ranges
}

// ---------------------------------------------------------------------------
// Test 1: Add subrecord to a record
// ---------------------------------------------------------------------------
#[test]
fn add_subrecord_changes_only_target_record() {
    let (input, _form_ids) = build_multi_record_sse_plugin(3);
    let reader = PluginReader::new(GameId::SkyrimSE);
    let mut plugin = reader.read_bytes(&input, None).expect("parse");

    // Grab byte ranges of all records BEFORE modification
    let original_ranges = extract_record_byte_ranges(&input, &plugin, 0);
    assert_eq!(original_ranges.len(), 3);

    // Modify record at index 1: add a new FULL subrecord
    let group = plugin.groups.get_mut(0).unwrap();
    let record = match group.children.get_mut(1).unwrap() {
        GroupChild::Record(r) => r,
        _ => panic!("expected record"),
    };
    let new_sr = xedit_dom::Subrecord::new(
        Signature::FULL,
        b"Test Name\0".to_vec(),
    );
    record.subrecords.push(new_sr);
    record.modified = true;
    // Clear raw data so writer re-serializes
    record.raw_data = None;
    record.raw_header = None;

    let output = PluginWriter::write_bytes(&plugin).expect("serialize");

    // Re-parse both
    let orig_plugin = reader.read_bytes(&input, None).unwrap();
    let new_plugin = reader.read_bytes(&output, None).unwrap();

    // Records 0 and 2 must be byte-identical to originals
    let orig_records = orig_plugin.all_records();
    let new_records = new_plugin.all_records();
    assert_eq!(new_records.len(), 3);

    for idx in [0, 2] {
        let orig_bytes = serialize_record_bytes(orig_records[idx]);
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_bytes, new_bytes,
            "Record at index {} should be byte-identical after modifying record 1",
            idx
        );
    }

    // Record 1 should now have 3 subrecords (EDID, DATA, FULL)
    assert_eq!(new_records[1].subrecords.len(), 3);
    let full_sr = new_records[1]
        .subrecords
        .iter()
        .find(|sr| sr.signature == Signature::FULL)
        .expect("modified record should have FULL subrecord");
    assert_eq!(full_sr.raw_data, b"Test Name\0");
}

// ---------------------------------------------------------------------------
// Test 2: Modify subrecord data
// ---------------------------------------------------------------------------
#[test]
fn modify_subrecord_data_changes_only_target_record() {
    let (input, _form_ids) = build_multi_record_sse_plugin(4);
    let reader = PluginReader::new(GameId::SkyrimSE);
    let mut plugin = reader.read_bytes(&input, None).expect("parse");

    // Modify record 2's DATA subrecord value
    let group = plugin.groups.get_mut(0).unwrap();
    let record = match group.children.get_mut(2).unwrap() {
        GroupChild::Record(r) => r,
        _ => panic!("expected record"),
    };
    let data_sr = record
        .subrecords
        .iter_mut()
        .find(|sr| sr.signature == Signature::DATA)
        .unwrap();
    // Change from 3.0f to 999.0f
    let mut new_val = Vec::new();
    new_val.write_f32::<LittleEndian>(999.0).unwrap();
    data_sr.raw_data = new_val;
    data_sr.modified = true;

    let output = PluginWriter::write_bytes(&plugin).expect("serialize");

    // Output should be same length (same-size subrecord replacement)
    assert_eq!(output.len(), input.len());

    // Re-parse both
    let orig_plugin = reader.read_bytes(&input, None).unwrap();
    let new_plugin = reader.read_bytes(&output, None).unwrap();
    let orig_records = orig_plugin.all_records();
    let new_records = new_plugin.all_records();

    // Records 0, 1, 3 should be byte-identical
    for idx in [0, 1, 3] {
        let orig_bytes = serialize_record_bytes(orig_records[idx]);
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_bytes, new_bytes,
            "Record at index {} should be byte-identical",
            idx
        );
    }

    // Record 2 should have the new value
    let modified_data = new_records[2]
        .subrecords
        .iter()
        .find(|sr| sr.signature == Signature::DATA)
        .unwrap();
    let val = f32::from_le_bytes([
        modified_data.raw_data[0],
        modified_data.raw_data[1],
        modified_data.raw_data[2],
        modified_data.raw_data[3],
    ]);
    assert!((val - 999.0).abs() < f32::EPSILON);

    // Header should be byte-identical
    let orig_header = serialize_record_bytes(&orig_plugin.header);
    let new_header = serialize_record_bytes(&new_plugin.header);
    assert_eq!(orig_header, new_header);
}

// ---------------------------------------------------------------------------
// Test 3: Delete subrecord from a record
// ---------------------------------------------------------------------------
#[test]
fn delete_subrecord_shrinks_only_target_record() {
    let (input, _form_ids) = build_multi_record_sse_plugin(3);
    let reader = PluginReader::new(GameId::SkyrimSE);
    let mut plugin = reader.read_bytes(&input, None).expect("parse");

    // Record 0 has EDID + DATA. Remove the DATA subrecord.
    let group = plugin.groups.get_mut(0).unwrap();
    let record = match group.children.get_mut(0).unwrap() {
        GroupChild::Record(r) => r,
        _ => panic!("expected record"),
    };
    let orig_subrecord_count = record.subrecords.len();
    assert_eq!(orig_subrecord_count, 2, "should have EDID + DATA");

    // Remove DATA
    record.subrecords.retain(|sr| sr.signature != Signature::DATA);
    assert_eq!(record.subrecords.len(), 1);
    record.modified = true;
    record.raw_data = None;
    record.raw_header = None;

    let output = PluginWriter::write_bytes(&plugin).expect("serialize");

    // Output should be shorter (removed 4 bytes of DATA + 6 bytes header = 10 bytes)
    assert_eq!(output.len(), input.len() - 10);

    // Re-parse both
    let orig_plugin = reader.read_bytes(&input, None).unwrap();
    let new_plugin = reader.read_bytes(&output, None).unwrap();
    let orig_records = orig_plugin.all_records();
    let new_records = new_plugin.all_records();

    // Record 0 should have only 1 subrecord now
    assert_eq!(new_records[0].subrecords.len(), 1);
    assert_eq!(new_records[0].subrecords[0].signature, Signature::EDID);

    // Records 1 and 2 should be byte-identical
    for idx in [1, 2] {
        let orig_bytes = serialize_record_bytes(orig_records[idx]);
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_bytes, new_bytes,
            "Record at index {} should be byte-identical after deleting subrecord from record 0",
            idx
        );
    }
}

// ---------------------------------------------------------------------------
// Test 4: Add record to a group (group header size updates)
// ---------------------------------------------------------------------------
#[test]
fn add_record_to_group_updates_group_size_preserves_existing() {
    let (input, _form_ids) = build_multi_record_sse_plugin(2);
    let reader = PluginReader::new(GameId::SkyrimSE);
    let mut plugin = reader.read_bytes(&input, None).expect("parse");

    let orig_records = {
        let p = reader.read_bytes(&input, None).unwrap();
        p.all_records()
            .iter()
            .map(|r| serialize_record_bytes(r))
            .collect::<Vec<_>>()
    };

    // Build a new GMST record to add
    let new_record = xedit_dom::Record {
        signature: Signature::GMST,
        flags: xedit_dom::record::RecordFlags::NONE,
        form_id: xedit_dom::FormId::new(0x0000_0900),
        vc_info: 0,
        version: 0,
        unknown: 0,
        subrecords: vec![
            xedit_dom::Subrecord::new(Signature::EDID, b"sNewSetting\0".to_vec()),
            xedit_dom::Subrecord::new(Signature::DATA, {
                let mut v = Vec::new();
                v.write_f32::<LittleEndian>(42.0).unwrap();
                v
            }),
        ],
        raw_header: None,
        raw_compressed_data: None,
        raw_data: None,
        source_offset: None,
        modified: true,
    };

    plugin.groups[0]
        .children
        .push(GroupChild::Record(new_record));

    let output = PluginWriter::write_bytes(&plugin).expect("serialize");

    // Re-parse
    let new_plugin = reader.read_bytes(&output, None).unwrap();
    let new_records = new_plugin.all_records();
    assert_eq!(new_records.len(), 3, "should now have 3 records");

    // Existing records (indices 0, 1) should be byte-identical
    for idx in 0..2 {
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_records[idx], new_bytes,
            "Existing record {} should be byte-identical after adding new record",
            idx
        );
    }

    // The group size in the output should be larger than in the input
    // Group header starts right after TES4 header record
    let orig_group_size = u32::from_le_bytes([
        input[input.len() - orig_records.iter().map(|r| r.len()).sum::<usize>() - 24 + 4],
        input[input.len() - orig_records.iter().map(|r| r.len()).sum::<usize>() - 24 + 5],
        input[input.len() - orig_records.iter().map(|r| r.len()).sum::<usize>() - 24 + 6],
        input[input.len() - orig_records.iter().map(|r| r.len()).sum::<usize>() - 24 + 7],
    ]);
    // Find GRUP in output
    let grup_offset_out = output
        .windows(4)
        .position(|w| w == b"GRUP")
        .expect("should find GRUP");
    let new_group_size = u32::from_le_bytes([
        output[grup_offset_out + 4],
        output[grup_offset_out + 5],
        output[grup_offset_out + 6],
        output[grup_offset_out + 7],
    ]);
    assert!(
        new_group_size > orig_group_size,
        "Group size should increase: {} > {}",
        new_group_size,
        orig_group_size
    );

    // New record should be the added one
    assert_eq!(new_records[2].form_id.raw(), 0x0000_0900);
    assert_eq!(
        new_records[2].editor_id(),
        Some("sNewSetting"),
    );
}

// ---------------------------------------------------------------------------
// Test 5: Multi-record isolation (modify index 2, verify 0, 1, 3 untouched)
// ---------------------------------------------------------------------------
#[test]
fn multi_record_isolation_modify_index2_others_identical() {
    let (input, _form_ids) = build_multi_record_sse_plugin(4);
    let reader = PluginReader::new(GameId::SkyrimSE);

    // Save original record bytes
    let orig_plugin = reader.read_bytes(&input, None).unwrap();
    let orig_record_bytes: Vec<Vec<u8>> = orig_plugin
        .all_records()
        .iter()
        .map(|r| serialize_record_bytes(r))
        .collect();
    assert_eq!(orig_record_bytes.len(), 4);

    // Load mutable copy and modify only record at index 2
    let mut plugin = reader.read_bytes(&input, None).unwrap();
    let group = plugin.groups.get_mut(0).unwrap();
    let record = match group.children.get_mut(2).unwrap() {
        GroupChild::Record(r) => r,
        _ => panic!("expected record"),
    };

    // Change EDID
    let edid_sr = record
        .subrecords
        .iter_mut()
        .find(|sr| sr.signature == Signature::EDID)
        .unwrap();
    edid_sr.raw_data = b"sModifiedSetting\0".to_vec();
    edid_sr.modified = true;

    // Change DATA
    let data_sr = record
        .subrecords
        .iter_mut()
        .find(|sr| sr.signature == Signature::DATA)
        .unwrap();
    let mut new_val = Vec::new();
    new_val.write_f32::<LittleEndian>(12345.0).unwrap();
    data_sr.raw_data = new_val;
    data_sr.modified = true;

    let output = PluginWriter::write_bytes(&plugin).expect("serialize");

    // Re-parse
    let new_plugin = reader.read_bytes(&output, None).unwrap();
    let new_records = new_plugin.all_records();
    assert_eq!(new_records.len(), 4);

    // Records 0, 1, 3 must be byte-identical
    for idx in [0usize, 1, 3] {
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_record_bytes[idx], new_bytes,
            "Record at index {} must be byte-identical when only record 2 was modified",
            idx
        );
    }

    // Record 2 should have the new EDID
    assert_eq!(new_records[2].editor_id(), Some("sModifiedSetting"));

    // Record 2's DATA should be 12345.0
    let data_sr = new_records[2]
        .subrecords
        .iter()
        .find(|sr| sr.signature == Signature::DATA)
        .unwrap();
    let val = f32::from_le_bytes([
        data_sr.raw_data[0],
        data_sr.raw_data[1],
        data_sr.raw_data[2],
        data_sr.raw_data[3],
    ]);
    assert!((val - 12345.0).abs() < f32::EPSILON);

    // TES4 header must be byte-identical
    let orig_header = serialize_record_bytes(&orig_plugin.header);
    let new_header = serialize_record_bytes(&new_plugin.header);
    assert_eq!(orig_header, new_header, "TES4 header must be preserved");
}

// ---------------------------------------------------------------------------
// Real-file tests (ignored by default, require game data on disk)
// ---------------------------------------------------------------------------

/// Helper: load a real plugin, modify one record, save, and verify all other
/// records are byte-identical.
fn real_file_surgical_patch_test(
    game_id: GameId,
    path: &std::path::Path,
    record_index: usize,
) {
    let reader = PluginReader::new(game_id);
    let orig_plugin = reader.read_file(path).expect("should load plugin");
    let orig_records: Vec<Vec<u8>> = orig_plugin
        .all_records()
        .iter()
        .map(|r| serialize_record_bytes(r))
        .collect();
    let total = orig_records.len();
    assert!(
        record_index < total,
        "record_index {} out of range ({})",
        record_index,
        total
    );

    // Load mutable copy
    let mut plugin = reader.read_file(path).expect("should load plugin");

    // Find and modify the target record
    // We need to walk groups to find the record at the flat index
    let mut flat_idx = 0usize;
    let mut found = false;
    'outer: for group in &mut plugin.groups {
        for child in &mut group.children {
            if let GroupChild::Record(r) = child {
                if flat_idx == record_index {
                    // Modify: change EDID if present, otherwise add a harmless subrecord
                    if let Some(edid_sr) = r.subrecords.iter_mut().find(|sr| sr.signature == Signature::EDID) {
                        // Append an 'X' before the null terminator
                        if let Some(null_pos) = edid_sr.raw_data.iter().position(|&b| b == 0) {
                            edid_sr.raw_data.insert(null_pos, b'X');
                        } else {
                            edid_sr.raw_data.push(b'X');
                        }
                        edid_sr.modified = true;
                    } else {
                        // Add a new subrecord
                        r.subrecords.push(xedit_dom::Subrecord::new(
                            Signature::EDID,
                            b"xPatched\0".to_vec(),
                        ));
                    }
                    r.modified = true;
                    r.raw_data = None;
                    r.raw_header = None;
                    found = true;
                    break 'outer;
                }
                flat_idx += 1;
            }
        }
    }
    assert!(found, "should have found record at index {}", record_index);

    let output = PluginWriter::write_bytes(&plugin).expect("should serialize");
    let new_plugin = reader
        .read_bytes(&output, None)
        .expect("output should re-parse");
    let new_records = new_plugin.all_records();
    assert_eq!(new_records.len(), total);

    // Verify all non-target records are byte-identical
    for idx in 0..total {
        if idx == record_index {
            continue;
        }
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_records[idx], new_bytes,
            "Record {} should be byte-identical (total={}, modified={})",
            idx, total, record_index
        );
    }
}

#[test]
fn real_sse_update_esm_surgical_patch() {
    let path = std::path::Path::new(
        "/home/luke/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data/Update.esm",
    );
    if !path.exists() {
        eprintln!("Skipping: {} not found", path.display());
        return;
    }
    real_file_surgical_patch_test(GameId::SkyrimSE, path, 2);
}

#[test]
fn real_sse_dawnguard_surgical_patch() {
    let path = std::path::Path::new(
        "/home/luke/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data/Dawnguard.esm",
    );
    if !path.exists() {
        eprintln!("Skipping: {} not found", path.display());
        return;
    }
    real_file_surgical_patch_test(GameId::SkyrimSE, path, 5);
}

/// Oblivion uses 20-byte record headers, but the writer's re-serialize path
/// currently always emits 24-byte headers. This test verifies that unmodified
/// Oblivion plugins roundtrip correctly (no record is touched), which exercises
/// the lossless raw-byte path.
#[test]
fn real_oblivion_roundtrip_unmodified() {
    let path = std::path::Path::new(
        "/home/luke/.local/share/Steam/steamapps/common/Oblivion/Data/Oblivion.esm",
    );
    if !path.exists() {
        eprintln!("Skipping: {} not found", path.display());
        return;
    }
    let reader = PluginReader::new(GameId::Oblivion);
    let plugin = reader.read_file(path).expect("should load Oblivion.esm");
    let output = PluginWriter::write_bytes(&plugin).expect("should serialize");
    let input = std::fs::read(path).expect("read file");
    assert_eq!(
        output.len(),
        input.len(),
        "Unmodified Oblivion.esm roundtrip must preserve file size"
    );
    assert_eq!(output, input, "Unmodified Oblivion.esm roundtrip must be byte-identical");
}

#[test]
fn real_sse_modify_subrecord_data_isolation() {
    let path = std::path::Path::new(
        "/home/luke/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data/Update.esm",
    );
    if !path.exists() {
        eprintln!("Skipping: {} not found", path.display());
        return;
    }
    let reader = PluginReader::new(GameId::SkyrimSE);
    let orig_plugin = reader.read_file(path).expect("load");
    let orig_records: Vec<Vec<u8>> = orig_plugin
        .all_records()
        .iter()
        .map(|r| serialize_record_bytes(r))
        .collect();
    let total = orig_records.len();
    assert!(total >= 4, "Update.esm should have plenty of records");

    // Load mutable and modify record 2's DATA subrecord in-place (same size)
    let mut plugin = reader.read_file(path).expect("load");
    let mut flat_idx = 0usize;
    'outer: for group in &mut plugin.groups {
        for child in &mut group.children {
            if let GroupChild::Record(r) = child {
                if flat_idx == 2 {
                    if let Some(data_sr) = r.subrecords.iter_mut().find(|sr| sr.signature == Signature::DATA) {
                        // Flip all bits
                        for b in data_sr.raw_data.iter_mut() {
                            *b = !*b;
                        }
                        data_sr.modified = true;
                    }
                    break 'outer;
                }
                flat_idx += 1;
            }
        }
    }

    let output = PluginWriter::write_bytes(&plugin).expect("serialize");
    let new_plugin = reader.read_bytes(&output, None).expect("re-parse");
    let new_records = new_plugin.all_records();

    for idx in 0..total {
        if idx == 2 {
            continue;
        }
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_records[idx], new_bytes,
            "Record {} must be byte-identical when only record 2 was modified",
            idx,
        );
    }
}

#[test]
fn real_sse_delete_subrecord_isolation() {
    let path = std::path::Path::new(
        "/home/luke/.local/share/Steam/steamapps/common/Skyrim Special Edition/Data/Update.esm",
    );
    if !path.exists() {
        eprintln!("Skipping: {} not found", path.display());
        return;
    }
    let reader = PluginReader::new(GameId::SkyrimSE);
    let orig_plugin = reader.read_file(path).expect("load");
    let orig_records: Vec<Vec<u8>> = orig_plugin
        .all_records()
        .iter()
        .map(|r| serialize_record_bytes(r))
        .collect();
    let total = orig_records.len();

    // Find a record with at least 2 subrecords and delete one
    let mut plugin = reader.read_file(path).expect("load");
    let mut flat_idx = 0usize;
    let mut target_idx = None;
    'outer: for group in &mut plugin.groups {
        for child in &mut group.children {
            if let GroupChild::Record(r) = child {
                if r.subrecords.len() >= 2 {
                    // Remove the last subrecord
                    r.subrecords.pop();
                    r.modified = true;
                    r.raw_data = None;
                    r.raw_header = None;
                    target_idx = Some(flat_idx);
                    break 'outer;
                }
                flat_idx += 1;
            }
        }
    }
    let target_idx = target_idx.expect("no record with >=2 subrecords");

    let output = PluginWriter::write_bytes(&plugin).expect("serialize");
    let new_plugin = reader.read_bytes(&output, None).expect("re-parse");
    let new_records = new_plugin.all_records();
    assert_eq!(new_records.len(), total);

    for idx in 0..total {
        if idx == target_idx {
            continue;
        }
        let new_bytes = serialize_record_bytes(new_records[idx]);
        assert_eq!(
            orig_records[idx], new_bytes,
            "Record {} must be byte-identical when only record {} was modified",
            idx, target_idx,
        );
    }
}
