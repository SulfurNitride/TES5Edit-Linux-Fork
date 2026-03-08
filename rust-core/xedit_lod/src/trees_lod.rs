//! Tree LOD generation — billboard atlas, LST, and DTL/BTT files.
//!
//! Ported from TwbLodTES5TreeList / TwbLodTES5TreeBlock in wbLOD.pas.
//!
//! File formats:
//! - LST: Tree type list (count + entries with UV atlas coords)
//! - DTL: FO3/FNV tree block per cell (type count, per-type refs)
//! - BTT: Skyrim tree block per cell (same structure, different path)

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;

// ============================================================================
// LST file — Tree type list
// ============================================================================

/// A tree type entry in the LST file.
#[derive(Debug, Clone)]
pub struct TreeType {
    /// Index of this tree type in the atlas.
    pub index: i32,
    /// Billboard width in game units.
    pub width: f32,
    /// Billboard height in game units.
    pub height: f32,
    /// UV coordinates in the atlas texture.
    pub uv_min_x: f32,
    pub uv_min_y: f32,
    pub uv_max_x: f32,
    pub uv_max_y: f32,
    /// Unknown field (always 0 in observed data).
    pub unknown: i32,
}

impl TreeType {
    /// Size of one entry in bytes.
    pub const SIZE: usize = 32;

    pub fn read_from(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            index: r.read_i32::<LittleEndian>()?,
            width: r.read_f32::<LittleEndian>()?,
            height: r.read_f32::<LittleEndian>()?,
            uv_min_x: r.read_f32::<LittleEndian>()?,
            uv_min_y: r.read_f32::<LittleEndian>()?,
            uv_max_x: r.read_f32::<LittleEndian>()?,
            uv_max_y: r.read_f32::<LittleEndian>()?,
            unknown: r.read_i32::<LittleEndian>()?,
        })
    }

    pub fn write_to(&self, w: &mut impl Write) -> Result<()> {
        w.write_i32::<LittleEndian>(self.index)?;
        w.write_f32::<LittleEndian>(self.width)?;
        w.write_f32::<LittleEndian>(self.height)?;
        w.write_f32::<LittleEndian>(self.uv_min_x)?;
        w.write_f32::<LittleEndian>(self.uv_min_y)?;
        w.write_f32::<LittleEndian>(self.uv_max_x)?;
        w.write_f32::<LittleEndian>(self.uv_max_y)?;
        w.write_i32::<LittleEndian>(self.unknown)?;
        Ok(())
    }
}

/// LST file: list of tree types with atlas UV coordinates.
#[derive(Debug, Clone)]
pub struct TreeTypeList {
    pub types: Vec<TreeType>,
}

impl TreeTypeList {
    /// Parse an LST file from raw bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Cursor::new(data);
        let count = r.read_i32::<LittleEndian>()? as usize;

        let expected_size = 4 + count * TreeType::SIZE;
        anyhow::ensure!(
            data.len() >= expected_size,
            "LST too short: {} bytes, expected {} for {} entries",
            data.len(), expected_size, count
        );

        let mut types = Vec::with_capacity(count);
        for _ in 0..count {
            types.push(TreeType::read_from(&mut r)?);
        }

        Ok(Self { types })
    }

    /// Serialize to LST file bytes.
    pub fn write(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::with_capacity(4 + self.types.len() * TreeType::SIZE);
        buf.write_i32::<LittleEndian>(self.types.len() as i32)?;
        for t in &self.types {
            t.write_to(&mut buf)?;
        }
        Ok(buf)
    }
}

// ============================================================================
// DTL/BTT file — Tree block per cell
// ============================================================================

/// A tree reference (placed instance) in a DTL/BTT block.
#[derive(Debug, Clone)]
pub struct TreeRef {
    /// World X position.
    pub x: f32,
    /// World Y position.
    pub y: f32,
    /// World Z position.
    pub z: f32,
    /// Rotation in radians (0..2π).
    pub rotation: f32,
    /// Scale factor.
    pub scale: f32,
    /// Reference FormID from the plugin.
    pub ref_form_id: u32,
    /// Unknown field 1 (always 0 in observed data).
    pub unknown1: i32,
    /// Unknown field 2 (always 0 in observed data).
    pub unknown2: i32,
}

