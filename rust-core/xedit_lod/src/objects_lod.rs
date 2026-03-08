//! Object LOD generation — reference scanning, texture atlas, LODGen export.
//!
//! Ported from wbGenerateLODTES5 (objects section) in wbLOD.pas.
//!
//! The object LOD pipeline:
//! 1. Scan plugins for STAT records with HasDistantLOD (0x8000) + MNAM LOD meshes
//! 2. Scan worldspace for REFR records referencing those STATs
//! 3. Extract textures from LOD NIF files
//! 4. Build texture atlases (4096x4096, DXT5 diffuse + DXT1 normal)
//! 5. Write LODGen.txt export file
//! 6. Write atlas map file
//! 7. Call LODGen.exe to combine NIF meshes into per-cell blocks

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::bin_packer::{BinBlock, fit_with_growth};
use crate::dds_util;
use crate::export_writer::{self, AtlasMapEntry, ExportConfig, LodGenRef};
use crate::lod_settings::LodSettings;
use crate::reference_scanner::{ObjectBaseInfo, RefInfo};
use crate::resource_loader::ResourceLoader;

/// Maximum atlas dimension.
const MAX_ATLAS_SIZE: u32 = 8192;

/// Atlas padding between textures.
const ATLAS_PADDING: u32 = 0;

/// Information about a texture extracted from a LOD NIF.
#[derive(Debug, Clone)]
pub struct LodTexture {
    /// Virtual path to the texture.
    pub path: String,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Whether this texture has alpha.
    pub has_alpha: bool,
    /// CRC32 for dedup.
    pub crc32: u32,
}

/// Result of building the object texture atlas.
#[derive(Debug)]
pub struct ObjectAtlasResult {
    /// Atlas DDS files: (atlas_name, diffuse_dds, normal_dds)
    pub atlases: Vec<(String, Vec<u8>, Option<Vec<u8>>)>,
    /// Atlas map entries for LODGen.txt.
    pub atlas_map: Vec<AtlasMapEntry>,
    /// Atlas dimensions.
    pub atlas_width: u32,
    pub atlas_height: u32,
}

/// Collect unique LOD texture paths from object base records.
///
/// For each STAT with LOD meshes, extracts the diffuse texture paths used.
/// Returns a map of texture path -> LodTexture info.
pub fn collect_lod_textures(
    object_bases: &HashMap<u32, ObjectBaseInfo>,
    loader: &ResourceLoader,
    max_texture_size: u32,
) -> Result<HashMap<String, LodTexture>> {
    let mut textures: HashMap<String, LodTexture> = HashMap::new();
    let mut checked_meshes: HashSet<String> = HashSet::new();

    for (_fid, base) in object_bases {
        for lod_mesh_path in &base.lod_models {
            if lod_mesh_path.is_empty() {
                continue;
            }

            let normalized = lod_mesh_path.to_lowercase().replace('\\', "/");
            if checked_meshes.contains(&normalized) {
                continue;
            }
            checked_meshes.insert(normalized.clone());

            // Extract texture paths from the NIF
            // For now, we derive the texture path from the mesh path:
            // meshes/foo/bar_lod.nif -> textures/foo/bar_lod.dds
            let tex_path = derive_texture_path(&normalized);

            if textures.contains_key(&tex_path) {
                continue;
            }

            // Try to load the texture to get dimensions
            match loader.load(&tex_path) {
                Ok(data) => {
                    match dds_util::read_dds_info(&data) {
                        Ok(info) => {
                            let w = info.width.min(max_texture_size);
                            let h = info.height.min(max_texture_size);
                            let crc = crc32fast::hash(&data);
                            textures.insert(tex_path.clone(), LodTexture {
                                path: tex_path,
                                width: w,
                                height: h,
                                has_alpha: info.format.contains("BC3") || info.format.contains("DXT5"),
                                crc32: crc,
                            });
                        }
                        Err(e) => {
                            debug!("Can't read DDS info for {}: {}", tex_path, e);
                        }
                    }
                }
                Err(_) => {
                    debug!("Texture not found: {}", tex_path);
                }
            }
        }
    }

    info!("Collected {} unique LOD textures from {} base objects",
        textures.len(), object_bases.len());
    Ok(textures)
}

