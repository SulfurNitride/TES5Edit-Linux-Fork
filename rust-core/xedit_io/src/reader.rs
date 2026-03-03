//! Plugin file reader - parses ESP/ESM/ESL files into the lossless DOM.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result};

use xedit_dom::*;
use xedit_dom::record::RecordFlags;

/// Reads Bethesda plugin files into the lossless DOM.
pub struct PluginReader {
    game_id: GameId,
}

impl PluginReader {
    pub fn new(game_id: GameId) -> Self {
        Self { game_id }
    }

    /// Load a plugin file from disk.
    pub fn read_file(&self, path: &Path) -> Result<Plugin> {
        let data = std::fs::read(path)
            .with_context(|| format!("Failed to read plugin: {}", path.display()))?;
        self.read_bytes(&data, Some(path.to_path_buf()))
    }

    /// Load a plugin from a byte slice.
    pub fn read_bytes(
        &self,
        data: &[u8],
        file_path: Option<std::path::PathBuf>,
    ) -> Result<Plugin> {
        let mut cursor = std::io::Cursor::new(data);

        // Read file header (TES3 or TES4)
        let header = self.read_record(&mut cursor)?;

        // Validate header signature
        let expected_sig = self.game_id.header_signature();
        anyhow::ensure!(
            header.signature == expected_sig,
            "Invalid header: expected {}, got {}",
            expected_sig,
            header.signature
        );

        // Extract masters from header
        let masters = self.extract_masters(&header);

        match self.game_id.dialect_family() {
            dialect::DialectFamily::TES3 => {
                // TES3: flat list of records, no GRUPs
                let mut records = Vec::new();
                while cursor.position() < data.len() as u64 {
                    let record = self.read_record(&mut cursor)?;
                    records.push(record);
                }
                Ok(Plugin {
                    game_id: self.game_id,
                    file_path,
                    header,
                    groups: Vec::new(),
                    tes3_records: records,
                    masters,
                    description: None,
                    author: None,
                    modified: false,
                })
            }
            dialect::DialectFamily::TES4Plus => {
                // TES4+: groups containing records
                let mut groups = Vec::new();
                while cursor.position() < data.len() as u64 {
                    let group = self.read_group(&mut cursor, data)?;
                    groups.push(group);
                }
                Ok(Plugin {
                    game_id: self.game_id,
                    file_path,
                    header,
                    groups,
                    tes3_records: Vec::new(),
                    masters,
                    description: None,
                    author: None,
                    modified: false,
                })
            }
        }
    }

    /// Read a single record from the stream.
    fn read_record<R: Read + Seek>(&self, reader: &mut R) -> Result<Record> {
        let source_offset = reader.stream_position()?;

        let header_len = match self.game_id {
            GameId::Morrowind => 16,
            GameId::Oblivion => 20,
            _ => 24,
        };

        let mut header_bytes = vec![0u8; header_len];
        reader.read_exact(&mut header_bytes)?;

        let sig = Signature::from_bytes(&[
            header_bytes[0],
            header_bytes[1],
            header_bytes[2],
            header_bytes[3],
        ]);
        let data_size = u32::from_le_bytes([
            header_bytes[4],
            header_bytes[5],
            header_bytes[6],
            header_bytes[7],
        ]);
        let flags = RecordFlags(u32::from_le_bytes([
            header_bytes[8],
            header_bytes[9],
            header_bytes[10],
            header_bytes[11],
        ]));
        let form_id = FormId::new(u32::from_le_bytes([
            header_bytes[12],
            header_bytes[13],
            header_bytes[14],
            header_bytes[15],
        ]));
        let vc_info = if header_len >= 20 {
            u32::from_le_bytes([
                header_bytes[16],
                header_bytes[17],
                header_bytes[18],
                header_bytes[19],
            ])
        } else {
            0
        };
        let version = if header_len >= 24 {
            u16::from_le_bytes([header_bytes[20], header_bytes[21]])
        } else {
            0
        };
        let unknown = if header_len >= 24 {
            u16::from_le_bytes([header_bytes[22], header_bytes[23]])
        } else {
            0
        };

        // Read record data
        let mut raw_data = vec![0u8; data_size as usize];
        reader.read_exact(&mut raw_data)?;

        // Handle compressed records
        let (subrecords, raw_compressed_data, keep_raw_data) = if flags.is_compressed() {
            // First 4 bytes = decompressed size, rest is zlib compressed.
            // Keep compressed bytes for lossless save; DON'T clone into raw_data
            // (saves ~1-2 GB for large mod lists with many compressed records).
            let decompressed = self.decompress_record(&raw_data, sig, source_offset)?;
            let subs = self.parse_subrecords(&decompressed)?;
            (subs, Some(raw_data), None)
        } else {
            let subs = self.parse_subrecords(&raw_data)?;
            // Keep raw_data for uncompressed records — needed for lossless roundtrip
            (subs, None, Some(raw_data))
        };

        Ok(Record {
            signature: sig,
            flags,
            form_id,
            vc_info,
            version,
            unknown,
            subrecords,
            raw_header: Some(header_bytes),
            raw_compressed_data,
            raw_data: keep_raw_data,
            source_offset: Some(source_offset),
            modified: false,
        })
    }

