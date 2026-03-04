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
use std::collections::HashMap;
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

/// Helper: is this game FO3 or FNV?
pub fn is_fo3_fnv(game_id: &str) -> bool {
    matches!(game_id, "Fallout3" | "FalloutNV")
}

/// Helper: is this game FO4?
pub fn is_fo4(game_id: &str) -> bool {
    matches!(game_id, "Fallout4" | "Fallout76")
}

/// Get the atlas texture path for a game/worldspace.
///
/// FO3/FNV:  textures\landscape\lod\<WS>\Blocks\<WS>.Buildings.dds
/// Skyrim:   textures\terrain\<WS>\Objects\<WS>ObjectsLOD.dds
/// FO4:      textures\terrain\<WS>\Objects\<WS>Objects.dds
pub fn atlas_texture_path(game_id: &str, worldspace: &str, sheet_index: u32) -> String {
    let ws = worldspace.to_lowercase();
    let suffix = if sheet_index > 0 { format!("{:02}", sheet_index) } else { String::new() };

    if is_fo3_fnv(game_id) {
        format!("textures\\landscape\\lod\\{}\\blocks\\{}.buildings{}.dds", ws, ws, suffix)
    } else if is_fo4(game_id) {
        format!("textures\\terrain\\{}\\objects\\{}objects{}.dds", ws, ws, suffix)
    } else {
        format!("textures\\terrain\\{}\\objects\\{}objectslod{}.dds", ws, ws, suffix)
    }
}

/// Get the output mesh directory for a game/worldspace.
///
/// FO3/FNV:  meshes\landscape\lod\<WS>\Blocks\
/// Skyrim:   meshes\terrain\<WS>\Objects\
pub fn mesh_output_dir(game_id: &str, worldspace: &str) -> String {
    let ws = worldspace.to_lowercase();
    if is_fo3_fnv(game_id) {
        format!("meshes/landscape/lod/{}/blocks", ws)
    } else {
        format!("meshes/terrain/{}/objects", ws)
    }
}

/// Get the atlas output directory for a game/worldspace (filesystem path).
pub fn atlas_output_dir(game_id: &str, worldspace: &str) -> String {
    let ws = worldspace.to_lowercase();
    if is_fo3_fnv(game_id) {
        format!("textures/landscape/lod/{}/blocks", ws)
    } else {
        format!("textures/terrain/{}/objects", ws)
    }
}

/// List worldspaces that have LOD settings files in the data directory.
/// Scans both loose files and BSA/BA2 archives for lodsettings/*.lod files.
pub fn list_worldspaces_with_lod_settings(data_path: &Path) -> anyhow::Result<Vec<String>> {
    let loader = ResourceLoader::new(data_path)?;
    let mut worldspaces = Vec::new();

    // Check loose files in lodsettings/ for both extensions
    let lod_dir = data_path.join("lodsettings");
    if lod_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&lod_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext.eq_ignore_ascii_case("lod") || ext.eq_ignore_ascii_case("dlodsettings") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !worldspaces.iter().any(|w: &String| w.eq_ignore_ascii_case(stem)) {
                            worldspaces.push(stem.to_string());
                        }
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
                if lower.starts_with("lodsettings\\") || lower.starts_with("lodsettings/") {
                    let filename = &file_path[12..]; // skip "lodsettings\" or "lodsettings/"
                    let has_ext = filename.to_lowercase().ends_with(".lod")
                        || filename.to_lowercase().ends_with(".dlodsettings");
                    if has_ext {
                        // Extract name without extension
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
    }

    worldspaces.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    worldspaces.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Ok(worldspaces)
}

/// Top-level entry point for LOD generation.
///
/// Pipeline matching Delphi xEdit:
/// 1. Parse LOD settings for cell bounds
/// 2. Filter references to the target worldspace
/// 3. Trees LOD (if enabled and not 3D trees mode)
/// 4. Objects LOD:
///    a. Build texture atlas FIRST
///    b. Then combine meshes with atlas UV remapping
/// 5. Terrain LOD
pub fn generate_lod(
    options: &LodOptions,
    data_path: &Path,
    references: &[LodReference],
    bases: &HashMap<u32, LodBase>,
    worldspace_map: &HashMap<String, u32>,
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

        // Parse LOD settings for this worldspace
        let exts = if options.lod_extension.is_empty() {
            vec!["dlodsettings", "lod"]
        } else {
            vec![options.lod_extension.as_str()]
        };

        let mut settings: Option<LodSettings> = None;
        let mut settings_ext = "lod";
        for ext in &exts {
            let result = loader.resolve(&format!("lodsettings\\{}.{}", worldspace, ext))
                .or_else(|_| loader.resolve(&format!("lodsettings/{}.{}", worldspace, ext)));
            if let Ok(data) = result {
                settings = Some(LodSettings::parse(&data, ext)?);
                settings_ext = ext;
                break;
            }
        }

        if let Some(ref s) = settings {
            tracing::info!(
                "LOD settings for {} ({}): SW({},{}) NE({},{}) stride={} levels={}-{} obj_level={}",
                worldspace, settings_ext,
                s.sw_cell_x, s.sw_cell_y, s.ne_cell_x, s.ne_cell_y,
                s.stride, s.lod_level_min, s.lod_level_max, s.object_level
            );
        } else {
            tracing::warn!("No LOD settings found for worldspace {}", worldspace);
        }

        let ws_refs_owned: Vec<LodReference> = ws_refs.iter().map(|r| (*r).clone()).collect();

        // Route to the appropriate pipelines
        if options.trees_lod {
            trees_lod::generate_trees_lod(
                options, worldspace, &ws_refs_owned, bases, &loader, progress,
            )?;
        }

        if options.objects_lod {
            // Build texture atlas FIRST (matching Delphi pipeline order)
            let atlas_map = if options.build_atlas {
                progress.report(&format!("Building texture atlas for {}...", worldspace));
                match build_texture_atlas(options, worldspace, &ws_refs_owned, bases, &loader) {
                    Ok(map) => {
                        tracing::info!("Atlas built with {} texture mappings", map.len());
                        map
                    }
                    Err(e) => {
                        tracing::warn!("Atlas build failed for {}: {:#}", worldspace, e);
                        HashMap::new()
                    }
                }
            } else {
                HashMap::new()
            };

            // Generate Objects LOD meshes with atlas UV remapping
            objects_lod::generate_objects_lod(
                options, worldspace, &ws_refs_owned, bases, &loader, progress,
                settings.as_ref(), &atlas_map,
            )?;
        }

        if options.terrain_lod {
            terrain_lod::generate_terrain_lod(options, worldspace, &loader, progress)?;
        }
    }

    progress.report("LOD generation complete");
    Ok(())
}

