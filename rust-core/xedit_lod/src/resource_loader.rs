//! Unified resource loader — reads assets from loose files and BSA archives.
//!
//! Search order: MO2 mod loose files -> mod BSAs -> vanilla BSAs -> vanilla loose files.
//! This matches xEdit's wbContainerHandler behavior.

use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use ba2::prelude::*;
use tracing::{debug, info, warn};

/// A source from which a file can be loaded.
#[derive(Debug, Clone)]
enum FileSource {
    /// Loose file on disk.
    Loose(PathBuf),
    /// File inside a BSA archive.
    Bsa {
        archive_path: PathBuf,
        dir_part: String,
        file_part: String,
    },
}

/// Unified resource loader that searches loose files and BSA archives.
pub struct ResourceLoader {
    /// Map of normalized virtual path -> file source (highest priority wins).
    file_map: HashMap<String, FileSource>,
}

impl ResourceLoader {
    /// Build a ResourceLoader from game data dir, MO2 mod directories, and BSA file list.
    ///
    /// # Arguments
    /// * `game_data_dir` - Path to the game's Data folder
    /// * `mod_dirs` - MO2 mod directories in priority order (lowest to highest)
    /// * `bsa_paths` - BSA archive paths in priority order (lowest to highest)
    pub fn new(
        game_data_dir: &Path,
        mod_dirs: &[PathBuf],
        bsa_paths: &[(PathBuf, usize)],
    ) -> Result<Self> {
        let mut loader = Self {
            file_map: HashMap::new(),
        };

        // Layer 1: Index BSA archives (lowest priority — will be overridden by loose files)
        // Index in priority order (lowest first, so higher priority overwrites)
        let mut sorted_bsas: Vec<_> = bsa_paths.to_vec();
        sorted_bsas.sort_by_key(|(_, priority)| *priority);

        for (bsa_path, _priority) in &sorted_bsas {
            if let Err(e) = loader.index_bsa(bsa_path) {
                warn!("Failed to index BSA {:?}: {}", bsa_path, e);
            }
        }

        // Layer 2: Vanilla loose files
        if game_data_dir.exists() {
            loader.index_loose_dir(game_data_dir)?;
        }

        // Layer 3: Mod loose files (highest priority — overwrites everything)
        for mod_dir in mod_dirs {
            if mod_dir.exists() {
                loader.index_loose_dir(mod_dir)?;
            }
        }

        info!("ResourceLoader built with {} indexed entries", loader.file_map.len());
        Ok(loader)
    }

    /// Build a BSA-only ResourceLoader from specific BSA archives.
    ///
    /// This matches xLODGen's behavior for terrain texture loading, which only
    /// reads from the game INI's `SArchiveList` BSAs (no mod loose files, no DLC BSAs).
    /// Use this for terrain LOD texture generation to produce matching output.
    pub fn new_bsa_only(bsa_paths: &[PathBuf]) -> Result<Self> {
        let mut loader = Self {
            file_map: HashMap::new(),
        };

        for bsa_path in bsa_paths {
            if let Err(e) = loader.index_bsa(bsa_path) {
                warn!("Failed to index BSA {:?}: {}", bsa_path, e);
            }
        }

        info!("ResourceLoader (BSA-only) built with {} indexed entries", loader.file_map.len());
        Ok(loader)
    }

    /// Index all files in a BSA archive.
    fn index_bsa(&mut self, bsa_path: &Path) -> Result<()> {
        let (archive, _meta) = ba2::tes4::Archive::read(bsa_path)
            .with_context(|| format!("Failed to read BSA: {:?}", bsa_path))?;

        let mut count = 0;
        for (dir_key, directory) in &archive {
            let dir_name = dir_key.name().to_string();
            for (file_key, _file) in directory {
                let file_name = file_key.name().to_string();
                let virtual_path = if dir_name.is_empty() {
                    file_name.to_lowercase()
                } else {
                    format!("{}/{}", dir_name, file_name).to_lowercase().replace('\\', "/")
                };

                self.file_map.insert(virtual_path, FileSource::Bsa {
                    archive_path: bsa_path.to_path_buf(),
                    dir_part: dir_name.clone(),
                    file_part: file_name,
                });
                count += 1;
            }
        }

        debug!("Indexed {} files from BSA: {:?}", count, bsa_path);
        Ok(())
    }