    /// Read a GRUP from the stream.
    fn read_group<R: Read + Seek>(&self, reader: &mut R, _full_data: &[u8]) -> Result<Group> {
        let source_offset = reader.stream_position()?;

        let header_len = match self.game_id {
            GameId::Oblivion => 20,
            _ => 24,
        };

        let mut header_bytes = vec![0u8; header_len];
        reader.read_exact(&mut header_bytes)?;

        // Verify GRUP signature
        let sig = &header_bytes[0..4];
        anyhow::ensure!(sig == b"GRUP", "Expected GRUP, got {:?}", sig);

        let group_size = u32::from_le_bytes([
            header_bytes[4],
            header_bytes[5],
            header_bytes[6],
            header_bytes[7],
        ]);
        let label_bytes = [
            header_bytes[8],
            header_bytes[9],
            header_bytes[10],
            header_bytes[11],
        ];
        let group_type_raw = u32::from_le_bytes([
            header_bytes[12],
            header_bytes[13],
            header_bytes[14],
            header_bytes[15],
        ]);
        let stamp = u32::from_le_bytes([
            header_bytes[16],
            header_bytes[17],
            header_bytes[18],
            header_bytes[19],
        ]);
        let unknown = if header_len >= 24 {
            u32::from_le_bytes([
                header_bytes[20],
                header_bytes[21],
                header_bytes[22],
                header_bytes[23],
            ])
        } else {
            0
        };

        let group_type = self.parse_group_type(group_type_raw, label_bytes)?;

        // Read children until we've consumed group_size bytes total (including header)
        let content_size = group_size as u64 - header_len as u64;
        let content_end = reader.stream_position()? + content_size;
        let mut children = Vec::new();

        while reader.stream_position()? < content_end {
            // Peek at next 4 bytes to determine if child is GRUP or record
            let mut peek = [0u8; 4];
            reader.read_exact(&mut peek)?;
            reader.seek(SeekFrom::Current(-4))?;

            if &peek == b"GRUP" {
                children.push(group::GroupChild::Group(
                    self.read_group(reader, _full_data)?,
                ));
            } else {
                children.push(group::GroupChild::Record(self.read_record(reader)?));
            }
        }

        Ok(Group {
            group_type,
            stamp,
            unknown,
            children,
            raw_header: Some(header_bytes),
            source_offset: Some(source_offset),
        })
    }

