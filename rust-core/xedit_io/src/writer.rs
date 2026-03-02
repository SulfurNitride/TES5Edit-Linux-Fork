//! Surgical patch writer - preserves untouched bytes, only rewrites modified spans.

use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use byteorder::{LittleEndian, WriteBytesExt};

use xedit_dom::*;

/// Writes a plugin back to disk with surgical patching.
///
/// Unmodified records are written from their original raw bytes.
/// Only modified records are re-serialized.
pub struct PluginWriter;

impl PluginWriter {
    /// Write a plugin to a file.
    pub fn write_file(plugin: &Plugin, path: &Path) -> Result<()> {
        let data = Self::write_bytes(plugin)?;
        std::fs::write(path, &data)
            .with_context(|| format!("Failed to write plugin: {}", path.display()))?;
        Ok(())
    }

    /// Serialize a plugin to bytes.
    pub fn write_bytes(plugin: &Plugin) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let family = plugin.game_id.dialect_family();

        // Write file header
        Self::write_record(&plugin.header, &mut buf, family)?;

        match family {
            dialect::DialectFamily::TES3 => {
                for record in &plugin.tes3_records {
                    Self::write_record(record, &mut buf, family)?;
                }
            }
            dialect::DialectFamily::TES4Plus => {
                for group in &plugin.groups {
                    Self::write_group(group, &mut buf, family)?;
                }
            }
        }