/// Atlas UV mapping: maps a texture path to its atlas coordinates.
#[derive(Debug, Clone)]
pub struct AtlasMapping {
    /// Atlas sheet index (for multiple atlas files)
    pub atlas_index: u32,
    /// UV offset and scale to remap original UVs to atlas space
    pub u_offset: f32,
    pub v_offset: f32,
    pub u_scale: f32,
    pub v_scale: f32,
    /// The atlas texture path (for setting on combined shapes)
    pub atlas_texture_path: String,
}

/// Build texture atlas for Objects LOD. Returns atlas UV mappings.
///
/// Pipeline matching Delphi:
/// 1. Collect unique textures from all LOD meshes
/// 2. Resolve texture DDS data from BSAs
/// 3. Sort by max(w,h) descending
/// 4. Pack into atlas sheets using binary tree packer
/// 5. Write atlas DDS + atlas map file
/// 6. Return UV mappings for mesh combination phase
fn build_texture_atlas(
    options: &LodOptions,
    worldspace: &str,
    references: &[LodReference],
    bases: &HashMap<u32, LodBase>,
    loader: &ResourceLoader,
) -> anyhow::Result<HashMap<String, AtlasMapping>> {
    use atlas_builder::AtlasBuilder;

    // Collect unique texture paths from all LOD meshes
    let nifly = match xedit_nif::NiflyLibrary::load() {
        Ok(lib) => lib,
        Err(_) => {
            tracing::warn!("nifly not available, skipping atlas build");
            return Ok(HashMap::new());
        }
    };

    let temp_dir = std::env::temp_dir().join("xedit_lod");
    std::fs::create_dir_all(&temp_dir)?;

    // First, resolve all LOD meshes and extract their textures
    let mut unique_textures: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Collect unique models referenced by actual references in this worldspace
    let mut unique_models: std::collections::HashSet<String> = std::collections::HashSet::new();
    for refr in references {
        if let Some(base) = bases.get(&refr.base_form_id) {
            // For FO3/FNV: LOD mesh = MODL + _lod.nif
            if is_fo3_fnv(&options.game_id) {
                if let Some(ref model) = base.full_model {
                    let lod_path = model_to_lod_path(model);
                    unique_models.insert(lod_path);
                }
            } else {
                // Skyrim: use MNAM LOD models
                for model in base.lod_models.iter().filter_map(|m| m.as_ref()) {
                    unique_models.insert(model.clone());
                }
                if let Some(ref model) = base.full_model {
                    unique_models.insert(model.clone());
                }
            }
        }
    }

    // Resolve each model, write to temp, extract textures
    for model_path in &unique_models {
        let mesh_data = loader.resolve(model_path)
            .or_else(|_| loader.resolve(&format!("meshes\\{}", model_path)));
        let mesh_data = match mesh_data {
            Ok(d) => d,
            Err(_) => continue,
        };

        let hash = simple_hash(model_path);
        let temp_path = temp_dir.join(format!("lod_{:016x}.nif", hash));
        if !temp_path.exists() {
            std::fs::write(&temp_path, &mesh_data)?;
        }

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

    if unique_textures.is_empty() {
        tracing::info!("No textures found for atlas (worldspace: {})", worldspace);
        return Ok(HashMap::new());
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

    // Resolve and add all textures
    let mut resolved_count = 0u32;
    for tex_path in &unique_textures {
        let tex_data = loader.resolve(tex_path)
            .or_else(|_| loader.resolve(&format!("textures\\{}", tex_path)));

        let tex_data = match tex_data {
            Ok(data) => data,
            Err(_) => continue,
        };

        match atlas.add_texture(tex_path, &tex_data) {
            Ok(_) => resolved_count += 1,
            Err(e) => {
                tracing::debug!("Could not add texture {}: {}", tex_path, e);
            }
        }
    }

    if resolved_count == 0 {
        tracing::info!("No textures could be resolved for atlas (worldspace: {})", worldspace);
        return Ok(HashMap::new());
    }

    // Pack all textures using binary tree packing with size sorting
    atlas.pack_all();

    if atlas.sheet_count == 0 {
        tracing::warn!("Atlas packing produced 0 sheets for {}", worldspace);
        return Ok(HashMap::new());
    }

    // Write atlas DDS files and build UV mappings
    let output_base = Path::new(&options.output_dir);
    let atlas_dir_rel = atlas_output_dir(&options.game_id, worldspace);
    let atlas_dir = output_base.join(&atlas_dir_rel);
    std::fs::create_dir_all(&atlas_dir)?;

    let mut mappings: HashMap<String, AtlasMapping> = HashMap::new();
    let mut atlas_map_lines: Vec<String> = Vec::new();

    for sheet_idx in 0..atlas.sheet_count {
        let atlas_tex_path = atlas_texture_path(&options.game_id, worldspace, sheet_idx);
        let (sheet_w, sheet_h) = atlas.sheet_dimensions(sheet_idx);

        if sheet_w == 0 || sheet_h == 0 {
            continue;
        }

        // Build DDS for this sheet
        let atlas_dds = atlas.build_sheet(sheet_idx, &options.compression_diffuse)?;
        if atlas_dds.is_empty() {
            continue;
        }

        // Write the DDS file
        let filename = if atlas.sheet_count > 1 {
            let base_name = atlas_tex_path.rsplit('\\').next().unwrap_or(&atlas_tex_path);
            base_name.to_string()
        } else {
            atlas_tex_path.rsplit('\\').next().unwrap_or(&atlas_tex_path).to_string()
        };
        let atlas_path = atlas_dir.join(&filename);
        std::fs::write(&atlas_path, &atlas_dds)?;

        tracing::info!(
            "Wrote atlas sheet {}: {} ({}x{}, {:.1} MB)",
            sheet_idx, atlas_path.display(), sheet_w, sheet_h,
            atlas_dds.len() as f64 / 1_048_576.0
        );

        // Build UV mappings for textures on this sheet
        for entry in &atlas.entries {
            if entry.atlas_index != sheet_idx {
                continue;
            }

            let u_scale = entry.width as f32 / sheet_w as f32;
            let v_scale = entry.height as f32 / sheet_h as f32;
            let u_offset = entry.x as f32 / sheet_w as f32;
            let v_offset = entry.y as f32 / sheet_h as f32;

            mappings.insert(entry.texture_path.to_lowercase(), AtlasMapping {
                atlas_index: sheet_idx,
                u_offset,
                v_offset,
                u_scale,
                v_scale,
                atlas_texture_path: atlas_tex_path.clone(),
            });

            // Atlas map line (tab-separated, matching Delphi format)
            atlas_map_lines.push(format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                entry.texture_path, entry.width, entry.height,
                entry.x, entry.y,
                atlas_tex_path, sheet_w, sheet_h
            ));
        }
    }

    // Write atlas map file
    let packed = atlas.entries.iter().filter(|e| e.atlas_index < atlas.sheet_count).count();
    tracing::info!(
        "Atlas complete: {} textures packed into {} sheet(s), {} textures total",
        packed, atlas.sheet_count, unique_textures.len()
    );

    let map_path = output_base.join(format!("LODGenAtlasMap_{}.txt", worldspace));
    std::fs::write(&map_path, atlas_map_lines.join("\n"))?;
    tracing::info!("Wrote atlas map: {}", map_path.display());

    Ok(mappings)
}

/// Convert a full model path to its LOD mesh path (FO3/FNV convention).
/// Strips extension, appends `_lod.nif`.
/// e.g. "meshes\architecture\building01.nif" -> "meshes\architecture\building01_lod.nif"
pub fn model_to_lod_path(model: &str) -> String {
    if let Some(dot) = model.rfind('.') {
        format!("{}_lod.nif", &model[..dot])
    } else {
        format!("{}_lod.nif", model)
    }
}

/// Simple non-cryptographic hash for generating temp file names.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.to_lowercase().bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
