//! Parser for binary LOD settings files.
//!
//! Skyrim/SSE/FO4 `.lod` format (16 bytes):
//!   bytes 0-1:  SWCell.x  (int16)
//!   bytes 2-3:  SWCell.y  (int16)
//!   bytes 4-7:  Stride    (int32)
//!   bytes 8-11: LODLevelMin (int32)
//!   bytes 12-15: LODLevelMax (int32)
//!
//! FO3/FNV `.dlodsettings` format (24 bytes):
//!   bytes 0-3:   LODLevelMin (int32)
//!   bytes 4-7:   LODLevelMax (int32)
//!   bytes 8-11:  Stride      (int32)
//!   bytes 12-13: SWCell.x    (int16)
//!   bytes 14-15: SWCell.y    (int16)
//!   bytes 16-17: NECell.x    (int16)
//!   bytes 18-19: NECell.y    (int16)
//!   bytes 20-23: ObjectLevel (int32)

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

/// Parsed LOD settings defining the cell bounds for a worldspace.
#[derive(Debug, Clone)]
pub struct LodSettings {
    pub sw_cell_x: i16,
    pub sw_cell_y: i16,
    pub ne_cell_x: i16,
    pub ne_cell_y: i16,
    pub stride: i32,
    pub lod_level_min: i32,
    pub lod_level_max: i32,
    /// FO3/FNV only: ObjectLevel (typically 8)
    pub object_level: i32,
}

impl LodSettings {
    /// Parse Skyrim/SSE/FO4 `.lod` format (16 bytes).
    pub fn parse_lod(data: &[u8]) -> Result<Self> {
        anyhow::ensure!(
            data.len() >= 16,
            "LOD settings file too small (need 16 bytes, got {})",
            data.len()
        );
        let mut c = Cursor::new(data);
        let sw_x = c.read_i16::<LittleEndian>()?;
        let sw_y = c.read_i16::<LittleEndian>()?;
        let stride = c.read_i32::<LittleEndian>()?;
        let lod_level_min = c.read_i32::<LittleEndian>()?;
        let lod_level_max = c.read_i32::<LittleEndian>()?;

        // Derive NE cell from stride: Size = ceil(Stride / sqrt(2))
        let size = ((stride as f64) / std::f64::consts::SQRT_2).ceil() as i16;
        let ne_x = sw_x + size;
        let ne_y = sw_y + size;

        Ok(Self {
            sw_cell_x: sw_x,
            sw_cell_y: sw_y,
            ne_cell_x: ne_x,
            ne_cell_y: ne_y,
            stride,
            lod_level_min,
            lod_level_max,
            object_level: 0,
        })
    }

    /// Parse FO3/FNV `.dlodsettings` format (24 bytes).
    pub fn parse_dlodsettings(data: &[u8]) -> Result<Self> {
        anyhow::ensure!(
            data.len() >= 24,
            "dlodsettings file too small (need 24 bytes, got {})",
            data.len()
        );
        let mut c = Cursor::new(data);
        let lod_level_min = c.read_i32::<LittleEndian>()?;
        let lod_level_max = c.read_i32::<LittleEndian>()?;
        let stride = c.read_i32::<LittleEndian>()?;
        let sw_x = c.read_i16::<LittleEndian>()?;
        let sw_y = c.read_i16::<LittleEndian>()?;
        let ne_x = c.read_i16::<LittleEndian>()?;
        let ne_y = c.read_i16::<LittleEndian>()?;
        let object_level = c.read_i32::<LittleEndian>()?;

        Ok(Self {
            sw_cell_x: sw_x,
            sw_cell_y: sw_y,
            ne_cell_x: ne_x,
            ne_cell_y: ne_y,
            stride,
            lod_level_min,
            lod_level_max,
            object_level,
        })
    }

    /// Parse with automatic format detection based on extension.
    pub fn parse(data: &[u8], extension: &str) -> Result<Self> {
        if extension.eq_ignore_ascii_case("dlodsettings") {
            Self::parse_dlodsettings(data)
        } else {
            Self::parse_lod(data)
        }
    }

    /// Get cell range as integer bounds (sw_x, sw_y, ne_x, ne_y).
    pub fn cell_range(&self) -> (i32, i32, i32, i32) {
        (
            self.sw_cell_x as i32,
            self.sw_cell_y as i32,
            self.ne_cell_x as i32,
            self.ne_cell_y as i32,
        )
    }

    /// Compute the block-aligned cell for a given cell coordinate and LOD level.
    /// BlockX = SWCell.x + ((CellX - SWCell.x) div LODLevel) * LODLevel
    pub fn block_for_cell(&self, cell_x: i32, cell_y: i32, lod_level: i32) -> (i32, i32) {
        let sw_x = self.sw_cell_x as i32;
        let sw_y = self.sw_cell_y as i32;
        let bx = sw_x + ((cell_x - sw_x).div_euclid(lod_level)) * lod_level;
        let by = sw_y + ((cell_y - sw_y).div_euclid(lod_level)) * lod_level;
        (bx, by)
    }

    /// Check if a cell is within the LOD bounds.
    pub fn cell_in_bounds(&self, cell_x: i32, cell_y: i32) -> bool {
        cell_x >= self.sw_cell_x as i32
            && cell_x <= self.ne_cell_x as i32
            && cell_y >= self.sw_cell_y as i32
            && cell_y <= self.ne_cell_y as i32
    }
}
