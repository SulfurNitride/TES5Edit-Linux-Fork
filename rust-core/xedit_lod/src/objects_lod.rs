//! Objects LOD generation -- mesh collection, texture atlas, NIF combination.
//!
//! Matches the Delphi xEdit pipeline:
//! 1. Atlas is built BEFORE this function (passed in as atlas_map)
//! 2. For each LOD level, group references by spatial quadrant
//! 3. Within each quadrant, load LOD meshes, apply REFR transforms,
//!    remap UVs to atlas coordinates, combine by texture, write NIF

use crate::nif_combiner::{combine_by_texture, CombinedMesh};
use crate::progress::Progress;
use crate::reference_scanner::{LodBase, LodReference};
use crate::resource_loader::ResourceLoader;
use crate::{is_fo3_fnv, mesh_output_dir, model_to_lod_path, AtlasMapping, LodOptions, LodSettings};
use anyhow::Result;

use std::collections::HashMap;
use std::path::Path;
use xedit_dom::Signature;

/// Maximum vertices per shape (u16 index limit).
const MAX_VERTS_PER_SHAPE: usize = 65535;

/// Map game ID string to nifly game version number.
/// 0=Oblivion, 1=Fallout3/FNV, 2=SkyrimLE, 3=SkyrimSE, 4=Fallout4, 5=Starfield
fn game_version_from_id(game_id: &str) -> u32 {
    match game_id {
        "Oblivion" => 0,
        "Fallout3" | "FalloutNV" => 1,
        "SkyrimLE" => 2,
        "SkyrimSE" => 3,
        "Fallout4" | "Fallout76" => 4,
        "Starfield" => 5,
        _ => 3,
    }
}

