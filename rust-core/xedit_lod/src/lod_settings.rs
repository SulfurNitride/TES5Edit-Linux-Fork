//! LOD settings file parser (.dlodsettings / .lod).
//!
//! Ported from TwbLodSettings in wbLOD.pas (lines 101-455).
//!
//! Format varies by game:
//! - FO3/FNV: 24 bytes — 2×i32 levels, 1×i32 stride, 4×i16 cells, 1×i32 obj_level
//! - Skyrim: 16 bytes — 2×i16 cells, 3×i32 stride+levels

use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

/// Parsed LOD settings for a worldspace.
#[derive(Debug, Clone)]
pub struct LodSettings {
    /// Southwest cell coordinates.
    pub sw_cell: (i32, i32),
    /// Northeast cell coordinates.
    pub ne_cell: (i32, i32),
    /// Grid stride (cell alignment).
    pub stride: i32,
    /// Minimum LOD level (e.g., 4).
    pub lod_level_min: i32,
    /// Maximum LOD level (e.g., 32).
    pub lod_level_max: i32,
    /// Object level (FO3/FNV only).
    pub object_level: i32,
}

impl LodSettings {
    /// Initialize with default LOD level range.
    pub fn default_levels() -> Self {
        Self {
            sw_cell: (0, 0),
            ne_cell: (0, 0),
            stride: 0,
            lod_level_min: 4,
            lod_level_max: 32,
            object_level: 0,
        }
    }

    /// Parse FO3/FNV format (24 bytes).
    ///
    /// Layout: LODmin(i32) LODmax(i32) Stride(i32) SWx(i16) SWy(i16) NEx(i16) NEy(i16) ObjLevel(i32)
    pub fn parse_fo3(data: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(data.len() >= 24, "FO3 lodsettings too short: {} bytes", data.len());
        let mut r = Cursor::new(data);
        let lod_level_min = r.read_i32::<LittleEndian>()?;
        let lod_level_max = r.read_i32::<LittleEndian>()?;
        let stride = r.read_i32::<LittleEndian>()?;
        let sw_x = r.read_i16::<LittleEndian>()? as i32;
        let sw_y = r.read_i16::<LittleEndian>()? as i32;
        let ne_x = r.read_i16::<LittleEndian>()? as i32;
        let ne_y = r.read_i16::<LittleEndian>()? as i32;
        let object_level = r.read_i32::<LittleEndian>()?;

        Ok(Self {
            sw_cell: (sw_x, sw_y),
            ne_cell: (ne_x, ne_y),
            stride,
            lod_level_min,
            lod_level_max,
            object_level,
        })
    }

    /// Parse Skyrim format (16 bytes).
    ///
    /// Layout: SWx(i16) SWy(i16) Stride(i32) LODmin(i32) LODmax(i32)
    pub fn parse_sse(data: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(data.len() >= 16, "SSE lodsettings too short: {} bytes", data.len());
        let mut r = Cursor::new(data);
        let sw_x = r.read_i16::<LittleEndian>()? as i32;
        let sw_y = r.read_i16::<LittleEndian>()? as i32;
        let stride = r.read_i32::<LittleEndian>()?;
        let lod_level_min = r.read_i32::<LittleEndian>()?;
        let lod_level_max = r.read_i32::<LittleEndian>()?;

        // NE cell derived from SW + stride * size
        let size = stride; // stride IS the grid size for SSE
        Ok(Self {
            sw_cell: (sw_x, sw_y),
            ne_cell: (sw_x + size, sw_y + size),
            stride,
            lod_level_min,
            lod_level_max,
            object_level: 0,
        })
    }

    /// Auto-detect format and parse.
    pub fn parse(data: &[u8], ext: &str) -> anyhow::Result<Self> {
        match ext.to_lowercase().as_str() {
            "dlodsettings" => Self::parse_fo3(data),
            "lod" => Self::parse_sse(data),
            _ => {
                // Try to auto-detect by size
                if data.len() == 24 {
                    Self::parse_fo3(data)
                } else if data.len() >= 16 {
                    Self::parse_sse(data)
                } else {
                    anyhow::bail!("Unknown lodsettings format ({} bytes)", data.len())
                }
            }
        }
    }

    /// Map a cell coordinate to its LOD block origin (aligned to LOD level).
    pub fn block_for_cell(x: i32, y: i32, lod_level: i32) -> (i32, i32) {
        let block_x = if x >= 0 {
            (x / lod_level) * lod_level
        } else {
            ((x + 1) / lod_level - 1) * lod_level
        };
        let block_y = if y >= 0 {
            (y / lod_level) * lod_level
        } else {
            ((y + 1) / lod_level - 1) * lod_level
        };
        (block_x, block_y)
    }

    /// Grid size from stride.
    pub fn get_size(&self) -> i32 {
        self.stride
    }

    /// Get the LOD levels as a slice (4, 8, 16, 32).
    pub fn lod_levels(&self) -> Vec<i32> {
        let mut levels = Vec::new();
        let mut level = self.lod_level_min;
        while level <= self.lod_level_max {
            levels.push(level);
            level *= 2;
        }
        levels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_for_cell() {
        assert_eq!(LodSettings::block_for_cell(5, 7, 4), (4, 4));
        assert_eq!(LodSettings::block_for_cell(0, 0, 4), (0, 0));
        assert_eq!(LodSettings::block_for_cell(-1, -1, 4), (-4, -4));
        assert_eq!(LodSettings::block_for_cell(-5, -5, 4), (-8, -8));
        assert_eq!(LodSettings::block_for_cell(15, 15, 8), (8, 8));
        assert_eq!(LodSettings::block_for_cell(16, 16, 8), (16, 16));
    }

    #[test]
    fn test_parse_fo3() {
        // Construct a 24-byte FO3 settings buffer
        let mut data = Vec::new();
        data.extend_from_slice(&4i32.to_le_bytes());   // lod_level_min
        data.extend_from_slice(&32i32.to_le_bytes());  // lod_level_max
        data.extend_from_slice(&128i32.to_le_bytes()); // stride
        data.extend_from_slice(&(-64i16).to_le_bytes()); // sw_x
        data.extend_from_slice(&(-64i16).to_le_bytes()); // sw_y
        data.extend_from_slice(&64i16.to_le_bytes());    // ne_x
        data.extend_from_slice(&64i16.to_le_bytes());    // ne_y
        data.extend_from_slice(&0i32.to_le_bytes());     // object_level

        let settings = LodSettings::parse_fo3(&data).unwrap();
        assert_eq!(settings.lod_level_min, 4);
        assert_eq!(settings.lod_level_max, 32);
        assert_eq!(settings.stride, 128);
        assert_eq!(settings.sw_cell, (-64, -64));
        assert_eq!(settings.ne_cell, (64, 64));
    }

    #[test]
    fn test_lod_levels() {
        let settings = LodSettings {
            sw_cell: (0, 0), ne_cell: (0, 0),
            stride: 0, lod_level_min: 4, lod_level_max: 32, object_level: 0,
        };
        assert_eq!(settings.lod_levels(), vec![4, 8, 16, 32]);
    }
}