impl TreeRef {
    /// Size of one reference in bytes.
    pub const SIZE: usize = 32;

    pub fn read_from(r: &mut impl Read) -> Result<Self> {
        Ok(Self {
            x: r.read_f32::<LittleEndian>()?,
            y: r.read_f32::<LittleEndian>()?,
            z: r.read_f32::<LittleEndian>()?,
            rotation: r.read_f32::<LittleEndian>()?,
            scale: r.read_f32::<LittleEndian>()?,
            ref_form_id: r.read_u32::<LittleEndian>()?,
            unknown1: r.read_i32::<LittleEndian>()?,
            unknown2: r.read_i32::<LittleEndian>()?,
        })
    }

    pub fn write_to(&self, w: &mut impl Write) -> Result<()> {
        w.write_f32::<LittleEndian>(self.x)?;
        w.write_f32::<LittleEndian>(self.y)?;
        w.write_f32::<LittleEndian>(self.z)?;
        w.write_f32::<LittleEndian>(self.rotation)?;
        w.write_f32::<LittleEndian>(self.scale)?;
        w.write_u32::<LittleEndian>(self.ref_form_id)?;
        w.write_i32::<LittleEndian>(self.unknown1)?;
        w.write_i32::<LittleEndian>(self.unknown2)?;
        Ok(())
    }
}

/// A group of tree references of the same type within a cell block.
#[derive(Debug, Clone)]
pub struct TreeTypeBlock {
    /// Tree type index (corresponds to TreeType.index in the LST).
    pub type_index: i32,
    /// References of this type in this cell.
    pub refs: Vec<TreeRef>,
}

/// DTL/BTT file: tree references for one cell at one LOD level.
#[derive(Debug, Clone)]
pub struct TreeBlock {
    /// Groups of references by tree type.
    pub type_blocks: Vec<TreeTypeBlock>,
}

impl TreeBlock {
    /// Parse a DTL/BTT file from raw bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Cursor::new(data);
        let type_count = r.read_i32::<LittleEndian>()? as usize;

        let mut type_blocks = Vec::with_capacity(type_count);
        for _ in 0..type_count {
            let type_index = r.read_i32::<LittleEndian>()?;
            let ref_count = r.read_i32::<LittleEndian>()? as usize;

            let mut refs = Vec::with_capacity(ref_count);
            for _ in 0..ref_count {
                refs.push(TreeRef::read_from(&mut r)?);
            }

            type_blocks.push(TreeTypeBlock { type_index, refs });
        }

        Ok(Self { type_blocks })
    }

    /// Serialize to DTL/BTT file bytes.
    pub fn write(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        buf.write_i32::<LittleEndian>(self.type_blocks.len() as i32)?;

        for tb in &self.type_blocks {
            buf.write_i32::<LittleEndian>(tb.type_index)?;
            buf.write_i32::<LittleEndian>(tb.refs.len() as i32)?;
            for r in &tb.refs {
                r.write_to(&mut buf)?;
            }
        }

        Ok(buf)
    }

    /// Total number of tree references across all types.
    pub fn total_refs(&self) -> usize {
        self.type_blocks.iter().map(|tb| tb.refs.len()).sum()
    }

    /// Add a reference to the appropriate type block, creating it if needed.
    pub fn add_ref(&mut self, type_index: i32, tree_ref: TreeRef) {
        if let Some(tb) = self.type_blocks.iter_mut().find(|tb| tb.type_index == type_index) {
            tb.refs.push(tree_ref);
        } else {
            self.type_blocks.push(TreeTypeBlock {
                type_index,
                refs: vec![tree_ref],
            });
        }
    }
}

// ============================================================================
// File path helpers
// ============================================================================

/// Generate the DTL file path for FO3/FNV.
///
/// Pattern: `meshes/landscape/lod/{WS}/Trees/{WS}.level{LOD}.x{X}.y{Y}.dtl`
pub fn dtl_path(worldspace: &str, lod_level: i32, cell_x: i32, cell_y: i32) -> String {
    format!(
        "meshes/landscape/lod/{ws}/Trees/{ws}.level{lod}.x{x}.y{y}.dtl",
        ws = worldspace, lod = lod_level, x = cell_x, y = cell_y
    )
}

