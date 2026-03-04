//! Terrain LOD generation — heightmap meshes.

use crate::lod_settings::LodSettings;
use crate::progress::Progress;
use crate::resource_loader::ResourceLoader;
use crate::LodOptions;
use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::Cursor;
use std::path::Path;

/// A parsed LAND record's heightfield data
#[derive(Debug, Clone)]
pub struct HeightfieldData {
    pub cell_x: i32,
    pub cell_y: i32,
    pub heights: [[f32; 33]; 33],
    pub normals: [[[i8; 3]; 33]; 33],
}

/// Terrain LOD generation entry point
pub fn generate_terrain_lod(
    options: &LodOptions,
    worldspace: &str,
    loader: &ResourceLoader,
    progress: &Progress,
) -> Result<()> {
    progress.report(&format!("Generating Terrain LOD for {}", worldspace));

    // Load LOD settings to get cell bounds
    let settings_path = format!("lodsettings\\{}.lod", worldspace);
    let settings_data = match loader.resolve(&settings_path) {
        Ok(data) => data,
        Err(_) => {
            tracing::warn!("No LOD settings for terrain: {}", worldspace);
            return Ok(());
        }
    };
    let settings = LodSettings::parse(&settings_data)?;
    let (sw_x, sw_y, ne_x, ne_y) = settings.cell_range();

    tracing::info!(
        "Terrain LOD bounds: ({}, {}) to ({}, {})",
        sw_x,
        sw_y,
        ne_x,
        ne_y
    );

    let output_base = Path::new(&options.output_dir);

    for &lod_level in &options.lod_levels {
        if progress.is_cancelled() {
            anyhow::bail!("Cancelled");
        }

        progress.report(&format!(
            "Terrain LOD level {} for {}",
            lod_level, worldspace
        ));

        // Calculate quadrant bounds
        let quad_size = lod_level as i32;
        let q_sw_x = sw_x.div_euclid(quad_size);
        let q_sw_y = sw_y.div_euclid(quad_size);
        let q_ne_x = ne_x.div_euclid(quad_size);
        let q_ne_y = ne_y.div_euclid(quad_size);

        let total_quads = ((q_ne_x - q_sw_x + 1) * (q_ne_y - q_sw_y + 1)) as u64;
        progress.set_total(total_quads);

        let mesh_dir = output_base
            .join("meshes")
            .join("terrain")
            .join(worldspace.to_lowercase());
        std::fs::create_dir_all(&mesh_dir)?;

        let tex_dir = output_base
            .join("textures")
            .join("terrain")
            .join(worldspace.to_lowercase());
        std::fs::create_dir_all(&tex_dir)?;

        for qy in q_sw_y..=q_ne_y {
            for qx in q_sw_x..=q_ne_x {
                if progress.is_cancelled() {
                    anyhow::bail!("Cancelled");
                }

                // Downsample grid sizes per LOD level:
                // LOD 4: 8x8 vertices per cell
                // LOD 8: 4x4 vertices per cell
                // LOD 16: 2x2 vertices per cell
                // LOD 32: 1x1 vertices per cell
                let verts_per_cell = match lod_level {
                    4 => 8,
                    8 => 4,
                    16 => 2,
                    32 => 1,
                    _ => 4,
                };

                // In a full implementation:
                // 1. For each cell (qx*quad_size..qx*quad_size+quad_size-1, same for y):
                //    a. Find LAND record for this cell
                //    b. Parse VHGT subrecord for height data
                //    c. Parse VNML subrecord for normals
                // 2. Downsample the 33x33 grid to verts_per_cell resolution
                // 3. Build mesh vertices (position = cell_origin + grid_step * idx, height from data)
                // 4. Build triangle strip
                // 5. Write .btr (terrain mesh) file
                // 6. Extract/blend landscape textures -> write .dds

                let _btr_path = mesh_dir.join(format!(
                    "{}.{}.{}.{}.btr",
                    worldspace, lod_level, qx, qy
                ));
                let _dds_path = tex_dir.join(format!(
                    "{}.{}.{}.{}.dds",
                    worldspace, lod_level, qx, qy
                ));

                tracing::debug!(
                    "Would process terrain quad ({}, {}) LOD {} ({} verts/cell)",
                    qx,
                    qy,
                    lod_level,
                    verts_per_cell
                );

                progress.increment();
            }
        }
    }

    progress.report(&format!("Terrain LOD complete for {}", worldspace));
    Ok(())
}

