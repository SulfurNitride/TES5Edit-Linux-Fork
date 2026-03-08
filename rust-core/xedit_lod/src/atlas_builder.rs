//! Tree billboard atlas builder.
//!
//! Loads billboard textures, packs them into an atlas, and produces
//! the tree atlas DDS and TreeTypes.lst files.
//!
//! Ported from TwbLodTES5TreeList.BuildAtlas in wbLOD.pas.

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::bin_packer::BinBlock;
use crate::dds_util;
use crate::reference_scanner::{TreeBaseInfo, billboard_path};
use crate::resource_loader::ResourceLoader;
use crate::trees_lod::{TreeType, TreeTypeList, BillboardConfig};

/// Maximum atlas size (8192x8192).
const MAX_ATLAS_SIZE: u32 = 8192;

/// Atlas padding between billboards.
const ATLAS_PADDING: u32 = 2;

/// Information about a loaded billboard texture.
#[derive(Debug)]
struct LoadedBillboard {
    /// Index of the tree type.
    type_index: i32,
    /// RGBA pixel data.
    rgba: Vec<u8>,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// CRC32 of the raw DDS data (for dedup).
    crc32: u32,
    /// Billboard config (width/height/shift/scale from .txt file).
    _config: BillboardConfig,
    /// Game-units width (from OBND or .txt).
    game_width: f32,
    /// Game-units height (from OBND or .txt).
    game_height: f32,
}

/// Result of building a tree atlas.
#[derive(Debug)]
pub struct AtlasResult {
    /// DXT5-compressed atlas DDS bytes (diffuse).
    pub diffuse_dds: Vec<u8>,
    /// DXT5-compressed atlas DDS bytes (normal map, if available).
    pub normal_dds: Option<Vec<u8>>,
    /// Tree type list for LST file.
    pub tree_types: TreeTypeList,
    /// Atlas dimensions.
    pub atlas_width: u32,
    pub atlas_height: u32,
}

