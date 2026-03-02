//! NIF metadata extraction — high-level struct summarizing a NIF file's contents.

use std::path::{Path, PathBuf};

use crate::error::NifError;
use crate::loader::NiflyLibrary;

/// Maximum number of texture slots to probe per shape.
/// Bethesda games typically use up to 10 slots (diffuse, normal, glow, parallax, etc.).
const MAX_TEXTURE_SLOTS: u32 = 10;

/// High-level metadata extracted from a NIF mesh file.
#[derive(Debug, Clone)]
pub struct NifMetadata {
    /// Path to the NIF file that was analyzed.
    pub file_path: PathBuf,
    /// Total number of blocks in the NIF header.
    pub block_count: u32,
    /// Number of shapes (geometry nodes) in the NIF.
    pub shape_count: u32,
    /// All non-empty texture paths referenced by shapes in the NIF.
    pub texture_paths: Vec<String>,
    /// Block type names for every block in the NIF (e.g. "NiNode", "BSTriShape").
    pub block_types: Vec<String>,
}

impl NifMetadata {
    /// Create a `NifMetadata` with the given values (useful for testing).
    pub fn new(
        file_path: PathBuf,
        block_count: u32,
        shape_count: u32,
        texture_paths: Vec<String>,
        block_types: Vec<String>,
    ) -> Self {
        Self {
            file_path,
            block_count,
            shape_count,
            texture_paths,
            block_types,
        }
    }
}

/// Extract full metadata from a NIF file.
///
/// Loads the NIF, enumerates all blocks and shapes, and collects every
/// non-empty texture slot path.
pub fn extract_metadata(nifly: &NiflyLibrary, nif_path: &Path) -> Result<NifMetadata, NifError> {
    let nif = nifly.load_nif(nif_path)?;

    let block_count = nif.block_count()?;
    let shape_count = nif.shape_count()?;

    // Collect block type names
    let mut block_types = Vec::with_capacity(block_count as usize);
    for i in 0..block_count {
        block_types.push(nif.block_type(i)?);
    }

    // Collect all texture paths from all shapes and slots
    let mut texture_paths = Vec::new();
    for shape_idx in 0..shape_count {
        for slot in 0..MAX_TEXTURE_SLOTS {
            match nif.texture_slot(shape_idx, slot) {
                Ok(Some(path)) if !path.is_empty() => {
                    if !texture_paths.contains(&path) {
                        texture_paths.push(path);
                    }
                }
                Ok(_) => {}
                // Slot out of range is not an error — just stop probing this shape
                Err(_) => break,
            }
        }
    }

    Ok(NifMetadata {
        file_path: nif_path.to_path_buf(),
        block_count,
        shape_count,
        texture_paths,
        block_types,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nif_metadata_construction() {
        let meta = NifMetadata::new(
            PathBuf::from("meshes/armor/iron/ironhelmet.nif"),
            12,
            3,
            vec![
                "textures/armor/iron/ironhelmet.dds".to_string(),
                "textures/armor/iron/ironhelmet_n.dds".to_string(),
            ],
            vec![
                "BSFadeNode".to_string(),
                "BSTriShape".to_string(),
                "NiNode".to_string(),
            ],
        );

        assert_eq!(meta.block_count, 12);
        assert_eq!(meta.shape_count, 3);
        assert_eq!(meta.texture_paths.len(), 2);
        assert_eq!(meta.block_types.len(), 3);
        assert_eq!(
            meta.file_path,
            PathBuf::from("meshes/armor/iron/ironhelmet.nif")
        );
    }

    #[test]
    fn test_nif_metadata_empty() {
        let meta = NifMetadata::new(PathBuf::from("empty.nif"), 0, 0, vec![], vec![]);

        assert_eq!(meta.block_count, 0);
        assert_eq!(meta.shape_count, 0);
        assert!(meta.texture_paths.is_empty());
        assert!(meta.block_types.is_empty());
    }

    #[test]
    fn test_nif_metadata_clone() {
        let meta = NifMetadata::new(
            PathBuf::from("test.nif"),
            5,
            2,
            vec!["tex.dds".to_string()],
            vec!["NiNode".to_string()],
        );
        let cloned = meta.clone();
        assert_eq!(cloned.block_count, meta.block_count);
        assert_eq!(cloned.texture_paths, meta.texture_paths);
    }

    #[test]
    fn test_extract_metadata_real_nif() {
        let nifly = match NiflyLibrary::load() {
            Ok(lib) => lib,
            Err(_) => { eprintln!("Skipping: nifly library not found"); return; }
        };
        let nif_path = Path::new("test_data/test.nif");
        if !nif_path.exists() { eprintln!("Skipping: {:?} not found", nif_path); return; }
        let meta = extract_metadata(&nifly, nif_path)
            .expect("metadata extraction");
        assert!(meta.block_count > 0);
    }
}
