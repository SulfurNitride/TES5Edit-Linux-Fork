//! Reference scanner for LOD generation.
//!
//! Scans loaded plugins to find:
//! - Tree base records (STAT/ACTI/TREE with HasTreeLOD flag 0x0040)
//! - Object base records (STAT with HasDistantLOD flag 0x8000)
//! - Worldspace REFR records with position, scale, and base reference
//!
//! Ported from wbGenerateLODTES5 reference collection in wbLOD.pas.

use std::collections::HashMap;
use std::path::Path;

use byteorder::{LittleEndian, ReadBytesExt};
use tracing::{debug, info};

use xedit_dom::{Plugin, Record, Signature};
use xedit_dom::group::{GroupChild, GroupType};

/// Flag indicating a record has tree LOD (0x0040).
const FLAG_HAS_TREE_LOD: u32 = 0x0040;

/// Flag indicating a record has distant LOD (0x8000).
const FLAG_HAS_DISTANT_LOD: u32 = 0x8000;

/// Flag indicating a REFR is initially disabled (0x0800).
const FLAG_INITIALLY_DISABLED: u32 = 0x0800;

/// Signature constants not yet in xedit_dom.
const SIG_TREE: Signature = Signature(*b"TREE");
const SIG_ACTI: Signature = Signature(*b"ACTI");
const SIG_XSCL: Signature = Signature(*b"XSCL");
const SIG_XESP: Signature = Signature(*b"XESP");
const SIG_MNAM: Signature = Signature(*b"MNAM");
const SIG_OBND: Signature = Signature(*b"OBND");

/// A base record that can have tree LOD.
#[derive(Debug, Clone)]
pub struct TreeBaseInfo {
    /// FormID of the base record.
    pub form_id: u32,
    /// Editor ID.
    pub editor_id: String,
    /// Model path (MODL subrecord).
    pub model_path: String,
    /// Source plugin filename.
    pub plugin_filename: String,
    /// Object bounds (X1, Y1, Z1, X2, Y2, Z2) from OBND.
    pub bounds: Option<[i16; 6]>,
    /// Width from OBND (X2 - X1).
    pub width: f32,
    /// Height from OBND (Z2 - Z1).
    pub height: f32,
}

/// A base record that can have object LOD.
#[derive(Debug, Clone)]
pub struct ObjectBaseInfo {
    pub form_id: u32,
    pub editor_id: String,
    /// LOD mesh paths from MNAM (up to 4 levels).
    pub lod_models: Vec<String>,
    /// Has distant LOD flag.
    pub has_distant_lod: bool,
    /// Has tree LOD flag (for 3D tree LOD fallback).
    pub has_tree_lod: bool,
}

/// A placed reference (REFR) in a worldspace.
#[derive(Debug, Clone)]
pub struct RefInfo {
    /// FormID of the REFR record.
    pub ref_form_id: u32,
    /// FormID of the base object (NAME subrecord).
    pub base_form_id: u32,
    /// World position (X, Y, Z) from DATA subrecord.
    pub position: [f32; 3],
    /// Rotation (X, Y, Z) in degrees from DATA subrecord.
    pub rotation: [f32; 3],
    /// Scale from XSCL subrecord (default 1.0).
    pub scale: f32,
    /// Whether the REFR is initially disabled.
    pub is_disabled: bool,
    /// Whether the REFR is deleted.
    pub is_deleted: bool,
    /// Whether the REFR has an enable parent (XESP).
    pub has_enable_parent: bool,
    /// Whether the REFR is persistent.
    pub is_persistent: bool,
    /// Cell coordinates derived from position.
    pub cell_x: i32,
    pub cell_y: i32,
}

/// Scan results from plugin processing.
#[derive(Debug, Default)]
pub struct ScanResults {
    /// Tree base records indexed by FormID.
    pub tree_bases: HashMap<u32, TreeBaseInfo>,
    /// Object base records indexed by FormID.
    pub object_bases: HashMap<u32, ObjectBaseInfo>,
    /// All REFR records in the target worldspace.
    pub refs: Vec<RefInfo>,
}

