//! NIF validation — detect common issues in NIF mesh files.

use std::fmt;
use std::path::Path;

use crate::error::NifError;
use crate::loader::NiflyLibrary;

/// Maximum number of texture slots to probe per shape.
const MAX_TEXTURE_SLOTS: u32 = 10;

/// Known valid NIF block type names from the Bethesda/NetImmerse/Gamebryo ecosystem.
const KNOWN_BLOCK_TYPES: &[&str] = &[
    "NiNode",
    "BSFadeNode",
    "BSLeafAnimNode",
    "BSTreeNode",
    "BSMultiBoundNode",
    "BSOrderedNode",
    "BSValueNode",
    "BSBlastNode",
    "BSDamageStage",
    "BSDebrisNode",
    "BSRangeNode",
    "NiBillboardNode",
    "NiSwitchNode",
    "NiLODNode",
    "NiSortAdjustNode",
    "BSTriShape",
    "BSDynamicTriShape",
    "BSMeshLODTriShape",
    "BSSubIndexTriShape",
    "BSEffectShaderProperty",
    "BSLightingShaderProperty",
    "BSShaderPPLightingProperty",
    "BSShaderNoLightingProperty",
    "BSSkyShaderProperty",
    "BSWaterShaderProperty",
    "BSShaderTextureSet",
    "NiTriShape",
    "NiTriShapeData",
    "NiTriStrips",
    "NiTriStripsData",
    "BSLODTriShape",
    "NiAlphaProperty",
    "NiStencilProperty",
    "NiTexturingProperty",
    "NiVertexColorProperty",
    "NiZBufferProperty",
    "NiMaterialProperty",
    "NiSpecularProperty",
    "NiSourceTexture",
    "NiCamera",
    "NiLight",
    "NiAmbientLight",
    "NiDirectionalLight",
    "NiPointLight",
    "NiSpotLight",
    "NiSkinInstance",
    "BSDismemberSkinInstance",
    "NiSkinData",
    "NiSkinPartition",
    "BSDecalPlacementVectorExtraData",
    "NiStringExtraData",
    "NiIntegerExtraData",
    "NiFloatExtraData",
    "NiBooleanExtraData",
    "NiBinaryExtraData",
    "BSXFlags",
    "BSBound",
    "BSInvMarker",
    "BSBehaviorGraphExtraData",
    "BSBoneLODExtraData",
    "BSConnectPoint",
    "BSFurnitureMarkerNode",
    "NiCollisionObject",
    "bhkCollisionObject",
    "bhkNPCollisionObject",
    "bhkBlendCollisionObject",
    "bhkSPCollisionObject",
    "bhkRigidBody",
    "bhkRigidBodyT",
    "bhkSimpleShapePhantom",
    "bhkCompressedMeshShape",
    "bhkCompressedMeshShapeData",
    "bhkConvexVerticesShape",
    "bhkBoxShape",
    "bhkSphereShape",
    "bhkCapsuleShape",
    "bhkListShape",
    "bhkMoppBvTreeShape",
    "bhkNiTriStripsShape",
    "bhkPackedNiTriStripsShape",
    "hkPackedNiTriStripsData",
    "bhkTransformShape",
    "bhkConvexTransformShape",
    "NiControllerManager",
    "NiControllerSequence",
    "NiMultiTargetTransformController",
    "NiTransformController",
    "NiTransformInterpolator",
    "NiTransformData",
    "NiFloatInterpolator",
    "NiFloatData",
    "NiBlendFloatInterpolator",
    "NiBlendTransformInterpolator",
    "NiTextKeyExtraData",
    "NiDefaultAVObjectPalette",
    "BSAnimNote",
    "BSAnimNotes",
    "NiParticleSystem",
    "NiPSysData",
    "NiMeshParticleSystem",
    "NiMeshPSysData",
    "BSStripParticleSystem",
    "BSStripPSysData",
    "BSFrustumFOVController",
    "BSProceduralLightningController",
    "BSLagBoneController",
    "NiExtraData",
];

