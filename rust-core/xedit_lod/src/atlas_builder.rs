//! Texture atlas builder — shelf-based bin-packing textures onto atlas sheets.

use anyhow::Result;

/// A placed texture entry on the atlas
#[derive(Debug, Clone)]
pub struct AtlasEntry {
    pub texture_path: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub raw_data: Vec<u8>, // Raw DDS data for this texture
}

/// Shelf-based bin-packing texture atlas builder
#[derive(Debug)]
pub struct AtlasBuilder {
    pub width: u32,
    pub height: u32,
    pub max_texture_size: u32,
    pub entries: Vec<AtlasEntry>,
    // Shelf packing state
    shelves: Vec<Shelf>,
}

#[derive(Debug)]
struct Shelf {
    y: u32,        // Top of this shelf
    height: u32,   // Height of tallest item on this shelf
    x_cursor: u32, // Next free X position on this shelf
}

impl AtlasBuilder {
    pub fn new(width: u32, height: u32, max_texture_size: u32) -> Self {
        Self {
            width,
            height,
            max_texture_size,
            entries: Vec::new(),
            shelves: vec![Shelf {
                y: 0,
                height: 0,
                x_cursor: 0,
            }],
        }
    }

    /// Add a texture to the atlas. Returns the entry index.
    /// The DDS data is stored for later atlas composition.
    pub fn add_texture(&mut self, path: &str, dds_data: &[u8]) -> Result<usize> {
        // Parse DDS header to get dimensions
        let (tex_w, tex_h) = parse_dds_dimensions(dds_data)?;

        // Clamp to max texture size
        let w = tex_w.min(self.max_texture_size);
        let h = tex_h.min(self.max_texture_size);

        // Find a shelf that fits this texture
        let (x, y) = self.pack(w, h)?;

        let idx = self.entries.len();
        self.entries.push(AtlasEntry {
            texture_path: path.to_string(),
            x,
            y,
            width: w,
            height: h,
            raw_data: dds_data.to_vec(),
        });

        Ok(idx)
    }

    fn pack(&mut self, w: u32, h: u32) -> Result<(u32, u32)> {
        // Try each existing shelf
        for shelf in &mut self.shelves {
            if shelf.x_cursor + w <= self.width
                && shelf.y + h.max(shelf.height) <= self.height
            {
                let x = shelf.x_cursor;
                let y = shelf.y;
                shelf.x_cursor += w;
                shelf.height = shelf.height.max(h);
                return Ok((x, y));
            }
        }

        // Start a new shelf
        let last_shelf = self.shelves.last().unwrap();
        let new_y = last_shelf.y + last_shelf.height;

        if new_y + h > self.height {
            anyhow::bail!(
                "Atlas full: cannot fit {}x{} texture (atlas is {}x{})",
                w,
                h,
                self.width,
                self.height
            );
        }

        let x = 0u32;
        self.shelves.push(Shelf {
            y: new_y,
            height: h,
            x_cursor: w,
        });

        Ok((x, new_y))
    }

    /// Build the final atlas as a raw DDS file.
    /// Returns an empty vec if no entries.
    pub fn build(&self, _compression: &str) -> Result<Vec<u8>> {
        if self.entries.is_empty() {
            return Ok(Vec::new());
        }

        // Compose an uncompressed RGBA DDS by decoding each entry and blitting
        // onto the atlas buffer. A full implementation would re-encode with the
        // requested compression format afterward.
        let pixel_count = (self.width * self.height) as usize;
        let mut rgba = vec![0u8; pixel_count * 4];

        for entry in &self.entries {
            if let Ok(pixels) = decode_dds_to_rgba(&entry.raw_data, entry.width, entry.height) {
                blit_rgba(
                    &mut rgba,
                    self.width,
                    &pixels,
                    entry.x,
                    entry.y,
                    entry.width,
                    entry.height,
                );
            }
        }

        Ok(write_dds_rgba(&rgba, self.width, self.height))
    }
}

/// Parse DDS header to extract width and height.
fn parse_dds_dimensions(data: &[u8]) -> Result<(u32, u32)> {
    anyhow::ensure!(data.len() >= 128, "DDS file too small for header");
    anyhow::ensure!(&data[0..4] == b"DDS ", "Not a DDS file (bad magic)");

    let height = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
    let width = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

    Ok((width, height))
}

/// Simple DDS decode to RGBA (handles uncompressed and DXT1/BC1).
fn decode_dds_to_rgba(data: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    if data.len() < 128 {
        anyhow::bail!("DDS too small");
    }

    let pixel_format_offset = 76; // offset to ddspf in header
    let fourcc_offset = pixel_format_offset + 8;

    let fourcc = if data.len() >= fourcc_offset + 4 {
        &data[fourcc_offset..fourcc_offset + 4]
    } else {
        b"\0\0\0\0"
    };

    let pixel_data = &data[128..]; // Skip header
    let pixel_count = (width * height) as usize;

    if fourcc == b"DXT1" {
        // BC1/DXT1 decode
        Ok(decode_bc1(pixel_data, width, height))
    } else {
        // Assume uncompressed RGBA or BGRA
        let rgb_size = &data[pixel_format_offset + 4..pixel_format_offset + 8];
        let bits_per_pixel =
            u32::from_le_bytes([rgb_size[0], rgb_size[1], rgb_size[2], rgb_size[3]]);

        if bits_per_pixel == 32 && pixel_data.len() >= pixel_count * 4 {
            Ok(pixel_data[..pixel_count * 4].to_vec())
        } else if bits_per_pixel == 24 && pixel_data.len() >= pixel_count * 3 {
            // RGB -> RGBA
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

            if block_offset + 8 > data.len() {
                break;
            }

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
                        let selector = (block[4 + (bit_idx / 4) as usize]
                            >> ((bit_idx % 4) * 2))
                            & 0x03;
                        let color = &colors[selector as usize];
                        let pixel_offset = ((y * width + x) * 4) as usize;
                        rgba[pixel_offset] = color[0];
                        rgba[pixel_offset + 1] = color[1];
                        rgba[pixel_offset + 2] = color[2];
                        rgba[pixel_offset + 3] = color[3];
                    }
                }
            }
        }
    }

    rgba
}