/// Convert world position to grid cell coordinates.
///
/// In Bethesda games, each cell is 4096 units. Cell (0,0) covers world coords (0,0) to (4095,4095).
pub fn position_to_cell(x: f32, y: f32) -> (i32, i32) {
    let cx = if x < 0.0 {
        (x / 4096.0).floor() as i32
    } else {
        (x / 4096.0) as i32
    };
    let cy = if y < 0.0 {
        (y / 4096.0).floor() as i32
    } else {
        (y / 4096.0) as i32
    };
    (cx, cy)
}

/// Scan a list of plugins for tree base records.
///
/// For FNV: scans STAT, ACTI, TREE groups for records with HasTreeLOD flag (0x0040).
/// Returns a map of FormID -> TreeBaseInfo.
pub fn scan_tree_bases(plugins: &[Plugin]) -> HashMap<u32, TreeBaseInfo> {
    let mut result = HashMap::new();
    let target_sigs = [Signature::STAT, SIG_ACTI, SIG_TREE];

    for plugin in plugins {
        let filename = plugin.file_path.as_ref()
            .and_then(|p| p.file_name())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        for group in &plugin.groups {
            // Only scan STAT, ACTI, TREE top-level groups
            if let GroupType::Top(sig) = group.group_type {
                if !target_sigs.contains(&sig) {
                    continue;
                }
            } else {
                continue;
            }

            for child in &group.children {
                if let GroupChild::Record(record) = child {
                    if record.flags.0 & FLAG_HAS_TREE_LOD == 0 {
                        continue;
                    }
                    if record.flags.is_deleted() {
                        continue;
                    }

                    let form_id = record.form_id.raw();
                    let editor_id = record.editor_id().unwrap_or("").to_string();
                    let model_path = extract_model_path(record);
                    let bounds = extract_obnd(record);

                    let (width, height) = if let Some(b) = &bounds {
                        ((b[3] - b[0]) as f32, (b[5] - b[2]) as f32)
                    } else {
                        (0.0, 0.0)
                    };

                    result.insert(form_id, TreeBaseInfo {
                        form_id,
                        editor_id,
                        model_path,
                        plugin_filename: filename.clone(),
                        bounds,
                        width,
                        height,
                    });
                }
            }
        }
    }

    info!("Found {} tree base records across {} plugins", result.len(), plugins.len());
    result
}

/// Scan a list of plugins for object base records with LOD models.
///
/// Finds STAT records with HasDistantLOD flag (0x8000) and MNAM subrecords.
pub fn scan_object_bases(plugins: &[Plugin]) -> HashMap<u32, ObjectBaseInfo> {
    let mut result = HashMap::new();

    for plugin in plugins {
        for group in &plugin.groups {
            if let GroupType::Top(sig) = group.group_type {
                if sig != Signature::STAT {
                    continue;
                }
            } else {
                continue;
            }

            for child in &group.children {
                if let GroupChild::Record(record) = child {
                    if record.flags.is_deleted() {
                        continue;
                    }

                    let has_distant = record.flags.0 & FLAG_HAS_DISTANT_LOD != 0;
                    let has_tree = record.flags.0 & FLAG_HAS_TREE_LOD != 0;

                    if !has_distant {
                        continue;
                    }

                    let form_id = record.form_id.raw();
                    let editor_id = record.editor_id().unwrap_or("").to_string();
                    let lod_models = extract_mnam_models(record);

                    result.insert(form_id, ObjectBaseInfo {
                        form_id,
                        editor_id,
                        lod_models,
                        has_distant_lod: has_distant,
                        has_tree_lod: has_tree,
                    });
                }
            }
        }
    }

    info!("Found {} object base records with LOD models", result.len());
    result
}

