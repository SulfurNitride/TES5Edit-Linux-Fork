//! Terrain LOD generation — heightmap meshes and landscape textures.
//!
//! Handles LAND record parsing, binary terrain data export for LODGen,
//! and landscape texture blending for terrain LOD textures.
//!
//! Pipeline:
//! 1. Scan plugins for LAND records in the target worldspace
//! 2. Extract VHGT heightmap data (33x33 grid per cell)
//! 3. Write binary terrain data file for LODGen.exe
//! 4. Write LODGen terrain config file (GameMode=TERRAINFNV)
//! 5. Call LODGen in terrain mode to generate meshes
//! 6. Generate terrain textures by blending landscape textures

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use rayon::prelude::*;
use tracing::{debug, info};

use xedit_dom::{Plugin, Record, Signature};
use xedit_dom::group::{GroupChild, GroupType};

use crate::dds_util;
use crate::resource_loader::ResourceLoader;

/// VHGT signature for heightmap subrecord.
const SIG_VHGT: Signature = Signature(*b"VHGT");
/// VNML signature for vertex normals.
const SIG_VNML: Signature = Signature(*b"VNML");
/// VCLR signature for vertex colors.
const SIG_VCLR: Signature = Signature(*b"VCLR");
/// ATXT signature for additional texture layers.
const SIG_ATXT: Signature = Signature(*b"ATXT");
/// BTXT signature for base texture.
const SIG_BTXT: Signature = Signature(*b"BTXT");
/// VTXT signature for alpha layer opacity data.
const SIG_VTXT: Signature = Signature(*b"VTXT");
/// XCLC signature for cell grid coordinates.
const SIG_XCLC: Signature = Signature(*b"XCLC");
/// XCLW signature for cell water height.
const SIG_XCLW: Signature = Signature(*b"XCLW");
/// LAND signature.
const SIG_LAND: Signature = Signature(*b"LAND");

/// Resolve a raw plugin-local FormID to a canonical load-order FormID.
///
/// In a plugin file, the high byte of a FormID is the master index.
/// This maps through the plugin's master list to find the canonical
/// load-order-based FormID.
fn resolve_formid(raw: u32, plugin_masters: &[String], plugin_names: &[String], self_idx: usize) -> u32 {
    let master_idx = (raw >> 24) as usize;
    let local_id = raw & 0x00FF_FFFF;

    if master_idx < plugin_masters.len() {
        // References a master file — find its load order position
        let target_name = &plugin_masters[master_idx];
        for (i, name) in plugin_names.iter().enumerate() {
            if name.eq_ignore_ascii_case(target_name) {
                return (i as u32) << 24 | local_id;
            }
        }
    }
    // master_idx >= plugin_masters.len() means "this plugin itself"
    (self_idx as u32) << 24 | local_id
}