fn decode_bc1_colors(c0: u16, c1: u16) -> [[u8; 4]; 4] {
    let r0 = ((c0 >> 11) & 0x1F) as u8;
    let g0 = ((c0 >> 5) & 0x3F) as u8;
    let b0 = (c0 & 0x1F) as u8;

    let r1 = ((c1 >> 11) & 0x1F) as u8;
    let g1 = ((c1 >> 5) & 0x3F) as u8;
    let b1 = (c1 & 0x1F) as u8;

    // Expand to 8-bit
    let r0 = (r0 << 3) | (r0 >> 2);
    let g0 = (g0 << 2) | (g0 >> 4);
    let b0 = (b0 << 3) | (b0 >> 2);
    let r1 = (r1 << 3) | (r1 >> 2);
    let g1 = (g1 << 2) | (g1 >> 4);
    let b1 = (b1 << 3) | (b1 >> 2);

    if c0 > c1 {
        [
            [r0, g0, b0, 255],
            [r1, g1, b1, 255],
            [
                ((2 * r0 as u16 + r1 as u16) / 3) as u8,
                ((2 * g0 as u16 + g1 as u16) / 3) as u8,
                ((2 * b0 as u16 + b1 as u16) / 3) as u8,
                255,
            ],
            [
                ((r0 as u16 + 2 * r1 as u16) / 3) as u8,
                ((g0 as u16 + 2 * g1 as u16) / 3) as u8,
                ((b0 as u16 + 2 * b1 as u16) / 3) as u8,
                255,
            ],
        ]
    } else {
        [
            [r0, g0, b0, 255],
            [r1, g1, b1, 255],
            [
                ((r0 as u16 + r1 as u16) / 2) as u8,
                ((g0 as u16 + g1 as u16) / 2) as u8,
                ((b0 as u16 + b1 as u16) / 2) as u8,
                255,
            ],
            [0, 0, 0, 0], // transparent
        ]
    }
}

/// Blit RGBA pixels onto the atlas buffer.
fn blit_rgba(
    atlas: &mut [u8],
    atlas_width: u32,
    src: &[u8],
    dst_x: u32,
    dst_y: u32,
    src_w: u32,
    src_h: u32,
) {
    for y in 0..src_h {
        for x in 0..src_w {
            let src_offset = ((y * src_w + x) * 4) as usize;
            let dst_offset = (((dst_y + y) * atlas_width + (dst_x + x)) * 4) as usize;
            if src_offset + 4 <= src.len() && dst_offset + 4 <= atlas.len() {
                atlas[dst_offset..dst_offset + 4]
                    .copy_from_slice(&src[src_offset..src_offset + 4]);
            }
        }
    }
}

/// Write an uncompressed RGBA DDS file.
fn write_dds_rgba(rgba: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(128 + rgba.len());

    // DDS magic
    out.extend_from_slice(b"DDS ");

    // DDS_HEADER (124 bytes)
    out.extend_from_slice(&124u32.to_le_bytes()); // dwSize
    // dwFlags: CAPS|HEIGHT|WIDTH|PITCH|PIXELFORMAT|MIPMAPCOUNT
    out.extend_from_slice(&0x0002_100Fu32.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes()); // dwHeight
    out.extend_from_slice(&width.to_le_bytes()); // dwWidth
    out.extend_from_slice(&(width * 4).to_le_bytes()); // dwPitchOrLinearSize
    out.extend_from_slice(&0u32.to_le_bytes()); // dwDepth
    out.extend_from_slice(&1u32.to_le_bytes()); // dwMipMapCount
    out.extend_from_slice(&[0u8; 44]); // dwReserved1[11]

    // DDS_PIXELFORMAT (32 bytes)
    out.extend_from_slice(&32u32.to_le_bytes()); // dwSize
    out.extend_from_slice(&0x41u32.to_le_bytes()); // dwFlags (RGB | ALPHAPIXELS)
    out.extend_from_slice(&0u32.to_le_bytes()); // dwFourCC
    out.extend_from_slice(&32u32.to_le_bytes()); // dwRGBBitCount
    out.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // dwRBitMask
    out.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // dwGBitMask
    out.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // dwBBitMask
    out.extend_from_slice(&0xFF00_0000u32.to_le_bytes()); // dwABitMask

    // Caps
    out.extend_from_slice(&0x1000u32.to_le_bytes()); // dwCaps (TEXTURE)
    out.extend_from_slice(&0u32.to_le_bytes()); // dwCaps2
    out.extend_from_slice(&0u32.to_le_bytes()); // dwCaps3
    out.extend_from_slice(&0u32.to_le_bytes()); // dwCaps4
    out.extend_from_slice(&0u32.to_le_bytes()); // dwReserved2

    // Pixel data
    out.extend_from_slice(rgba);

    out
}