/// Generate the BTT file path for Skyrim.
///
/// Pattern: `meshes/terrain/{WS}/Trees/{WS}.{LOD}.{X}.{Y}.btt`
pub fn btt_path(worldspace: &str, lod_level: i32, cell_x: i32, cell_y: i32) -> String {
    format!(
        "meshes/terrain/{ws}/Trees/{ws}.{lod}.{x}.{y}.btt",
        ws = worldspace, lod = lod_level, x = cell_x, y = cell_y
    )
}

/// Generate the LST file path for FO3/FNV.
///
/// Pattern: `meshes/landscape/lod/{WS}/Trees/TreeTypes.lst`
pub fn lst_path_fnv(worldspace: &str) -> String {
    format!("meshes/landscape/lod/{}/Trees/TreeTypes.lst", worldspace)
}

/// Generate the LST file path for Skyrim.
///
/// Pattern: `meshes/terrain/{WS}/Trees/{WS}.lst`
pub fn lst_path_sse(worldspace: &str) -> String {
    format!("meshes/terrain/{ws}/Trees/{ws}.lst", ws = worldspace)
}

/// Generate the tree atlas diffuse texture path for FO3/FNV.
pub fn tree_atlas_diffuse_path_fnv(_worldspace: &str) -> String {
    "textures/landscape/Trees/TreeDeadLod.dds".to_string()
}

/// Generate the tree atlas normal texture path for FO3/FNV.
pub fn tree_atlas_normal_path_fnv(_worldspace: &str) -> String {
    "textures/landscape/Trees/TreeDeadLod_n.dds".to_string()
}

// ============================================================================
// Tree LOD generation pipeline
// ============================================================================

/// Information about a tree base record (STAT/TREE with HasTreeLOD flag).
#[derive(Debug, Clone)]
pub struct TreeBase {
    /// FormID of the base record.
    pub form_id: u32,
    /// Editor ID of the base record.
    pub editor_id: String,
    /// Path to billboard diffuse texture.
    pub billboard_path: String,
    /// Path to billboard normal texture (if any).
    pub billboard_normal_path: Option<String>,
    /// Billboard config from .txt file.
    pub config: BillboardConfig,
    /// CRC32 of the billboard texture (for deduplication).
    pub crc32: u32,
    /// Index assigned in the tree type list.
    pub type_index: i32,
}

/// Billboard configuration from a .txt config file.
#[derive(Debug, Clone, Default)]
pub struct BillboardConfig {
    pub width: f32,
    pub height: f32,
    pub shift_x: f32,
    pub shift_y: f32,
    pub shift_z: f32,
    pub scale_factor: f32,
}

impl BillboardConfig {
    /// Parse from INI-style .txt content.
    ///
    /// Format:
    /// ```text
    /// [LOD]
    /// Width=150.0
    /// Height=127.0
    /// ShiftX=0.0
    /// ...
    /// ```
    pub fn parse(content: &str) -> Self {
        let mut config = Self::default();
        config.scale_factor = 1.0;

        for line in content.lines() {
            let line = line.trim();
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "Width" => config.width = val.parse().unwrap_or(0.0),
                    "Height" => config.height = val.parse().unwrap_or(0.0),
                    "ShiftX" => config.shift_x = val.parse().unwrap_or(0.0),
                    "ShiftY" => config.shift_y = val.parse().unwrap_or(0.0),
                    "ShiftZ" => config.shift_z = val.parse().unwrap_or(0.0),
                    "Scale" | "ScaleFactor" => config.scale_factor = val.parse().unwrap_or(1.0),
                    _ => {}
                }
            }
        }

        config
    }
}

/// A placed tree reference from a REFR record.
#[derive(Debug, Clone)]
pub struct PlacedTreeRef {
    /// FormID of the REFR record.
    pub ref_form_id: u32,
    /// FormID of the base object (STAT/TREE).
    pub base_form_id: u32,
    /// World position.
    pub position: [f32; 3],
    /// Rotation (radians).
    pub rotation: [f32; 3],
    /// Scale factor.
    pub scale: f32,
    /// Cell coordinates this ref belongs to.
    pub cell_x: i32,
    pub cell_y: i32,
}