/// Derive the diffuse texture path from a LOD mesh path.
///
/// Common patterns:
/// - `meshes/foo/bar_lod.nif` -> `textures/foo/bar_lod.dds`
/// - `meshes/foo/bar.nif` -> `textures/foo/bar.dds`
fn derive_texture_path(mesh_path: &str) -> String {
    let path = mesh_path
        .replace("meshes/", "textures/")
        .replace("meshes\\", "textures\\");

    // Change extension
    if let Some(dot_pos) = path.rfind('.') {
        format!("{}.dds", &path[..dot_pos])
    } else {
        format!("{}.dds", path)
    }
}

/// Build object texture atlases.
///
/// Packs textures into 4096x4096 atlas pages, creating multiple atlases
/// (Buildings, Buildings01, Buildings02, etc.) if needed.
pub fn build_object_atlas(
    textures: &HashMap<String, LodTexture>,
    loader: &ResourceLoader,
    worldspace: &str,
    atlas_width: u32,
    atlas_height: u32,
    diffuse_format: u32,
    normal_format: u32,
    _alpha_threshold: u32,
) -> Result<ObjectAtlasResult> {
    if textures.is_empty() {
        return Ok(ObjectAtlasResult {
            atlases: Vec::new(),
            atlas_map: Vec::new(),
            atlas_width,
            atlas_height,
        });
    }

    // Build blocks for packing
    let tex_list: Vec<(&String, &LodTexture)> = textures.iter().collect();
    let mut blocks: Vec<BinBlock> = tex_list.iter().enumerate().map(|(i, (_, tex))| {
        BinBlock {
            w: tex.width,
            h: tex.height,
            x: 0,
            y: 0,
            index: i,
            placed: false,
        }
    }).collect();

    // Try to fit everything in one atlas
    let result = fit_with_growth(
        &mut blocks,
        atlas_width, atlas_height,
        MAX_ATLAS_SIZE, MAX_ATLAS_SIZE,
        ATLAS_PADDING, ATLAS_PADDING,
    );

    let mut atlases = Vec::new();
    let mut atlas_map = Vec::new();

    if result.is_some() {
        // Single atlas case
        let (aw, ah) = result.unwrap();
        let atlas_name = format!(
            "textures/landscape/lod/{}/Blocks/{}.Buildings.dds",
            worldspace.to_lowercase(), worldspace
        );
        let normal_name = atlas_name.replace(".dds", "_n.dds");

        let (diffuse_dds, normal_dds, entries) = build_single_atlas(
            &blocks, &tex_list, loader,
            aw, ah,
            &atlas_name, &normal_name,
            diffuse_format, normal_format,
        )?;

        atlas_map.extend(entries);
        atlases.push((atlas_name, diffuse_dds, Some(normal_dds)));
    } else {
        // Multi-atlas: split textures across multiple pages
        info!("Textures don't fit in single atlas, splitting into multiple pages");

        // Simple split: sort by size and distribute across pages
        let mut remaining: Vec<(usize, &LodTexture)> = tex_list.iter()
            .enumerate()
            .map(|(i, (_, tex))| (i, *tex))
            .collect();
        remaining.sort_by(|a, b| (b.1.width * b.1.height).cmp(&(a.1.width * a.1.height)));

        let mut page = 0;
        while !remaining.is_empty() {
            let mut page_blocks: Vec<BinBlock> = remaining.iter().enumerate().map(|(_bi, (orig_i, tex))| {
                BinBlock {
                    w: tex.width,
                    h: tex.height,
                    x: 0,
                    y: 0,
                    index: *orig_i,
                    placed: false,
                }
            }).collect();

            let packer = crate::bin_packer::BinPacker::new(atlas_width, atlas_height)
                .with_padding(ATLAS_PADDING, ATLAS_PADDING);
            packer.fit(&mut page_blocks);

            let placed: Vec<_> = page_blocks.iter().filter(|b| b.placed).cloned().collect();
            let unplaced_indices: HashSet<usize> = page_blocks.iter()
                .filter(|b| !b.placed)
                .map(|b| b.index)
                .collect();

            if placed.is_empty() {
                warn!("Can't fit any more textures in atlas page {}", page);
                break;
            }

            let page_suffix = if page == 0 { String::new() } else { format!("{:02}", page) };
            let atlas_name = format!(
                "textures/landscape/lod/{}/Blocks/{}.Buildings{}.dds",
                worldspace.to_lowercase(), worldspace, page_suffix
            );
            let normal_name = atlas_name.replace(".dds", "_n.dds");

            let placed_tex_list: Vec<(&String, &LodTexture)> = placed.iter()
                .map(|b| (tex_list[b.index].0, tex_list[b.index].1))
                .collect();

            let (diffuse_dds, normal_dds, entries) = build_single_atlas(
                &placed, &placed_tex_list, loader,
                atlas_width, atlas_height,
                &atlas_name, &normal_name,
                diffuse_format, normal_format,
            )?;

            atlas_map.extend(entries);
            atlases.push((atlas_name, diffuse_dds, Some(normal_dds)));

            remaining.retain(|(i, _)| unplaced_indices.contains(i));
            page += 1;
        }
    }

    info!("Built {} atlas pages with {} map entries", atlases.len(), atlas_map.len());

    Ok(ObjectAtlasResult {
        atlases,
        atlas_map,
        atlas_width,
        atlas_height,
    })
}