    /// Index loose files in a directory (overwrites existing entries = higher priority).
    fn index_loose_dir(&mut self, dir: &Path) -> Result<()> {
        let mut count = 0;
        for entry in walkdir::WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(dir) {
                let virtual_path = rel.to_string_lossy().to_lowercase().replace('\\', "/");
                self.file_map.insert(virtual_path, FileSource::Loose(path.to_path_buf()));
                count += 1;
            }
        }
        debug!("Indexed {} loose files from: {:?}", count, dir);
        Ok(())
    }

    /// Load a file by its virtual path (case-insensitive, forward slashes).
    ///
    /// Example: `load("lodsettings/wastelandnv.dlodsettings")`
    pub fn load(&self, virtual_path: &str) -> Result<Vec<u8>> {
        let normalized = virtual_path.to_lowercase().replace('\\', "/");

        match self.file_map.get(&normalized) {
            Some(FileSource::Loose(path)) => {
                fs::read(path).with_context(|| format!("Failed to read loose file: {:?}", path))
            }
            Some(FileSource::Bsa { archive_path, dir_part, file_part }) => {
                self.read_from_bsa(archive_path, dir_part, file_part)
            }
            None => anyhow::bail!("File not found in resources: {}", virtual_path),
        }
    }

    /// Check if a virtual path exists in the resource system.
    pub fn exists(&self, virtual_path: &str) -> bool {
        let normalized = virtual_path.to_lowercase().replace('\\', "/");
        self.file_map.contains_key(&normalized)
    }

    /// Read a file from a BSA archive.
    fn read_from_bsa(&self, archive_path: &Path, dir_part: &str, file_part: &str) -> Result<Vec<u8>> {
        let (archive, meta) = ba2::tes4::Archive::read(archive_path)
            .with_context(|| format!("Failed to open BSA: {:?}", archive_path))?;

        let dir_key = ba2::tes4::ArchiveKey::from(dir_part.as_bytes());
        let file_key = ba2::tes4::DirectoryKey::from(file_part.as_bytes());

        let directory = archive.get(&dir_key)
            .ok_or_else(|| anyhow::anyhow!("Directory not found in BSA: {}", dir_part))?;
        let file = directory.get(&file_key)
            .ok_or_else(|| anyhow::anyhow!("File not found in BSA: {}/{}", dir_part, file_part))?;

        let options: ba2::tes4::FileCompressionOptions = meta.into();
        let mut buf = Vec::new();
        file.write(&mut Cursor::new(&mut buf), &options)?;

        Ok(buf)
    }

    /// Extract multiple files to a staging directory on disk.
    ///
    /// Useful for batch DDS processing — extracts all needed textures at once
    /// so they can be processed without holding everything in RAM.
    pub fn extract_batch(
        &self,
        virtual_paths: &[String],
        staging_dir: &Path,
    ) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(staging_dir)?;

        let mut output_paths = Vec::new();
        for vpath in virtual_paths {
            let normalized = vpath.to_lowercase().replace('\\', "/");
            let out_path = staging_dir.join(&normalized);

            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }

            match self.load(&normalized) {
                Ok(data) => {
                    fs::write(&out_path, &data)?;
                    output_paths.push(out_path);
                }
                Err(e) => {
                    warn!("Failed to extract {}: {}", vpath, e);
                }
            }
        }

        info!("Extracted {} files to staging: {:?}", output_paths.len(), staging_dir);
        Ok(output_paths)
    }

    /// List all indexed paths matching a prefix.
    pub fn list_prefix(&self, prefix: &str) -> Vec<String> {
        let norm = prefix.to_lowercase().replace('\\', "/");
        self.file_map.keys()
            .filter(|k| k.starts_with(&norm))
            .cloned()
            .collect()
    }

    /// Get the total number of indexed entries.
    pub fn entry_count(&self) -> usize {
        self.file_map.len()
    }
}