/// Generate Objects LOD meshes for a worldspace.
pub fn generate_objects_lod(
    options: &LodOptions,
    worldspace: &str,
    references: &[LodReference],
    bases: &HashMap<u32, LodBase>,
    loader: &ResourceLoader,
    progress: &Progress,
    settings: Option<&LodSettings>,
    atlas_map: &HashMap<String, AtlasMapping>,
) -> Result<()> {
    progress.report(&format!("Generating Objects LOD for {}", worldspace));

    let nifly = match xedit_nif::NiflyLibrary::load() {
        Ok(lib) => lib,
        Err(e) => {
            anyhow::bail!("Cannot generate Objects LOD: nifly library not available: {}", e);
        }
    };

    let game_version = game_version_from_id(&options.game_id);
    let is_fnv = is_fo3_fnv(&options.game_id);
    tracing::info!("Using nifly game version {} for game '{}'", game_version, options.game_id);

    let tree_sig = Signature::from_bytes(b"TREE");
    let scol_sig = Signature::from_bytes(b"SCOL");
    let acti_sig = Signature::from_bytes(b"ACTI");
    let mstt_sig = Signature::from_bytes(b"MSTT");

    // Filter to LOD-eligible references based on game
    let obj_refs: Vec<&LodReference> = references
        .iter()
        .filter(|r| {
            if let Some(base) = bases.get(&r.base_form_id) {
                if is_fnv {
                    // FO3/FNV: STAT, SCOL, ACTI, MSTT
                    base.signature == Signature::STAT
                        || base.signature == scol_sig
                        || base.signature == acti_sig
                        || base.signature == mstt_sig
                } else {
                    // Skyrim: STAT and TREE (if trees_3d)
                    base.signature == Signature::STAT
                        || (options.trees_3d && base.signature == tree_sig)
                }
            } else {
                false
            }
        })
        // Filter by LOD cell bounds if we have settings
        .filter(|r| {
            if let Some(s) = settings {
                let cell_x = (r.position[0] / 4096.0).floor() as i32;
                let cell_y = (r.position[1] / 4096.0).floor() as i32;
                s.cell_in_bounds(cell_x, cell_y)
            } else {
                true
            }
        })
        .collect();

    if obj_refs.is_empty() {
        tracing::info!("No object references found for worldspace {}", worldspace);
        return Ok(());
    }

    tracing::info!(
        "Processing {} object references for worldspace {}",
        obj_refs.len(), worldspace
    );

    let output_base = Path::new(&options.output_dir);

    // Pre-resolve all unique mesh paths
    progress.report(&format!(
        "Pre-loading LOD meshes for {} ({} references)...",
        worldspace, obj_refs.len()
    ));

    let mut unique_model_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
    for refr in &obj_refs {
        let base = match bases.get(&refr.base_form_id) {
            Some(b) => b,
            None => continue,
        };

        if is_fnv {
            // FO3/FNV: only one LOD level, derived from MODL + _lod.nif
            if let Some(ref model) = base.full_model {
                unique_model_paths.insert(model_to_lod_path(model));
            }
        } else {
            // Skyrim: use MNAM LOD models per level
            for lod_level in &options.lod_levels {
                let lod_idx = match lod_level {
                    4 => 0, 8 => 1, 16 => 2, 32 => 3, _ => 0,
                };
                let model_path = base.lod_models.get(lod_idx).and_then(|m| m.as_ref())
                    .or_else(|| (0..lod_idx).rev().find_map(|i| base.lod_models.get(i).and_then(|m| m.as_ref())))
                    .or(base.full_model.as_ref());
                if let Some(p) = model_path {
                    unique_model_paths.insert(p.clone());
                }
            }
        }
    }

    tracing::info!(
        "Pre-resolving {} unique LOD mesh paths for {}",
        unique_model_paths.len(), worldspace
    );

    // Resolve all mesh data into cache
    let mut resolve_failures = 0u32;
    let mesh_cache: HashMap<String, Vec<u8>> = unique_model_paths
        .iter()
        .filter_map(|path| {
            let result = loader.resolve(path)
                .or_else(|_| loader.resolve(&format!("meshes\\{}", path)));
            match result {
                Ok(data) => Some((path.clone(), data)),
                Err(_) => { resolve_failures += 1; None }
            }
        })
        .collect();

    if resolve_failures > 0 {
        tracing::info!(
            "{}/{} mesh paths not found (may be normal for missing LOD meshes)",
            resolve_failures, unique_model_paths.len()
        );
    }

    tracing::info!(
        "Loaded {} meshes into cache ({:.1} MB)",
        mesh_cache.len(),
        mesh_cache.values().map(|v| v.len()).sum::<usize>() as f64 / 1_048_576.0
    );

    if mesh_cache.is_empty() {
        tracing::warn!("No meshes could be resolved for worldspace {}", worldspace);
        return Ok(());
    }

    let mut total_nifs_written = 0u32;

    // For FO3/FNV, there's only one LOD level
    let effective_levels: Vec<u32> = if is_fnv {
        vec![4] // Single LOD level for FO3/FNV
    } else {
        options.lod_levels.clone()
    };

    for &lod_level in &effective_levels {
        if progress.is_cancelled() {
            anyhow::bail!("Cancelled");
        }

        // Group references by quadrant using LOD settings block alignment
        let mut quadrants: HashMap<(i32, i32), Vec<&LodReference>> = HashMap::new();

        for refr in &obj_refs {
            let cell_x = (refr.position[0] / 4096.0).floor() as i32;
            let cell_y = (refr.position[1] / 4096.0).floor() as i32;

            let (quad_x, quad_y) = if let Some(s) = settings {
                s.block_for_cell(cell_x, cell_y, lod_level as i32)
            } else {
                let ql = lod_level as i32;
                (cell_x.div_euclid(ql) * ql, cell_y.div_euclid(ql) * ql)
            };

            quadrants.entry((quad_x, quad_y)).or_default().push(refr);
        }

        let num_quadrants = quadrants.len();
        progress.set_total(num_quadrants as u64);
        progress.report(&format!(
            "Objects LOD level {} for {} ({} quadrants)",
            lod_level, worldspace, num_quadrants
        ));

        // Prepare output directory (game-specific path)
        let mesh_dir_rel = mesh_output_dir(&options.game_id, worldspace);
        let mesh_dir = output_base.join(&mesh_dir_rel);
        std::fs::create_dir_all(&mesh_dir)?;

        // Process quadrants sequentially (nifly is not thread-safe)
        for ((qx, qy), refs) in &quadrants {
            if progress.is_cancelled() {
                anyhow::bail!("Cancelled");
            }
            match process_quadrant(
                options, worldspace, *qx, *qy, lod_level, refs, bases,
                &mesh_cache, &mesh_dir, &nifly, game_version, atlas_map,
            ) {
                Ok(wrote) => { if wrote { total_nifs_written += 1; } }
                Err(e) => {
                    tracing::warn!("Quadrant ({},{}) LOD{} error: {:#}", qx, qy, lod_level, e);
                }
            }
            progress.increment();
        }
    }

    tracing::info!(
        "Objects LOD complete for {} — wrote {} NIF files",
        worldspace, total_nifs_written
    );
    progress.report(&format!(
        "Objects LOD complete for {} ({} files written)",
        worldspace, total_nifs_written
    ));
    Ok(())
}

