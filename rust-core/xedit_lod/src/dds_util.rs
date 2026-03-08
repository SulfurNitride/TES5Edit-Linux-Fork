//! DDS texture utilities using image_dds + directxtex.
//!
//! Handles reading, writing, compression, and compositing for LOD atlas building.
//! Format codes: 200=DXT1, 202=DXT5, 205=BC5

use anyhow::{Context, Result};
use image_dds::{ddsfile::{Dds, FourCC, PixelFormat}, ImageFormat, Mipmaps, Quality, SurfaceRgba8};
use image_dds::ddsfile::PixelFormatFlags;
use std::io::Cursor;

/// Basic DDS file information.
#[derive(Debug, Clone)]
pub struct DdsInfo {
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub mipmap_count: u32,
}

/// Map format code (from settings file) to image_dds ImageFormat.
pub fn format_from_code(code: u32) -> Result<ImageFormat> {
    match code {
        200 => Ok(ImageFormat::BC1RgbaUnorm),
        202 => Ok(ImageFormat::BC3RgbaUnorm),
        205 => Ok(ImageFormat::BC5RgUnorm),
        _ => anyhow::bail!("Unknown DDS format code: {}", code),
    }
}

/// Read DDS header information from raw bytes.
pub fn read_dds_info(data: &[u8]) -> Result<DdsInfo> {
    let cursor = Cursor::new(data);
    let dds = Dds::read(cursor).context("Failed to parse DDS")?;

    let format = image_dds::dds_image_format(&dds)
        .map(|f| format!("{:?}", f))
        .unwrap_or_else(|_| "Unknown".to_string());

    Ok(DdsInfo {
        width: dds.header.width,
        height: dds.header.height,
        format,
        mipmap_count: dds.header.mip_map_count.unwrap_or(1),
    })
}

/// Decompress a DDS texture to RGBA8 pixels.
///
/// Returns (rgba_pixels, width, height).
pub fn decompress_to_rgba(data: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    let cursor = Cursor::new(data);
    let dds = Dds::read(cursor).context("Failed to parse DDS")?;

    // Try image_dds first
    match image_dds::image_from_dds(&dds, 0) {
        Ok(rgba_image) => {
            let w = rgba_image.width();
            let h = rgba_image.height();
            Ok((rgba_image.into_raw(), w, h))
        }
        Err(_e) => {
            // Fallback to directxtex for legacy formats
            decompress_with_directxtex(data)
        }
    }
}

/// Decompress using DirectXTex (handles legacy L8, RGB565, etc.)
fn decompress_with_directxtex(data: &[u8]) -> Result<(Vec<u8>, u32, u32)> {
    use directxtex::{ScratchImage, DXGI_FORMAT, DDS_FLAGS, TEX_FILTER_FLAGS};

    let flags = DDS_FLAGS::DDS_FLAGS_ALLOW_LARGE_FILES | DDS_FLAGS::DDS_FLAGS_EXPAND_LUMINANCE;
    let scratch = ScratchImage::load_dds(data, flags, None, None)
        .context("DirectXTex: failed to load DDS")?;

    let metadata = scratch.metadata();
    let width = metadata.width as u32;
    let height = metadata.height as u32;

    let rgba_scratch = if metadata.format != DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM {
        scratch.convert(
            DXGI_FORMAT::DXGI_FORMAT_R8G8B8A8_UNORM,
            TEX_FILTER_FLAGS::TEX_FILTER_DEFAULT,
            0.5,
        ).context("DirectXTex: conversion to RGBA failed")?
    } else {
        scratch
    };

    let images = rgba_scratch.images();
    anyhow::ensure!(!images.is_empty(), "DirectXTex: no images in scratch");

    let image = &images[0];
    let pixel_data = unsafe {
        std::slice::from_raw_parts(image.pixels, image.slice_pitch)
    };

    Ok((pixel_data.to_vec(), width, height))
}

