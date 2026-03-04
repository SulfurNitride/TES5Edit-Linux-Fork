pub mod progress;
pub mod resource_loader;
pub mod lod_settings;
pub mod reference_scanner;
pub mod atlas_builder;
pub mod trees_lod;
pub mod objects_lod;
pub mod terrain_lod;
pub mod nif_combiner;
pub mod btt_writer;

pub use lod_settings::LodSettings;
pub use progress::Progress;
pub use resource_loader::ResourceLoader;
pub use reference_scanner::{LodBase, LodReference};

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct LodOptions {
    pub worldspaces: Vec<String>,
    pub output_dir: String,
    pub objects_lod: bool,
    pub trees_lod: bool,
    pub terrain_lod: bool,
    pub lod_levels: Vec<u32>,
    pub build_atlas: bool,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub atlas_texture_size: u32,
    pub compression_diffuse: String,
    pub compression_normal: String,
    pub no_tangents: bool,
    pub no_vertex_colors: bool,
    pub trees_brightness: i32,
    pub trees_3d: bool,
    /// Game-specific lodsettings extension ("lod" or "dlodsettings")
    #[serde(default = "default_lod_ext")]
    pub lod_extension: String,
    /// Game ID string for NIF version selection ("FalloutNV", "SkyrimSE", etc.)
    #[serde(default)]
    pub game_id: String,
}

fn default_lod_ext() -> String {
    "lod".to_string()
}

