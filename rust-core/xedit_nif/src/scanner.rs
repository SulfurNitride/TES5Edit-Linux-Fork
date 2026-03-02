//! Asset path scanning — extract texture references from NIF files.

use std::path::{Path, PathBuf};

use crate::error::NifError;
use crate::loader::NiflyLibrary;

/// Maximum number of texture slots to probe per shape.
const MAX_TEXTURE_SLOTS: u32 = 10;

/// Extract all unique texture paths referenced by a single NIF file.
///
/// Iterates over every shape and every texture slot, collecting non-empty
/// texture paths. Duplicates within the same NIF are removed.
pub fn scan_nif_textures(nifly: &NiflyLibrary, nif_path: &Path) -> Result<Vec<String>, NifError> {
    let nif = nifly.load_nif(nif_path)?;
    let shape_count = nif.shape_count()?;

    let mut textures = Vec::new();
    for shape_idx in 0..shape_count {
        for slot in 0..MAX_TEXTURE_SLOTS {
            match nif.texture_slot(shape_idx, slot) {
                Ok(Some(path)) if !path.is_empty() => {
                    if !textures.contains(&path) {
                        textures.push(path);
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    }

    Ok(textures)
}

/// Batch-scan all `.nif` files in a directory (non-recursive).
///
/// Returns a list of `(nif_path, texture_paths)` pairs. Files that fail
/// to load are silently skipped (logged at warn level).
pub fn scan_directory_nifs(
    nifly: &NiflyLibrary,
    dir: &Path,
) -> Result<Vec<(PathBuf, Vec<String>)>, NifError> {
    if !dir.is_dir() {
        return Err(NifError::InvalidFile(format!(
            "Not a directory: {}",
            dir.display()
        )));
    }

    let mut results = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| {
        NifError::InvalidFile(format!("Cannot read directory {}: {}", dir.display(), e))
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();

        // Only process .nif files
        let is_nif = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("nif"))
            .unwrap_or(false);
        if !is_nif {
            continue;
        }

        match scan_nif_textures(nifly, &path) {
            Ok(textures) => {
                results.push((path, textures));
            }
            Err(e) => {
                tracing::warn!("Skipping {}: {}", path.display(), e);
            }
        }
    }

    Ok(results)
}

/// Normalize a texture path to a consistent format.
///
/// Converts backslashes to forward slashes and lowercases the path,
/// which is the standard convention for Bethesda asset paths.
pub fn normalize_texture_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_texture_path_backslashes() {
        assert_eq!(
            normalize_texture_path("Textures\\Armor\\Iron\\IronHelmet.dds"),
            "textures/armor/iron/ironhelmet.dds"
        );
    }

    #[test]
    fn test_normalize_texture_path_already_normalized() {
        assert_eq!(
            normalize_texture_path("textures/armor/iron/ironhelmet.dds"),
            "textures/armor/iron/ironhelmet.dds"
        );
    }

    #[test]
    fn test_normalize_texture_path_mixed() {
        assert_eq!(
            normalize_texture_path("Textures/Armor\\Iron/IronHelmet_N.dds"),
            "textures/armor/iron/ironhelmet_n.dds"
        );
    }

    #[test]
    fn test_normalize_texture_path_empty() {
        assert_eq!(normalize_texture_path(""), "");
    }

    #[test]
    fn test_scan_nif_textures_real() {
        let nifly = match NiflyLibrary::load() {
            Ok(lib) => lib,
            Err(_) => { eprintln!("Skipping: nifly library not found"); return; }
        };
        let nif_path = Path::new("test_data/test.nif");
        if !nif_path.exists() { eprintln!("Skipping: {:?} not found", nif_path); return; }
        let textures = scan_nif_textures(&nifly, nif_path)
            .expect("texture scan");
        // A valid NIF should have at least one texture
        assert!(!textures.is_empty());
    }

    #[test]
    fn test_scan_directory_nifs_real() {
        let nifly = match NiflyLibrary::load() {
            Ok(lib) => lib,
            Err(_) => { eprintln!("Skipping: nifly library not found"); return; }
        };
        let test_dir = Path::new("test_data");
        if !test_dir.exists() { eprintln!("Skipping: {:?} not found", test_dir); return; }
        let results = scan_directory_nifs(&nifly, test_dir)
            .expect("directory scan");
        assert!(!results.is_empty());
    }
}