/// Build a single atlas page from the given blocks.
fn build_single_atlas(
    blocks: &[BinBlock],
    tex_list: &[(&String, &LodTexture)],
    loader: &ResourceLoader,
    atlas_w: u32,
    atlas_h: u32,
    atlas_name: &str,
    _normal_name: &str,
    diffuse_format: u32,
    normal_format: u32,
) -> Result<(Vec<u8>, Vec<u8>, Vec<AtlasMapEntry>)> {
    let mut diffuse_canvas = dds_util::create_canvas(atlas_w, atlas_h);
    let mut normal_canvas = dds_util::create_canvas(atlas_w, atlas_h);

    // Fill normal canvas with default flat normal
    for pixel in normal_canvas.chunks_exact_mut(4) {
        pixel[0] = 128;
        pixel[1] = 128;
        pixel[2] = 255;
        pixel[3] = 255;
    }

    let mut entries = Vec::new();

    for block in blocks {
        if !block.placed {
            continue;
        }

        if block.index >= tex_list.len() {
            continue;
        }

        let (tex_path, tex_info) = &tex_list[block.index];

        // Load and decompress the diffuse texture
        if let Ok(data) = loader.load(tex_path) {
            if let Ok((rgba, w, h)) = dds_util::decompress_to_rgba(&data) {
                // Resize if needed
                let rgba = if w != tex_info.width || h != tex_info.height {
                    dds_util::resize_rgba(&rgba, w, h, tex_info.width, tex_info.height)
                } else {
                    rgba
                };

                dds_util::composite_rect(
                    &mut diffuse_canvas, atlas_w,
                    &rgba, tex_info.width, tex_info.height,
                    block.x, block.y,
                );
            }
        }

        // Try to load the normal map
        let normal_path = tex_path.replace(".dds", "_n.dds");
        if let Ok(data) = loader.load(&normal_path) {
            if let Ok((rgba, w, h)) = dds_util::decompress_to_rgba(&data) {
                let rgba = if w != tex_info.width || h != tex_info.height {
                    dds_util::resize_rgba(&rgba, w, h, tex_info.width, tex_info.height)
                } else {
                    rgba
                };

                dds_util::composite_rect(
                    &mut normal_canvas, atlas_w,
                    &rgba, tex_info.width, tex_info.height,
                    block.x, block.y,
                );
            }
        }

        // Add atlas map entry
        entries.push(AtlasMapEntry {
            source_texture: tex_path.replace('/', "\\"),
            src_width: tex_info.width,
            src_height: tex_info.height,
            atlas_x: block.x,
            atlas_y: block.y,
            atlas_texture: atlas_name.replace('/', "\\"),
            atlas_width: atlas_w,
            atlas_height: atlas_h,
        });
    }

    // Compress atlases
    let diffuse_dds = dds_util::compress_to_dds(&diffuse_canvas, atlas_w, atlas_h, diffuse_format, true)
        .context("Failed to compress diffuse atlas")?;
    let normal_dds = dds_util::compress_to_dds(&normal_canvas, atlas_w, atlas_h, normal_format, true)
        .context("Failed to compress normal atlas")?;

    Ok((diffuse_dds, normal_dds, entries))
}