/// Scan worldspace for REFR records.
///
/// Traverses the WRLD group hierarchy to find all REFR records in the target worldspace.
pub fn scan_worldspace_refs(
    plugins: &[Plugin],
    worldspace_editor_id: &str,
) -> Vec<RefInfo> {
    let mut refs = Vec::new();

    for plugin in plugins {
        for group in &plugin.groups {
            if let GroupType::Top(sig) = group.group_type {
                if sig != Signature::WRLD {
                    continue;
                }
            } else {
                continue;
            }

            // Find the target worldspace record and its children
            scan_wrld_group(group, worldspace_editor_id, &mut refs);
        }
    }

    info!("Found {} REFR records in worldspace '{}'", refs.len(), worldspace_editor_id);
    refs
}

/// Recursively scan a WRLD group for REFRs in the target worldspace.
fn scan_wrld_group(
    group: &xedit_dom::Group,
    worldspace_editor_id: &str,
    refs: &mut Vec<RefInfo>,
) {
    let mut found_worldspace = false;
    let mut worldspace_form_id: Option<u32> = None;

    for child in &group.children {
        match child {
            GroupChild::Record(record) => {
                if record.signature == Signature::WRLD {
                    // Check if this is the worldspace we want
                    if let Some(edid) = record.editor_id() {
                        if edid.eq_ignore_ascii_case(worldspace_editor_id) {
                            found_worldspace = true;
                            worldspace_form_id = Some(record.form_id.raw());
                            debug!("Found worldspace '{}' with FormID {:08X}", edid, record.form_id.raw());
                        }
                    }
                } else if record.signature == Signature::REFR {
                    // REFR inside the worldspace group
                    if found_worldspace {
                        if let Some(ref_info) = parse_refr(record) {
                            refs.push(ref_info);
                        }
                    }
                } else if record.signature == Signature::CELL {
                    // Cell record inside worldspace — refs follow in cell children groups
                }
            }
            GroupChild::Group(subgroup) => {
                match subgroup.group_type {
                    GroupType::WorldChildren(fid) => {
                        if worldspace_form_id == Some(fid) || found_worldspace {
                            scan_world_children(subgroup, refs);
                        }
                    }
                    _ => {
                        // Recurse into other group types
                        scan_wrld_group(subgroup, worldspace_editor_id, refs);
                    }
                }
            }
        }
    }
}

/// Scan world children group for REFRs.
fn scan_world_children(
    group: &xedit_dom::Group,
    refs: &mut Vec<RefInfo>,
) {
    for child in &group.children {
        match child {
            GroupChild::Record(record) => {
                if record.signature == Signature::REFR {
                    if let Some(ref_info) = parse_refr(record) {
                        refs.push(ref_info);
                    }
                }
            }
            GroupChild::Group(subgroup) => {
                match subgroup.group_type {
                    GroupType::ExteriorCellBlock { .. } |
                    GroupType::ExteriorCellSubBlock { .. } |
                    GroupType::CellChildren(_) |
                    GroupType::CellTemporaryChildren(_) |
                    GroupType::CellPersistentChildren(_) |
                    GroupType::CellVisibleDistantChildren(_) => {
                        scan_world_children(subgroup, refs);
                    }
                    _ => {
                        scan_world_children(subgroup, refs);
                    }
                }
            }
        }
    }
}

