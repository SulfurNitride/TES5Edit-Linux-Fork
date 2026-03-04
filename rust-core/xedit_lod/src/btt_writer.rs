//! Binary Tree Type (.btt) file writer for tree LOD placement data.

use anyhow::Result;
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::Write;

#[derive(Debug, Clone)]
pub struct BttEntry {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rotation: f32,
    pub scale: f32,
    pub tree_type_index: u32,
}

pub fn write_btt<W: Write>(writer: &mut W, entries: &[BttEntry]) -> Result<()> {
    // Write count
    writer.write_u32::<LittleEndian>(entries.len() as u32)?;

    for entry in entries {
        writer.write_f32::<LittleEndian>(entry.x)?;
        writer.write_f32::<LittleEndian>(entry.y)?;
        writer.write_f32::<LittleEndian>(entry.z)?;
        writer.write_f32::<LittleEndian>(entry.rotation)?;
        writer.write_f32::<LittleEndian>(entry.scale)?;
        writer.write_u32::<LittleEndian>(entry.tree_type_index)?;
    }

    Ok(())
}
