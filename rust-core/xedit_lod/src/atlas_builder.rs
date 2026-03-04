//! Texture atlas builder — bin-packing textures onto atlas sheets.
//!
//! Matches the Delphi xEdit atlas building:
//! 1. Sort textures by max(width, height) descending
//! 2. Binary tree bin-packing
//! 3. Multiple atlas sheets when textures don't fit (numbered 00, 01...)
//! 4. Crop atlas to next power-of-2

use anyhow::Result;

/// A placed texture entry on an atlas sheet
#[derive(Debug, Clone)]
pub struct AtlasEntry {
    pub texture_path: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Which atlas sheet this entry is on (0-based)
    pub atlas_index: u32,
    pub raw_data: Vec<u8>,
}

/// Binary tree node for rectangle packing
#[derive(Debug)]
struct PackNode {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    used: bool,
    right: Option<Box<PackNode>>,
    down: Option<Box<PackNode>>,
}

impl PackNode {
    fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, w, h, used: false, right: None, down: None }
    }

    /// Try to find a spot for a block of size (w, h).
    fn find(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.used {
            // Try right child first, then down
            if let Some(ref mut right) = self.right {
                if let Some(pos) = right.find(w, h) {
                    return Some(pos);
                }
            }
            if let Some(ref mut down) = self.down {
                return down.find(w, h);
            }
            None
        } else if w <= self.w && h <= self.h {
            // Fits here — split this node
            self.used = true;
            self.right = Some(Box::new(PackNode::new(
                self.x + w, self.y,
                self.w.saturating_sub(w), h,
            )));
            self.down = Some(Box::new(PackNode::new(
                self.x, self.y + h,
                self.w, self.h.saturating_sub(h),
            )));
            Some((self.x, self.y))
        } else {
            None
        }
    }
}

/// Atlas builder supporting multiple sheets
pub struct AtlasBuilder {
    pub width: u32,
    pub height: u32,
    pub max_texture_size: u32,
    pub entries: Vec<AtlasEntry>,
    pub sheet_count: u32,
}

impl AtlasBuilder {
    pub fn new(width: u32, height: u32, max_texture_size: u32) -> Self {
        Self {
            width,
            height,
            max_texture_size,
            entries: Vec::new(),
            sheet_count: 0,
        }
    }

    /// Collect and pack all textures. Call add_texture() first, then pack_all().
    /// For simpler usage, use add_texture + build which packs a single sheet.
    pub fn add_texture(&mut self, path: &str, dds_data: &[u8]) -> Result<usize> {
        let (tex_w, tex_h) = parse_dds_dimensions(dds_data)?;
        let w = tex_w.min(self.max_texture_size);
        let h = tex_h.min(self.max_texture_size);

        let idx = self.entries.len();
        self.entries.push(AtlasEntry {
            texture_path: path.to_string(),
            x: 0,
            y: 0,
            width: w,
            height: h,
            atlas_index: 0,
            raw_data: dds_data.to_vec(),
        });

        Ok(idx)
    }

    /// Pack all added textures into atlas sheets using binary tree packing.
    /// Sorts by max(w,h) descending, then packs. Textures that don't fit
    /// on the current sheet go to the next sheet.
    pub fn pack_all(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Sort indices by max(w,h) descending
        let mut indices: Vec<usize> = (0..self.entries.len()).collect();
        indices.sort_by(|&a, &b| {
            let max_a = self.entries[a].width.max(self.entries[a].height);
            let max_b = self.entries[b].width.max(self.entries[b].height);
            max_b.cmp(&max_a)
        });

        let mut current_sheet = 0u32;
        let mut remaining = indices;

        while !remaining.is_empty() {
            let mut root = PackNode::new(0, 0, self.width, self.height);
            let mut packed = Vec::new();
            let mut unpacked = Vec::new();

            for &idx in &remaining {
                let w = self.entries[idx].width;
                let h = self.entries[idx].height;

                if let Some((x, y)) = root.find(w, h) {
                    self.entries[idx].x = x;
                    self.entries[idx].y = y;
                    self.entries[idx].atlas_index = current_sheet;
                    packed.push(idx);
                } else {
                    unpacked.push(idx);
                }
            }

            if packed.is_empty() {
                // Nothing fits even on a fresh sheet — skip these textures
                tracing::warn!(
                    "{} textures too large for {}x{} atlas, skipping",
                    unpacked.len(), self.width, self.height
                );
                break;
            }

            tracing::info!(
                "Atlas sheet {}: packed {} textures ({} remaining)",
                current_sheet, packed.len(), unpacked.len()
            );

            current_sheet += 1;
            remaining = unpacked;
        }

        self.sheet_count = current_sheet;
    }