/// Parse a REFR record into RefInfo.
fn parse_refr(record: &Record) -> Option<RefInfo> {
    // Get base FormID from NAME subrecord
    let name_sr = record.subrecords.iter().find(|sr| sr.signature == Signature::NAME)?;
    if name_sr.raw_data.len() < 4 {
        return None;
    }
    let base_form_id = u32::from_le_bytes([
        name_sr.raw_data[0], name_sr.raw_data[1],
        name_sr.raw_data[2], name_sr.raw_data[3],
    ]);

    // Get position and rotation from DATA subrecord
    // DATA = 6 floats: PosX, PosY, PosZ, RotX, RotY, RotZ (24 bytes)
    let data_sr = record.subrecords.iter().find(|sr| sr.signature == Signature::DATA)?;
    if data_sr.raw_data.len() < 24 {
        return None;
    }
    let mut cursor = std::io::Cursor::new(&data_sr.raw_data);
    let pos_x = cursor.read_f32::<LittleEndian>().ok()?;
    let pos_y = cursor.read_f32::<LittleEndian>().ok()?;
    let pos_z = cursor.read_f32::<LittleEndian>().ok()?;
    let rot_x = cursor.read_f32::<LittleEndian>().ok()?;
    let rot_y = cursor.read_f32::<LittleEndian>().ok()?;
    let rot_z = cursor.read_f32::<LittleEndian>().ok()?;

    // Convert rotation from radians to degrees for the fallen tree check
    let rot_x_deg = rot_x.to_degrees().rem_euclid(360.0);
    let rot_y_deg = rot_y.to_degrees().rem_euclid(360.0);

    // Get scale from XSCL subrecord (optional, default 1.0)
    let scale = record.subrecords.iter()
        .find(|sr| sr.signature == SIG_XSCL)
        .and_then(|sr| {
            if sr.raw_data.len() >= 4 {
                Some(f32::from_le_bytes([sr.raw_data[0], sr.raw_data[1], sr.raw_data[2], sr.raw_data[3]]))
            } else {
                None
            }
        })
        .unwrap_or(1.0);

    // Check for enable parent (XESP)
    let has_enable_parent = record.subrecords.iter().any(|sr| sr.signature == SIG_XESP);

    // Check flags
    let is_disabled = record.flags.0 & FLAG_INITIALLY_DISABLED != 0;
    let is_deleted = record.flags.is_deleted();

    // Persistent flag (check group type — persistent refs are in CellPersistentChildren)
    // We can't easily determine this from the record alone; approximate from flags
    let is_persistent = record.flags.0 & 0x0400 != 0; // 0x0400 = Quest item / Persistent

    let (cell_x, cell_y) = position_to_cell(pos_x, pos_y);

    Some(RefInfo {
        ref_form_id: record.form_id.raw(),
        base_form_id,
        position: [pos_x, pos_y, pos_z],
        rotation: [rot_x_deg, rot_y_deg, rot_z.to_degrees().rem_euclid(360.0)],
        scale,
        is_disabled,
        is_deleted,
        has_enable_parent,
        is_persistent,
        cell_x,
        cell_y,
    })
}

/// Extract the model path (MODL subrecord) from a record.
fn extract_model_path(record: &Record) -> String {
    record.subrecords.iter()
        .find(|sr| sr.signature == Signature::MODL)
        .map(|sr| {
            let len = sr.raw_data.iter().position(|&b| b == 0).unwrap_or(sr.raw_data.len());
            String::from_utf8_lossy(&sr.raw_data[..len]).to_string()
        })
        .unwrap_or_default()
}

/// Extract OBND (object bounds) from a record.
///
/// OBND = 12 bytes: X1 i16, Y1 i16, Z1 i16, X2 i16, Y2 i16, Z2 i16
fn extract_obnd(record: &Record) -> Option<[i16; 6]> {
    let sr = record.subrecords.iter().find(|sr| sr.signature == SIG_OBND)?;
    if sr.raw_data.len() < 12 {
        return None;
    }
    let mut cursor = std::io::Cursor::new(&sr.raw_data);
    Some([
        cursor.read_i16::<LittleEndian>().ok()?,
        cursor.read_i16::<LittleEndian>().ok()?,
        cursor.read_i16::<LittleEndian>().ok()?,
        cursor.read_i16::<LittleEndian>().ok()?,
        cursor.read_i16::<LittleEndian>().ok()?,
        cursor.read_i16::<LittleEndian>().ok()?,
    ])
}