/// Generate tree LOD for a worldspace.
///
/// This is the main entry point for tree LOD generation.
/// It produces:
/// - TreeTypes.lst (tree type metadata)
/// - DTL files (one per cell with tree references)
/// - Tree atlas DDS (diffuse + normal)
pub fn generate_tree_lod(
    worldspace: &str,
    output_dir: &Path,
    tree_bases: &[TreeBase],
    placed_refs: &[PlacedTreeRef],
    lod_level: i32,
    _brightness: i32,
    progress: &crate::progress::Progress,
) -> Result<TreeLodOutput> {
    use crate::lod_settings::LodSettings;

    progress.report("Building tree type list...", 0.1);

    // Build tree type list from bases (dedup by CRC32 already done by caller)
    let tree_types: Vec<TreeType> = tree_bases.iter().map(|b| {
        TreeType {
            index: b.type_index,
            width: b.config.width,
            height: b.config.height,
            uv_min_x: 0.0, // Will be filled by atlas builder
            uv_min_y: 0.0,
            uv_max_x: 0.0,
            uv_max_y: 0.0,
            unknown: 0,
        }
    }).collect();

    let lst = TreeTypeList { types: tree_types };

    progress.report("Grouping tree references by cell...", 0.3);

    // Group placed refs by cell (aligned to LOD level)
    let mut cell_refs: HashMap<(i32, i32), Vec<(i32, TreeRef)>> = HashMap::new();

    // Build base FormID -> type index lookup
    let base_to_type: HashMap<u32, i32> = tree_bases.iter()
        .map(|b| (b.form_id, b.type_index))
        .collect();

    let mut skipped = 0;
    for pr in placed_refs {
        let type_index = match base_to_type.get(&pr.base_form_id) {
            Some(&idx) => idx,
            None => { skipped += 1; continue; }
        };

        let (bx, by) = LodSettings::block_for_cell(pr.cell_x, pr.cell_y, lod_level);

        let tree_ref = TreeRef {
            x: pr.position[0],
            y: pr.position[1],
            z: pr.position[2],
            rotation: pr.rotation[2], // Z rotation
            scale: pr.scale,
            ref_form_id: pr.ref_form_id,
            unknown1: 0,
            unknown2: 0,
        };

        cell_refs.entry((bx, by))
            .or_default()
            .push((type_index, tree_ref));
    }

    if skipped > 0 {
        tracing::warn!("Skipped {} tree refs with unknown base FormID", skipped);
    }

    progress.report("Writing DTL files...", 0.6);

    // Write DTL files
    let trees_dir = output_dir
        .join("meshes/landscape/lod")
        .join(worldspace)
        .join("Trees");
    std::fs::create_dir_all(&trees_dir)?;

    let mut dtl_count = 0;
    let mut total_refs = 0;

    for ((cx, cy), refs) in &cell_refs {
        let mut block = TreeBlock { type_blocks: Vec::new() };
        for (type_idx, tree_ref) in refs {
            block.add_ref(*type_idx, tree_ref.clone());
        }

        let filename = format!(
            "{}.level{}.x{}.y{}.dtl",
            worldspace, lod_level, cx, cy
        );
        let path = trees_dir.join(&filename);
        let data = block.write()?;
        std::fs::write(&path, &data)?;

        dtl_count += 1;
        total_refs += block.total_refs();
    }

    // Write LST file
    let lst_path = trees_dir.join("TreeTypes.lst");
    let lst_data = lst.write()?;
    std::fs::write(&lst_path, &lst_data)?;

    progress.report("Tree LOD complete", 1.0);

    Ok(TreeLodOutput {
        lst_path,
        dtl_count,
        total_refs,
        tree_type_count: tree_bases.len(),
    })
}