/// List worldspaces that have LOD settings files in the data directory.
/// Scans both loose files and BSA/BA2 archives for lodsettings/*.lod files.
pub fn list_worldspaces_with_lod_settings(data_path: &Path) -> anyhow::Result<Vec<String>> {
    let loader = ResourceLoader::new(data_path)?;
    let mut worldspaces = Vec::new();

    // Check loose files in lodsettings/
    let lod_dir = data_path.join("lodsettings");
    if lod_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&lod_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("lod") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        worldspaces.push(stem.to_string());
                    }
                }
            }
        }
    }

    // Check BSA archives for lodsettings files
    for bsa_path in loader.bsa_paths() {
        if let Ok(files) = resource_loader::list_bsa_files(bsa_path) {
            for file_path in files {
                let lower = file_path.to_lowercase();
                if lower.starts_with("lodsettings\\") && lower.ends_with(".lod") {
                    let filename = &file_path[12..]; // skip "lodsettings\"
                    if let Some(dot) = filename.rfind('.') {
                        let name = &filename[..dot];
                        if !worldspaces.iter().any(|w| w.eq_ignore_ascii_case(name)) {
                            worldspaces.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    worldspaces.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    worldspaces.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Ok(worldspaces)
}

/// Top-level entry point for LOD generation.
///
/// Takes LOD options, a path to the game's Data directory, pre-scanned references
/// and bases, a worldspace name→form_id mapping, and a Progress handle.
/// For each worldspace in the options, it:
/// 1. Resolves the worldspace editor ID to a form ID
/// 2. Filters references to only those in the target worldspace
/// 3. Parses LOD settings
/// 4. Routes to trees/objects/terrain pipelines based on options
pub fn generate_lod(
    options: &LodOptions,
    data_path: &Path,
    references: &[LodReference],
    bases: &std::collections::HashMap<u32, LodBase>,
    worldspace_map: &std::collections::HashMap<String, u32>,
    progress: &Progress,
) -> anyhow::Result<()> {
    let loader = ResourceLoader::new(data_path)?;

    progress.report(&format!(
        "Starting LOD generation for {} worldspace(s) ({} total references, {} bases)",
        options.worldspaces.len(),
        references.len(),
        bases.len(),
    ));

    for worldspace in &options.worldspaces {
        if progress.is_cancelled() {
            anyhow::bail!("LOD generation cancelled by user");
        }

        // Resolve worldspace editor ID to form ID
        let ws_form_id = match worldspace_map.get(&worldspace.to_lowercase()) {
            Some(&id) => id,
            None => {
                tracing::warn!(
                    "Could not resolve worldspace '{}' to a form ID, skipping",
                    worldspace
                );
                continue;
            }
        };

        // Filter references to this worldspace
        let ws_refs: Vec<&LodReference> = references
            .iter()
            .filter(|r| r.worldspace_form_id == ws_form_id)
            .collect();

        progress.report(&format!(
            "Processing worldspace: {} ({} references)",
            worldspace,
            ws_refs.len()
        ));

        // Parse LOD settings for this worldspace — try all extension/path combos
        let exts = if options.lod_extension.is_empty() {
            vec!["dlodsettings".to_string(), "lod".to_string()]
        } else {
            vec![options.lod_extension.clone()]
        };
        let _settings = exts.iter()
            .find_map(|ext| {
                loader.resolve(&format!("lodsettings\\{}.{}", worldspace, ext))
                    .or_else(|_| loader.resolve(&format!("lodsettings/{}.{}", worldspace, ext)))
                    .ok()
            })
            .map(|data| LodSettings::parse(&data))
            .transpose()?;

        if _settings.is_none() {
            tracing::warn!("No LOD settings found for worldspace {}", worldspace);
        }

        // Collect owned refs for pipeline functions that take &[LodReference]
        let ws_refs_owned: Vec<LodReference> = ws_refs.iter().map(|r| (*r).clone()).collect();

        // Route to the appropriate pipelines
        if options.trees_lod {
            trees_lod::generate_trees_lod(
                options, worldspace, &ws_refs_owned, bases, &loader, &progress,
            )?;
        }

        if options.objects_lod {
            objects_lod::generate_objects_lod(
                options, worldspace, &ws_refs_owned, bases, &loader, &progress,
            )?;

            // Build texture atlas if enabled
            if options.build_atlas {
                progress.report(&format!("Building texture atlas for {}...", worldspace));
                if let Err(e) = build_texture_atlas(options, worldspace, bases, &loader) {
                    tracing::warn!("Atlas build failed for {}: {:#}", worldspace, e);
                }
            }
        }

        if options.terrain_lod {
            terrain_lod::generate_terrain_lod(options, worldspace, &loader, &progress)?;
        }
    }

    progress.report("LOD generation complete");
    Ok(())
}

/// Build texture atlas for Objects LOD.
///
/// Collects all unique diffuse textures from LOD base records, resolves them
/// from BSAs, packs them into an atlas using the shelf-based bin packer,
/// and writes the atlas DDS + atlas map file to the output directory.
fn build_texture_atlas(
    options: &LodOptions,
    worldspace: &str,
    bases: &std::collections::HashMap<u32, LodBase>,
    loader: &ResourceLoader,
) -> anyhow::Result<()> {
    use atlas_builder::AtlasBuilder;

    // Collect unique texture paths from all LOD meshes
    // We need to load each cached mesh NIF to get texture paths,
    // but since objects_lod already extracted them, we'll collect from base records' models.
    // For now, collect texture paths by loading LOD NIFs.
    let mut unique_textures: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Scan all LOD model paths and extract their textures
    let nifly = match xedit_nif::NiflyLibrary::load() {
        Ok(lib) => lib,
        Err(_) => {
            tracing::warn!("nifly not available, skipping atlas build");
            return Ok(());
        }
    };

    let temp_dir = std::env::temp_dir().join("xedit_lod");

    for base in bases.values() {
        // Check all LOD model paths
        let models: Vec<&String> = base.lod_models.iter()
            .filter_map(|m| m.as_ref())
            .chain(base.full_model.as_ref())
            .collect();

        for model_path in models {
            let hash = {
                let mut h: u64 = 0xcbf29ce484222325;
                for byte in model_path.bytes() {
                    h ^= byte as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
                h
            };
            let temp_path = temp_dir.join(format!("lod_{:016x}.nif", hash));

            if temp_path.exists() {
                if let Ok(nif) = nifly.load_nif(&temp_path) {
                    let shape_count = nif.shape_count().unwrap_or(0);
                    for si in 0..shape_count {
                        if let Ok(Some(tex)) = nif.texture_slot(si, 0) {
                            if !tex.is_empty() {
                                unique_textures.insert(tex.to_lowercase());
                            }
                        }
                    }
                }
            }
        }
    }

    if unique_textures.is_empty() {
        tracing::info!("No textures found for atlas (worldspace: {})", worldspace);
        return Ok(());
    }

    tracing::info!(
        "Building atlas from {} unique textures for {}",
        unique_textures.len(), worldspace
    );

    let mut atlas = AtlasBuilder::new(
        options.atlas_width,
        options.atlas_height,
        options.atlas_texture_size,
    );

    let mut atlas_entries = 0u32;
    let mut atlas_map_lines: Vec<String> = Vec::new();

    for tex_path in &unique_textures {
        // Try to resolve texture from BSAs
        let tex_data = loader.resolve(tex_path)
            .or_else(|_| {
                let prefixed = format!("textures\\{}", tex_path);
                loader.resolve(&prefixed)
            });

        let tex_data = match tex_data {
            Ok(data) => data,
            Err(_) => continue,
        };

        match atlas.add_texture(tex_path, &tex_data) {
            Ok(idx) => {
                let entry = &atlas.entries[idx];
                // Write atlas map line: texture, w, h, atlas_x, atlas_y, atlas_file, atlas_w, atlas_h
                atlas_map_lines.push(format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    tex_path, entry.width, entry.height,
                    entry.x, entry.y,
                    format!("textures\\terrain\\{}\\{}_objects.dds", worldspace.to_lowercase(), worldspace.to_lowercase()),
                    atlas.width, atlas.height
                ));
                atlas_entries += 1;
            }
            Err(e) => {
                tracing::debug!("Could not fit {} in atlas: {}", tex_path, e);
            }
        }
    }

    if atlas_entries == 0 {
        tracing::info!("No textures could be packed into atlas for {}", worldspace);
        return Ok(());
    }

    // Build and write atlas DDS
    let output_base = Path::new(&options.output_dir);
    let tex_dir = output_base
        .join("textures")
        .join("terrain")
        .join(worldspace.to_lowercase());
    std::fs::create_dir_all(&tex_dir)?;

    let atlas_dds = atlas.build(&options.compression_diffuse)?;
    let atlas_path = tex_dir.join(format!("{}_objects.dds", worldspace.to_lowercase()));
    std::fs::write(&atlas_path, &atlas_dds)?;

    tracing::info!(
        "Wrote atlas: {} ({} textures, {:.1} MB)",
        atlas_path.display(), atlas_entries,
        atlas_dds.len() as f64 / 1_048_576.0
    );

    // Write atlas map file
    let map_path = output_base.join(format!("LODGenAtlasMap_{}.txt", worldspace));
    std::fs::write(&map_path, atlas_map_lines.join("\n"))?;
    tracing::info!("Wrote atlas map: {}", map_path.display());

    Ok(())
}
