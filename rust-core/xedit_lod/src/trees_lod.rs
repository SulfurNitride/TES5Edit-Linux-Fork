//! Trees LOD generation — billboards, atlas, .lst/.btt output.

use crate::atlas_builder::AtlasBuilder;
use crate::btt_writer::{BttEntry, write_btt};
use crate::progress::Progress;
use crate::reference_scanner::{LodBase, LodReference};
use crate::resource_loader::ResourceLoader;
use crate::LodOptions;
use anyhow::Result;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use xedit_dom::Signature;

pub fn generate_trees_lod(
    options: &LodOptions,
    worldspace: &str,
    references: &[LodReference],
    bases: &HashMap<u32, LodBase>,
    loader: &ResourceLoader,
    progress: &Progress,
) -> Result<()> {
    // If trees_3d is enabled, trees are handled by Objects LOD pipeline
    if options.trees_3d {
        tracing::info!("Trees 3D mode: trees will be generated as Objects LOD");
        return Ok(());
    }

    progress.report(&format!("Generating Trees LOD for {}", worldspace));

    // Filter tree references
    let tree_sig = Signature::from_bytes(b"TREE");
    let tree_refs: Vec<&LodReference> = references
        .iter()
        .filter(|r| {
            bases
                .get(&r.base_form_id)
                .map(|b| b.signature == tree_sig)
                .unwrap_or(false)
        })
        .collect();

    if tree_refs.is_empty() {
        tracing::info!("No tree references found for worldspace {}", worldspace);
        return Ok(());
    }

    // Collect unique tree types
    let mut unique_trees: Vec<u32> = tree_refs.iter().map(|r| r.base_form_id).collect();
    unique_trees.sort_unstable();
    unique_trees.dedup();

    progress.set_total(unique_trees.len() as u64);

    // Build atlas from billboard textures
    let mut atlas = AtlasBuilder::new(
        options.atlas_width,
        options.atlas_height,
        options.atlas_texture_size,
    );

    let mut tree_type_indices: HashMap<u32, usize> = HashMap::new();
    let mut tree_type_names: Vec<String> = Vec::new();

    for &form_id in &unique_trees {
        if progress.is_cancelled() {
            anyhow::bail!("Cancelled");
        }

        let base = match bases.get(&form_id) {
            Some(b) => b,
            None => continue,
        };

        // Try to resolve billboard texture
        let billboard_path = format!(
            "textures\\terrain\\lodgen\\{}_flat.dds",
            base.editor_id.to_lowercase()
        );

        match loader.resolve(&billboard_path) {
            Ok(dds_data) => {
                let idx = atlas.add_texture(&billboard_path, &dds_data)?;
                tree_type_indices.insert(form_id, idx);
                tree_type_names.push(base.editor_id.clone());
            }
            Err(_) => {
                tracing::warn!(
                    "Billboard not found for tree {}: {}",
                    base.editor_id,
                    billboard_path
                );
            }
        }

        progress.increment();
    }

    // Write output files
    let output_base = Path::new(&options.output_dir);

    // Write atlas DDS
    let atlas_dir = output_base
        .join("textures")
        .join("terrain")
        .join(worldspace.to_lowercase())
        .join("trees");
    std::fs::create_dir_all(&atlas_dir)?;

    let atlas_dds = atlas.build(&options.compression_diffuse)?;
    if !atlas_dds.is_empty() {
        let atlas_path = atlas_dir.join(format!("{}TreeLod.dds", worldspace));
        std::fs::write(&atlas_path, &atlas_dds)?;
        tracing::info!("Wrote tree atlas: {}", atlas_path.display());
    }

    // Write .lst file
    let mesh_dir = output_base
        .join("meshes")
        .join("terrain")
        .join(worldspace.to_lowercase())
        .join("trees");
    std::fs::create_dir_all(&mesh_dir)?;

    let lst_path = mesh_dir.join(format!("{}.lst", worldspace));
    let mut lst_file = std::fs::File::create(&lst_path)?;
    writeln!(lst_file, "{}", tree_type_names.len())?;
    for (idx, name) in tree_type_names.iter().enumerate() {
        if let Some(e) = atlas.entries.get(idx) {
            writeln!(
                lst_file,
                "{},{},{},{},{},{},{}",
                name,
                e.width,
                e.height,
                e.x as f32 / options.atlas_width as f32,
                e.y as f32 / options.atlas_height as f32,
                (e.x + e.width) as f32 / options.atlas_width as f32,
                (e.y + e.height) as f32 / options.atlas_height as f32,
            )?;
        }
    }
    tracing::info!("Wrote tree list: {}", lst_path.display());

    // Write .btt files per cell
    // Cell coordinates: cell_x = floor(pos_x / 4096), cell_y = floor(pos_y / 4096)
    let mut cells: HashMap<(i32, i32), Vec<BttEntry>> = HashMap::new();

    for refr in &tree_refs {
        let cell_x = (refr.position[0] / 4096.0).floor() as i32;
        let cell_y = (refr.position[1] / 4096.0).floor() as i32;

        if let Some(&type_idx) = tree_type_indices.get(&refr.base_form_id) {
            cells
                .entry((cell_x, cell_y))
                .or_default()
                .push(BttEntry {
                    x: refr.position[0],
                    y: refr.position[1],
                    z: refr.position[2],
                    rotation: refr.rotation[2], // Z rotation
                    scale: refr.scale,
                    tree_type_index: type_idx as u32,
                });
        }
    }

    for ((cx, cy), entries) in &cells {
        let btt_path = mesh_dir.join(format!("{}.4.{}.{}.btt", worldspace, cx, cy));
        let mut file = std::fs::File::create(&btt_path)?;
        write_btt(&mut file, entries)?;
    }

    tracing::info!(
        "Wrote {} BTT cell files for {}",
        cells.len(),
        worldspace
    );
    Ok(())
}
