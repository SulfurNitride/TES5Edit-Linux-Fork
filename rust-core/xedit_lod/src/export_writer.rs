//! LODGen.txt export file writer for external LODGen.exe.
//!
//! Writes tab-separated text files matching the format expected by LODGen.exe.
//! Also writes the AtlasMap file for texture atlas coordinate mapping.
//!
//! LODGen.txt format:
//! Header:
//!   GameMode={FNV|SSE|FO3|...}
//!   Worldspace={EditorID}
//!   CellSW={X} {Y}
//!   AlphaDoubleSided=False
//!   TextureAtlasMap={path to atlas map file}
//!   AtlasTolerance={value}
//!   PathData={game data path}
//!   PathOutput={output mesh path}
//!   Resource={bsa path} (repeated for each BSA)
//!   IgnoreTranslation=nif
//!
//! Per-reference line (tab-separated):
//!   FormID  Flags  PosX  PosY  PosZ  RotX  RotY  RotZ  Scale  EditorID  GridFlags  [empty]  FullModel  LOD4  LOD8  [trailing tab]

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::info;

/// Configuration for the LODGen export.
pub struct ExportConfig {
    pub game_mode: String,
    pub worldspace: String,
    pub cell_sw: (i32, i32),
    pub atlas_map_path: PathBuf,
    pub atlas_tolerance: f32,
    pub path_data: PathBuf,
    pub path_output: PathBuf,
    pub bsa_paths: Vec<PathBuf>,
    pub mod_dirs: Vec<PathBuf>,
    pub alpha_double_sided: bool,
    pub ignore_translation: bool,
}

/// A reference entry for LODGen.txt.
pub struct LodGenRef {
    pub form_id: u32,
    pub flags: u32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub scale: f32,
    pub editor_id: String,
    pub grid_flags: u32,
    pub full_model: String,
    pub lod4_model: String,
    pub lod8_model: String,
}

/// An entry in the texture atlas map.
#[derive(Debug, Clone)]
pub struct AtlasMapEntry {
    pub source_texture: String,
    pub src_width: u32,
    pub src_height: u32,
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub atlas_texture: String,
    pub atlas_width: u32,
    pub atlas_height: u32,
}

/// Write the LODGen.txt export file.
pub fn write_lodgen_txt(
    path: &Path,
    config: &ExportConfig,
    refs: &[LodGenRef],
) -> Result<()> {
    let mut f = std::fs::File::create(path)
        .with_context(|| format!("Failed to create LODGen.txt: {}", path.display()))?;

    // Header
    writeln!(f, "GameMode={}", config.game_mode)?;
    writeln!(f, "Worldspace={}", config.worldspace)?;
    writeln!(f, "CellSW={} {}", config.cell_sw.0, config.cell_sw.1)?;
    writeln!(f, "AlphaDoubleSided={}", if config.alpha_double_sided { "True" } else { "False" })?;

    // Use native paths since LODGen is now built for .NET 10 Linux
    writeln!(f, "TextureAtlasMap={}", config.atlas_map_path.display())?;
    writeln!(f, "AtlasTolerance={:.1}", config.atlas_tolerance)?;

    writeln!(f, "PathData={}/", config.path_data.display())?;
    writeln!(f, "PathOutput={}/", config.path_output.display())?;

    // BSA resources
    for bsa in &config.bsa_paths {
        writeln!(f, "Resource={}", bsa.display())?;
    }

    // Mod directories as resources
    for mod_dir in &config.mod_dirs {
        writeln!(f, "Resource={}", mod_dir.display())?;
    }

    if config.ignore_translation {
        writeln!(f, "IgnoreTranslation=nif")?;
    }

    // Reference entries
    for r in refs {
        write!(f, "{:08X}\t{:08X}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{}\t{}\t{:08X}\t\t{}\t{}\t{}\t",
            r.form_id,
            r.flags,
            format_float(r.pos_x),
            format_float(r.pos_y),
            format_float(r.pos_z),
            r.rot_x,
            r.rot_y,
            r.rot_z,
            format_scale(r.scale),
            r.editor_id,
            r.grid_flags,
            r.full_model,
            r.lod4_model,
            r.lod8_model,
        )?;
        writeln!(f)?;
    }

    info!("Wrote LODGen.txt with {} refs to {}", refs.len(), path.display());
    Ok(())
}

/// Write the texture atlas map file.
pub fn write_atlas_map(
    path: &Path,
    entries: &[AtlasMapEntry],
) -> Result<()> {
    let mut f = std::fs::File::create(path)
        .with_context(|| format!("Failed to create atlas map: {}", path.display()))?;

    for entry in entries {
        writeln!(f, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            entry.source_texture,
            entry.src_width,
            entry.src_height,
            entry.atlas_x,
            entry.atlas_y,
            entry.atlas_texture,
            entry.atlas_width,
            entry.atlas_height,
        )?;
    }

    info!("Wrote atlas map with {} entries to {}", entries.len(), path.display());
    Ok(())
}

/// Convert a Linux path to Wine Z: path format.
#[allow(dead_code)]
fn to_wine_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.starts_with('/') {
        format!("Z:{}", s.replace('/', "\\"))
    } else {
        s.replace('/', "\\")
    }
}

/// Format a float matching xLODGen's output style.
fn format_float(v: f32) -> String {
    if v == v.floor() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{}", v)
    }
}

/// Format scale value.
fn format_scale(v: f32) -> String {
    if (v - 1.0).abs() < 1e-6 {
        "1.0".to_string()
    } else {
        format!("{:.6}", v).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Compute grid flags for a REFR based on its cell position and LOD settings.
///
/// Grid flags encode which LOD levels a reference appears in,
/// based on the cell position relative to the worldspace grid.
pub fn compute_grid_flags(
    cell_x: i32,
    cell_y: i32,
    has_lod4: bool,
    has_lod8: bool,
    sw_x: i32,
    sw_y: i32,
) -> u32 {
    let mut flags = 0u32;

    // Encode cell block coordinates in the grid flags
    // The grid flags encode LOD level visibility based on cell position
    let block_x = ((cell_x - sw_x) / 4) as u32;
    let block_y = ((cell_y - sw_y) / 4) as u32;

    // Basic LOD level flags
    if has_lod4 {
        flags |= 0x0200; // LOD4 visible
    }
    if has_lod8 {
        flags |= 0x8000; // LOD8 visible
    }

    // Encode block position
    flags |= (block_x & 0xFF) << 24;
    flags |= (block_y & 0xFF) << 16;

    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_wine_path() {
        assert_eq!(
            to_wine_path(Path::new("/home/luke/Games/data")),
            "Z:\\home\\luke\\Games\\data"
        );
    }

    #[test]
    fn test_format_float() {
        assert_eq!(format_float(1386.0), "1386");
        assert_eq!(format_float(-1386.08068847656f32), "-1386.0807");
    }

    #[test]
    fn test_format_scale() {
        assert_eq!(format_scale(1.0), "1.0");
        assert_eq!(format_scale(1.2), "1.2");
    }
}