/// Process a single quadrant: load meshes, apply transforms, remap UVs, combine, write NIF.
fn process_quadrant(
    options: &LodOptions,
    worldspace: &str,
    qx: i32,
    qy: i32,
    lod_level: u32,
    refs: &[&LodReference],
    bases: &HashMap<u32, LodBase>,
    mesh_cache: &HashMap<String, Vec<u8>>,
    mesh_dir: &Path,
    nifly: &xedit_nif::NiflyLibrary,
    game_version: u32,
    atlas_map: &HashMap<String, AtlasMapping>,
) -> Result<bool> {
    let is_fnv = is_fo3_fnv(&options.game_id);
    let mut per_ref_meshes: Vec<CombinedMesh> = Vec::new();

    for refr in refs {
        let base = match bases.get(&refr.base_form_id) {
            Some(b) => b,
            None => continue,
        };

        let model_path = if is_fnv {
            // FO3/FNV: MODL + _lod.nif
            base.full_model.as_ref().map(|m| model_to_lod_path(m))
        } else {
            // Skyrim: use MNAM LOD models
            let lod_idx = match lod_level {
                4 => 0, 8 => 1, 16 => 2, 32 => 3, _ => 0,
            };
            base.lod_models.get(lod_idx).and_then(|m| m.as_ref()).cloned()
                .or_else(|| (0..lod_idx).rev().find_map(|i| base.lod_models.get(i).and_then(|m| m.as_ref()).cloned()))
                .or_else(|| base.full_model.clone())
        };

        let model_path = match model_path {
            Some(p) => p,
            None => continue,
        };

        let mesh_data = match mesh_cache.get(&model_path) {
            Some(data) => data,
            None => continue,
        };

        match extract_and_transform_mesh(nifly, mesh_data, refr, &model_path, atlas_map) {
            Ok(meshes) => per_ref_meshes.extend(meshes),
            Err(e) => {
                tracing::debug!(
                    "Failed to process mesh {} for REFR 0x{:08X}: {:#}",
                    model_path, refr.form_id, e
                );
            }
        }
    }

    if per_ref_meshes.is_empty() {
        return Ok(false);
    }

    // Combine meshes that share the same texture
    let combined_shapes = combine_by_texture(per_ref_meshes)?;

    // Output filename: <worldspace>.<qx>.<qy>.<level>.nif  (matching Delphi .bto naming)
    let nif_filename = format!(
        "{}.{}.{}.{}.nif",
        worldspace.to_lowercase(), qx, qy, lod_level
    );
    let nif_path = mesh_dir.join(&nif_filename);

    write_combined_nif(nifly, game_version, &combined_shapes, &nif_path)?;

    tracing::debug!(
        "Wrote Objects LOD NIF: {} ({} shapes)",
        nif_path.display(), combined_shapes.len()
    );

    Ok(true)
}