/// Compress RGBA8 pixels to DDS with the specified format code.
///
/// Format codes: 200=DXT1, 202=DXT5, 205=BC5
pub fn compress_to_dds(
    rgba: &[u8],
    width: u32,
    height: u32,
    format_code: u32,
    generate_mipmaps: bool,
) -> Result<Vec<u8>> {
    let image_format = format_from_code(format_code)?;

    let mipmaps = if generate_mipmaps {
        Mipmaps::GeneratedAutomatic
    } else {
        Mipmaps::Disabled
    };

    // Build an RgbaImage from the raw pixels
    let rgba_image = image::RgbaImage::from_raw(width, height, rgba.to_vec())
        .context("Invalid RGBA dimensions")?;

    let surface = SurfaceRgba8::from_image(&rgba_image);
    let encoded = surface.encode(image_format, Quality::Normal, mipmaps)
        .map_err(|e| anyhow::anyhow!("Failed to encode DDS: {}", e))?;

    let mut dds = encoded.to_dds()
        .map_err(|e| anyhow::anyhow!("Failed to create DDS: {}", e))?;

    // Downgrade DX10 extended header to legacy D3D FourCC for game compatibility.
    // Gamebryo-era engines (FO3/FNV/Oblivion/Skyrim LE) don't understand DX10 headers.
    if dds.header10.is_some() {
        let legacy_fourcc = match format_code {
            200 => Some(u32::from_le_bytes(*b"DXT1")),
            202 => Some(u32::from_le_bytes(*b"DXT5")),
            _ => None, // BC5 has no legacy equivalent; keep DX10
        };
        if let Some(fourcc_val) = legacy_fourcc {
            dds.header.spf = PixelFormat {
                size: 32,
                flags: PixelFormatFlags::FOURCC,
                fourcc: Some(FourCC(fourcc_val)),
                rgb_bit_count: None,
                r_bit_mask: None,
                g_bit_mask: None,
                b_bit_mask: None,
                a_bit_mask: None,
            };
            dds.header10 = None;
        }
    }

    // Ensure depth and mipmap count fields match reference conventions
    if dds.header.depth.is_none() || dds.header.depth == Some(0) {
        dds.header.depth = Some(1);
    }
    if !generate_mipmaps {
        dds.header.mip_map_count = Some(1);
    }

    let mut buf = Vec::new();
    dds.write(&mut buf).context("Failed to write DDS data")?;

    Ok(buf)
}

/// Compress RGBA8 pixels to DXT1 (format code 200).
pub fn compress_dxt1(rgba: &[u8], width: u32, height: u32, mipmaps: bool) -> Result<Vec<u8>> {
    compress_to_dds(rgba, width, height, 200, mipmaps)
}

/// Compress RGBA8 pixels to DXT5 (format code 202).
pub fn compress_dxt5(rgba: &[u8], width: u32, height: u32, mipmaps: bool) -> Result<Vec<u8>> {
    compress_to_dds(rgba, width, height, 202, mipmaps)
}

/// Compress RGBA8 pixels to BC5 (format code 205).
pub fn compress_bc5(rgba: &[u8], width: u32, height: u32, mipmaps: bool) -> Result<Vec<u8>> {
    compress_to_dds(rgba, width, height, 205, mipmaps)
}

/// Composite (blit) a source rectangle onto a destination RGBA buffer.
///
/// Copies `src_w × src_h` pixels from `src` to position `(dst_x, dst_y)` in `dst`.
/// Both `src` and `dst` are RGBA8 (4 bytes per pixel).
pub fn composite_rect(
    dst: &mut [u8],
    dst_w: u32,
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_x: u32,
    dst_y: u32,
) {
    let dst_stride = (dst_w * 4) as usize;
    let src_stride = (src_w * 4) as usize;

    for row in 0..src_h {
        let src_offset = (row as usize) * src_stride;
        let dst_offset = ((dst_y + row) as usize) * dst_stride + (dst_x as usize) * 4;

        let src_end = src_offset + src_stride;
        let dst_end = dst_offset + src_stride;

        if src_end <= src.len() && dst_end <= dst.len() {
            dst[dst_offset..dst_end].copy_from_slice(&src[src_offset..src_end]);
        }
    }
}

/// Create a blank RGBA8 canvas of the given dimensions (all zeros).
pub fn create_canvas(width: u32, height: u32) -> Vec<u8> {
    vec![0u8; (width * height * 4) as usize]
}

/// Resize RGBA8 image using bilinear interpolation.
pub fn resize_rgba(src: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let rgba_image = image::RgbaImage::from_raw(src_w, src_h, src.to_vec())
        .expect("Invalid RGBA dimensions for resize");
    let dynamic = image::DynamicImage::ImageRgba8(rgba_image);
    let resized = dynamic.resize_exact(dst_w, dst_h, image::imageops::FilterType::Lanczos3);
    resized.into_rgba8().into_raw()
}