    /// Build the atlas DDS for a specific sheet index.
    pub fn build_sheet(&self, sheet_index: u32, _compression: &str) -> Result<Vec<u8>> {
        let sheet_entries: Vec<&AtlasEntry> = self.entries.iter()
            .filter(|e| e.atlas_index == sheet_index)
            .collect();

        if sheet_entries.is_empty() {
            return Ok(Vec::new());
        }

        // Find actual used area and crop to next power-of-2
        let max_x = sheet_entries.iter().map(|e| e.x + e.width).max().unwrap_or(0);
        let max_y = sheet_entries.iter().map(|e| e.y + e.height).max().unwrap_or(0);
        let actual_w = next_power_of_2(max_x).min(self.width);
        let actual_h = next_power_of_2(max_y).min(self.height);

        let pixel_count = (actual_w * actual_h) as usize;
        let mut rgba = vec![0u8; pixel_count * 4];

        for entry in &sheet_entries {
            if let Ok(pixels) = decode_dds_to_rgba(&entry.raw_data, entry.width, entry.height) {
                blit_rgba(
                    &mut rgba, actual_w,
                    &pixels, entry.x, entry.y, entry.width, entry.height,
                );
            }
        }

        Ok(write_dds_rgba(&rgba, actual_w, actual_h))
    }

    /// Build a single atlas DDS (sheet 0). For backwards compatibility.
    pub fn build(&self, compression: &str) -> Result<Vec<u8>> {
        self.build_sheet(0, compression)
    }

    /// Get the actual dimensions of a specific sheet (cropped to power-of-2).
    pub fn sheet_dimensions(&self, sheet_index: u32) -> (u32, u32) {
        let sheet_entries: Vec<&AtlasEntry> = self.entries.iter()
            .filter(|e| e.atlas_index == sheet_index)
            .collect();

        if sheet_entries.is_empty() {
            return (0, 0);
        }

        let max_x = sheet_entries.iter().map(|e| e.x + e.width).max().unwrap_or(0);
        let max_y = sheet_entries.iter().map(|e| e.y + e.height).max().unwrap_or(0);
        let w = next_power_of_2(max_x).min(self.width);
        let h = next_power_of_2(max_y).min(self.height);
        (w, h)
    }
}

/// Next power of 2 >= n.
fn next_power_of_2(n: u32) -> u32 {
    if n == 0 { return 1; }
    let mut v = n - 1;
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v + 1
}

/// Parse DDS header to extract width and height.
pub fn parse_dds_dimensions(data: &[u8]) -> Result<(u32, u32)> {
    anyhow::ensure!(data.len() >= 128, "DDS file too small for header");
    anyhow::ensure!(&data[0..4] == b"DDS ", "Not a DDS file (bad magic)");

    let height = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let width = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

    Ok((width, height))
}

/// Simple DDS decode to RGBA (handles uncompressed, DXT1/BC1, DXT3/BC2, DXT5/BC3).
fn decode_dds_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    if data.len() < 128 {
        anyhow::bail!("DDS too small");
    }

    let pixel_format_offset = 76;
    let fourcc_offset = pixel_format_offset + 8;

    let fourcc = if data.len() >= fourcc_offset + 4 {
        &data[fourcc_offset..fourcc_offset + 4]
    } else {
        b"\0\0\0\0"
    };

    let pixel_data = &data[128..];
    let pixel_count = (width * height) as usize;

    if fourcc == b"DXT1" {
        Ok(decode_bc1(pixel_data, width, height))
    } else if fourcc == b"DXT3" {
        Ok(decode_bc2(pixel_data, width, height))
    } else if fourcc == b"DXT5" {
        Ok(decode_bc3(pixel_data, width, height))
    } else {
        // Assume uncompressed RGBA or BGRA
        let rgb_size = &data[pixel_format_offset + 4..pixel_format_offset + 8];
        let bits_per_pixel = u32::from_le_bytes([rgb_size[0], rgb_size[1], rgb_size[2], rgb_size[3]]);

        if bits_per_pixel == 32 && pixel_data.len() >= pixel_count * 4 {
            Ok(pixel_data[..pixel_count * 4].to_vec())
        } else if bits_per_pixel == 24 && pixel_data.len() >= pixel_count * 3 {
            let mut rgba = vec![0u8; pixel_count * 4];
            for i in 0..pixel_count {
                rgba[i * 4] = pixel_data[i * 3];
                rgba[i * 4 + 1] = pixel_data[i * 3 + 1];
                rgba[i * 4 + 2] = pixel_data[i * 3 + 2];
                rgba[i * 4 + 3] = 255;
            }
            Ok(rgba)
        } else {
            // Fallback: solid gray
            Ok(vec![128u8; pixel_count * 4])
        }
    }
}

