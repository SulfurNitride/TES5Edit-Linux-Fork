//! LOD generation for xEdit — tree, object, and terrain LOD pipelines.
//!
//! Ported from wbLOD.pas in the original xEdit Pascal source.
//! Supports Fallout 3/NV, Skyrim, Fallout 4, and Oblivion.

pub mod resource_loader;
pub mod lod_settings;
pub mod bin_packer;
pub mod dds_util;
pub mod trees_lod;
pub mod atlas_builder;
pub mod objects_lod;
pub mod terrain_lod;
pub mod export_writer;
pub mod nif_combiner;
pub mod reference_scanner;
pub mod progress;

use serde::{Deserialize, Serialize};

/// DDS compression format codes (matching xLODGen settings file values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum DdsFormat {
    /// DXT1 / BC1 — 4bpp, no alpha or 1-bit alpha
    Dxt1 = 200,
    /// DXT5 / BC3 — 8bpp, full alpha
    Dxt5 = 202,
    /// BC5 / ATI2N — 8bpp, two-channel (normal/specular)
    Bc5 = 205,
}

impl DdsFormat {
    pub fn from_code(code: u32) -> Option<Self> {
        match code {
            200 => Some(Self::Dxt1),
            202 => Some(Self::Dxt5),
            205 => Some(Self::Bc5),
            _ => None,
        }
    }
}

/// Complete LOD generation options, parsed from settings file or GUI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodOptions {
    // Objects LOD
    pub objects_lod: bool,
    pub build_atlas: bool,
    pub atlas_width: u32,
    pub atlas_height: u32,
    pub atlas_texture_size: u32,
    pub atlas_texture_uv_range: f32,
    pub atlas_diffuse_format: u32,
    pub atlas_normal_format: u32,
    pub atlas_specular_format: u32,
    pub default_alpha_threshold: u32,
    pub objects_no_tangents: bool,
    pub objects_no_vertex_colors: bool,

    // Trees LOD
    pub trees_lod: bool,
    pub tree_atlas_diffuse_format: u32,
    pub tree_atlas_normal_format: u32,
    pub tree_normal_map: bool,
    pub trees_brightness: i32,

    // Terrain LOD
    pub terrain_lod: bool,
    pub terrain_build_meshes: bool,
    pub terrain_build_diffuse: bool,
    pub terrain_build_normal: bool,

    // Per-level terrain settings
    pub terrain_quality: [u32; 4],        // LOD4, LOD8, LOD16, LOD32
    pub terrain_max_verts: [u32; 4],
    pub terrain_optimize_unseen: [bool; 4],
    pub terrain_diffuse_size: [u32; 4],
    pub terrain_diffuse_comp: [u32; 4],
    pub terrain_diffuse_mipmap: [bool; 4],
    pub terrain_normal_size: [u32; 4],
    pub terrain_normal_comp: [u32; 4],
    pub terrain_normal_mipmap: [bool; 4],
    pub terrain_gamma: [f32; 4],
    pub terrain_brightness: [i32; 4],
    pub terrain_contrast: [i32; 4],
    pub terrain_default_diffuse_size: u32,
    pub terrain_default_normal_size: u32,
    pub terrain_vertex_color_multiplier: f32,

    // General
    pub worldspaces: Vec<String>,
    pub output_dir: String,
    pub data_dir: String,
}

impl LodOptions {
    /// Parse from an xLODGen settings INI file (e.g. Plugins.fnvviewsettings).
    pub fn from_settings_file(content: &str) -> anyhow::Result<Self> {
        let get = |key: &str| -> String {
            for line in content.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix(key) {
                    if let Some(val) = val.strip_prefix('=') {
                        return val.trim().to_string();
                    }
                }
            }
            String::new()
        };

        let get_u32 = |key: &str, default: u32| -> u32 {
            get(key).parse().unwrap_or(default)
        };
        let get_i32 = |key: &str, default: i32| -> i32 {
            get(key).parse().unwrap_or(default)
        };
        let get_f32 = |key: &str, default: f32| -> f32 {
            get(key).parse().unwrap_or(default)
        };
        let get_bool = |key: &str| -> bool {
            get_u32(key, 0) != 0
        };

