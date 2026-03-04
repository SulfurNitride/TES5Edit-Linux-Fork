//! Scan REFR records for LOD-relevant references in a worldspace.

use anyhow::Result;
use std::collections::HashMap;
use xedit_dom::group::GroupChild;
use xedit_dom::{Group, Plugin, Record, Signature};

/// A reference placement in the worldspace.
#[derive(Debug, Clone)]
pub struct LodReference {
    pub form_id: u32,
    pub base_form_id: u32,
    pub base_signature: Signature,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
    pub plugin_index: usize,
    /// Form ID of the parent worldspace (from WorldChildren group type)
    pub worldspace_form_id: u32,
}

/// A base object with LOD data.
#[derive(Debug, Clone)]
pub struct LodBase {
    pub form_id: u32,
    pub signature: Signature,
    pub editor_id: String,
    /// LOD model paths [lod0, lod1, lod2, lod3] from MNAM subrecords
    pub lod_models: Vec<Option<String>>,
    /// Full model path from MODL subrecord
    pub full_model: Option<String>,
}

/// Scan plugins for REFR records that reference STAT/TREE bases with LOD data.
pub fn scan_references(
    plugins: &[&Plugin],
    _worldspace_editor_id: &str,
) -> Result<(Vec<LodReference>, HashMap<u32, LodBase>)> {
    let mut all_refs = Vec::new();
    let mut all_bases: HashMap<u32, LodBase> = HashMap::new();

    // First pass: collect all STAT and TREE records with LOD models
    for plugin in plugins.iter() {
        for group in &plugin.groups {
            let group_sig = match group.group_type {
                xedit_dom::group::GroupType::Top(sig) => sig,
                _ => continue,
            };
            if group_sig != Signature::STAT && group_sig != Signature::from_bytes(b"TREE") {
                continue;
            }
            let records = collect_all_records(group);
            for record in records {
                let form_id = record.form_id.raw();
                let mut lod_models = vec![None; 4];
                let mut full_model = None;
                let mut editor_id = String::new();

                for sr in &record.subrecords {
                    if sr.signature == Signature::EDID {
                        let len = sr
                            .raw_data
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(sr.raw_data.len());
                        editor_id =
                            String::from_utf8_lossy(&sr.raw_data[..len]).to_string();
                    } else if sr.signature == Signature::MODL {
                        let len = sr
                            .raw_data
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(sr.raw_data.len());
                        full_model =
                            Some(String::from_utf8_lossy(&sr.raw_data[..len]).to_string());
                    } else if sr.signature == Signature::from_bytes(b"MNAM") {
                        if let Some(models) = parse_mnam(&sr.raw_data) {
                            lod_models = models;
                        }
                    }
                }

                if lod_models.iter().any(|m| m.is_some()) || full_model.is_some() {
                    all_bases.insert(
                        form_id,
                        LodBase {
                            form_id,
                            signature: record.signature,
                            editor_id,
                            lod_models,
                            full_model,
                        },
                    );
                }
            }
        }
    }

    // Second pass: collect REFR records in the target worldspace
    for (plugin_idx, plugin) in plugins.iter().enumerate() {
        for group in &plugin.groups {
            let group_sig = match group.group_type {
                xedit_dom::group::GroupType::Top(sig) => sig,
                _ => continue,
            };
            if group_sig != Signature::WRLD {
                continue;
            }
            let records = collect_all_records(group);
            for record in records {
                if record.signature == Signature::REFR {
                    if let Some(lod_ref) = parse_refr(record, plugin_idx) {
                        if all_bases.contains_key(&lod_ref.base_form_id) {
                            all_refs.push(lod_ref);
                        }
                    }
                }
            }
        }
    }

    Ok((all_refs, all_bases))
}

/// Recursively collect all records from a group and its nested groups.
fn collect_all_records(group: &Group) -> Vec<&Record> {
    let mut out = Vec::new();
    for child in &group.children {
        match child {
            GroupChild::Record(r) => out.push(r),
            GroupChild::Group(g) => out.extend(collect_all_records(g)),
        }
    }
    out
}

fn parse_refr(record: &Record, plugin_index: usize) -> Option<LodReference> {
    let mut base_form_id = 0u32;
    let mut position = [0.0f32; 3];
    let mut rotation = [0.0f32; 3];
    let mut scale = 1.0f32;

    for sr in &record.subrecords {
        if sr.signature == Signature::NAME && sr.raw_data.len() >= 4 {
            base_form_id = u32::from_le_bytes([
                sr.raw_data[0],
                sr.raw_data[1],
                sr.raw_data[2],
                sr.raw_data[3],
            ]);
        } else if sr.signature == Signature::DATA && sr.raw_data.len() >= 24 {
            use byteorder::{LittleEndian, ReadBytesExt};
            use std::io::Cursor;
            let mut c = Cursor::new(&sr.raw_data);
            position[0] = c.read_f32::<LittleEndian>().ok()?;
            position[1] = c.read_f32::<LittleEndian>().ok()?;
            position[2] = c.read_f32::<LittleEndian>().ok()?;
            rotation[0] = c.read_f32::<LittleEndian>().ok()?;
            rotation[1] = c.read_f32::<LittleEndian>().ok()?;
            rotation[2] = c.read_f32::<LittleEndian>().ok()?;
        } else if sr.signature == Signature::from_bytes(b"XSCL") && sr.raw_data.len() >= 4 {
            scale = f32::from_le_bytes([
                sr.raw_data[0],
                sr.raw_data[1],
                sr.raw_data[2],
                sr.raw_data[3],
            ]);
        }
    }

    if base_form_id == 0 {
        return None;
    }

    Some(LodReference {
        form_id: record.form_id.raw(),
        base_form_id,
        base_signature: Signature::STAT, // will be corrected during base lookup
        position,
        rotation,
        scale,
        plugin_index,
        worldspace_form_id: 0, // set by FFI scanner from WorldChildren group type
    })
}

fn parse_mnam(data: &[u8]) -> Option<Vec<Option<String>>> {
    // MNAM contains up to 4 LOD model paths as null-terminated strings
    let mut models = vec![None; 4];
    let mut offset = 0;
    let mut level = 0;

    while offset < data.len() && level < 4 {
        // Find null terminator
        let end = data[offset..]
            .iter()
            .position(|&b| b == 0)
            .map(|pos| offset + pos)
            .unwrap_or(data.len());

        if end > offset {
            let path = String::from_utf8_lossy(&data[offset..end]).to_string();
            if !path.is_empty() {
                models[level] = Some(path);
            }
        }
        offset = end + 1;
        level += 1;
    }

    if models.iter().any(|m| m.is_some()) {
        Some(models)
    } else {
        None
    }
}