        Ok(buf)
    }

    /// Write a single record, using raw bytes if unmodified.
    fn write_record<W: Write>(record: &Record, writer: &mut W, family: dialect::DialectFamily) -> Result<()> {
        if !record.is_modified() {
            // Lossless path: write original raw header + original data
            if let Some(ref raw_header) = record.raw_header {
                writer.write_all(raw_header)?;
                if let Some(ref raw_data) = record.raw_data {
                    writer.write_all(raw_data)?;
                } else if let Some(ref compressed) = record.raw_compressed_data {
                    writer.write_all(compressed)?;
                } else {
                    // Write subrecords from raw bytes
                    for sr in &record.subrecords {
                        Self::write_subrecord(sr, writer, family)?;
                    }
                }
                return Ok(());
            }
        }

        // Modified path: re-serialize the record
        let subrecord_data = Self::serialize_subrecords(&record.subrecords, family)?;

        // Write 24-byte header
        writer.write_all(record.signature.as_bytes())?;
        writer.write_u32::<LittleEndian>(subrecord_data.len() as u32)?;
        writer.write_u32::<LittleEndian>(record.flags.0)?;
        writer.write_u32::<LittleEndian>(record.form_id.raw())?;
        writer.write_u32::<LittleEndian>(record.vc_info)?;
        writer.write_u16::<LittleEndian>(record.version)?;
        writer.write_u16::<LittleEndian>(record.unknown)?;

        writer.write_all(&subrecord_data)?;

        Ok(())
    }

    /// Write a GRUP, using raw header bytes if unmodified.
    fn write_group<W: Write>(group: &Group, writer: &mut W, family: dialect::DialectFamily) -> Result<()> {
        // First, serialize all children to compute group size
        let mut children_buf = Vec::new();
        for child in &group.children {
            match child {
                group::GroupChild::Record(r) => Self::write_record(r, &mut children_buf, family)?,
                group::GroupChild::Group(g) => Self::write_group(g, &mut children_buf, family)?,
            }
        }

        if let Some(ref raw_header) = group.raw_header {
            let stored_size = u32::from_le_bytes([
                raw_header[4],
                raw_header[5],
                raw_header[6],
                raw_header[7],
            ]) as usize;
            let expected_size = raw_header.len() + children_buf.len();
            if stored_size == expected_size {
                writer.write_all(raw_header)?;
                writer.write_all(&children_buf)?;
                return Ok(());
            }
        }

        let header_len = group.raw_header.as_ref().map_or(24usize, |h| h.len());
        let total_size = header_len as u32 + children_buf.len() as u32;

        // Write GRUP header
        writer.write_all(b"GRUP")?;
        writer.write_u32::<LittleEndian>(total_size)?;
        Self::write_group_label(&group.group_type, writer)?;
        writer.write_u32::<LittleEndian>(group.stamp)?;
        if header_len >= 24 {
            writer.write_u32::<LittleEndian>(group.unknown)?;
        }

        // Write children
        writer.write_all(&children_buf)?;

        Ok(())
    }

    /// Write a subrecord.
    ///
    /// TES3 uses 8-byte headers (4-byte sig + 4-byte u32 size).
    /// TES4+ uses 6-byte headers (4-byte sig + 2-byte u16 size).
    fn write_subrecord<W: Write>(sr: &Subrecord, writer: &mut W, family: dialect::DialectFamily) -> Result<()> {
        writer.write_all(sr.signature.as_bytes())?;
        match family {
            dialect::DialectFamily::TES3 => {
                writer.write_u32::<LittleEndian>(sr.raw_data.len() as u32)?;
            }
            dialect::DialectFamily::TES4Plus => {
                writer.write_u16::<LittleEndian>(sr.raw_data.len() as u16)?;
            }
        }
        writer.write_all(&sr.raw_data)?;
        Ok(())
    }

    /// Serialize all subrecords to a byte buffer.
    fn serialize_subrecords(subrecords: &[Subrecord], family: dialect::DialectFamily) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        for sr in subrecords {
            Self::write_subrecord(sr, &mut buf, family)?;
        }
        Ok(buf)
    }

    /// Write group label bytes based on group type.
    fn write_group_label<W: Write>(
        group_type: &group::GroupType,
        writer: &mut W,
    ) -> Result<()> {
        match group_type {
            group::GroupType::Top(sig) => {
                writer.write_all(sig.as_bytes())?;
                writer.write_u32::<LittleEndian>(0)?;
            }
            group::GroupType::WorldChildren(id) => {
                writer.write_u32::<LittleEndian>(*id)?;
                writer.write_u32::<LittleEndian>(1)?;
            }
            group::GroupType::InteriorCellBlock(n) => {
                writer.write_i32::<LittleEndian>(*n)?;
                writer.write_u32::<LittleEndian>(2)?;
            }
            group::GroupType::InteriorCellSubBlock(n) => {
                writer.write_i32::<LittleEndian>(*n)?;
                writer.write_u32::<LittleEndian>(3)?;
            }
            group::GroupType::ExteriorCellBlock { y, x } => {
                writer.write_i16::<LittleEndian>(*y)?;
                writer.write_i16::<LittleEndian>(*x)?;
                writer.write_u32::<LittleEndian>(4)?;
            }
            group::GroupType::ExteriorCellSubBlock { y, x } => {
                writer.write_i16::<LittleEndian>(*y)?;
                writer.write_i16::<LittleEndian>(*x)?;
                writer.write_u32::<LittleEndian>(5)?;
            }
            group::GroupType::CellChildren(id) => {
                writer.write_u32::<LittleEndian>(*id)?;
                writer.write_u32::<LittleEndian>(6)?;
            }
            group::GroupType::TopicChildren(id) => {
                writer.write_u32::<LittleEndian>(*id)?;
                writer.write_u32::<LittleEndian>(7)?;
            }
            group::GroupType::CellPersistentChildren(id) => {
                writer.write_u32::<LittleEndian>(*id)?;
                writer.write_u32::<LittleEndian>(8)?;
            }
            group::GroupType::CellTemporaryChildren(id) => {
                writer.write_u32::<LittleEndian>(*id)?;
                writer.write_u32::<LittleEndian>(9)?;
            }
            group::GroupType::CellVisibleDistantChildren(id) => {
                writer.write_u32::<LittleEndian>(*id)?;
                writer.write_u32::<LittleEndian>(10)?;
            }
        }
        Ok(())
    }
}