/// Get the plugin filename from its file_path.
fn plugin_filename(plugin: &Plugin) -> String {
    plugin.file_path.as_ref()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// Terrain cell data extracted from a LAND record.
#[derive(Debug, Clone)]
pub struct TerrainCell {
    /// Cell grid X coordinate.
    pub cell_x: i32,
    /// Cell grid Y coordinate.
    pub cell_y: i32,
    /// Water height for this cell.
    pub water_height: f32,
    /// Land flags (from LAND record flags).
    pub land_flags: i32,
    /// Height offset (base height, multiplied by 8 in the record).
    pub height_offset: f32,
    /// Height deltas: 33x33 grid of signed byte deltas.
    /// Stored row-major: [row * 33 + col] where row=0..33, col=0..33.
    pub height_deltas: Vec<i8>,
    /// Trailing 3 bytes from VHGT subrecord (unused/padding, but preserved for binary output).
    pub vhgt_footer: [u8; 3],
    /// Vertex normals (optional): 33x33 x 3 bytes (X, Y, Z per vertex).
    pub vertex_normals: Option<Vec<u8>>,
    /// Vertex colors (optional): 33x33 x 3 bytes (R, G, B per vertex).
    pub vertex_colors: Option<Vec<u8>>,
    /// Base textures per quadrant (from BTXT): quadrant -> LTEX FormID.
    pub base_textures: [Option<u32>; 4],
    /// Fallback base textures per quadrant from earlier plugins.
    /// Used when the winning override's BTXT points to a DLC texture that
    /// can't be loaded from SArchiveList BSAs.
    pub fallback_btxt: [Option<u32>; 4],
    /// Additional texture layers: (LTEX FormID, quadrant, layer_index).
    pub additional_textures: Vec<(u32, u8, u16)>,
    /// Alpha layer opacity data (from VTXT): per-layer alpha values.
    /// Each entry: (quadrant, layer_index, Vec<(position, opacity)>).
    pub alpha_layers: Vec<(u8, u16, Vec<(u16, f32)>)>,
}

/// Scan plugins for LAND records in a worldspace and extract terrain data.
///
/// Returns a map of (cell_x, cell_y) -> TerrainCell.
/// FormIDs in BTXT/ATXT are resolved through each plugin's master list
/// to canonical load-order-based FormIDs.
pub fn scan_terrain_cells(
    plugins: &[Plugin],
    worldspace_editor_id: &str,
) -> HashMap<(i32, i32), TerrainCell> {
    let mut cells: HashMap<(i32, i32), TerrainCell> = HashMap::new();

    // Build plugin name list for FormID resolution
    let plugin_names: Vec<String> = plugins.iter()
        .map(|p| plugin_filename(p))
        .collect();

    for (plugin_idx, plugin) in plugins.iter().enumerate() {
        for group in &plugin.groups {
            if let GroupType::Top(sig) = group.group_type {
                if sig != Signature::WRLD {
                    continue;
                }
            } else {
                continue;
            }

            scan_wrld_for_land(group, worldspace_editor_id, &mut cells,
                &plugin.masters, &plugin_names, plugin_idx);
        }
    }

    info!("Found {} terrain cells in worldspace '{}'", cells.len(), worldspace_editor_id);
    cells
}

/// Recursively scan a WRLD group for LAND records.
fn scan_wrld_for_land(
    group: &xedit_dom::Group,
    worldspace_editor_id: &str,
    cells: &mut HashMap<(i32, i32), TerrainCell>,
    plugin_masters: &[String],
    plugin_names: &[String],
    plugin_idx: usize,
) {
    let mut found_worldspace = false;
    let mut worldspace_form_id: Option<u32> = None;

    // Track current cell coordinates
    let mut current_cell_x: Option<i32> = None;
    let mut current_cell_y: Option<i32> = None;
    let mut current_water_height: f32 = f32::MIN;

    for child in &group.children {
        match child {
            GroupChild::Record(record) => {
                if record.signature == Signature::WRLD {
                    // Check if this WRLD record matches the target worldspace.
                    // Reset found_worldspace for each new WRLD record to avoid
                    // accidentally including LAND from other worldspaces in the
                    // same plugin (e.g., ZionNP cells in HonestHearts.esm).
                    let is_match = record.editor_id()
                        .map(|edid| edid.eq_ignore_ascii_case(worldspace_editor_id))
                        .unwrap_or(false);
                    if is_match {
                        found_worldspace = true;
                        worldspace_form_id = Some(record.form_id.raw());
                    } else if worldspace_form_id.is_some() {
                        // New WRLD that doesn't match — stop scanning this group
                        // unless the FormID matches a previous match
                        found_worldspace = false;
                    }
                } else if record.signature == Signature::CELL && found_worldspace {
                    if let Some(xclc) = record.subrecords.iter().find(|sr| sr.signature == SIG_XCLC) {
                        if xclc.raw_data.len() >= 8 {
                            let mut cursor = std::io::Cursor::new(&xclc.raw_data);
                            current_cell_x = Some(cursor.read_i32::<LittleEndian>().unwrap_or(0));
                            current_cell_y = Some(cursor.read_i32::<LittleEndian>().unwrap_or(0));
                        }
                    }
                    if let Some(xclw) = record.subrecords.iter().find(|sr| sr.signature == SIG_XCLW) {
                        if xclw.raw_data.len() >= 4 {
                            let mut cursor = std::io::Cursor::new(&xclw.raw_data);
                            current_water_height = cursor.read_f32::<LittleEndian>().unwrap_or(f32::MIN);
                        }
                    }
                } else if record.signature == SIG_LAND && found_worldspace {
                    if let (Some(cx), Some(cy)) = (current_cell_x, current_cell_y) {
                        if let Some(mut cell) = parse_land_record(
                            record, cx, cy, current_water_height,
                            plugin_masters, plugin_names, plugin_idx,
                        ) {
                            // Preserve subrecords from earlier plugins if the override lacks them.
                            // Mod plugins often override LAND to change heightmaps or textures
                            // without including all subrecords (VCLR, VNML, BTXT, ATXT/VTXT).
                            if let Some(prev) = cells.get(&(cx, cy)) {
                                let new_has_btxt = cell.base_textures.iter().any(|bt| bt.is_some());
                                let prev_has_btxt = prev.base_textures.iter().any(|bt| bt.is_some());

                                if cell.vertex_colors.is_none() && prev.vertex_colors.is_some() {
                                    cell.vertex_colors = prev.vertex_colors.clone();
                                }
                                // If the override provides VCLR but no BTXT, and the previous
                                // plugin had BTXT+VCLR, prefer the previous VCLR. DLC plugins
                                // (e.g. LonesomeRoad) often override LAND with dark default VCLR
                                // without providing textures, creating black patches when their
                                // VCLR is applied to preserved BTXT textures.
                                if !new_has_btxt && prev_has_btxt
                                    && cell.vertex_colors.is_some() && prev.vertex_colors.is_some()
                                {
                                    cell.vertex_colors = prev.vertex_colors.clone();
                                }
                                if cell.vertex_normals.is_none() && prev.vertex_normals.is_some() {
                                    cell.vertex_normals = prev.vertex_normals.clone();
                                }
                                // Preserve base textures if the override has no BTXT at all
                                if !new_has_btxt && prev_has_btxt {
                                    cell.base_textures = prev.base_textures;
                                    cell.fallback_btxt = prev.fallback_btxt;
                                } else if new_has_btxt && prev_has_btxt {
                                    // Store the previous BTXT as fallback per-quadrant.
                                    // When a mod overrides BTXT with DLC FormIDs, we can
                                    // fall back to the previous (base game) BTXT at paint time.
                                    for q in 0..4 {
                                        if prev.base_textures[q].is_some() {
                                            cell.fallback_btxt[q] = prev.base_textures[q];
                                        }
                                    }
                                }
                                // Preserve additional textures/alpha layers if override has none
                                if cell.additional_textures.is_empty() && !prev.additional_textures.is_empty() {
                                    cell.additional_textures = prev.additional_textures.clone();
                                    cell.alpha_layers = prev.alpha_layers.clone();
                                }
                            }
                            cells.insert((cx, cy), cell);
                        }
                    }
                }
            }
            GroupChild::Group(subgroup) => {
                match subgroup.group_type {
                    GroupType::WorldChildren(fid) => {
                        if worldspace_form_id == Some(fid) || found_worldspace {
                            scan_world_children_for_land(
                                subgroup, cells,
                                None, None, f32::MIN,
                                plugin_masters, plugin_names, plugin_idx,
                            );
                        }
                    }
                    _ => {
                        scan_wrld_for_land(subgroup, worldspace_editor_id, cells,
                            plugin_masters, plugin_names, plugin_idx);
                    }
                }
            }
        }
    }
}

/// Scan world children groups for LAND records.
///
/// The group hierarchy is:
///   WorldChildren → ExteriorCellBlock → ExteriorCellSubBlock → CELL record → CellChildren/CellTemporaryChildren → LAND record
///
/// We track the most recent CELL's coordinates as we descend through the hierarchy.
fn scan_world_children_for_land(
    group: &xedit_dom::Group,
    cells: &mut HashMap<(i32, i32), TerrainCell>,
    cell_x: Option<i32>,
    cell_y: Option<i32>,
    water_height: f32,
    plugin_masters: &[String],
    plugin_names: &[String],
    plugin_idx: usize,
) {
    let mut current_cell_x = cell_x;
    let mut current_cell_y = cell_y;
    let mut current_water_height = water_height;

    for child in &group.children {
        match child {
            GroupChild::Record(record) => {
                if record.signature == Signature::CELL {
                    if let Some(xclc) = record.subrecords.iter().find(|sr| sr.signature == SIG_XCLC) {
                        if xclc.raw_data.len() >= 8 {
                            let mut cursor = std::io::Cursor::new(&xclc.raw_data);
                            current_cell_x = Some(cursor.read_i32::<LittleEndian>().unwrap_or(0));
                            current_cell_y = Some(cursor.read_i32::<LittleEndian>().unwrap_or(0));
                        }
                    }
                    if let Some(xclw) = record.subrecords.iter().find(|sr| sr.signature == SIG_XCLW) {
                        if xclw.raw_data.len() >= 4 {
                            let mut cursor = std::io::Cursor::new(&xclw.raw_data);
                            current_water_height = cursor.read_f32::<LittleEndian>().unwrap_or(f32::MIN);
                        }
                    }
                } else if record.signature == SIG_LAND {
                    if let (Some(cx), Some(cy)) = (current_cell_x, current_cell_y) {
                        if let Some(mut cell) = parse_land_record(
                            record, cx, cy, current_water_height,
                            plugin_masters, plugin_names, plugin_idx,
                        ) {
                            // Preserve subrecords from earlier plugins if override lacks them
                            if let Some(prev) = cells.get(&(cx, cy)) {
                                let new_has_btxt = cell.base_textures.iter().any(|bt| bt.is_some());
                                let prev_has_btxt = prev.base_textures.iter().any(|bt| bt.is_some());

                                if cell.vertex_colors.is_none() && prev.vertex_colors.is_some() {
                                    cell.vertex_colors = prev.vertex_colors.clone();
                                }
                                // Prefer previous VCLR when override has no BTXT (DLC dark VCLR fix)
                                if !new_has_btxt && prev_has_btxt
                                    && cell.vertex_colors.is_some() && prev.vertex_colors.is_some()
                                {
                                    cell.vertex_colors = prev.vertex_colors.clone();
                                }
                                if cell.vertex_normals.is_none() && prev.vertex_normals.is_some() {
                                    cell.vertex_normals = prev.vertex_normals.clone();
                                }
                                if !new_has_btxt && prev_has_btxt {
                                    cell.base_textures = prev.base_textures;
                                    cell.fallback_btxt = prev.fallback_btxt;
                                } else if new_has_btxt && prev_has_btxt {
                                    for q in 0..4 {
                                        if prev.base_textures[q].is_some() {
                                            cell.fallback_btxt[q] = prev.base_textures[q];
                                        }
                                    }
                                }
                                if cell.additional_textures.is_empty() && !prev.additional_textures.is_empty() {
                                    cell.additional_textures = prev.additional_textures.clone();
                                    cell.alpha_layers = prev.alpha_layers.clone();
                                }
                            }
                            cells.insert((cx, cy), cell);
                        }
                    }
                }
            }
            GroupChild::Group(subgroup) => {
                scan_world_children_for_land(
                    subgroup, cells,
                    current_cell_x, current_cell_y, current_water_height,
                    plugin_masters, plugin_names, plugin_idx,
                );
            }
        }
    }
}

/// Parse a LAND record to extract terrain data.
fn parse_land_record(
    record: &Record,
    cell_x: i32,
    cell_y: i32,
    water_height: f32,
    plugin_masters: &[String],
    plugin_names: &[String],
    plugin_idx: usize,
) -> Option<TerrainCell> {
    // Find VHGT subrecord (heightmap)
    let vhgt = record.subrecords.iter().find(|sr| sr.signature == SIG_VHGT)?;

    // VHGT format:
    // - 4 bytes: float32 offset (height offset, stored as offset/8 in raw)
    // - 1089 bytes: 33x33 signed byte height deltas
    // - 3 bytes: unused (short + byte)
    if vhgt.raw_data.len() < 4 + 1089 {
        debug!("VHGT too short for cell ({}, {}): {} bytes", cell_x, cell_y, vhgt.raw_data.len());
        return None;
    }

    let mut cursor = std::io::Cursor::new(&vhgt.raw_data);
    let height_offset = cursor.read_f32::<LittleEndian>().unwrap_or(0.0);

    let mut height_deltas = vec![0i8; 1089];
    for i in 0..1089 {
        height_deltas[i] = cursor.read_i8().unwrap_or(0);
    }

    // Read the 3-byte footer (unused padding in VHGT)
    let mut vhgt_footer = [0u8; 3];
    if vhgt.raw_data.len() >= 4 + 1089 + 3 {
        vhgt_footer[0] = vhgt.raw_data[4 + 1089];
        vhgt_footer[1] = vhgt.raw_data[4 + 1089 + 1];
        vhgt_footer[2] = vhgt.raw_data[4 + 1089 + 2];
    }

    // Extract vertex normals (optional)
    let vertex_normals = record.subrecords.iter()
        .find(|sr| sr.signature == SIG_VNML)
        .and_then(|sr| {
            if sr.raw_data.len() >= 33 * 33 * 3 {
                Some(sr.raw_data[..33 * 33 * 3].to_vec())
            } else {
                None
            }
        });

    // Extract vertex colors (optional)
    let vertex_colors = record.subrecords.iter()
        .find(|sr| sr.signature == SIG_VCLR)
        .and_then(|sr| {
            if sr.raw_data.len() >= 33 * 33 * 3 {
                Some(sr.raw_data[..33 * 33 * 3].to_vec())
            } else {
                None
            }
        });

    // Extract base textures per quadrant (BTXT)
    // BTXT format: FormID (4) + quadrant (1) + padding (1) + layer (2) = 8 bytes
    let mut base_textures = [None; 4];
    for sr in &record.subrecords {
        if sr.signature == SIG_BTXT && sr.raw_data.len() >= 8 {
            let raw_fid = u32::from_le_bytes([
                sr.raw_data[0], sr.raw_data[1],
                sr.raw_data[2], sr.raw_data[3],
            ]);
            let form_id = resolve_formid(raw_fid, plugin_masters, plugin_names, plugin_idx);
            let quadrant = sr.raw_data[4] as usize;
            if quadrant < 4 {
                base_textures[quadrant] = Some(form_id);
            }
        }
    }

    // Extract additional textures (ATXT) and their alpha data (VTXT)
    // ATXT and VTXT come in pairs: each ATXT is followed by its VTXT
    let mut additional_textures = Vec::new();
    let mut alpha_layers = Vec::new();
    let mut current_atxt: Option<(u32, u8, u16)> = None;

    for sr in &record.subrecords {
        if sr.signature == SIG_ATXT && sr.raw_data.len() >= 8 {
            let raw_fid = u32::from_le_bytes([
                sr.raw_data[0], sr.raw_data[1],
                sr.raw_data[2], sr.raw_data[3],
            ]);
            let form_id = resolve_formid(raw_fid, plugin_masters, plugin_names, plugin_idx);
            let quadrant = sr.raw_data[4];
            let layer = u16::from_le_bytes([sr.raw_data[6], sr.raw_data[7]]);
            additional_textures.push((form_id, quadrant, layer));
            current_atxt = Some((form_id, quadrant, layer));
        } else if sr.signature == SIG_VTXT && sr.raw_data.len() >= 8 {
            // VTXT format: repeated entries of (position: u16, padding: 2 bytes, opacity: f32) = 8 bytes each
            if let Some((_fid, quad, layer_idx)) = current_atxt {
                let mut entries = Vec::new();
                let mut offset = 0;
                while offset + 8 <= sr.raw_data.len() {
                    let position = u16::from_le_bytes([sr.raw_data[offset], sr.raw_data[offset + 1]]);
                    let opacity = f32::from_le_bytes([
                        sr.raw_data[offset + 4], sr.raw_data[offset + 5],
                        sr.raw_data[offset + 6], sr.raw_data[offset + 7],
                    ]);
                    entries.push((position, opacity));
                    offset += 8;
                }
                alpha_layers.push((quad, layer_idx, entries));
            }
            current_atxt = None;
        }
    }

    Some(TerrainCell {
        cell_x,
        cell_y,
        water_height,
        land_flags: 0,
        height_offset,
        height_deltas,
        vhgt_footer,
        vertex_normals,
        vertex_colors,
        base_textures,
        fallback_btxt: [None; 4],
        additional_textures,
        alpha_layers,
    })
}

/// Write the binary terrain data file that LODGen reads.
///
/// Format per cell (1112 bytes):
///   - int32: cell X
///   - int32: cell Y
///   - float32: water height
///   - int32: land flags
///   - float32: height offset (raw from VHGT, NOT multiplied by 8)
///   - 1089 × sbyte: height deltas (33×33 grid)
///   - int16: unknown footer (from VHGT trailing bytes or 0)
///   - byte: unknown footer (from VHGT trailing bytes or 0)
pub fn write_terrain_data_bin(
    path: &Path,
    cells: &HashMap<(i32, i32), TerrainCell>,
) -> Result<()> {
    let mut f = std::fs::File::create(path)
        .with_context(|| format!("Failed to create terrain data: {}", path.display()))?;

    let mut count = 0;
    for ((_cx, _cy), cell) in cells {
        f.write_i32::<LittleEndian>(cell.cell_x)?;
        f.write_i32::<LittleEndian>(cell.cell_y)?;
        f.write_f32::<LittleEndian>(cell.water_height)?;
        f.write_i32::<LittleEndian>(cell.land_flags)?;
        f.write_f32::<LittleEndian>(cell.height_offset)?;

        // Write 1089 height deltas
        for i in 0..1089 {
            let delta = if i < cell.height_deltas.len() {
                cell.height_deltas[i]
            } else {
                0i8
            };
            f.write_i8(delta)?;
        }

        // Write footer (3 bytes from VHGT padding)
        f.write_all(&cell.vhgt_footer)?;

        count += 1;
    }

    info!("Wrote terrain data for {} cells to {}", count, path.display());
    Ok(())
}

/// Write the LODGen terrain configuration file.
///
/// This tells LODGen to generate terrain meshes.
pub fn write_terrain_lodgen_config(
    path: &Path,
    worldspace: &str,
    sw_cell: (i32, i32),
    terrain_data_path: &Path,
    output_path: &Path,
    options: &crate::LodOptions,
    default_water_height: f32,
    default_land_height: f32,
) -> Result<()> {
    let mut f = std::fs::File::create(path)
        .with_context(|| format!("Failed to create terrain config: {}", path.display()))?;

    // GameMode: TERRAINFNV for Fallout NV, TERRAINTES5 for Skyrim, etc.
    writeln!(f, "GameMode=TERRAINFNV")?;
    writeln!(f, "Worldspace={}", worldspace)?;
    writeln!(f, "CellSW={} {}", sw_cell.0, sw_cell.1)?;
    writeln!(f, "MaxLevel=32")?;
    writeln!(f, "TerrainData={}", terrain_data_path.display())?;
    writeln!(f, "IgnoreWater=False")?;
    writeln!(f, "LandHeight={}", default_land_height)?;
    writeln!(f, "WaterHeight={}", default_water_height)?;
    writeln!(f, "ProtectCellBorders=False")?;
    writeln!(f, "HideQuads=False")?;

    // Per-level quality settings
    let levels = [4, 8, 16, 32];
    for (i, level) in levels.iter().enumerate() {
        writeln!(f, "QualityLOD{}={}", level, options.terrain_quality[i])?;
        writeln!(f, "MaxVertsLOD{}={}", level, options.terrain_max_verts[i])?;
        let water_delta = if options.terrain_optimize_unseen[i] { 0 } else { -1 };
        writeln!(f, "WaterDeltaLOD{}={}", level, water_delta)?;
    }

    // Skirts value (typically -14098 for FNV)
    writeln!(f, "Skirts=-14098")?;

    writeln!(f, "PathOutput={}", output_path.display())?;

    info!("Wrote terrain LODGen config to {}", path.display());
    Ok(())
}

/// Generate terrain LOD textures by blending landscape textures.
///
/// For each cell at the given LOD level, blends the landscape textures
/// to produce diffuse and normal DDS files.
///
/// Output naming: `{worldspace}.n.level{level}.x{x}.y{y}.dds`
pub fn generate_terrain_textures(
    cells: &HashMap<(i32, i32), TerrainCell>,
    worldspace: &str,
    output_dir: &Path,
    loader: &ResourceLoader,
    ltex_map: &HashMap<u32, String>,
    lod_level: u32,
    tex_size: u32,
    diffuse_format: u32,
    normal_format: u32,
    diffuse_mipmap: bool,
    _normal_mipmap: bool,
    brightness: i32,
    _contrast: i32,
    gamma: f32,
    progress: &crate::progress::Progress,
) -> Result<u32> {
    let step = lod_level as i32;
    let ws_lower = worldspace.to_lowercase();

    // Create output directories
    let diffuse_dir = output_dir
        .join("textures/landscape/lod")
        .join(&ws_lower)
        .join("diffuse");
    let normal_dir = output_dir
        .join("textures/landscape/lod")
        .join(&ws_lower)
        .join("normals");
    std::fs::create_dir_all(&diffuse_dir)?;
    std::fs::create_dir_all(&normal_dir)?;

    // Find cell range aligned to the largest LOD level (32) so all levels
    // cover the same physical area, matching xLODGen behavior.
    let (min_x, max_x, min_y, max_y) = cell_range(cells);
    let max_step = 32i32;
    let aligned_min_x = align_to_step(min_x, max_step);
    let aligned_min_y = align_to_step(min_y, max_step);
    let aligned_max_x = align_to_step(max_x, max_step) + max_step - step;
    let aligned_max_y = align_to_step(max_y, max_step) + max_step - step;

    // Phase 1: Pre-load all landscape textures into a shared cache
    info!("Pre-loading landscape textures for Level{}...", lod_level);
    // Downsample landscape textures to per-cell resolution (tex_size / step).
    // xLODGen tiles textures to preserve material pattern at LOD scale.
    // From decompilation: GridSize=16, TilingFactor=GridSize/2=8, TileSize=16.
    // Each texture is resized to 128×128, then tile_and_resize shrinks to 16×16
    // and tiles 8× = 128×128 with visible repeating material pattern.
    // This is critical — without tiling, all textures become uniform color blobs
    // and the terrain looks like "one material with different colors" (just VCLR tinting).
    let downsample = 128u32;
    let tile_count = 8u32;
    let tex_cache = preload_landscape_textures(cells, loader, ltex_map, downsample, tile_count);
    let tex_cache = Arc::new(tex_cache);
    info!("Pre-loaded {} unique textures (downsample={}, tile_count={})",
        tex_cache.len(), downsample, tile_count);

    // Phase 2: Build block coordinate list
    let mut blocks: Vec<(i32, i32)> = Vec::new();
    let mut y = aligned_min_y;
    while y <= aligned_max_y {
        let mut x = aligned_min_x;
        while x <= aligned_max_x {
            blocks.push((x, y));
            x += step;
        }
        y += step;
    }
    let total = blocks.len() as f32;
    info!("Processing {} blocks for Level{} using rayon", blocks.len(), lod_level);

    // Phase 3: Process blocks in parallel
    let count = AtomicU32::new(0);
    let processed = AtomicU32::new(0);

    blocks.par_iter().for_each(|&(bx, by)| {
        let (diffuse_rgba, normal_rgba) = blend_terrain_block_cached(
            cells, &tex_cache, ltex_map,
            bx, by, step, tex_size,
            brightness, gamma,
        );

        // Compress and write diffuse DDS
        let name = format!("{}.n.level{}.x{}.y{}.dds", ws_lower, lod_level, bx, by);
        if let Ok(diffuse_dds) = dds_util::compress_to_dds(
            &diffuse_rgba, tex_size, tex_size,
            diffuse_format, diffuse_mipmap,
        ) {
            let _ = std::fs::write(diffuse_dir.join(&name), &diffuse_dds);
        }

        // Compress and write normal DDS
        if let Ok(normal_dds) = dds_util::compress_to_dds(
            &normal_rgba, tex_size, tex_size,
            normal_format, false,
        ) {
            let _ = std::fs::write(normal_dir.join(&name), &normal_dds);
        }

        count.fetch_add(2, Ordering::Relaxed);
        let p = processed.fetch_add(1, Ordering::Relaxed) + 1;
        if p % 50 == 0 {
            progress.report(
                &format!("Terrain textures Level{}: {:.0}%", lod_level, p as f32 / total * 100.0),
                (p as f32 / total) as f64,
            );
        }
    });

    let final_count = count.load(Ordering::Relaxed);
    info!("Generated {} terrain texture files for Level{}", final_count, lod_level);
    Ok(final_count)
}

/// Pre-load all landscape textures referenced by cells into a HashMap.
///
/// Textures are downsampled to `downsample_size` using area filtering to remove
/// high-frequency detail. This matches xLODGen's TerrainDefaultDiffuseSize behavior
/// and prevents checkerboard artifacts from sharp texture transitions at cell boundaries.
// Note: DLC texture filtering removed. The BSA-only ResourceLoader
// (new_bsa_only with SArchiveList BSAs) handles this correctly — textures
// not in base game BSAs simply won't load and fall back to default.
// DLCAnch textures ARE in base game BSAs and should be loaded.

/// Tile an RGBA texture NxN and resize back to the original size.
///
/// Matches xLODGen's texture loader (FUN_01b96440): the texture is tiled NxN
/// to create a larger image, then resized back down. The resize step applies
/// Lanczos filtering across tile boundaries, which inherently smooths the
/// texture and reduces high-frequency detail. This eliminates checkerboard
/// artifacts from sharp texture transitions at cell boundaries.
fn tile_and_resize(rgba: &[u8], size: u32, tile_count: u32) -> Vec<u8> {
    if tile_count <= 1 {
        return rgba.to_vec();
    }
    // xLODGen approach: shrink to (size/tileCount), then tile to fill size.
    // This creates a repeated pattern at the correct frequency for the LOD level
    // without the Lanczos blur that our previous tile-then-resize approach created.
    let small_size = (size / tile_count).max(1);
    let small = dds_util::resize_rgba(rgba, size, size, small_size, small_size);
    let ssz = small_size as usize;
    let sz = size as usize;
    let mut result = vec![0u8; sz * sz * 4];

    for ty in 0..tile_count as usize {
        for tx in 0..tile_count as usize {
            for y in 0..ssz {
                let src_offset = y * ssz * 4;
                let dst_y = ty * ssz + y;
                if dst_y >= sz { break; }
                let dst_x = tx * ssz;
                if dst_x >= sz { break; }
                let copy_w = ssz.min(sz - dst_x);
                let dst_offset = (dst_y * sz + dst_x) * 4;
                result[dst_offset..dst_offset + copy_w * 4]
                    .copy_from_slice(&small[src_offset..src_offset + copy_w * 4]);
            }
        }
    }

    result
}

fn preload_landscape_textures(
    cells: &HashMap<(i32, i32), TerrainCell>,
    loader: &ResourceLoader,
    ltex_map: &HashMap<u32, String>,
    downsample_size: u32,
    tile_count: u32,
) -> HashMap<String, Vec<u8>> {
    // Collect all unique texture paths
    let mut tex_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Default texture
    let default_path = ltex_map.get(&0x00015457)
        .cloned()
        .unwrap_or_else(|| "Landscape\\DirtWasteland01.dds".to_string());
    tex_paths.insert(default_path.to_lowercase());

    for cell in cells.values() {
        for btxt_fid in cell.base_textures.iter().flatten() {
            if let Some(path) = ltex_map.get(btxt_fid) {
                tex_paths.insert(path.to_lowercase());
            }
        }
        for btxt_fid in cell.fallback_btxt.iter().flatten() {
            if let Some(path) = ltex_map.get(btxt_fid) {
                tex_paths.insert(path.to_lowercase());
            }
        }
        for (fid, _, _) in &cell.additional_textures {
            if let Some(path) = ltex_map.get(fid) {
                tex_paths.insert(path.to_lowercase());
            }
        }
    }

    let ds = downsample_size;

    // Load all textures in parallel, downsampling and then tiling+resizing
    // to match xLODGen's texture loader behavior (FUN_01b96440).
    let paths_vec: Vec<String> = tex_paths.into_iter().collect();
    let results: Vec<(String, Option<Vec<u8>>)> = paths_vec.par_iter().map(|tex_path| {
        let full_path = if tex_path.starts_with("textures/") || tex_path.starts_with("textures\\") {
            tex_path.to_string()
        } else {
            format!("textures/{}", tex_path)
        };
        let rgba = loader.load(&full_path).ok().and_then(|data| {
            dds_util::decompress_to_rgba(&data).ok().map(|(rgba, w, h)| {
                let downsampled = if w > ds && h > ds {
                    dds_util::resize_rgba(&rgba, w, h, ds, ds)
                } else {
                    rgba
                };
                // Tile NxN and resize back to smooth out high-frequency detail.
                // This matches xLODGen's FUN_01b96440 texture loading pipeline.
                tile_and_resize(&downsampled, ds, tile_count)
            })
        });
        (tex_path.clone(), rgba)
    }).collect();

    let mut cache = HashMap::new();
    for (path, rgba) in results {
        if let Some(data) = rgba {
            cache.insert(path, data);
        }
    }
    cache
}

/// Build a 33×33 alpha grid from VTXT entries using 3×3 stamp blitting.
///
/// Matches xLODGen's algorithm (FUN_01ba0360 + FUN_01b9cb10 + FUN_00fc2fe0):
/// Each VTXT point at (idx%17, idx/17) in the 17×17 grid maps to position
/// (col*2, row*2) in the 33×33 grid. A 3×3 stamp is blitted there:
/// - Even index → Square stamp: all 9 pixels, center=0xFF, edges=0x80
/// - Odd index → Diamond stamp: center=0xFF, 4 cardinals=0x80, corners=0x00
///
/// The stamp RGB carries the target mask weight. The stamp alpha is set to
/// the VTXT opacity. Blitting uses standard alpha-blend:
///   mask = stamp_rgb * stamp_a/255 + mask * (255 - stamp_a)/255
fn build_alpha_grid_33(entries: &[(u16, f32)]) -> [f32; 33 * 33] {
    let mut grid = [0.0f32; 33 * 33];

    // Square stamp weights (3×3): all pixels filled
    const SQUARE_WEIGHTS: [[u8; 3]; 3] = [
        [0x80, 0x80, 0x80],
        [0x80, 0xFF, 0x80],
        [0x80, 0x80, 0x80],
    ];
    // Diamond stamp weights (3×3): center + 4 cardinals only
    const DIAMOND_WEIGHTS: [[u8; 3]; 3] = [
        [0x00, 0x80, 0x00],
        [0x80, 0xFF, 0x80],
        [0x00, 0x80, 0x00],
    ];

    for &(position, opacity) in entries {
        let pos = position as usize;
        if pos >= 17 * 17 { continue; }

        let col17 = pos % 17;
        let row17 = pos / 17;
        let cx = (col17 * 2) as i32;
        let cy = (row17 * 2) as i32;

        let alpha = (opacity * 255.0).clamp(0.0, 255.0) as u8;
        if alpha == 0 { continue; }

        let is_even = (pos & 1) == 0;
        let weights = if is_even { &SQUARE_WEIGHTS } else { &DIAMOND_WEIGHTS };

        let sa = alpha as u32;
        let inv_sa = 255 - sa;

        for sy in 0..3i32 {
            for sx in 0..3i32 {
                let stamp_rgb = weights[sy as usize][sx as usize] as u32;
                if stamp_rgb == 0 && sa < 255 { continue; }

                let gx = cx + sx - 1;
                let gy = cy + sy - 1;
                if gx < 0 || gx >= 33 || gy < 0 || gy >= 33 { continue; }

                if stamp_rgb == 0 { continue; }

                let idx = gy as usize * 33 + gx as usize;
                let old = (grid[idx] * 255.0) as u32;

                // Standard alpha blend: dst = src * srcA/255 + dst * (255-srcA)/255
                let new_val = (stamp_rgb * sa + old * inv_sa) / 255;
                grid[idx] = new_val.min(255) as f32 / 255.0;
            }
        }
    }

    grid
}

/// Lanczos3 windowed-sinc kernel.
///
/// L(x) = sinc(x) * sinc(x/3)  for |x| < 3, 0 otherwise.
/// Matches Vampyre Imaging Library's sfLanczos filter used by xLODGen.
#[inline]
fn lanczos3_kernel(x: f32) -> f32 {
    if x.abs() < 1e-7 { return 1.0; }
    if x.abs() >= 3.0 { return 0.0; }
    let pi_x = std::f32::consts::PI * x;
    (pi_x.sin() / pi_x) * ((pi_x / 3.0).sin() / (pi_x / 3.0))
}

/// Separable Lanczos3 resize for a single-channel f32 image.
///
/// Matches xLODGen's FUN_00fa87c0 + FUN_00fa9040 (Vampyre Imaging StretchResample).
/// Two-pass: horizontal then vertical.
fn lanczos3_resize_f32(
    src: &[f32], src_w: usize, src_h: usize,
    dst_w: usize, dst_h: usize,
) -> Vec<f32> {
    const RADIUS: f32 = 3.0;

    // Pass 1: horizontal (src_w → dst_w, keep src_h)
    let mut tmp = vec![0.0f32; dst_w * src_h];
    let scale_h = dst_w as f32 / src_w.max(1) as f32;
    let support_h = if scale_h < 1.0 { RADIUS / scale_h } else { RADIUS };

    for y in 0..src_h {
        for x in 0..dst_w {
            let center = if scale_h >= 1.0 {
                x as f32 * src_w as f32 / dst_w.max(1) as f32
            } else {
                (x as f32 + 0.5) / scale_h - 0.5
            };
            let left = (center - support_h).ceil() as i32;
            let right = (center + support_h).floor() as i32;
            let left = left.max(0) as usize;
            let right = (right as usize).min(src_w - 1);

            let mut sum = 0.0f32;
            let mut wsum = 0.0f32;
            for k in left..=right {
                let w = if scale_h < 1.0 {
                    lanczos3_kernel((k as f32 - center) * scale_h)
                } else {
                    lanczos3_kernel(k as f32 - center)
                };
                sum += src[y * src_w + k] * w;
                wsum += w;
            }
            tmp[y * dst_w + x] = if wsum.abs() > 1e-7 { sum / wsum } else { 0.0 };
        }
    }

    // Pass 2: vertical (src_h → dst_h, keep dst_w)
    let mut dst = vec![0.0f32; dst_w * dst_h];
    let scale_v = dst_h as f32 / src_h.max(1) as f32;
    let support_v = if scale_v < 1.0 { RADIUS / scale_v } else { RADIUS };

    for x in 0..dst_w {
        for y in 0..dst_h {
            let center = if scale_v >= 1.0 {
                y as f32 * src_h as f32 / dst_h.max(1) as f32
            } else {
                (y as f32 + 0.5) / scale_v - 0.5
            };
            let left = (center - support_v).ceil() as i32;
            let right = (center + support_v).floor() as i32;
            let left = left.max(0) as usize;
            let right = (right as usize).min(src_h - 1);

            let mut sum = 0.0f32;
            let mut wsum = 0.0f32;
            for k in left..=right {
                let w = if scale_v < 1.0 {
                    lanczos3_kernel((k as f32 - center) * scale_v)
                } else {
                    lanczos3_kernel(k as f32 - center)
                };
                sum += tmp[k * dst_w + x] * w;
                wsum += w;
            }
            dst[y * dst_w + x] = if wsum.abs() > 1e-7 { sum / wsum } else { 0.0 };
        }
    }

    dst
}

/// Resample a 33×33 alpha mask to texture pixel dimensions using Lanczos3.
///
/// Matches xLODGen FUN_01b9c580: upscale to (W + W/32, H + H/32) with Lanczos3,
/// then extract a centered W × H region.
fn resample_mask_lanczos3(grid: &[f32; 33 * 33], w: usize, h: usize) -> Vec<f32> {
    let up_w = w + w / 32;
    let up_h = h + h / 32;
    let up_w = up_w.max(1);
    let up_h = up_h.max(1);

    // Lanczos3 upscale from 33×33 to up_w × up_h
    let upscaled = lanczos3_resize_f32(grid, 33, 33, up_w, up_h);

    // Center-crop to w × h
    let off_x = (up_w - w) / 2;
    let off_y = (up_h - h) / 2 + 1;

    let mut result = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            let sx = x + off_x;
            let sy = y + off_y;
            if sx < up_w && sy < up_h {
                result[y * w + x] = upscaled[sy * up_w + sx].clamp(0.0, 1.0);
            }
        }
    }
    result
}