/// Decode BC1/DXT1 compressed data to RGBA.
fn decode_bc1(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_idx = (by * blocks_x + bx) as usize;
            let block_offset = block_idx * 8;
            if block_offset + 8 > data.len() { break; }

            let block = &data[block_offset..block_offset + 8];
            let c0 = u16::from_le_bytes([block[0], block[1]]);
            let c1 = u16::from_le_bytes([block[2], block[3]]);
            let colors = decode_bc1_colors(c0, c1);

            for py in 0..4u32 {
                for px in 0..4u32 {
                    let x = bx * 4 + px;
                    let y = by * 4 + py;
                    if x < width && y < height {
                        let bit_idx = py * 4 + px;
                        let selector = (block[4 + (bit_idx / 4) as usize] >> ((bit_idx % 4) * 2)) & 0x03;
                        let color = &colors[selector as usize];
                        let pixel_offset = ((y * width + x) * 4) as usize;
                        rgba[pixel_offset..pixel_offset + 4].copy_from_slice(color);
                    }
                }
            }
        }
    }
    rgba
}

/// Decode BC2/DXT3 compressed data to RGBA.
fn decode_bc2(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_idx = (by * blocks_x + bx) as usize;
            let block_offset = block_idx * 16; // 16 bytes per BC2 block
            if block_offset + 16 > data.len() { break; }

            let alpha_block = &data[block_offset..block_offset + 8];
            let color_block = &data[block_offset + 8..block_offset + 16];

            let c0 = u16::from_le_bytes([color_block[0], color_block[1]]);
            let c1 = u16::from_le_bytes([color_block[2], color_block[3]]);
            let colors = decode_bc1_colors_opaque(c0, c1);

            for py in 0..4u32 {
                for px in 0..4u32 {
                    let x = bx * 4 + px;
                    let y = by * 4 + py;
                    if x < width && y < height {
                        let bit_idx = py * 4 + px;
                        let selector = (color_block[4 + (bit_idx / 4) as usize] >> ((bit_idx % 4) * 2)) & 0x03;
                        let color = &colors[selector as usize];
                        let pixel_offset = ((y * width + x) * 4) as usize;
                        rgba[pixel_offset] = color[0];
                        rgba[pixel_offset + 1] = color[1];
                        rgba[pixel_offset + 2] = color[2];
                        // Explicit alpha from alpha block (4-bit per pixel)
                        let alpha_byte = alpha_block[(py * 2 + px / 2) as usize];
                        let alpha_nibble = if px % 2 == 0 { alpha_byte & 0x0F } else { alpha_byte >> 4 };
                        rgba[pixel_offset + 3] = alpha_nibble | (alpha_nibble << 4);
                    }
                }
            }
        }
    }
    rgba
}

/// Decode BC3/DXT5 compressed data to RGBA.
fn decode_bc3(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let blocks_x = (width + 3) / 4;
    let blocks_y = (height + 3) / 4;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let block_idx = (by * blocks_x + bx) as usize;
            let block_offset = block_idx * 16;
            if block_offset + 16 > data.len() { break; }

            let alpha_block = &data[block_offset..block_offset + 8];
            let color_block = &data[block_offset + 8..block_offset + 16];

            let c0 = u16::from_le_bytes([color_block[0], color_block[1]]);
            let c1 = u16::from_le_bytes([color_block[2], color_block[3]]);
            let colors = decode_bc1_colors_opaque(c0, c1);

            // Decode interpolated alpha
            let a0 = alpha_block[0];
            let a1 = alpha_block[1];
            let alpha_lut = decode_bc3_alpha_lut(a0, a1);
            let mut alpha_bits = 0u64;
            for i in 2..8 {
                alpha_bits |= (alpha_block[i] as u64) << ((i - 2) * 8);
            }

            for py in 0..4u32 {
                for px in 0..4u32 {
                    let x = bx * 4 + px;
                    let y = by * 4 + py;
                    if x < width && y < height {
                        let bit_idx = py * 4 + px;
                        let selector = (color_block[4 + (bit_idx / 4) as usize] >> ((bit_idx % 4) * 2)) & 0x03;
                        let color = &colors[selector as usize];
                        let pixel_offset = ((y * width + x) * 4) as usize;
                        rgba[pixel_offset] = color[0];
                        rgba[pixel_offset + 1] = color[1];
                        rgba[pixel_offset + 2] = color[2];
                        let alpha_idx = (alpha_bits >> (bit_idx * 3)) & 0x07;
                        rgba[pixel_offset + 3] = alpha_lut[alpha_idx as usize];
                    }
                }
            }
        }
    }
    rgba
}