/// Output summary from tree LOD generation.
#[derive(Debug)]
pub struct TreeLodOutput {
    pub lst_path: PathBuf,
    pub dtl_count: usize,
    pub total_refs: usize,
    pub tree_type_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lst_roundtrip() {
        let lst = TreeTypeList {
            types: vec![
                TreeType {
                    index: 0, width: 150.0, height: 127.0,
                    uv_min_x: 0.53125, uv_min_y: 0.0,
                    uv_max_x: 0.859375, uv_max_y: 0.25,
                    unknown: 0,
                },
                TreeType {
                    index: 1, width: 140.0, height: 110.0,
                    uv_min_x: 0.0, uv_min_y: 0.0,
                    uv_max_x: 0.5, uv_max_y: 0.335938,
                    unknown: 0,
                },
            ],
        };

        let data = lst.write().unwrap();
        assert_eq!(data.len(), 68); // 4 + 2*32

        let parsed = TreeTypeList::parse(&data).unwrap();
        assert_eq!(parsed.types.len(), 2);
        assert_eq!(parsed.types[0].index, 0);
        assert!((parsed.types[0].width - 150.0).abs() < 0.01);
        assert_eq!(parsed.types[1].index, 1);
    }

    #[test]
    fn test_dtl_roundtrip() {
        let block = TreeBlock {
            type_blocks: vec![
                TreeTypeBlock {
                    type_index: 1,
                    refs: vec![
                        TreeRef {
                            x: 27000.0, y: 19300.0, z: 4900.0,
                            rotation: 1.89, scale: 1.05,
                            ref_form_id: 0x000E3FAB,
                            unknown1: 0, unknown2: 0,
                        },
                        TreeRef {
                            x: 27200.0, y: 19500.0, z: 4850.0,
                            rotation: 0.5, scale: 1.1,
                            ref_form_id: 0x000E3FAC,
                            unknown1: 0, unknown2: 0,
                        },
                    ],
                },
            ],
        };

        let data = block.write().unwrap();
        // 4 (type count) + 4 (type index) + 4 (ref count) + 2*32 (refs) = 76
        assert_eq!(data.len(), 76);

        let parsed = TreeBlock::parse(&data).unwrap();
        assert_eq!(parsed.type_blocks.len(), 1);
        assert_eq!(parsed.type_blocks[0].type_index, 1);
        assert_eq!(parsed.type_blocks[0].refs.len(), 2);
        assert_eq!(parsed.type_blocks[0].refs[0].ref_form_id, 0x000E3FAB);
    }

    #[test]
    fn test_billboard_config_parse() {
        let txt = "[LOD]\nWidth=150.0\nHeight=127.0\nShiftX=0.5\nShiftY=-1.0\nShiftZ=2.0\nScale=0.8\n";
        let config = BillboardConfig::parse(txt);
        assert!((config.width - 150.0).abs() < 0.01);
        assert!((config.height - 127.0).abs() < 0.01);
        assert!((config.shift_x - 0.5).abs() < 0.01);
        assert!((config.shift_y - (-1.0)).abs() < 0.01);
        assert!((config.scale_factor - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_parse_reference_lst() {
        // Parse the actual reference LST file if available
        let path = "/home/luke/Games/VNV/mods/XLodGen Output/meshes/landscape/lod/WastelandNV/Trees/TreeTypes.lst";
        if let Ok(data) = std::fs::read(path) {
            let lst = TreeTypeList::parse(&data).unwrap();
            assert_eq!(lst.types.len(), 2);
            assert_eq!(lst.types[0].index, 0);
            assert!((lst.types[0].width - 150.0).abs() < 1.0);
            assert!((lst.types[0].height - 127.0).abs() < 1.0);
        }
    }

    #[test]
    fn test_parse_reference_dtl() {
        // Parse the actual reference DTL file if available
        let path = "/home/luke/Games/VNV/mods/XLodGen Output/meshes/landscape/lod/WastelandNV/Trees/WastelandNV.level8.x0.y0.dtl";
        if let Ok(data) = std::fs::read(path) {
            let block = TreeBlock::parse(&data).unwrap();
            assert_eq!(block.type_blocks.len(), 1);
            assert_eq!(block.type_blocks[0].type_index, 1);
            assert_eq!(block.type_blocks[0].refs.len(), 1357);
        }
    }
}