/// Alpha-blend a texture into a region using premultiplied src-over compositing.
///
/// Matches xLODGen FUN_01b9c580 + FUN_01b9c810:
/// 1. Build 33×33 mask from VTXT stamps
/// 2. Lanczos3 resize mask to region pixel dimensions
/// 3. Premultiply texture RGB by mask value
/// 4. Composite: dst = src + dst * (1 - src.A)
fn alpha_blend_texture_33(
    tex_rgba: &[u8],
    alpha_grid: &[f32; 33 * 33],
    canvas: &mut [u8],
    canvas_size: usize,
    region_x: usize,
    region_y: usize,
    region_size: usize,
) {
    let tex_pixels = tex_rgba.len() / 4;
    let tex_size = (tex_pixels as f64).sqrt() as usize;
    if tex_size == 0 || region_size == 0 { return; }

    // Lanczos3 resize the 33×33 mask to region pixel dimensions
    let mask = resample_mask_lanczos3(alpha_grid, region_size, region_size);

    let ts = tex_size as f32;
    let rs = region_size as f32;

    for py in 0..region_size {
        let out_y = region_y + py;
        if out_y >= canvas_size { break; }

        for px in 0..region_size {
            let out_x = region_x + px;
            if out_x >= canvas_size { break; }

            let alpha = mask[py * region_size + px];
            if alpha < 0.001 { continue; }

            let sx_f = (px as f32 * ts / rs) % ts;
            let sy_f = (py as f32 * ts / rs) % ts;
            let (tr, tg, tb) = bilinear_sample(tex_rgba, tex_size, sx_f, sy_f);

            let a = alpha.clamp(0.0, 1.0);
            let inv_a = 1.0 - a;

            // Premultiplied src-over: dst = (src * a) + dst * (1 - a)
            let oi = (out_y * canvas_size + out_x) * 4;
            canvas[oi] = ((tr as f32 * a) + canvas[oi] as f32 * inv_a).clamp(0.0, 255.0) as u8;
            canvas[oi + 1] = ((tg as f32 * a) + canvas[oi + 1] as f32 * inv_a).clamp(0.0, 255.0) as u8;
            canvas[oi + 2] = ((tb as f32 * a) + canvas[oi + 2] as f32 * inv_a).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Embed a quadrant's 33×33 alpha mask into a 65×65 cell-wide grid.
///
/// The 33×33 mask covers one quadrant (17×17 VTXT expanded via stamps).
/// The 65×65 grid represents the full cell (2 quadrants × 32 intervals + 1 = 65 per axis).
/// The mask is placed at the quadrant's position; surrounding areas are zero.
/// When this 65×65 grid is Lanczos3-upscaled, the filter naturally feathers the
/// mask edges in ALL directions across quadrant boundaries.
fn embed_quad_mask_in_cell(quad_mask: &[f32; 33 * 33], quad: u8) -> [f32; 65 * 65] {
    let mut cell_grid = [0.0f32; 65 * 65];
    // Quadrant offsets in the 65×65 grid (each quadrant occupies 33 vertices, overlapping by 1)
    let (off_x, off_y) = match quad {
        0 => (0usize, 0usize),  // top-left
        1 => (32usize, 0usize), // top-right
        2 => (0usize, 32usize), // bottom-left
        3 => (32usize, 32usize), // bottom-right
        _ => (0, 0),
    };
    for gy in 0..33usize {
        for gx in 0..33usize {
            let cx = off_x + gx;
            let cy = off_y + gy;
            if cx < 65 && cy < 65 {
                let ci = cy * 65 + cx;
                let qi = gy * 33 + gx;
                // Max with existing value to handle shared border vertex
                cell_grid[ci] = cell_grid[ci].max(quad_mask[qi]);
            }
        }
    }
    cell_grid
}

/// Composite a tiled texture onto the cell canvas using a cell-wide alpha mask.
///
/// The mask covers the entire cell (comp_cell_size × comp_cell_size).
/// The texture is tiled across the cell using bilinear sampling.
/// Premultiplied src-over: dst = src*alpha + dst*(1-alpha)
fn composite_with_cellwide_mask(
    tex_rgba: &[u8],
    mask: &[f32],
    canvas: &mut [u8],
    canvas_size: usize,
) {
    let tex_pixels = tex_rgba.len() / 4;
    let tex_size = (tex_pixels as f64).sqrt() as usize;
    if tex_size == 0 { return; }
    let ts = tex_size as f32;
    let cs = canvas_size as f32;

    for py in 0..canvas_size {
        for px in 0..canvas_size {
            let mi = py * canvas_size + px;
            if mi >= mask.len() { continue; }
            let alpha = mask[mi].clamp(0.0, 1.0);
            if alpha < 0.001 { continue; }

            // Tile texture across the cell
            let sx_f = (px as f32 * ts / cs) % ts;
            let sy_f = (py as f32 * ts / cs) % ts;
            let (tr, tg, tb) = bilinear_sample(tex_rgba, tex_size, sx_f, sy_f);

            let inv_a = 1.0 - alpha;
            let oi = (py * canvas_size + px) * 4;
            canvas[oi]     = (tr as f32 * alpha + canvas[oi] as f32 * inv_a).clamp(0.0, 255.0) as u8;
            canvas[oi + 1] = (tg as f32 * alpha + canvas[oi + 1] as f32 * inv_a).clamp(0.0, 255.0) as u8;
            canvas[oi + 2] = (tb as f32 * alpha + canvas[oi + 2] as f32 * inv_a).clamp(0.0, 255.0) as u8;
        }
    }
}

#[allow(dead_code)]
/// Premultiplied src-over composite at a quadrant offset.
fn premultiplied_composite_at_offset(
    tex_rgba: &[u8],
    alpha_grid: &[f32; 33 * 33],
    canvas: &mut [u8],
    canvas_size: usize,
    offset_x: usize,
    offset_y: usize,
) {
    let tex_pixels = tex_rgba.len() / 4;
    let tex_size = (tex_pixels as f64).sqrt() as usize;
    if tex_size == 0 { return; }

    // Resize mask to TEXTURE size (not quadrant size) — this is the key to feathering.
    // The mask extends beyond the quadrant, providing smooth transitions.
    let mask = resample_mask_lanczos3(alpha_grid, tex_size, tex_size);

    for py in 0..tex_size {
        let out_y = offset_y + py;
        if out_y >= canvas_size { break; }

        for px in 0..tex_size {
            let out_x = offset_x + px;
            if out_x >= canvas_size { break; }

            let alpha = mask[py * tex_size + px].clamp(0.0, 1.0);
            if alpha < 0.001 { continue; }

            // Sample texture directly (1:1 mapping)
            let ti = (py * tex_size + px) * 4;
            let tr = tex_rgba[ti] as f32;
            let tg = tex_rgba[ti + 1] as f32;
            let tb = tex_rgba[ti + 2] as f32;

            // Premultiplied src-over: dst = src*alpha + dst*(1-alpha)
            let inv_a = 1.0 - alpha;
            let oi = (out_y * canvas_size + out_x) * 4;
            canvas[oi]     = (tr * alpha + canvas[oi] as f32 * inv_a).clamp(0.0, 255.0) as u8;
            canvas[oi + 1] = (tg * alpha + canvas[oi + 1] as f32 * inv_a).clamp(0.0, 255.0) as u8;
            canvas[oi + 2] = (tb * alpha + canvas[oi + 2] as f32 * inv_a).clamp(0.0, 255.0) as u8;
        }
    }
}

/// Alpha-blend a texture layer into a cell region using cell-sized alpha mask with quadrant offset.
///
/// Matches xLODGen's BlendLayerInto: the 33×33 alpha mask is resampled across the full cell,
/// but the texture is sampled with a quadrant offset. This allows alpha to feather beyond the
/// quadrant boundary for smoother transitions between adjacent quadrants.
fn alpha_blend_texture_cell(
    tex_rgba: &[u8],
    alpha_grid: &[f32; 33 * 33],
    canvas: &mut [u8],
    canvas_size: usize,
    cell_x: usize,
    cell_y: usize,
    cell_size: usize,
    quad_off_x: usize,
    quad_off_y: usize,
) {
    let tex_pixels = tex_rgba.len() / 4;
    let tex_size = (tex_pixels as f64).sqrt() as usize;
    if tex_size == 0 { return; }

    let ts = tex_size as f32;

    for py in 0..cell_size {
        let out_y = cell_y + py;
        if out_y >= canvas_size { break; }

        // Texture source Y with quadrant offset — skip if out of texture bounds
        let sy_raw = py + quad_off_y;
        if sy_raw >= cell_size { continue; }

        for px in 0..cell_size {
            let out_x = cell_x + px;
            if out_x >= canvas_size { break; }

            // Texture source X with quadrant offset — skip if out of texture bounds
            let sx_raw = px + quad_off_x;
            if sx_raw >= cell_size { continue; }

            // Sample alpha (bilinear from grid)
            let gx = (px as f32 / cell_size.max(1) as f32 * 32.0).clamp(0.0, 32.0);
            let gy = (py as f32 / cell_size.max(1) as f32 * 32.0).clamp(0.0, 32.0);
            let gx0 = (gx as usize).min(31);
            let gy0 = (gy as usize).min(31);
            let fx = gx - gx0 as f32;
            let fy = gy - gy0 as f32;
            let alpha = alpha_grid[gy0 * 33 + gx0] * (1.0 - fx) * (1.0 - fy)
                + alpha_grid[gy0 * 33 + gx0 + 1] * fx * (1.0 - fy)
                + alpha_grid[(gy0 + 1) * 33 + gx0] * (1.0 - fx) * fy
                + alpha_grid[(gy0 + 1) * 33 + gx0 + 1] * fx * fy;
            if alpha < 0.001 { continue; }

            // Sample texture at quadrant-offset position
            let sx_f = (sx_raw as f32 * ts / cell_size as f32) % ts;
            let sy_f = (sy_raw as f32 * ts / cell_size as f32) % ts;
            let (tr, tg, tb) = bilinear_sample(tex_rgba, tex_size, sx_f, sy_f);

            let oi = (out_y * canvas_size + out_x) * 4;
            let a = alpha.clamp(0.0, 1.0);
            let inv_a = 1.0 - a;
            canvas[oi] = (canvas[oi] as f32 * inv_a + tr as f32 * a) as u8;
            canvas[oi + 1] = (canvas[oi + 1] as f32 * inv_a + tg as f32 * a) as u8;
            canvas[oi + 2] = (canvas[oi + 2] as f32 * inv_a + tb as f32 * a) as u8;
        }
    }
}

/// Blend terrain textures for a single LOD block using pre-loaded texture cache.
///
/// Matches xLODGen's algorithm:
/// 1. Fill with default landscape texture
/// 2. Paint BTXT base textures per quadrant
/// 3. Build 33×33 alpha grids with square/diamond kernel convolution from VTXT data
/// 4. Alpha-blend ATXT overlay layers using the 33×33 grids
/// 5. Apply VCLR vertex colors block-wide with bilinear interpolation
/// 6. Horizontal flip for FNV engine UV convention
fn blend_terrain_block_cached(
    cells: &HashMap<(i32, i32), TerrainCell>,
    tex_cache: &HashMap<String, Vec<u8>>,
    ltex_map: &HashMap<u32, String>,
    block_x: i32,
    block_y: i32,
    step: i32,
    tex_size: u32,
    brightness: i32,
    gamma: f32,
) -> (Vec<u8>, Vec<u8>) {
    let size = tex_size as usize;
    let mut diffuse = vec![0u8; size * size * 4];
    let mut normal = vec![0u8; size * size * 4];

    // Fill normal map default
    for pixel in normal.chunks_exact_mut(4) {
        pixel[0] = 128; pixel[1] = 128; pixel[2] = 255; pixel[3] = 255;
    }

    let cells_per_side = step as usize;
    let pixels_per_cell = size / cells_per_side;

    // Match xLODGen's compositing: cell canvas = texture cache size (128×128).
    // Each quadrant is half the cell canvas. ATXT layers are composited at
    // the quadrant offset with their FULL texture size (128×128), which
    // overflows beyond the quadrant boundary into adjacent quadrants.
    // This overflow is the key to smooth cross-quadrant feathering.
    // After compositing, area-downsample to the output cell size.
    let comp_cell_size = 128usize; // Match texture cache size
    let comp_quad_size = comp_cell_size / 2; // 64

    let default_tex_path = ltex_map.get(&0x00015457)
        .cloned()
        .unwrap_or_else(|| "Landscape\\DirtWasteland01.dds".to_string());
    let default_tex = tex_cache.get(&default_tex_path.to_lowercase());

    // Fill entire block with default landscape texture for cells without data.
    if let Some(dtex) = default_tex {
        tile_texture_to_region(dtex, &mut diffuse, size, 0, 0, size);
    } else {
        for pixel in diffuse.chunks_exact_mut(4) {
            pixel[0] = 128; pixel[1] = 120; pixel[2] = 100; pixel[3] = 255;
        }
    }

    // Composite each cell at high resolution, then downsample into the block.
    for dy in 0..step {
        for dx in 0..step {
            let cx = block_x + dx;
            let cy = block_y + dy;

            let cell = match cells.get(&(cx, cy)) {
                Some(c) => c,
                None => continue,
            };

            let cell_px_x = dx as usize * pixels_per_cell;
            let cell_px_y = dy as usize * pixels_per_cell;

            // Create high-res cell canvas for compositing
            let mut cell_diffuse = vec![0u8; comp_cell_size * comp_cell_size * 4];

            // Fill cell with default texture
            if let Some(dtex) = default_tex {
                tile_texture_to_region(dtex, &mut cell_diffuse, comp_cell_size, 0, 0, comp_cell_size);
            } else {
                for pixel in cell_diffuse.chunks_exact_mut(4) {
                    pixel[0] = 128; pixel[1] = 120; pixel[2] = 100; pixel[3] = 255;
                }
            }

            // Pass 1: Paint BTXT base textures per quadrant.
            // Use tile_texture_to_region to fill each quadrant exactly.
            for quad in 0..4u8 {
                let qx = if quad == 1 || quad == 3 { comp_quad_size } else { 0 };
                let qy = if quad == 2 || quad == 3 { comp_quad_size } else { 0 };

                let primary_path = cell.base_textures[quad as usize]
                    .and_then(|fid| ltex_map.get(&fid));
                let fallback_path = cell.fallback_btxt[quad as usize]
                    .and_then(|fid| ltex_map.get(&fid));

                let btxt_tex = primary_path
                    .and_then(|p| tex_cache.get(&p.to_lowercase()))
                    .or_else(|| fallback_path.and_then(|p| tex_cache.get(&p.to_lowercase())))
                    .or_else(|| tex_cache.get(&default_tex_path.to_lowercase()));

                if let Some(tex_data) = btxt_tex {
                    tile_texture_to_region(
                        tex_data, &mut cell_diffuse, comp_cell_size,
                        qx, qy, comp_quad_size,
                    );
                }
            }

            // Pass 2: Overlay ATXT alpha layers with cell-wide masks.
            // Each quadrant's 33×33 mask is embedded into a 65×65 cell-wide grid
            // at its correct quadrant position, then Lanczos3-upscaled to cell size.
            // This creates natural bidirectional feathering at ALL quadrant boundaries —
            // the Lanczos3 filter smooths the mask edges where the quadrant data
            // meets the zero-padded adjacent regions.
            for quad in 0..4u8 {
                for (a_quad, a_layer, ref entries) in &cell.alpha_layers {
                    if *a_quad != quad { continue; }
                    let atxt_fid = cell.additional_textures.iter()
                        .find(|(_, q, l)| *q == quad && *l == *a_layer)
                        .map(|(fid, _, _)| *fid);
                    if let Some(fid) = atxt_fid {
                        if let Some(tex_path) = ltex_map.get(&fid) {
                            if let Some(tex_data) = tex_cache.get(&tex_path.to_lowercase()) {
                                let alpha_grid = build_alpha_grid_33(entries);
                                // Embed quadrant mask into cell-wide 65×65 grid
                                let cell_mask = embed_quad_mask_in_cell(&alpha_grid, quad);
                                // Lanczos3 upscale to cell pixel size
                                let mask = lanczos3_resize_f32(&cell_mask, 65, 65, comp_cell_size, comp_cell_size);
                                // Composite texture using cell-wide mask at (0,0)
                                composite_with_cellwide_mask(
                                    tex_data, &mask,
                                    &mut cell_diffuse, comp_cell_size,
                                );
                            }
                        }
                    }
                }
            }

            // Downsample the composited cell to the output cell size
            if comp_cell_size > pixels_per_cell {
                let downsampled = area_downsample_rgba(&cell_diffuse, comp_cell_size, pixels_per_cell);
                copy_cell_to_block(&downsampled, pixels_per_cell, &mut diffuse, size, cell_px_x, cell_px_y);
            } else {
                copy_cell_to_block(&cell_diffuse, pixels_per_cell, &mut diffuse, size, cell_px_x, cell_px_y);
            }

            // Write vertex normals
            if let Some(ref vnml) = cell.vertex_normals {
                write_vertex_normals(
                    vnml, &mut normal, size,
                    cell_px_x, cell_px_y, pixels_per_cell,
                );
            }
        }
    }

    // Apply VCLR as a block-wide pass with bilinear interpolation.
    apply_block_vclr(
        cells, &mut diffuse, size,
        block_x, block_y, step,
        brightness, gamma,
    );

    // FNV terrain LOD: horizontal flip for engine UV convention.
    flip_horizontal(&mut diffuse, size);
    flip_horizontal(&mut normal, size);

    (diffuse, normal)
}


/// Area-average downsample an RGBA image from src_size to dst_size.
///
/// Each output pixel averages a (scale × scale) block of source pixels.
/// This preserves color accuracy while eliminating high-frequency detail
/// and alpha blending artifacts from the high-res compositing pass.
fn area_downsample_rgba(src: &[u8], src_size: usize, dst_size: usize) -> Vec<u8> {
    if src_size == dst_size { return src.to_vec(); }
    let scale = src_size / dst_size;
    if scale == 0 { return src.to_vec(); }
    let mut dst = vec![0u8; dst_size * dst_size * 4];
    let n = (scale * scale) as u32;

    for dy in 0..dst_size {
        for dx in 0..dst_size {
            let mut r = 0u32;
            let mut g = 0u32;
            let mut b = 0u32;
            let mut a = 0u32;
            for sy in 0..scale {
                for sx in 0..scale {
                    let si = ((dy * scale + sy) * src_size + (dx * scale + sx)) * 4;
                    r += src[si] as u32;
                    g += src[si + 1] as u32;
                    b += src[si + 2] as u32;
                    a += src[si + 3] as u32;
                }
            }
            let di = (dy * dst_size + dx) * 4;
            dst[di] = (r / n) as u8;
            dst[di + 1] = (g / n) as u8;
            dst[di + 2] = (b / n) as u8;
            dst[di + 3] = (a / n) as u8;
        }
    }
    dst
}

/// Copy a cell-sized RGBA buffer into a block canvas at the given position.
fn copy_cell_to_block(
    cell: &[u8],
    cell_size: usize,
    block: &mut [u8],
    block_size: usize,
    dst_x: usize,
    dst_y: usize,
) {
    for y in 0..cell_size {
        let src_offset = y * cell_size * 4;
        let dst_offset = ((dst_y + y) * block_size + dst_x) * 4;
        let row_bytes = cell_size * 4;
        block[dst_offset..dst_offset + row_bytes]
            .copy_from_slice(&cell[src_offset..src_offset + row_bytes]);
    }
}

/// Apply a Gaussian-approximation blur to an RGBA canvas.
///
/// Uses 3 passes of separable box blur which closely approximates a Gaussian.
/// Only blurs RGB channels; alpha is left unchanged.
fn gaussian_blur_rgba(canvas: &mut [u8], size: usize, radius: i32) {
    if radius <= 0 { return; }
    let r = radius;

    for _pass in 0..3 {
        // Horizontal pass
        let src = canvas.to_vec();
        for y in 0..size {
            for x in 0..size {
                let mut rs = 0u32;
                let mut gs = 0u32;
                let mut bs = 0u32;
                let mut count = 0u32;
                for dx in -r..=r {
                    let nx = x as i32 + dx;
                    if nx >= 0 && nx < size as i32 {
                        let si = (y * size + nx as usize) * 4;
                        rs += src[si] as u32;
                        gs += src[si + 1] as u32;
                        bs += src[si + 2] as u32;
                        count += 1;
                    }
                }
                let oi = (y * size + x) * 4;
                canvas[oi] = (rs / count) as u8;
                canvas[oi + 1] = (gs / count) as u8;
                canvas[oi + 2] = (bs / count) as u8;
            }
        }
        // Vertical pass
        let src = canvas.to_vec();
        for y in 0..size {
            for x in 0..size {
                let mut rs = 0u32;
                let mut gs = 0u32;
                let mut bs = 0u32;
                let mut count = 0u32;
                for dy in -r..=r {
                    let ny = y as i32 + dy;
                    if ny >= 0 && ny < size as i32 {
                        let si = (ny as usize * size + x) * 4;
                        rs += src[si] as u32;
                        gs += src[si + 1] as u32;
                        bs += src[si + 2] as u32;
                        count += 1;
                    }
                }
                let oi = (y * size + x) * 4;
                canvas[oi] = (rs / count) as u8;
                canvas[oi + 1] = (gs / count) as u8;
                canvas[oi + 2] = (bs / count) as u8;
            }
        }
    }
}

/// Smooth cell and quadrant boundary artifacts.
///
/// After BTXT hard fills and ATXT alpha blending, transitions between adjacent
/// cells/quadrants can show hard rectangular edges where different textures meet.
/// This applies a localized 2D box blur in strips along boundary lines to soften
/// the transitions, matching xLODGen's smoother output.
fn smooth_boundaries(
    canvas: &mut [u8],
    canvas_size: usize,
    cells_per_side: usize,
    pixels_per_cell: usize,
    pixels_per_quad: usize,
) {
    // Radius of smoothing around each boundary line (in pixels).
    // Larger radius = smoother transitions but more blurring.
    let radius: usize = pixels_per_quad;
    let blur_r = radius as i32;

    // Make a copy for reading source pixels during blur
    let src = canvas.to_vec();

    // Collect all boundary X and Y coordinates (cell and quadrant boundaries)
    let mut boundary_xs: Vec<usize> = Vec::new();
    let mut boundary_ys: Vec<usize> = Vec::new();

    for i in 0..=cells_per_side {
        let cell_edge = i * pixels_per_cell;
        if cell_edge > 0 && cell_edge < canvas_size {
            boundary_xs.push(cell_edge);
            boundary_ys.push(cell_edge);
        }
        // Quadrant boundaries (midpoints within cells)
        let quad_edge = i * pixels_per_cell + pixels_per_quad;
        if quad_edge > 0 && quad_edge < canvas_size {
            boundary_xs.push(quad_edge);
            boundary_ys.push(quad_edge);
        }
    }

    // Apply horizontal smoothing along vertical boundary lines
    for &bx in &boundary_xs {
        let x_start = (bx as i32 - blur_r).max(0) as usize;
        let x_end = ((bx as i32 + blur_r) as usize).min(canvas_size - 1);
        for y in 0..canvas_size {
            for x in x_start..=x_end {
                // Distance-based weight: stronger blur near boundary, less at edges
                let dist = (x as i32 - bx as i32).unsigned_abs() as usize;
                let weight = 1.0 - (dist as f32 / (radius as f32 + 1.0));
                if weight <= 0.0 { continue; }

                // Box blur sample
                let mut r_sum = 0u32;
                let mut g_sum = 0u32;
                let mut b_sum = 0u32;
                let mut count = 0u32;
                for dx in -blur_r..=blur_r {
                    let sx = x as i32 + dx;
                    if sx < 0 || sx >= canvas_size as i32 { continue; }
                    let si = (y * canvas_size + sx as usize) * 4;
                    r_sum += src[si] as u32;
                    g_sum += src[si + 1] as u32;
                    b_sum += src[si + 2] as u32;
                    count += 1;
                }
                if count == 0 { continue; }

                let oi = (y * canvas_size + x) * 4;
                let blurred_r = (r_sum / count) as f32;
                let blurred_g = (g_sum / count) as f32;
                let blurred_b = (b_sum / count) as f32;
                canvas[oi] = (src[oi] as f32 * (1.0 - weight) + blurred_r * weight) as u8;
                canvas[oi + 1] = (src[oi + 1] as f32 * (1.0 - weight) + blurred_g * weight) as u8;
                canvas[oi + 2] = (src[oi + 2] as f32 * (1.0 - weight) + blurred_b * weight) as u8;
            }
        }
    }

    // Update source for vertical pass
    let src2 = canvas.to_vec();

    // Apply vertical smoothing along horizontal boundary lines
    for &by in &boundary_ys {
        let y_start = (by as i32 - blur_r).max(0) as usize;
        let y_end = ((by as i32 + blur_r) as usize).min(canvas_size - 1);
        for x in 0..canvas_size {
            for y in y_start..=y_end {
                let dist = (y as i32 - by as i32).unsigned_abs() as usize;
                let weight = 1.0 - (dist as f32 / (radius as f32 + 1.0));
                if weight <= 0.0 { continue; }

                let mut r_sum = 0u32;
                let mut g_sum = 0u32;
                let mut b_sum = 0u32;
                let mut count = 0u32;
                for dy in -blur_r..=blur_r {
                    let sy = y as i32 + dy;
                    if sy < 0 || sy >= canvas_size as i32 { continue; }
                    let si = (sy as usize * canvas_size + x) * 4;
                    r_sum += src2[si] as u32;
                    g_sum += src2[si + 1] as u32;
                    b_sum += src2[si + 2] as u32;
                    count += 1;
                }
                if count == 0 { continue; }

                let oi = (y * canvas_size + x) * 4;
                let blurred_r = (r_sum / count) as f32;
                let blurred_g = (g_sum / count) as f32;
                let blurred_b = (b_sum / count) as f32;
                canvas[oi] = (src2[oi] as f32 * (1.0 - weight) + blurred_r * weight) as u8;
                canvas[oi + 1] = (src2[oi + 1] as f32 * (1.0 - weight) + blurred_g * weight) as u8;
                canvas[oi + 2] = (src2[oi + 2] as f32 * (1.0 - weight) + blurred_b * weight) as u8;
            }
        }
    }
}

/// Flip an RGBA canvas horizontally in-place (mirror left↔right).
fn flip_horizontal(canvas: &mut [u8], size: usize) {
    for y in 0..size {
        let row = y * size * 4;
        for x in 0..size / 2 {
            let left = row + x * 4;
            let right = row + (size - 1 - x) * 4;
            for c in 0..4 {
                canvas.swap(left + c, right + c);
            }
        }
    }
}

#[allow(dead_code)]
/// Flip an RGBA canvas vertically in-place (mirror top↔bottom).
fn flip_vertical(canvas: &mut [u8], size: usize) {
    let row_bytes = size * 4;
    let mut temp = vec![0u8; row_bytes];
    for y in 0..size / 2 {
        let top = y * row_bytes;
        let bot = (size - 1 - y) * row_bytes;
        temp.copy_from_slice(&canvas[top..top + row_bytes]);
        canvas.copy_within(bot..bot + row_bytes, top);
        canvas[bot..bot + row_bytes].copy_from_slice(&temp);
    }
}

/// Blit a texture at an offset in the canvas (opaque copy, clipping at canvas bounds).
///
/// Matches xLODGen's BTXT fill: the full texture is placed at the quadrant offset.
/// Pixels beyond the canvas boundary are clipped. Each quadrant overwrites the
/// previous one, so the final result has each quadrant showing the top-left portion
/// of its texture at full tile resolution.
fn blit_texture_at_offset(
    tex_rgba: &[u8],
    canvas: &mut [u8],
    canvas_size: usize,
    offset_x: usize,
    offset_y: usize,
) {
    let tex_pixels = tex_rgba.len() / 4;
    let tex_size = (tex_pixels as f64).sqrt() as usize;
    if tex_size == 0 { return; }

    for py in 0..tex_size {
        let out_y = offset_y + py;
        if out_y >= canvas_size { break; }
        for px in 0..tex_size {
            let out_x = offset_x + px;
            if out_x >= canvas_size { break; }
            let ti = (py * tex_size + px) * 4;
            let oi = (out_y * canvas_size + out_x) * 4;
            canvas[oi]     = tex_rgba[ti];
            canvas[oi + 1] = tex_rgba[ti + 1];
            canvas[oi + 2] = tex_rgba[ti + 2];
            canvas[oi + 3] = 255;
        }
    }
}

/// Tile a texture into a region of the output canvas using bilinear sampling.
fn tile_texture_to_region(
    tex_rgba: &[u8],
    canvas: &mut [u8],
    canvas_size: usize,
    region_x: usize,
    region_y: usize,
    region_size: usize,
) {
    let tex_pixels = tex_rgba.len() / 4;
    let tex_size = (tex_pixels as f64).sqrt() as usize;
    if tex_size == 0 { return; }

    let ts = tex_size as f32;
    let rs = region_size as f32;

    for py in 0..region_size {
        for px in 0..region_size {
            let sx_f = (px as f32 * ts / rs) % ts;
            let sy_f = (py as f32 * ts / rs) % ts;

            let (r, g, b) = bilinear_sample(tex_rgba, tex_size, sx_f, sy_f);

            let out_x = region_x + px;
            let out_y = region_y + py;
            let oi = (out_y * canvas_size + out_x) * 4;

            if oi + 3 < canvas.len() {
                canvas[oi] = r;
                canvas[oi + 1] = g;
                canvas[oi + 2] = b;
                canvas[oi + 3] = 255;
            }
        }
    }
}

/// Bilinear sample from an RGBA texture at fractional coordinates.
#[inline]
fn bilinear_sample(tex: &[u8], tex_size: usize, x: f32, y: f32) -> (u8, u8, u8) {
    let x0 = (x as usize).min(tex_size - 1);
    let y0 = (y as usize).min(tex_size - 1);
    let x1 = (x0 + 1) % tex_size;
    let y1 = (y0 + 1) % tex_size;
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;

    let i00 = (y0 * tex_size + x0) * 4;
    let i10 = (y0 * tex_size + x1) * 4;
    let i01 = (y1 * tex_size + x0) * 4;
    let i11 = (y1 * tex_size + x1) * 4;

    let max_i = tex.len().saturating_sub(4);
    let (i00, i10, i01, i11) = (i00.min(max_i), i10.min(max_i), i01.min(max_i), i11.min(max_i));

    let lerp = |c: usize| -> u8 {
        let v00 = tex[i00 + c] as f32;
        let v10 = tex[i10 + c] as f32;
        let v01 = tex[i01 + c] as f32;
        let v11 = tex[i11 + c] as f32;
        let top = v00 * (1.0 - fx) + v10 * fx;
        let bot = v01 * (1.0 - fx) + v11 * fx;
        (top * (1.0 - fy) + bot * fy) as u8
    };

    (lerp(0), lerp(1), lerp(2))
}

/// Apply VCLR to a single cell's region in the canvas.
///
/// Matches xLODGen's per-cell VCLR application: the 33×33 VCLR grid is bilinearly
/// sampled to cover the cell's pixel region, and each pixel is multiplied by the
/// interpolated vertex color. xLODGen expands the VCLR bitmap by 33/32 ratio
/// before sampling; we achieve the same by mapping the full 0..32 vertex range
/// across the cell pixels.
fn apply_cell_vclr(
    vclr: &[u8],
    canvas: &mut [u8],
    canvas_size: usize,
    cell_x: usize,
    cell_y: usize,
    cell_size: usize,
    brightness: i32,
    gamma: f32,
) {
    let apply_gamma = (gamma - 1.0).abs() > 0.01;

    for py in 0..cell_size {
        let out_y = cell_y + py;
        if out_y >= canvas_size { break; }

        for px in 0..cell_size {
            let out_x = cell_x + px;
            if out_x >= canvas_size { break; }

            // Map pixel to VCLR grid position (33 vertices across cell)
            let vx_f = px as f32 * 32.0 / (cell_size - 1) as f32;
            let vy_f = py as f32 * 32.0 / (cell_size - 1) as f32;

            let vx0 = (vx_f as usize).min(31);
            let vy0 = (vy_f as usize).min(31);
            let vx1 = vx0 + 1;
            let vy1 = vy0 + 1;
            let fx = vx_f - vx0 as f32;
            let fy = vy_f - vy0 as f32;

            // Bilinear VCLR for each channel
            let vi00 = (vy0 * 33 + vx0) * 3;
            let vi10 = (vy0 * 33 + vx1) * 3;
            let vi01 = (vy1 * 33 + vx0) * 3;
            let vi11 = (vy1 * 33 + vx1) * 3;

            let r_vc = (vclr[vi00] as f32 * (1.0 - fx) * (1.0 - fy)
                + vclr[vi10] as f32 * fx * (1.0 - fy)
                + vclr[vi01] as f32 * (1.0 - fx) * fy
                + vclr[vi11] as f32 * fx * fy) / 255.0;
            let g_vc = (vclr[vi00 + 1] as f32 * (1.0 - fx) * (1.0 - fy)
                + vclr[vi10 + 1] as f32 * fx * (1.0 - fy)
                + vclr[vi01 + 1] as f32 * (1.0 - fx) * fy
                + vclr[vi11 + 1] as f32 * fx * fy) / 255.0;
            let b_vc = (vclr[vi00 + 2] as f32 * (1.0 - fx) * (1.0 - fy)
                + vclr[vi10 + 2] as f32 * fx * (1.0 - fy)
                + vclr[vi01 + 2] as f32 * (1.0 - fx) * fy
                + vclr[vi11 + 2] as f32 * fx * fy) / 255.0;

            let oi = (out_y * canvas_size + out_x) * 4;
            if oi + 3 >= canvas.len() { continue; }

            let mut r = (canvas[oi] as f32 * r_vc) as i32;
            let mut g = (canvas[oi + 1] as f32 * g_vc) as i32;
            let mut b = (canvas[oi + 2] as f32 * b_vc) as i32;

            r = (r + brightness).clamp(0, 255);
            g = (g + brightness).clamp(0, 255);
            b = (b + brightness).clamp(0, 255);

            if apply_gamma {
                r = (((r as f32 / 255.0).powf(1.0 / gamma)) * 255.0) as i32;
                g = (((g as f32 / 255.0).powf(1.0 / gamma)) * 255.0) as i32;
                b = (((b as f32 / 255.0).powf(1.0 / gamma)) * 255.0) as i32;
            }

            canvas[oi] = r.clamp(0, 255) as u8;
            canvas[oi + 1] = g.clamp(0, 255) as u8;
            canvas[oi + 2] = b.clamp(0, 255) as u8;
        }
    }
}

#[allow(dead_code)]
/// Apply vertex colors across an entire block with bilinear interpolation.
///
/// Builds a block-wide VCLR grid from all cells and applies it smoothly,
/// avoiding hard cell-boundary artifacts. At shared cell borders, VCLR values
/// from adjacent cells are averaged for seamless transitions.
fn apply_block_vclr(
    cells: &HashMap<(i32, i32), TerrainCell>,
    canvas: &mut [u8],
    canvas_size: usize,
    block_x: i32,
    block_y: i32,
    step: i32,
    brightness: i32,
    gamma: f32,
) {
    let n = step as usize;
    // Block-wide VCLR grid: each cell has 33 vertices (0..=32), but adjacent
    // cells share border vertices. Total: n*32+1 per side.
    let grid_w = n * 32 + 1;
    let grid_size = grid_w * grid_w;
    let mut vclr_r = vec![255.0f32; grid_size];
    let mut vclr_g = vec![255.0f32; grid_size];
    let mut vclr_b = vec![255.0f32; grid_size];
    let mut vclr_count = vec![0u8; grid_size]; // track how many cells contribute

    // Populate the grid from each cell's VCLR.
    // Only apply VCLR to cells that have actual landscape texture data (BTXT).
    // Cells without BTXT use the default texture fill and should keep neutral
    // vertex colors (255,255,255) to avoid crushing them to near-black.
    for dy in 0..step {
        for dx in 0..step {
            let cx = block_x + dx;
            let cy = block_y + dy;
            let cell = match cells.get(&(cx, cy)) {
                Some(c) => c,
                None => continue,
            };
            let vclr = match cell.vertex_colors.as_ref() {
                Some(v) if v.len() >= 33 * 33 * 3 => v,
                _ => continue,
            };

            let base_gx = dx as usize * 32;
            let base_gy = dy as usize * 32;

            for vy in 0..33usize {
                for vx in 0..33usize {
                    let vi = (vy * 33 + vx) * 3;
                    let gx = base_gx + vx;
                    let gy = base_gy + vy;
                    let gi = gy * grid_w + gx;

                    if vclr_count[gi] == 0 {
                        vclr_r[gi] = vclr[vi] as f32;
                        vclr_g[gi] = vclr[vi + 1] as f32;
                        vclr_b[gi] = vclr[vi + 2] as f32;
                        vclr_count[gi] = 1;
                    } else {
                        // Average with existing value at shared border vertex
                        let c = vclr_count[gi] as f32;
                        vclr_r[gi] = (vclr_r[gi] * c + vclr[vi] as f32) / (c + 1.0);
                        vclr_g[gi] = (vclr_g[gi] * c + vclr[vi + 1] as f32) / (c + 1.0);
                        vclr_b[gi] = (vclr_b[gi] * c + vclr[vi + 2] as f32) / (c + 1.0);
                        vclr_count[gi] += 1;
                    }
                }
            }
        }
    }

    // Apply the block-wide VCLR grid to the canvas with bilinear interpolation
    let apply_gamma = (gamma - 1.0).abs() > 0.01;

    for py in 0..canvas_size {
        for px in 0..canvas_size {
            // Map pixel to grid position with sub-vertex precision
            let gx_f = px as f32 * (grid_w - 1) as f32 / (canvas_size - 1) as f32;
            let gy_f = py as f32 * (grid_w - 1) as f32 / (canvas_size - 1) as f32;

            let gx0 = (gx_f as usize).min(grid_w - 2);
            let gy0 = (gy_f as usize).min(grid_w - 2);
            let fx = gx_f - gx0 as f32;
            let fy = gy_f - gy0 as f32;

            let i00 = gy0 * grid_w + gx0;
            let i10 = i00 + 1;
            let i01 = i00 + grid_w;
            let i11 = i01 + 1;

            // Bilinear interpolation
            let r_vc = (vclr_r[i00] * (1.0 - fx) * (1.0 - fy)
                + vclr_r[i10] * fx * (1.0 - fy)
                + vclr_r[i01] * (1.0 - fx) * fy
                + vclr_r[i11] * fx * fy) / 255.0;
            let g_vc = (vclr_g[i00] * (1.0 - fx) * (1.0 - fy)
                + vclr_g[i10] * fx * (1.0 - fy)
                + vclr_g[i01] * (1.0 - fx) * fy
                + vclr_g[i11] * fx * fy) / 255.0;
            let b_vc = (vclr_b[i00] * (1.0 - fx) * (1.0 - fy)
                + vclr_b[i10] * fx * (1.0 - fy)
                + vclr_b[i01] * (1.0 - fx) * fy
                + vclr_b[i11] * fx * fy) / 255.0;

            let oi = (py * canvas_size + px) * 4;
            if oi + 3 >= canvas.len() { continue; }

            let mut r = (canvas[oi] as f32 * r_vc) as i32;
            let mut g = (canvas[oi + 1] as f32 * g_vc) as i32;
            let mut b = (canvas[oi + 2] as f32 * b_vc) as i32;

            r = (r + brightness).clamp(0, 255);
            g = (g + brightness).clamp(0, 255);
            b = (b + brightness).clamp(0, 255);

            if apply_gamma {
                r = (((r as f32 / 255.0).powf(1.0 / gamma)) * 255.0) as i32;
                g = (((g as f32 / 255.0).powf(1.0 / gamma)) * 255.0) as i32;
                b = (((b as f32 / 255.0).powf(1.0 / gamma)) * 255.0) as i32;
            }

            canvas[oi] = r.clamp(0, 255) as u8;
            canvas[oi + 1] = g.clamp(0, 255) as u8;
            canvas[oi + 2] = b.clamp(0, 255) as u8;
        }
    }
}

/// Write vertex normals to the normal map canvas.
fn write_vertex_normals(
    vnml: &[u8],
    canvas: &mut [u8],
    canvas_size: usize,
    cell_px_x: usize,
    cell_px_y: usize,
    pixels_per_cell: usize,
) {
    for py in 0..pixels_per_cell {
        for px in 0..pixels_per_cell {
            let vx = (px * 32) / pixels_per_cell;
            let vy = (py * 32) / pixels_per_cell;
            let vi = (vy * 33 + vx) * 3;
            if vi + 2 >= vnml.len() { continue; }

            let out_x = cell_px_x + px;
            let out_y = cell_px_y + py;
            let oi = (out_y * canvas_size + out_x) * 4;
            if oi + 3 >= canvas.len() { continue; }

            canvas[oi] = vnml[vi];
            canvas[oi + 1] = vnml[vi + 1];
            canvas[oi + 2] = vnml[vi + 2];
            canvas[oi + 3] = 255;
        }
    }
}

/// Scan plugins for LTEX (Landscape Texture) records.
///
/// Returns a map of FormID -> texture path.
pub fn scan_landscape_textures(
    plugins: &[Plugin],
) -> HashMap<u32, String> {
    let mut ltex_map = HashMap::new();
    let sig_ltex = Signature(*b"LTEX");
    let sig_tnam = Signature(*b"TNAM");
    let sig_txst = Signature(*b"TXST");
    let sig_tx00 = Signature(*b"TX00");

    // Build plugin name list for FormID resolution
    let plugin_names: Vec<String> = plugins.iter()
        .map(|p| plugin_filename(p))
        .collect();

    // First pass: collect LTEX -> TXST mappings
    let mut ltex_to_txst: HashMap<u32, u32> = HashMap::new();
    let mut txst_textures: HashMap<u32, String> = HashMap::new();

    for (pi, plugin) in plugins.iter().enumerate() {
        for record in plugin.all_records() {
            if record.signature == sig_ltex {
                let form_id = resolve_formid(
                    record.form_id.raw(), &plugin.masters, &plugin_names, pi,
                );
                if let Some(tnam) = record.subrecords.iter().find(|sr| sr.signature == sig_tnam) {
                    if tnam.raw_data.len() >= 4 {
                        let raw_txst = u32::from_le_bytes([
                            tnam.raw_data[0], tnam.raw_data[1],
                            tnam.raw_data[2], tnam.raw_data[3],
                        ]);
                        let txst_fid = resolve_formid(raw_txst, &plugin.masters, &plugin_names, pi);
                        ltex_to_txst.insert(form_id, txst_fid);
                    }
                }

                let sig_icon = Signature(*b"ICON");
                if let Some(icon) = record.subrecords.iter().find(|sr| sr.signature == sig_icon) {
                    let tex_path = String::from_utf8_lossy(&icon.raw_data)
                        .trim_end_matches('\0')
                        .to_string();
                    if !tex_path.is_empty() {
                        ltex_map.insert(form_id, tex_path);
                    }
                }
            } else if record.signature == sig_txst {
                let form_id = resolve_formid(
                    record.form_id.raw(), &plugin.masters, &plugin_names, pi,
                );
                // TX00 is the diffuse texture path
                if let Some(tx00) = record.subrecords.iter().find(|sr| sr.signature == sig_tx00) {
                    let tex_path = String::from_utf8_lossy(&tx00.raw_data)
                        .trim_end_matches('\0')
                        .to_string();
                    if !tex_path.is_empty() {
                        txst_textures.insert(form_id, tex_path);
                    }
                }
            }
        }
    }

    // Resolve LTEX -> TXST -> texture path
    for (ltex_fid, txst_fid) in &ltex_to_txst {
        if !ltex_map.contains_key(ltex_fid) {
            if let Some(tex) = txst_textures.get(txst_fid) {
                ltex_map.insert(*ltex_fid, tex.clone());
            }
        }
    }

    info!("Found {} landscape textures", ltex_map.len());
    ltex_map
}

/// Get the cell range from a set of terrain cells.
fn cell_range(cells: &HashMap<(i32, i32), TerrainCell>) -> (i32, i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;

    for (cx, cy) in cells.keys() {
        min_x = min_x.min(*cx);
        max_x = max_x.max(*cx);
        min_y = min_y.min(*cy);
        max_y = max_y.max(*cy);
    }

    (min_x, max_x, min_y, max_y)
}

/// Public wrapper for build_alpha_grid_33 (for testing/diagnostics).
pub fn build_alpha_grid_33_pub(entries: &[(u16, f32)]) -> [f32; 33 * 33] {
    build_alpha_grid_33(entries)
}

/// Align a coordinate to the nearest lower step boundary.
fn align_to_step(val: i32, step: i32) -> i32 {
    if val >= 0 {
        (val / step) * step
    } else {
        ((val - step + 1) / step) * step
    }
}

/// Run the complete terrain LOD generation pipeline.
pub fn generate_terrain_lod(
    worldspace: &str,
    output_dir: &Path,
    cells: &HashMap<(i32, i32), TerrainCell>,
    ltex_map: &HashMap<u32, String>,
    loader: &ResourceLoader,
    options: &crate::LodOptions,
    sw_cell: (i32, i32),
    lodgen_dll: &Path,
    progress: &crate::progress::Progress,
) -> Result<TerrainLodOutput> {
    // 1. Write binary terrain data
    progress.report("Writing terrain data...", 0.05);
    let terrain_bin_path = output_dir.join(format!("LODGen_Terrain_{}.bin", worldspace));
    write_terrain_data_bin(&terrain_bin_path, cells)?;
    info!("Wrote terrain binary: {} cells", cells.len());

    // 2. Write terrain LODGen config
    progress.report("Writing terrain LODGen config...", 0.10);
    let mesh_output = output_dir
        .join("meshes/landscape/lod")
        .join(worldspace);
    std::fs::create_dir_all(&mesh_output)?;

    let terrain_config_path = output_dir.join(format!("LODGen_Terrain_{}.txt", worldspace));
    write_terrain_lodgen_config(
        &terrain_config_path,
        worldspace,
        sw_cell,
        &terrain_bin_path,
        &mesh_output,
        options,
        -2300.0, // Default water height for FNV
        -2500.0, // Default land height for FNV
    )?;

    // 3. Call LODGen in terrain mode
    progress.report("Generating terrain meshes (LODGen)...", 0.15);
    crate::nif_combiner::run_lodgen(
        lodgen_dll,
        &terrain_config_path,
        Some(&|line| info!("LODGen: {}", line)),
    )?;

    // 4. Count generated mesh files
    let mesh_count = count_files_with_ext(&mesh_output, "nif");
    info!("Generated {} terrain mesh files", mesh_count);

    // 5. Generate terrain textures for each LOD level
    let mut texture_count = 0u32;
    let levels = [4u32, 8, 16, 32];
    for (i, &level) in levels.iter().enumerate() {
        if !options.terrain_build_diffuse && !options.terrain_build_normal {
            continue;
        }

        let level_progress = 0.20 + (i as f64 * 0.20);
        progress.report(&format!("Generating Level{} textures...", level), level_progress);

        let count = generate_terrain_textures(
            cells, worldspace, output_dir, loader, ltex_map,
            level,
            options.terrain_diffuse_size[i],
            options.terrain_diffuse_comp[i],
            options.terrain_normal_comp[i],
            options.terrain_diffuse_mipmap[i],
            options.terrain_normal_mipmap[i],
            options.terrain_brightness[i],
            options.terrain_contrast[i],
            options.terrain_gamma[i],
            progress,
        )?;
        texture_count += count;
    }

    progress.report("Terrain LOD complete", 1.0);

    Ok(TerrainLodOutput {
        cell_count: cells.len(),
        mesh_count,
        texture_count,
    })
}

/// Count files with a specific extension in a directory (recursive).
fn count_files_with_ext(dir: &Path, ext: &str) -> u32 {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(e) = path.extension() {
                    if e.eq_ignore_ascii_case(ext) {
                        count += 1;
                    }
                }
            } else if path.is_dir() {
                count += count_files_with_ext(&path, ext);
            }
        }
    }
    count
}