/// Generate the complete object LOD pipeline.
///
/// This is the main entry point for object LOD generation.
pub fn generate_object_lod(
    worldspace: &str,
    output_dir: &Path,
    object_bases: &HashMap<u32, ObjectBaseInfo>,
    refs: &[RefInfo],
    loader: &ResourceLoader,
    lod_settings: &LodSettings,
    options: &crate::LodOptions,
    bsa_paths: &[(PathBuf, usize)],
    mod_dirs: &[PathBuf],
    game_data_dir: &Path,
    progress: &crate::progress::Progress,
) -> Result<ObjectLodOutput> {
    progress.report("Collecting LOD textures...", 0.1);

    // Filter refs to only those with LOD bases
    let valid_refs: Vec<&RefInfo> = refs.iter()
        .filter(|r| object_bases.contains_key(&r.base_form_id))
        .filter(|r| !r.is_disabled && !r.is_deleted)
        .collect();

    info!("Object LOD: {} valid refs from {} object bases", valid_refs.len(), object_bases.len());

    if valid_refs.is_empty() {
        return Ok(ObjectLodOutput {
            ref_count: 0,
            atlas_count: 0,
            lodgen_path: None,
        });
    }

    progress.report("Collecting LOD textures from meshes...", 0.2);

    // Collect textures from LOD meshes
    let textures = collect_lod_textures(object_bases, loader, options.atlas_texture_size)?;
    info!("Found {} unique LOD textures", textures.len());

    progress.report("Building texture atlases...", 0.4);

    // Build atlas
    let atlas_result = build_object_atlas(
        &textures, loader, worldspace,
        options.atlas_width, options.atlas_height,
        options.atlas_diffuse_format, options.atlas_normal_format,
        options.default_alpha_threshold,
    )?;

    // Write atlas DDS files
    let atlas_dir = output_dir
        .join("textures/landscape/lod")
        .join(worldspace.to_lowercase())
        .join("Blocks");
    std::fs::create_dir_all(&atlas_dir)?;

    for (name, diffuse, normal) in &atlas_result.atlases {
        let diffuse_path = output_dir.join(name);
        if let Some(parent) = diffuse_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&diffuse_path, diffuse)?;

        if let Some(normal_data) = normal {
            let normal_path = diffuse_path.to_string_lossy().replace(".dds", "_n.dds");
            std::fs::write(&normal_path, normal_data)?;
        }
    }

    progress.report("Writing LODGen export file...", 0.6);

    // Write atlas map file
    let atlas_map_path = output_dir.join(format!("FNVLODGen-AtlasMap{}.txt", worldspace));
    export_writer::write_atlas_map(&atlas_map_path, &atlas_result.atlas_map)?;

    // Build LODGen ref entries
    let lodgen_refs: Vec<LodGenRef> = valid_refs.iter().map(|r| {
        let base = &object_bases[&r.base_form_id];
        let lod4 = base.lod_models.first().map(|s| s.as_str()).unwrap_or("");
        let lod8 = base.lod_models.get(1).map(|s| s.as_str()).unwrap_or(lod4);

        LodGenRef {
            form_id: r.ref_form_id,
            flags: if base.has_distant_lod { 0x8000 } else { 0 },
            pos_x: r.position[0],
            pos_y: r.position[1],
            pos_z: r.position[2],
            rot_x: r.rotation[0],
            rot_y: r.rotation[1],
            rot_z: r.rotation[2],
            scale: r.scale,
            editor_id: base.editor_id.clone(),
            grid_flags: 0, // TODO: compute proper grid flags
            full_model: String::new(), // Full model not needed for LODGen
            lod4_model: lod4.to_string(),
            lod8_model: lod8.to_string(),
        }
    }).collect();

    // Write LODGen.txt
    let mesh_output = output_dir
        .join("meshes/landscape/lod")
        .join(worldspace)
        .join("Blocks");
    std::fs::create_dir_all(&mesh_output)?;

    let lodgen_path = output_dir.join("LODGen.txt");
    let config = ExportConfig {
        game_mode: "FNV".to_string(),
        worldspace: worldspace.to_string(),
        cell_sw: lod_settings.sw_cell,
        atlas_map_path: atlas_map_path.clone(),
        atlas_tolerance: 9999.0,
        path_data: game_data_dir.to_path_buf(),
        path_output: mesh_output.clone(),
        bsa_paths: bsa_paths.iter().map(|(p, _)| p.clone()).collect(),
        mod_dirs: mod_dirs.to_vec(),
        alpha_double_sided: false,
        ignore_translation: true,
    };

    export_writer::write_lodgen_txt(&lodgen_path, &config, &lodgen_refs)?;

    progress.report("Object LOD export complete", 1.0);

    Ok(ObjectLodOutput {
        ref_count: valid_refs.len(),
        atlas_count: atlas_result.atlases.len(),
        lodgen_path: Some(lodgen_path),
    })
}

/// Output summary from object LOD generation.
#[derive(Debug)]
pub struct ObjectLodOutput {
    pub ref_count: usize,
    pub atlas_count: usize,
    pub lodgen_path: Option<PathBuf>,
}