    /// Parse subrecords from raw data bytes.
    ///
    /// TES3 (Morrowind) uses 8-byte subrecord headers: 4-byte signature + 4-byte u32 size.
    /// TES4+ uses 6-byte subrecord headers: 4-byte signature + 2-byte u16 size, with XXXX
    /// extended-size markers for subrecords larger than 65535 bytes.
    fn parse_subrecords(&self, data: &[u8]) -> Result<Vec<Subrecord>> {
        let family = self.game_id.dialect_family();
        let header_size: usize = match family {
            dialect::DialectFamily::TES3 => 8,
            dialect::DialectFamily::TES4Plus => 6,
        };

        let mut subrecords = Vec::new();
        let mut pos = 0;
        let mut extended_size: Option<usize> = None;

        while pos + header_size <= data.len() {
            let sig =
                Signature::from_bytes(&[data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);

            let short_size = match family {
                dialect::DialectFamily::TES3 => {
                    // 4-byte u32 size field
                    u32::from_le_bytes([
                        data[pos + 4],
                        data[pos + 5],
                        data[pos + 6],
                        data[pos + 7],
                    ]) as usize
                }
                dialect::DialectFamily::TES4Plus => {
                    // 2-byte u16 size field
                    u16::from_le_bytes([data[pos + 4], data[pos + 5]]) as usize
                }
            };
            pos += header_size;

            // TES4+ extended-size subrecord marker (XXXX). Not used in TES3.
            if family == dialect::DialectFamily::TES4Plus
                && sig == Signature::from_bytes(b"XXXX")
            {
                anyhow::ensure!(
                    short_size == 4 && pos + 4 <= data.len(),
                    "Invalid XXXX subrecord at offset {} (size {}, remaining {})",
                    pos - header_size,
                    short_size,
                    data.len().saturating_sub(pos)
                );
                extended_size = Some(u32::from_le_bytes([
                    data[pos],
                    data[pos + 1],
                    data[pos + 2],
                    data[pos + 3],
                ]) as usize);
                pos += 4;
                continue;
            }

            let size = extended_size.take().unwrap_or(short_size);

            anyhow::ensure!(
                pos + size <= data.len(),
                "Subrecord {} data extends past record boundary (offset {}, size {}, record len {})",
                sig,
                pos,
                size,
                data.len()
            );

            let raw_data = data[pos..pos + size].to_vec();
            let mut sr = Subrecord::new(sig, raw_data);
            sr.source_offset = Some((pos - header_size) as u64);
            subrecords.push(sr);

            pos += size;
        }

        anyhow::ensure!(
            extended_size.is_none(),
            "Dangling XXXX subrecord at end of record"
        );

        Ok(subrecords)
    }

    /// Decompress a compressed record's data.
    fn decompress_record(
        &self,
        data: &[u8],
        sig: Signature,
        offset: u64,
    ) -> Result<Vec<u8>> {
        anyhow::ensure!(
            data.len() >= 4,
            "Compressed record {} too short at offset {}",
            sig,
            offset
        );

        let decompressed_size =
            u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let compressed = &data[4..];

        use flate2::read::{ZlibDecoder, DeflateDecoder};

        // Try zlib (with header) first. Some Bethesda files (FNV, FO3) have
        // zlib streams with corrupted adler32 checksums. In that case, fall
        // back to raw deflate, skipping the 2-byte zlib header (CMF + FLG).
        let mut decompressed = Vec::with_capacity(decompressed_size);
        let zlib_result = {
            let mut decoder = ZlibDecoder::new(compressed);
            decoder.read_to_end(&mut decompressed)
        };

        if zlib_result.is_err() {
            decompressed.clear();
            // Skip the 2-byte zlib header if present (0x78 xx)
            let raw = if compressed.len() >= 2 && compressed[0] == 0x78 {
                &compressed[2..]
            } else {
                compressed
            };
            let mut decoder = DeflateDecoder::new(raw);
            decoder
                .read_to_end(&mut decompressed)
                .map_err(|e| DomError::DecompressionFailed {
                    sig,
                    offset,
                    source: e,
                })?;
        }

        Ok(decompressed)
    }

    /// Parse the group type from raw header values.
    fn parse_group_type(
        &self,
        type_raw: u32,
        label: [u8; 4],
    ) -> Result<group::GroupType> {
        Ok(match type_raw {
            0 => group::GroupType::Top(Signature::from_bytes(&label)),
            1 => group::GroupType::WorldChildren(u32::from_le_bytes(label)),
            2 => group::GroupType::InteriorCellBlock(i32::from_le_bytes(label)),
            3 => group::GroupType::InteriorCellSubBlock(i32::from_le_bytes(label)),
            4 => group::GroupType::ExteriorCellBlock {
                y: i16::from_le_bytes([label[0], label[1]]),
                x: i16::from_le_bytes([label[2], label[3]]),
            },
            5 => group::GroupType::ExteriorCellSubBlock {
                y: i16::from_le_bytes([label[0], label[1]]),
                x: i16::from_le_bytes([label[2], label[3]]),
            },
            6 => group::GroupType::CellChildren(u32::from_le_bytes(label)),
            7 => group::GroupType::TopicChildren(u32::from_le_bytes(label)),
            8 => group::GroupType::CellPersistentChildren(u32::from_le_bytes(label)),
            9 => group::GroupType::CellTemporaryChildren(u32::from_le_bytes(label)),
            10 => group::GroupType::CellVisibleDistantChildren(u32::from_le_bytes(label)),
            _ => anyhow::bail!("Unknown group type: {}", type_raw),
        })
    }

    /// Extract master file list from the file header record.
    fn extract_masters(&self, header: &Record) -> Vec<String> {
        header
            .subrecords_by_sig(Signature::from_bytes(b"MAST"))
            .filter_map(|sr| {
                let data = &sr.raw_data;
                let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                std::str::from_utf8(&data[..len]).ok().map(String::from)
            })
            .collect()
    }
}