fn decode_bc3_alpha_lut(a0: u8, a1: u8) -> [u8; 8] {
    let mut lut = [0u8; 8];
    lut[0] = a0;
    lut[1] = a1;
    if a0 > a1 {
        for i in 2..8 {
            lut[i] = (((8 - i) as u16 * a0 as u16 + (i - 1) as u16 * a1 as u16) / 7) as u8;
        }
    } else {
        for i in 2..6 {
            lut[i] = (((6 - i) as u16 * a0 as u16 + (i - 1) as u16 * a1 as u16) / 5) as u8;
        }
        lut[6] = 0;
        lut[7] = 255;
    }
    lut
}

fn decode_bc1_colors(c0: u16, c1: u16) -> [[u8; 4]; 4] {
    let (r0, g0, b0) = expand_565(c0);
    let (r1, g1, b1) = expand_565(c1);

    if c0 > c1 {
        [
            [r0, g0, b0, 255],
            [r1, g1, b1, 255],
            [lerp3(r0, r1, 2, 1), lerp3(g0, g1, 2, 1), lerp3(b0, b1, 2, 1), 255],
            [lerp3(r0, r1, 1, 2), lerp3(g0, g1, 1, 2), lerp3(b0, b1, 1, 2), 255],
        ]
    } else {
        [
            [r0, g0, b0, 255],
            [r1, g1, b1, 255],
            [lerp2(r0, r1), lerp2(g0, g1), lerp2(b0, b1), 255],
            [0, 0, 0, 0],
        ]
    }
}

fn decode_bc1_colors_opaque(c0: u16, c1: u16) -> [[u8; 4]; 4] {
    let (r0, g0, b0) = expand_565(c0);
    let (r1, g1, b1) = expand_565(c1);

    [
        [r0, g0, b0, 255],
        [r1, g1, b1, 255],
        [lerp3(r0, r1, 2, 1), lerp3(g0, g1, 2, 1), lerp3(b0, b1, 2, 1), 255],
        [lerp3(r0, r1, 1, 2), lerp3(g0, g1, 1, 2), lerp3(b0, b1, 1, 2), 255],
    ]
}

fn expand_565(c: u16) -> (u8, u8, u8) {
    let r = ((c >> 11) & 0x1F) as u8;
    let g = ((c >> 5) & 0x3F) as u8;
    let b = (c & 0x1F) as u8;
    ((r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2))
}

fn lerp3(a: u8, b: u8, wa: u16, wb: u16) -> u8 {
    ((wa * a as u16 + wb * b as u16) / 3) as u8
}

fn lerp2(a: u8, b: u8) -> u8 {
    ((a as u16 + b as u16) / 2) as u8
}

/// Blit RGBA pixels onto the atlas buffer.
fn blit_rgba(
    atlas: &mut [u8], atlas_width: u32,
    src: &[u8], dst_x: u32, dst_y: u32, src_w: u32, src_h: u32,
) {
    for y in 0..src_h {
        for x in 0..src_w {
            let src_offset = ((y * src_w + x) * 4) as usize;
            let dst_offset = (((dst_y + y) * atlas_width + (dst_x + x)) * 4) as usize;
            if src_offset + 4 <= src.len() && dst_offset + 4 <= atlas.len() {
                atlas[dst_offset..dst_offset + 4].copy_from_slice(&src[src_offset..src_offset + 4]);
            }
        }
    }
}

/// Write an uncompressed RGBA DDS file.
fn write_dds_rgba(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(128 + rgba.len());

    out.extend_from_slice(b"DDS ");
    out.extend_from_slice(&124u32.to_le_bytes());
    out.extend_from_slice(&0x0002_100Fu32.to_le_bytes()); // flags
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&(width * 4).to_le_bytes()); // pitch
    out.extend_from_slice(&0u32.to_le_bytes()); // depth
    out.extend_from_slice(&1u32.to_le_bytes()); // mipmap count
    out.extend_from_slice(&[0u8; 44]); // reserved

    // Pixel format
    out.extend_from_slice(&32u32.to_le_bytes()); // size
    out.extend_from_slice(&0x41u32.to_le_bytes()); // flags (RGB|ALPHA)
    out.extend_from_slice(&0u32.to_le_bytes()); // fourcc
    out.extend_from_slice(&32u32.to_le_bytes()); // bits
    out.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // R mask
    out.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // G mask
    out.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // B mask
    out.extend_from_slice(&0xFF00_0000u32.to_le_bytes()); // A mask

    out.extend_from_slice(&0x1000u32.to_le_bytes()); // caps
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    out.extend_from_slice(rgba);
    out
}