/// A validation issue found in a NIF file.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationIssue {
    /// A texture path referenced by a shape could not be verified as existing.
    /// Contains the shape index and the texture path.
    MissingTexture {
        shape_index: u32,
        slot: u32,
        texture_path: String,
    },

    /// A block type name is not recognized as a standard NIF block type.
    InvalidBlockType {
        block_index: u32,
        block_type: String,
    },

    /// The NIF contains no shapes (geometry), which is unusual for a mesh file.
    EmptyMesh,

    /// The NIF has zero blocks, suggesting it is empty or corrupt.
    EmptyFile,

    /// A shape has no textures assigned in any slot.
    UntexturedShape {
        shape_index: u32,
    },

    /// A texture path uses non-standard conventions (e.g. absolute path,
    /// wrong separators, or not under the expected directory).
    NonStandardTexturePath {
        shape_index: u32,
        slot: u32,
        texture_path: String,
        reason: String,
    },
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationIssue::MissingTexture {
                shape_index,
                slot,
                texture_path,
            } => write!(
                f,
                "Shape {} slot {}: missing texture '{}'",
                shape_index, slot, texture_path
            ),
            ValidationIssue::InvalidBlockType {
                block_index,
                block_type,
            } => write!(
                f,
                "Block {}: unrecognized type '{}'",
                block_index, block_type
            ),
            ValidationIssue::EmptyMesh => write!(f, "NIF contains no shapes"),
            ValidationIssue::EmptyFile => write!(f, "NIF has zero blocks (empty or corrupt)"),
            ValidationIssue::UntexturedShape { shape_index } => {
                write!(f, "Shape {}: no textures assigned", shape_index)
            }
            ValidationIssue::NonStandardTexturePath {
                shape_index,
                slot,
                texture_path,
                reason,
            } => write!(
                f,
                "Shape {} slot {}: non-standard path '{}' ({})",
                shape_index, slot, texture_path, reason
            ),
        }
    }
}

/// Check whether a block type name is recognized.
pub fn is_known_block_type(block_type: &str) -> bool {
    KNOWN_BLOCK_TYPES.contains(&block_type)
}

/// Check whether a texture path follows standard Bethesda conventions.
///
/// Returns `None` if the path is standard, or `Some(reason)` describing the issue.
pub fn check_texture_path(texture_path: &str) -> Option<String> {
    if texture_path.is_empty() {
        return None; // Empty paths are handled separately
    }

    // Check for absolute paths (drive letters or leading /)
    if texture_path.len() >= 2 && texture_path.as_bytes()[1] == b':' {
        return Some("absolute path with drive letter".to_string());
    }
    if texture_path.starts_with('/') {
        return Some("absolute path starting with /".to_string());
    }

    // Check file extension
    let lower = texture_path.to_lowercase();
    if !lower.ends_with(".dds")
        && !lower.ends_with(".tga")
        && !lower.ends_with(".bmp")
        && !lower.ends_with(".png")
    {
        return Some(format!(
            "unexpected texture extension (expected .dds, .tga, .bmp, or .png)"
        ));
    }

    None
}

