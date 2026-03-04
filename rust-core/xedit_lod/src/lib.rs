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
        }

        if options.terrain_lod {
            terrain_lod::generate_terrain_lod(options, worldspace, &loader, &progress)?;
        }
    }

    progress.report("LOD generation complete");
    Ok(())
}
