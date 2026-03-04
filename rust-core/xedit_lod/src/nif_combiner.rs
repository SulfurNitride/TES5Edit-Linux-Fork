//! NIF mesh combination -- merging multiple LOD meshes into combined quadrant NIFs.

use anyhow::Result;

/// Combined mesh data from multiple sources.
#[derive(Debug, Clone)]
pub struct CombinedMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    pub texture_path: String,
}

impl CombinedMesh {
    /// Create an empty combined mesh with the given texture path.
    pub fn new(texture_path: String) -> Self {
        Self {
            vertices: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            triangles: Vec::new(),
            texture_path,
        }
    }

    /// Total vertex count.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Total triangle count.
    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Returns true if this mesh has no geometry.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Flatten vertices to a contiguous [x, y, z, ...] buffer for nifly.
    pub fn flat_vertices(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.vertices.len() * 3);
        for v in &self.vertices {
            out.push(v[0]);
            out.push(v[1]);
            out.push(v[2]);
        }
        out
    }

    /// Flatten normals to a contiguous [x, y, z, ...] buffer for nifly.
    pub fn flat_normals(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.normals.len() * 3);
        for n in &self.normals {
            out.push(n[0]);
            out.push(n[1]);
            out.push(n[2]);
        }
        out
    }

    /// Flatten UVs to a contiguous [u, v, ...] buffer for nifly.
    pub fn flat_uvs(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.uvs.len() * 2);
        for uv in &self.uvs {
            out.push(uv[0]);
            out.push(uv[1]);
        }
        out
    }

    /// Flatten triangle indices to u16 [i0, i1, i2, ...] buffer for nifly.
    /// Returns None if any index exceeds u16::MAX.
    pub fn flat_triangles_u16(&self) -> Option<Vec<u16>> {
        let mut out = Vec::with_capacity(self.triangles.len() * 3);
        for tri in &self.triangles {
            for &idx in tri {
                if idx > u16::MAX as u32 {
                    return None;
                }
                out.push(idx as u16);
            }
        }
        Some(out)
    }
}

/// Merge multiple meshes into one, offsetting triangle indices.
///
/// All input meshes are concatenated. Triangle indices are adjusted by the
/// running vertex offset so they reference the correct vertices in the
/// combined buffer.
pub fn combine_meshes(meshes: &[CombinedMesh]) -> Result<CombinedMesh> {
    let total_verts: usize = meshes.iter().map(|m| m.vertices.len()).sum();
    let total_tris: usize = meshes.iter().map(|m| m.triangles.len()).sum();

    let mut combined = CombinedMesh {
        vertices: Vec::with_capacity(total_verts),
        normals: Vec::with_capacity(total_verts),
        uvs: Vec::with_capacity(total_verts),
        triangles: Vec::with_capacity(total_tris),
        texture_path: meshes
            .first()
            .map(|m| m.texture_path.clone())
            .unwrap_or_default(),
    };

    for mesh in meshes {
        let vert_offset = combined.vertices.len() as u32;

        combined.vertices.extend_from_slice(&mesh.vertices);
        combined.normals.extend_from_slice(&mesh.normals);
        combined.uvs.extend_from_slice(&mesh.uvs);

        // Offset triangle indices
        for tri in &mesh.triangles {
            combined.triangles.push([
                tri[0] + vert_offset,
                tri[1] + vert_offset,
                tri[2] + vert_offset,
            ]);
        }
    }

    Ok(combined)
}

/// Group meshes by texture path, combining those that share the same texture.
///
/// This reduces the number of shapes in the final NIF by merging meshes that
/// use the same material. Returns one CombinedMesh per unique texture.
pub fn combine_by_texture(meshes: Vec<CombinedMesh>) -> Result<Vec<CombinedMesh>> {
    use std::collections::HashMap;

    let mut groups: HashMap<String, Vec<CombinedMesh>> = HashMap::new();
    for mesh in meshes {
        groups
            .entry(mesh.texture_path.to_lowercase())
            .or_default()
            .push(mesh);
    }

    let mut results = Vec::with_capacity(groups.len());
    for (_key, group) in groups {
        results.push(combine_meshes(&group)?);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combine_two_meshes() {
        let m1 = CombinedMesh {
            vertices: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            triangles: vec![[0, 1, 2]],
            texture_path: "textures/test.dds".into(),
        };
        let m2 = CombinedMesh {
            vertices: vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            triangles: vec![[0, 1, 2]],
            texture_path: "textures/test.dds".into(),
        };

        let combined = combine_meshes(&[m1, m2]).unwrap();
        assert_eq!(combined.vertices.len(), 6);
        assert_eq!(combined.triangles.len(), 2);
        // Second triangle should have offset indices
        assert_eq!(combined.triangles[1], [3, 4, 5]);
    }

    #[test]
    fn test_flat_triangles_u16_overflow() {
        let mesh = CombinedMesh {
            vertices: vec![[0.0; 3]; 70000],
            normals: vec![],
            uvs: vec![],
            triangles: vec![[0, 1, 69999]],
            texture_path: String::new(),
        };
        // 69999 > u16::MAX (65535), so this should return None
        assert!(mesh.flat_triangles_u16().is_none());
    }

    #[test]
    fn test_empty_combine() {
        let combined = combine_meshes(&[]).unwrap();
        assert!(combined.is_empty());
    }
}