/// Parse VHGT subrecord data into a 33x33 height grid.
/// VHGT format: f32 base_height + i8[33*33] cumulative row offsets
pub fn parse_vhgt(data: &[u8]) -> Result<[[f32; 33]; 33]> {
    anyhow::ensure!(data.len() >= 4 + 33 * 33, "VHGT data too small");

    let mut cursor = Cursor::new(data);
    let base_height = cursor.read_f32::<LittleEndian>()?;

    let mut heights = [[0.0f32; 33]; 33];

    for row in 0..33 {
        let mut row_height = base_height;
        for col in 0..33 {
            let offset = cursor.read_i8()? as f32;
            if col == 0 && row > 0 {
                // First column: offset from the cell at [row-1][0]
                row_height = heights[row - 1][0] + offset * 8.0;
            } else if col == 0 {
                row_height = base_height + offset * 8.0;
            } else {
                row_height += offset * 8.0;
            }
            heights[row][col] = row_height;
        }
    }

    Ok(heights)
}

/// Parse VNML subrecord data into 33x33 normal vectors
pub fn parse_vnml(data: &[u8]) -> Result<[[[i8; 3]; 33]; 33]> {
    anyhow::ensure!(data.len() >= 33 * 33 * 3, "VNML data too small");

    let mut normals = [[[0i8; 3]; 33]; 33];
    let mut offset = 0;

    for row in 0..33 {
        for col in 0..33 {
            normals[row][col] = [
                data[offset] as i8,
                data[offset + 1] as i8,
                data[offset + 2] as i8,
            ];
            offset += 3;
        }
    }

    Ok(normals)
}

/// Downsample a 33x33 height grid to a lower resolution grid
pub fn downsample_heights(heights: &[[f32; 33]; 33], target_size: usize) -> Vec<Vec<f32>> {
    let step = 32.0 / (target_size - 1) as f32;
    let mut result = vec![vec![0.0f32; target_size]; target_size];

    for row in 0..target_size {
        for col in 0..target_size {
            let src_row = (row as f32 * step).min(32.0);
            let src_col = (col as f32 * step).min(32.0);

            // Bilinear interpolation
            let r0 = src_row.floor() as usize;
            let c0 = src_col.floor() as usize;
            let r1 = (r0 + 1).min(32);
            let c1 = (c0 + 1).min(32);
            let fr = src_row.fract();
            let fc = src_col.fract();

            result[row][col] = heights[r0][c0] * (1.0 - fr) * (1.0 - fc)
                + heights[r0][c1] * (1.0 - fr) * fc
                + heights[r1][c0] * fr * (1.0 - fc)
                + heights[r1][c1] * fr * fc;
        }
    }

    result
}

/// Build terrain mesh vertices and triangles from a downsampled height grid
pub fn build_terrain_mesh(
    heights: &[Vec<f32>],
    cell_x: i32,
    cell_y: i32,
    grid_size: usize,
) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let cell_size = 4096.0;
    let step = cell_size / (grid_size - 1) as f32;
    let origin_x = cell_x as f32 * cell_size;
    let origin_y = cell_y as f32 * cell_size;

    let mut vertices = Vec::with_capacity(grid_size * grid_size);
    let mut triangles = Vec::new();

    for row in 0..grid_size {
        for col in 0..grid_size {
            vertices.push([
                origin_x + col as f32 * step,
                origin_y + row as f32 * step,
                heights[row][col],
            ]);
        }
    }

    // Build triangle strip
    for row in 0..grid_size - 1 {
        for col in 0..grid_size - 1 {
            let i0 = (row * grid_size + col) as u32;
            let i1 = i0 + 1;
            let i2 = ((row + 1) * grid_size + col) as u32;
            let i3 = i2 + 1;

            triangles.push([i0, i2, i1]);
            triangles.push([i1, i2, i3]);
        }
    }

    (vertices, triangles)
}