/// Output summary from terrain LOD generation.
#[derive(Debug)]
pub struct TerrainLodOutput {
    pub cell_count: usize,
    pub mesh_count: u32,
    pub texture_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_to_step() {
        assert_eq!(align_to_step(0, 4), 0);
        assert_eq!(align_to_step(3, 4), 0);
        assert_eq!(align_to_step(4, 4), 4);
        assert_eq!(align_to_step(-1, 4), -4);
        assert_eq!(align_to_step(-4, 4), -4);
        assert_eq!(align_to_step(-5, 4), -8);
        assert_eq!(align_to_step(0, 8), 0);
        assert_eq!(align_to_step(7, 8), 0);
        assert_eq!(align_to_step(-64, 32), -64);
    }

    #[test]
    fn test_cell_range() {
        let mut cells = HashMap::new();
        cells.insert((-10, -20), TerrainCell {
            cell_x: -10, cell_y: -20,
            water_height: 0.0, land_flags: 0,
            height_offset: 0.0, height_deltas: vec![],
            vhgt_footer: [0; 3],
            vertex_normals: None, vertex_colors: None,
            base_textures: [None; 4], fallback_btxt: [None; 4], additional_textures: vec![], alpha_layers: vec![],
        });
        cells.insert((30, 40), TerrainCell {
            cell_x: 30, cell_y: 40,
            water_height: 0.0, land_flags: 0,
            height_offset: 0.0, height_deltas: vec![],
            vhgt_footer: [0; 3],
            vertex_normals: None, vertex_colors: None,
            base_textures: [None; 4], fallback_btxt: [None; 4], additional_textures: vec![], alpha_layers: vec![],
        });

        let (min_x, max_x, min_y, max_y) = cell_range(&cells);
        assert_eq!(min_x, -10);
        assert_eq!(max_x, 30);
        assert_eq!(min_y, -20);
        assert_eq!(max_y, 40);
    }
}