/// Extract MNAM LOD model paths from a STAT record.
///
/// MNAM contains multiple null-terminated strings for LOD mesh paths.
fn extract_mnam_models(record: &Record) -> Vec<String> {
    let mut models = Vec::new();
    for sr in record.subrecords.iter().filter(|sr| sr.signature == SIG_MNAM) {
        let s = sr.raw_data.iter()
            .position(|&b| b == 0)
            .map(|len| String::from_utf8_lossy(&sr.raw_data[..len]).to_string())
            .unwrap_or_else(|| String::from_utf8_lossy(&sr.raw_data).to_string());
        if !s.is_empty() {
            models.push(s);
        }
    }
    models
}

/// Generate the billboard texture path for a tree base record.
///
/// FNV format: `Textures/Terrain/LODGen/{PluginFileName}/{ModelBaseName}_{FormID}.dds`
///
/// This matches the Delphi `BillboardFileName` function in wbLOD.pas.
pub fn billboard_path(plugin_filename: &str, model_path: &str, form_id: u32) -> String {
    // Extract model base name (without extension, without directory)
    // Model paths may use backslashes, so normalize first
    let normalized = model_path.replace('\\', "/");
    let model_name = Path::new(&normalized)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();

    // FormID without file index (lower 24 bits), formatted as 8 hex uppercase
    let local_id = form_id & 0x00FF_FFFF;

    format!(
        "textures/terrain/lodgen/{}/{}_{:08X}.dds",
        plugin_filename,
        model_name,
        local_id,
    ).to_lowercase()
}

/// Filter REFRs for tree LOD generation.
///
/// Applies the same filters as wbLOD.pas:
/// - Skip disabled, deleted, and enable-parent refs
/// - Skip "fallen" trees (rotation > 30 degrees on X or Y)
/// - Match base to known tree bases
pub fn filter_tree_refs(
    refs: &[RefInfo],
    tree_bases: &HashMap<u32, TreeBaseInfo>,
) -> Vec<RefInfo> {
    refs.iter()
        .filter(|r| {
            // Skip invisible references
            if r.is_disabled || r.is_deleted || r.has_enable_parent {
                return false;
            }

            // Must have a known tree base
            if !tree_bases.contains_key(&r.base_form_id) {
                return false;
            }

            // Skip fallen trees (rotation > 30 on X or Y, excluding near 0 and near 360)
            let rot_x = r.rotation[0];
            let rot_y = r.rotation[1];
            if (rot_x > 30.0 && rot_x < 330.0) || (rot_y > 30.0 && rot_y < 330.0) {
                return false;
            }

            true
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_to_cell() {
        assert_eq!(position_to_cell(0.0, 0.0), (0, 0));
        assert_eq!(position_to_cell(4096.0, 4096.0), (1, 1));
        assert_eq!(position_to_cell(-1.0, -1.0), (-1, -1));
        assert_eq!(position_to_cell(8191.0, 8191.0), (1, 1));
        assert_eq!(position_to_cell(-4097.0, -4097.0), (-2, -2));
    }

    #[test]
    fn test_billboard_path() {
        let path = billboard_path(
            "FalloutNV.esm",
            "Architecture\\Goodsprings\\GSDeadTree01.nif",
            0x000E3FAB,
        );
        assert_eq!(path, "textures/terrain/lodgen/falloutnv.esm/gsdeadtree01_000e3fab.dds");
    }

    #[test]
    fn test_billboard_path_with_subdir() {
        let path = billboard_path(
            "SomePlugin.esp",
            "Trees\\TreeBranchless01.nif",
            0x0100ABCD,
        );
        // File index (01) is zeroed, so 0x00ABCD → 0000ABCD
        assert_eq!(path, "textures/terrain/lodgen/someplugin.esp/treebranchless01_0000abcd.dds");
    }
}