/// Validate a NIF file for common issues.
///
/// This performs a structural check of the NIF, looking for:
/// - Empty files (zero blocks)
/// - Meshes with no shapes
/// - Unrecognized block types
/// - Shapes with no textures
/// - Non-standard texture path conventions
///
/// Does NOT check whether texture files actually exist on disk (that requires
/// knowledge of the Data directory layout).
pub fn validate_nif(nifly: &NiflyLibrary, nif_path: &Path) -> Result<Vec<ValidationIssue>, NifError> {
    let nif = nifly.load_nif(nif_path)?;
    let mut issues = Vec::new();

    let block_count = nif.block_count()?;
    let shape_count = nif.shape_count()?;

    // Check for empty file
    if block_count == 0 {
        issues.push(ValidationIssue::EmptyFile);
        return Ok(issues);
    }

    // Check for empty mesh
    if shape_count == 0 {
        issues.push(ValidationIssue::EmptyMesh);
    }

    // Validate block types
    for i in 0..block_count {
        if let Ok(bt) = nif.block_type(i) {
            if !is_known_block_type(&bt) {
                issues.push(ValidationIssue::InvalidBlockType {
                    block_index: i,
                    block_type: bt,
                });
            }
        }
    }

    // Validate shapes and their textures
    for shape_idx in 0..shape_count {
        let mut has_texture = false;

        for slot in 0..MAX_TEXTURE_SLOTS {
            match nif.texture_slot(shape_idx, slot) {
                Ok(Some(path)) if !path.is_empty() => {
                    has_texture = true;

                    // Check for non-standard path conventions
                    if let Some(reason) = check_texture_path(&path) {
                        issues.push(ValidationIssue::NonStandardTexturePath {
                            shape_index: shape_idx,
                            slot,
                            texture_path: path,
                            reason,
                        });
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        if !has_texture {
            issues.push(ValidationIssue::UntexturedShape {
                shape_index: shape_idx,
            });
        }
    }

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_block_types() {
        assert!(is_known_block_type("NiNode"));
        assert!(is_known_block_type("BSFadeNode"));
        assert!(is_known_block_type("BSTriShape"));
        assert!(is_known_block_type("BSLightingShaderProperty"));
        assert!(!is_known_block_type("FakeBlockType"));
        assert!(!is_known_block_type(""));
    }

    #[test]
    fn test_check_texture_path_valid() {
        assert!(check_texture_path("textures/armor/iron/ironhelmet.dds").is_none());
        assert!(check_texture_path("textures\\armor\\iron\\ironhelmet.dds").is_none());
        assert!(check_texture_path("textures/effects/glow.tga").is_none());
        assert!(check_texture_path("textures/landscape/dirt.bmp").is_none());
        assert!(check_texture_path("textures/test.png").is_none());
    }

    #[test]
    fn test_check_texture_path_absolute() {
        let result = check_texture_path("C:\\Textures\\test.dds");
        assert!(result.is_some());
        assert!(result.unwrap().contains("drive letter"));

        let result = check_texture_path("/usr/share/textures/test.dds");
        assert!(result.is_some());
        assert!(result.unwrap().contains("absolute"));
    }

    #[test]
    fn test_check_texture_path_bad_extension() {
        let result = check_texture_path("textures/test.jpg");
        assert!(result.is_some());
        assert!(result.unwrap().contains("extension"));
    }

    #[test]
    fn test_check_texture_path_empty() {
        assert!(check_texture_path("").is_none());
    }

    #[test]
    fn test_validation_issue_display() {
        let issue = ValidationIssue::EmptyMesh;
        assert_eq!(format!("{}", issue), "NIF contains no shapes");

        let issue = ValidationIssue::EmptyFile;
        assert_eq!(format!("{}", issue), "NIF has zero blocks (empty or corrupt)");

        let issue = ValidationIssue::MissingTexture {
            shape_index: 0,
            slot: 1,
            texture_path: "textures/missing.dds".to_string(),
        };
        assert!(format!("{}", issue).contains("missing.dds"));

        let issue = ValidationIssue::InvalidBlockType {
            block_index: 3,
            block_type: "FakeType".to_string(),
        };
        assert!(format!("{}", issue).contains("FakeType"));

        let issue = ValidationIssue::UntexturedShape { shape_index: 2 };
        assert!(format!("{}", issue).contains("Shape 2"));
    }

    #[test]
    fn test_validation_issue_equality() {
        let a = ValidationIssue::EmptyMesh;
        let b = ValidationIssue::EmptyMesh;
        assert_eq!(a, b);

        let c = ValidationIssue::EmptyFile;
        assert_ne!(a, c);
    }

    #[test]
    fn test_validation_issue_clone() {
        let issue = ValidationIssue::MissingTexture {
            shape_index: 1,
            slot: 0,
            texture_path: "test.dds".to_string(),
        };
        let cloned = issue.clone();
        assert_eq!(issue, cloned);
    }

    #[test]
    fn test_validate_nif_real() {
        let nifly = match NiflyLibrary::load() {
            Ok(lib) => lib,
            Err(_) => { eprintln!("Skipping: nifly library not found"); return; }
        };
        let nif_path = Path::new("test_data/test.nif");
        if !nif_path.exists() { eprintln!("Skipping: {:?} not found", nif_path); return; }
        let issues = validate_nif(&nifly, nif_path)
            .expect("validation");
        // Print issues for manual inspection
        for issue in &issues {
            eprintln!("  {}", issue);
        }
    }
}