/// Extract geometry from NIF data and apply REFR transforms + atlas UV remapping.
fn extract_and_transform_mesh(
    nifly: &xedit_nif::NiflyLibrary,
    nif_data: &[u8],
    refr: &LodReference,
    model_path: &str,
    atlas_map: &HashMap<String, AtlasMapping>,
) -> Result<Vec<CombinedMesh>> {
    let temp_dir = std::env::temp_dir().join("xedit_lod");
    std::fs::create_dir_all(&temp_dir)?;

    let hash = crate::simple_hash(model_path);
    let temp_path = temp_dir.join(format!("lod_{:016x}.nif", hash));

    if !temp_path.exists() {
        std::fs::write(&temp_path, nif_data)?;
    }

    let nif = nifly.load_nif(&temp_path)?;
    let shape_count = nif.shape_count()?;
    let mut meshes = Vec::with_capacity(shape_count as usize);

    for shape_idx in 0..shape_count {
        let vert_data = nif.get_vertices(shape_idx)?;
        let tri_data = nif.get_triangles(shape_idx)?;
        let uv_data = nif.get_uvs(shape_idx)?;
        let normal_data = nif.get_normals(shape_idx)?;

        if vert_data.is_empty() || tri_data.is_empty() {
            continue;
        }

        let vert_count = vert_data.len() / 3;

        // Transform vertices by REFR placement
        let mut vertices = Vec::with_capacity(vert_count);
        for i in 0..vert_count {
            let v = [vert_data[i * 3], vert_data[i * 3 + 1], vert_data[i * 3 + 2]];
            vertices.push(transform_vertex(v, refr.position, refr.rotation, refr.scale));
        }

        // Rotate normals
        let normal_count = normal_data.len() / 3;
        let mut normals = Vec::with_capacity(normal_count);
        for i in 0..normal_count {
            let n = [normal_data[i * 3], normal_data[i * 3 + 1], normal_data[i * 3 + 2]];
            normals.push(rotate_normal(n, refr.rotation));
        }

        // Get diffuse texture path
        let texture_path = nif.texture_slot(shape_idx, 0)?.unwrap_or_default();

        // Remap UVs to atlas coordinates if texture is in atlas
        let uv_count = uv_data.len() / 2;
        let mut uvs = Vec::with_capacity(uv_count);
        let (final_texture, atlas_remapped) = if let Some(mapping) = atlas_map.get(&texture_path.to_lowercase()) {
            // Remap UVs: new_u = u * u_scale + u_offset, new_v = v * v_scale + v_offset
            for i in 0..uv_count {
                let u = uv_data[i * 2];
                let v = uv_data[i * 2 + 1];
                uvs.push([
                    u * mapping.u_scale + mapping.u_offset,
                    v * mapping.v_scale + mapping.v_offset,
                ]);
            }
            (mapping.atlas_texture_path.clone(), true)
        } else {
            // No atlas mapping — keep original UVs
            for i in 0..uv_count {
                uvs.push([uv_data[i * 2], uv_data[i * 2 + 1]]);
            }
            (texture_path, false)
        };

        // Convert triangles
        let tri_count = tri_data.len() / 3;
        let mut triangles = Vec::with_capacity(tri_count);
        for i in 0..tri_count {
            triangles.push([
                tri_data[i * 3] as u32,
                tri_data[i * 3 + 1] as u32,
                tri_data[i * 3 + 2] as u32,
            ]);
        }

        if atlas_remapped {
            tracing::trace!("Atlas remapped UVs for texture -> {}", final_texture);
        }

        meshes.push(CombinedMesh {
            vertices,
            normals,
            uvs,
            triangles,
            texture_path: final_texture,
        });
    }

    Ok(meshes)
}

