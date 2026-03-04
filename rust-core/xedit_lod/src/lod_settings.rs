//! Parser for binary LOD settings files.

use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;

/// Parsed LOD settings defining the cell bounds for a worldspace.
#[derive(Debug, Clone)]
pub struct LodSettings {
    pub sw_cell_x: f32,
    pub sw_cell_y: f32,
    pub ne_cell_x: f32,
    pub ne_cell_y: f32,
}

impl LodSettings {
    /// Parse LOD settings from raw binary data (minimum 16 bytes: 4 little-endian f32s).
    pub fn parse(data: &[u8]) -> Result<Self> {
        anyhow::ensure!(
            data.len() >= 16,
            "LOD settings file too small (need 16 bytes, got {})",
            data.len()
        );
        let mut cursor = Cursor::new(data);
        Ok(Self {
            sw_cell_x: cursor.read_f32::<LittleEndian>()?,
            sw_cell_y: cursor.read_f32::<LittleEndian>()?,
            ne_cell_x: cursor.read_f32::<LittleEndian>()?,
            ne_cell_y: cursor.read_f32::<LittleEndian>()?,
        })
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
}