        Ok(Self {
            objects_lod: get_bool("ObjectsLOD"),
            build_atlas: get_bool("BuildAtlas"),
            atlas_width: get_u32("AtlasWidth", 4096),
            atlas_height: get_u32("AtlasHeight", 4096),
            atlas_texture_size: get_u32("AtlasTextureSize", 1024),
            atlas_texture_uv_range: get_u32("AtlasTextureUVRange", 10000) as f32 / 10000.0,
            atlas_diffuse_format: get_u32("AtlasDiffuseFormat", 202),
            atlas_normal_format: get_u32("AtlasNormalFormat", 200),
            atlas_specular_format: get_u32("AtlasSpecularFormat", 205),
            default_alpha_threshold: get_u32("DefaultAlphaThreshold", 128),
            objects_no_tangents: get_bool("ObjectsNoTangents"),
            objects_no_vertex_colors: get_bool("ObjectsNoVertexColors"),

            trees_lod: get_bool("TreesLOD"),
            tree_atlas_diffuse_format: get_u32("TreeAtlasDiffuseFormat", 202),
            tree_atlas_normal_format: get_u32("TreeAtlasNormalFormat", 202),
            tree_normal_map: get_bool("TreeNormalMap"),
            trees_brightness: get_i32("TreesBrightness", 0),

            terrain_lod: get_bool("TerrainLOD"),
            terrain_build_meshes: get_bool("TerrainBuildMeshes"),
            terrain_build_diffuse: get_bool("TerrainBuildDiffuseTextures"),
            terrain_build_normal: get_bool("TerrainBuildNormalTextures"),

            terrain_quality: [
                get_u32("TerrainQualityLOD4", 10),
                get_u32("TerrainQualityLOD8", 15),
                get_u32("TerrainQualityLOD16", 20),
                get_u32("TerrainQualityLOD32", 25),
            ],
            terrain_max_verts: [
                get_u32("TerrainMaxVertsLOD4", 32767),
                get_u32("TerrainMaxVertsLOD8", 32767),
                get_u32("TerrainMaxVertsLOD16", 32767),
                get_u32("TerrainMaxVertsLOD32", 32767),
            ],
            terrain_optimize_unseen: [
                get_bool("TerrainWaterDeltaLOD4"),
                get_bool("TerrainWaterDeltaLOD8"),
                get_bool("TerrainWaterDeltaLOD16"),
                get_bool("TerrainWaterDeltaLOD32"),
            ],
            terrain_diffuse_size: [
                get_u32("TerrainDiffuseSizeLOD4", 512),
                get_u32("TerrainDiffuseSizeLOD8", 512),
                get_u32("TerrainDiffuseSizeLOD16", 512),
                get_u32("TerrainDiffuseSizeLOD32", 512),
            ],
            terrain_diffuse_comp: [
                get_u32("TerrainDiffuseCompLOD4", 200),
                get_u32("TerrainDiffuseCompLOD8", 200),
                get_u32("TerrainDiffuseCompLOD16", 200),
                get_u32("TerrainDiffuseCompLOD32", 200),
            ],
            terrain_diffuse_mipmap: [
                get_bool("TerrainDiffuseMipMapLOD4"),
                get_bool("TerrainDiffuseMipMapLOD8"),
                get_bool("TerrainDiffuseMipMapLOD16"),
                get_bool("TerrainDiffuseMipMapLOD32"),
            ],
            terrain_normal_size: [
                get_u32("TerrainNormalSizeLOD4", 512),
                get_u32("TerrainNormalSizeLOD8", 512),
                get_u32("TerrainNormalSizeLOD16", 512),
                get_u32("TerrainNormalSizeLOD32", 512),
            ],
            terrain_normal_comp: [
                get_u32("TerrainNormalCompLOD4", 200),
                get_u32("TerrainNormalCompLOD8", 200),
                get_u32("TerrainNormalCompLOD16", 200),
                get_u32("TerrainNormalCompLOD32", 200),
            ],
            terrain_normal_mipmap: [
                get_bool("TerrainNormalMipMapLOD4"),
                get_bool("TerrainNormalMipMapLOD8"),
                get_bool("TerrainNormalMipMapLOD16"),
                get_bool("TerrainNormalMipMapLOD32"),
            ],
            terrain_gamma: [
                get_f32("TerrainGammaLOD4", 1.0),
                get_f32("TerrainGammaLOD8", 1.0),
                get_f32("TerrainGammaLOD16", 1.0),
                get_f32("TerrainGammaLOD32", 1.0),
            ],
            terrain_brightness: [
                get_i32("TerrainBrightnessLOD4", 0),
                get_i32("TerrainBrightnessLOD8", 0),
                get_i32("TerrainBrightnessLOD16", 0),
                get_i32("TerrainBrightnessLOD32", 0),
            ],
            terrain_contrast: [
                get_i32("TerrainContrastLOD4", 0),
                get_i32("TerrainContrastLOD8", 0),
                get_i32("TerrainContrastLOD16", 0),
                get_i32("TerrainContrastLOD32", 0),
            ],
            terrain_default_diffuse_size: get_u32("TerrainDefaultDiffuseSize", 128),
            terrain_default_normal_size: get_u32("TerrainDefaultNormalSize", 128),
            terrain_vertex_color_multiplier: get_f32("TerrainVertexColorMultiplier", 1.0),

            worldspaces: Vec::new(),
            output_dir: String::new(),
            data_dir: String::new(),
        })
    }
}