/// Write combined shapes to a NIF file using nifly.
fn write_combined_nif(
    nifly: &xedit_nif::NiflyLibrary,
    game_version: u32,
    shapes: &[CombinedMesh],
    output_path: &Path,
) -> Result<()> {
    let nif = nifly.create_nif(game_version)?;
    let mut shape_num = 0u32;

    for shape in shapes {
        if shape.is_empty() {
            continue;
        }

        if shape.vertices.len() > MAX_VERTS_PER_SHAPE {
            let sub_shapes = split_shape(shape);
            for sub in &sub_shapes {
                write_shape_to_nif(&nif, sub, shape_num, output_path)?;
                shape_num += 1;
            }
        } else {
            write_shape_to_nif(&nif, shape, shape_num, output_path)?;
            shape_num += 1;
        }
    }

    if shape_num == 0 {
        tracing::debug!("No shapes to write for {}", output_path.display());
        return Ok(());
    }

    nif.save(output_path)?;
    Ok(())
}

fn write_shape_to_nif(
    nif: &xedit_nif::NifFile,
    shape: &CombinedMesh,
    shape_num: u32,
    output_path: &Path,
) -> Result<()> {
    let flat_verts = shape.flat_vertices();
    let flat_normals = shape.flat_normals();
    let flat_uvs = shape.flat_uvs();

    let flat_tris = match shape.flat_triangles_u16() {
        Some(t) => t,
        None => {
            tracing::warn!(
                "Shape {} in {} has >65535 vertices ({}) after split, skipping",
                shape_num, output_path.display(), shape.vertices.len()
            );
            return Ok(());
        }
    };

    let shape_name = format!("LODShape{}", shape_num);
    let normals_ref = if flat_normals.is_empty() { None } else { Some(flat_normals.as_slice()) };
    let uvs_ref = if flat_uvs.is_empty() { None } else { Some(flat_uvs.as_slice()) };

    let shape_idx = nif.add_shape(&shape_name, &flat_verts, &flat_tris, uvs_ref, normals_ref)?;

    if !shape.texture_path.is_empty() {
        nif.set_texture(shape_idx, 0, &shape.texture_path)?;
    }

    Ok(())
}

/// Split a combined mesh into sub-meshes with <= MAX_VERTS_PER_SHAPE vertices.
fn split_shape(shape: &CombinedMesh) -> Vec<CombinedMesh> {
    let mut result = Vec::new();
    let mut current_verts: Vec<[f32; 3]> = Vec::new();
    let mut current_normals: Vec<[f32; 3]> = Vec::new();
    let mut current_uvs: Vec<[f32; 2]> = Vec::new();
    let mut current_tris: Vec<[u32; 3]> = Vec::new();
    let mut index_map: HashMap<u32, u32> = HashMap::new();

    for tri in &shape.triangles {
        let mut new_verts_needed = 0;
        for &idx in tri {
            if !index_map.contains_key(&idx) {
                new_verts_needed += 1;
            }
        }

        if current_verts.len() + new_verts_needed > MAX_VERTS_PER_SHAPE {
            if !current_tris.is_empty() {
                result.push(CombinedMesh {
                    vertices: std::mem::take(&mut current_verts),
                    normals: std::mem::take(&mut current_normals),
                    uvs: std::mem::take(&mut current_uvs),
                    triangles: std::mem::take(&mut current_tris),
                    texture_path: shape.texture_path.clone(),
                });
            }
            index_map.clear();
        }

        let mut new_tri = [0u32; 3];
        for (i, &old_idx) in tri.iter().enumerate() {
            let new_idx = *index_map.entry(old_idx).or_insert_with(|| {
                let idx = current_verts.len() as u32;
                let oi = old_idx as usize;
                current_verts.push(shape.vertices[oi]);
                if oi < shape.normals.len() { current_normals.push(shape.normals[oi]); }
                if oi < shape.uvs.len() { current_uvs.push(shape.uvs[oi]); }
                idx
            });
            new_tri[i] = new_idx;
        }
        current_tris.push(new_tri);
    }

    if !current_tris.is_empty() {
        result.push(CombinedMesh {
            vertices: current_verts,
            normals: current_normals,
            uvs: current_uvs,
            triangles: current_tris,
            texture_path: shape.texture_path.clone(),
        });
    }

    tracing::info!(
        "Split shape with {} verts into {} sub-shapes",
        shape.vertices.len(), result.len()
    );

    result
}