/// Build the tree billboard atlas from tree base records.
///
/// # Arguments
/// * `tree_bases` - Tree base records with billboard paths
/// * `loader` - Resource loader for reading textures
/// * `diffuse_format` - DDS format code for diffuse atlas (usually 202 = DXT5)
/// * `normal_format` - DDS format code for normal atlas (usually 202 = DXT5)
/// * `build_normal` - Whether to build a normal map atlas
/// * `brightness` - Brightness adjustment (-255..255, 0 = none)
pub fn build_tree_atlas(
    tree_bases: &[TreeBaseInfo],
    loader: &ResourceLoader,
    diffuse_format: u32,
    normal_format: u32,
    build_normal: bool,
    brightness: i32,
) -> Result<AtlasResult> {
    // Step 1: Load billboard textures and configs
    let mut billboards = Vec::new();
    let mut seen_crc: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    let mut type_index = 0i32;

    for base in tree_bases {
        let bb_path = billboard_path(&base.plugin_filename, &base.model_path, base.form_id);

        // Try to load the billboard DDS
        let dds_data = match loader.load(&bb_path) {
            Ok(data) => data,
            Err(_) => {
                warn!("Billboard not found: {} (base {:08X} {})", bb_path, base.form_id, base.editor_id);
                continue;
            }
        };

        // CRC32 for dedup
        let crc = crc32fast::hash(&dds_data);

        // Load billboard .txt config
        let txt_path = bb_path.replace(".dds", ".txt");
        let config = match loader.load(&txt_path) {
            Ok(txt_data) => {
                let content = String::from_utf8_lossy(&txt_data);
                BillboardConfig::parse(&content)
            }
            Err(_) => BillboardConfig::default(),
        };

        // Width/Height: prefer .txt values, fall back to OBND
        let game_width = if config.width > 0.0 { config.width } else { base.width };
        let game_height = if config.height > 0.0 { config.height } else { base.height };

        // Check for duplicate (by CRC32)
        if seen_crc.contains_key(&crc) {
            debug!("Duplicate billboard CRC {:08X} for {:08X}, reusing", crc, base.form_id);
        }

        // Decompress to RGBA
        let (rgba, w, h) = match dds_util::decompress_to_rgba(&dds_data) {
            Ok(result) => result,
            Err(e) => {
                warn!("Failed to decompress billboard {}: {}", bb_path, e);
                continue;
            }
        };

        let idx = type_index;
        type_index += 1;

        seen_crc.entry(crc).or_insert(billboards.len());

        billboards.push(LoadedBillboard {
            type_index: idx,
            rgba,
            width: w,
            height: h,
            crc32: crc,
            _config: config,
            game_width,
            game_height,
        });
    }

    if billboards.is_empty() {
        anyhow::bail!("No valid billboard textures found");
    }

    info!("Loaded {} billboard textures", billboards.len());

    // Step 2: Build unique blocks for packing (dedup by CRC32)
    let mut unique_blocks: Vec<BinBlock> = Vec::new();
    let mut crc_to_block: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();

    for bb in &billboards {
        if crc_to_block.contains_key(&bb.crc32) {
            continue;
        }
        crc_to_block.insert(bb.crc32, unique_blocks.len());
        unique_blocks.push(BinBlock {
            w: bb.width,
            h: bb.height,
            x: 0,
            y: 0,
            index: bb.type_index as usize,
            placed: false,
        });
    }

    // Step 3: Pack into atlas with auto-growth
    let (atlas_w, atlas_h) = crate::bin_packer::fit_with_growth(
        &mut unique_blocks,
        512, 512,
        MAX_ATLAS_SIZE, MAX_ATLAS_SIZE,
        ATLAS_PADDING, ATLAS_PADDING,
    ).ok_or_else(|| anyhow::anyhow!("Can't fit billboards on atlas, not enough space"))?;

    info!("Atlas size: {}x{} for {} unique textures", atlas_w, atlas_h, unique_blocks.len());

    // Step 4: Composite atlas
    let mut canvas = dds_util::create_canvas(atlas_w, atlas_h);

    for bb in &billboards {
        let block_idx = crc_to_block[&bb.crc32];
        let block = &unique_blocks[block_idx];

        dds_util::composite_rect(
            &mut canvas,
            atlas_w,
            &bb.rgba,
            bb.width,
            bb.height,
            block.x,
            block.y,
        );
    }

    // Apply brightness adjustment
    if brightness != 0 {
        apply_brightness(&mut canvas, brightness);
    }

    // Step 5: Build tree type list with UV coordinates
    let mut tree_types = Vec::new();
    for bb in &billboards {
        let block_idx = crc_to_block[&bb.crc32];
        let block = &unique_blocks[block_idx];

        tree_types.push(TreeType {
            index: bb.type_index,
            width: bb.game_width,
            height: bb.game_height,
            uv_min_x: block.x as f32 / atlas_w as f32,
            uv_min_y: block.y as f32 / atlas_h as f32,
            uv_max_x: (block.x + bb.width) as f32 / atlas_w as f32,
            uv_max_y: (block.y + bb.height) as f32 / atlas_h as f32,
            unknown: 0,
        });
    }

    let lst = TreeTypeList { types: tree_types };

    // Step 6: Compress atlas to DDS
    let diffuse_dds = dds_util::compress_to_dds(&canvas, atlas_w, atlas_h, diffuse_format, true)
        .context("Failed to compress diffuse atlas")?;

    // Step 7: Build normal map atlas if requested
    let normal_dds = if build_normal {
        let mut normal_canvas = dds_util::create_canvas(atlas_w, atlas_h);
        // Fill with default normal (128, 128, 255, 255) = flat normal
        for pixel in normal_canvas.chunks_exact_mut(4) {
            pixel[0] = 128; // R
            pixel[1] = 128; // G
            pixel[2] = 255; // B
            pixel[3] = 255; // A
        }

        // Load and composite normal billboards
        for base in tree_bases {
            let bb_path = billboard_path(&base.plugin_filename, &base.model_path, base.form_id);
            let normal_path = bb_path.replace(".dds", "_n.dds");

            if let Ok(normal_data) = loader.load(&normal_path) {
                if let Ok((rgba, w, h)) = dds_util::decompress_to_rgba(&normal_data) {
                    // Find which billboard index this corresponds to
                    let dds_data = match loader.load(&bb_path) {
                        Ok(d) => d,
                        Err(_) => continue,
                    };
                    let crc = crc32fast::hash(&dds_data);
                    if let Some(&block_idx) = crc_to_block.get(&crc) {
                        let block = &unique_blocks[block_idx];
                        dds_util::composite_rect(
                            &mut normal_canvas,
                            atlas_w,
                            &rgba,
                            w,
                            h,
                            block.x,
                            block.y,
                        );
                    }
                }
            }
        }

        Some(dds_util::compress_to_dds(&normal_canvas, atlas_w, atlas_h, normal_format, true)
            .context("Failed to compress normal atlas")?)
    } else {
        None
    };

    Ok(AtlasResult {
        diffuse_dds,
        normal_dds,
        tree_types: lst,
        atlas_width: atlas_w,
        atlas_height: atlas_h,
    })
}

/// Apply brightness adjustment to RGBA pixels.
fn apply_brightness(rgba: &mut [u8], brightness: i32) {
    for pixel in rgba.chunks_exact_mut(4) {
        // Only adjust RGB, not alpha
        for channel in &mut pixel[..3] {
            let val = (*channel as i32) + brightness;
            *channel = val.clamp(0, 255) as u8;
        }
    }
}