/// Apply position, rotation, and scale transform to a vertex.
fn transform_vertex(v: [f32; 3], pos: [f32; 3], rot: [f32; 3], scale: f32) -> [f32; 3] {
    let sx = v[0] * scale;
    let sy = v[1] * scale;
    let sz = v[2] * scale;

    let (sin_x, cos_x) = rot[0].sin_cos();
    let (sin_y, cos_y) = rot[1].sin_cos();
    let (sin_z, cos_z) = rot[2].sin_cos();

    let rx = cos_y * cos_z * sx
        + (sin_x * sin_y * cos_z - cos_x * sin_z) * sy
        + (cos_x * sin_y * cos_z + sin_x * sin_z) * sz;
    let ry = cos_y * sin_z * sx
        + (sin_x * sin_y * sin_z + cos_x * cos_z) * sy
        + (cos_x * sin_y * sin_z - sin_x * cos_z) * sz;
    let rz = -sin_y * sx + sin_x * cos_y * sy + cos_x * cos_y * sz;

    [rx + pos[0], ry + pos[1], rz + pos[2]]
}

/// Apply rotation to a normal vector (no translation or scale).
fn rotate_normal(n: [f32; 3], rot: [f32; 3]) -> [f32; 3] {
    let (sin_x, cos_x) = rot[0].sin_cos();
    let (sin_y, cos_y) = rot[1].sin_cos();
    let (sin_z, cos_z) = rot[2].sin_cos();

    let rx = cos_y * cos_z * n[0]
        + (sin_x * sin_y * cos_z - cos_x * sin_z) * n[1]
        + (cos_x * sin_y * cos_z + sin_x * sin_z) * n[2];
    let ry = cos_y * sin_z * n[0]
        + (sin_x * sin_y * sin_z + cos_x * cos_z) * n[1]
        + (cos_x * sin_y * sin_z - sin_x * cos_z) * n[2];
    let rz = -sin_y * n[0] + sin_x * cos_y * n[1] + cos_x * cos_y * n[2];

    let len = (rx * rx + ry * ry + rz * rz).sqrt();
    if len > 1e-6 { [rx / len, ry / len, rz / len] } else { [0.0, 0.0, 1.0] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_vertex_identity() {
        let v = [1.0, 2.0, 3.0];
        let result = transform_vertex(v, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0);
        assert!((result[0] - 1.0).abs() < 1e-5);
        assert!((result[1] - 2.0).abs() < 1e-5);
        assert!((result[2] - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_transform_vertex_translation() {
        let v = [0.0, 0.0, 0.0];
        let result = transform_vertex(v, [10.0, 20.0, 30.0], [0.0, 0.0, 0.0], 1.0);
        assert!((result[0] - 10.0).abs() < 1e-5);
        assert!((result[1] - 20.0).abs() < 1e-5);
        assert!((result[2] - 30.0).abs() < 1e-5);
    }

    #[test]
    fn test_transform_vertex_scale() {
        let v = [1.0, 2.0, 3.0];
        let result = transform_vertex(v, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 2.0);
        assert!((result[0] - 2.0).abs() < 1e-5);
        assert!((result[1] - 4.0).abs() < 1e-5);
        assert!((result[2] - 6.0).abs() < 1e-5);
    }

    #[test]
    fn test_game_version_mapping() {
        assert_eq!(game_version_from_id("FalloutNV"), 1);
        assert_eq!(game_version_from_id("Fallout3"), 1);
        assert_eq!(game_version_from_id("SkyrimSE"), 3);
        assert_eq!(game_version_from_id("Fallout4"), 4);
        assert_eq!(game_version_from_id("Unknown"), 3);
    }
}
