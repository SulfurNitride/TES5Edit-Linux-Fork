//! Virtual File System that simulates MO2's file layering.
//!
//! In MO2:
//! - Game Data files are the base layer (lowest priority)
//! - Mods override in priority order (higher priority = bottom of mod list)
//! - Loose files always beat BSA files at the same priority level

use super::profile::{Mod, Profile};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use walkdir::WalkDir;

/// Source of a file in the virtual file system.
#[derive(Debug, Clone)]
pub enum FileSource {
    /// File from game Data directory (vanilla).
    Vanilla(PathBuf),
    /// File from a mod.
    Mod {
        mod_name: String,
        mod_priority: usize,
        file_path: PathBuf,
    },
}

impl FileSource {
    /// Get the priority of this source (higher = wins conflicts).
    pub fn priority(&self) -> usize {
        match self {
            FileSource::Vanilla(_) => 0,
            FileSource::Mod { mod_priority, .. } => *mod_priority,
        }
    }

    /// Get the physical path on disk.
    pub fn physical_path(&self) -> &Path {
        match self {
            FileSource::Vanilla(p) | FileSource::Mod { file_path: p, .. } => p,
        }
    }
}

/// Virtual File System that simulates MO2's file layering.
///
/// Maps normalized virtual paths (relative to Data/) to their winning
/// file source, accounting for mod priority.
pub struct VirtualFileSystem {
    /// Map of normalized virtual path -> winning file source.
    /// Virtual path is relative to Data/ (e.g., "textures/foo/bar.dds").
    file_map: HashMap<String, FileSource>,
    profile: Profile,
}

impl VirtualFileSystem {
    /// Build the VFS from an MO2 profile.
    pub fn new(profile: Profile) -> Result<Self> {
        let mut vfs = Self {
            file_map: HashMap::new(),
            profile,
        };

        vfs.build_file_map()?;
        Ok(vfs)
    }

    /// Build the complete file map by layering all sources.
    fn build_file_map(&mut self) -> Result<()> {
        info!("Building VFS file map...");

        // Layer 1: Vanilla game files (lowest priority)
        self.index_vanilla_files()?;

        // Layer 2: Enabled mods in priority order (lowest to highest)
        let enabled_mods: Vec<_> = self.profile.enabled_mods().cloned().collect();
        for mod_entry in &enabled_mods {
            self.index_mod_files(mod_entry)?;
        }

        info!(
            "VFS built with {} unique file entries",
            self.file_map.len()
        );
        Ok(())
    }

    /// Index vanilla game Data directory.
    fn index_vanilla_files(&mut self) -> Result<()> {
        let data_dir = &self.profile.game_data_dir;
        if !data_dir.exists() {
            info!("Game Data directory not found: {:?}", data_dir);
            return Ok(());
        }

        let mut count = 0;
        for entry in WalkDir::new(data_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if let Ok(rel_path) = path.strip_prefix(data_dir) {
                let virtual_path = normalize_path(rel_path);
                self.file_map
                    .insert(virtual_path, FileSource::Vanilla(path.to_path_buf()));
                count += 1;
            }
        }

        debug!("Indexed {} vanilla files", count);
        Ok(())
    }

    /// Index files from a single mod.
    fn index_mod_files(&mut self, mod_entry: &Mod) -> Result<()> {
        if !mod_entry.path.exists() {
            debug!("Mod path doesn't exist, skipping: {:?}", mod_entry.path);
            return Ok(());
        }

        let mut count = 0;
        for entry in WalkDir::new(&mod_entry.path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            if let Ok(rel_path) = path.strip_prefix(&mod_entry.path) {
                let virtual_path = normalize_path(rel_path);

                self.file_map
                    .entry(virtual_path)
                    .and_modify(|existing| {
                        if mod_entry.priority > existing.priority() {
                            *existing = FileSource::Mod {
                                mod_name: mod_entry.name.clone(),
                                mod_priority: mod_entry.priority,
                                file_path: path.to_path_buf(),
                            };
                        }
                    })
                    .or_insert_with(|| FileSource::Mod {
                        mod_name: mod_entry.name.clone(),
                        mod_priority: mod_entry.priority,
                        file_path: path.to_path_buf(),
                    });

                count += 1;
            }
        }

        debug!("Indexed {} files from mod: {}", count, mod_entry.name);
        Ok(())
    }

    /// Resolve a virtual path to its file source.
    ///
    /// The path should be relative to Data/ (e.g., "textures/foo/bar.dds").
    pub fn resolve_file(&self, virtual_path: &str) -> Option<&FileSource> {
        let normalized = virtual_path.to_lowercase().replace('\\', "/");
        self.file_map.get(&normalized)
    }

    /// List plugin files (.esp, .esm, .esl) resolved through the VFS.
    ///
    /// For each plugin in the profile's load order, resolves the actual
    /// file path by checking mod folders first, then vanilla Data.
    pub fn list_plugins(&self) -> Vec<PathBuf> {
        let mut result = Vec::new();

        for plugin_name in &self.profile.plugin_order {
            let normalized = plugin_name.to_lowercase();

            // Check if any mod provides this plugin
            if let Some(source) = self.file_map.get(&normalized) {
                result.push(source.physical_path().to_path_buf());
            } else {
                // Fall back to vanilla Data directory
                let vanilla_path = self.profile.game_data_dir.join(plugin_name);
                if vanilla_path.exists() {
                    result.push(vanilla_path);
                }
            }
        }

        result
    }

    /// Get VFS statistics.
    pub fn get_statistics(&self) -> VfsStatistics {
        let mut stats = VfsStatistics::default();

        for source in self.file_map.values() {
            match source {
                FileSource::Vanilla(_) => stats.vanilla_files += 1,
                FileSource::Mod { .. } => stats.mod_files += 1,
            }
        }

        stats.total_files = self.file_map.len();
        stats
    }

    /// Get reference to the profile.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }
}

/// Normalize a path for case-insensitive comparison.
///
/// Lowercases and converts backslashes to forward slashes.
pub fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().to_lowercase().replace('\\', "/")
}

/// VFS statistics summary.
#[derive(Debug, Default)]
pub struct VfsStatistics {
    pub total_files: usize,
    pub vanilla_files: usize,
    pub mod_files: usize,
}

impl std::fmt::Display for VfsStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VFS Stats: {} total ({} vanilla, {} from mods)",
            self.total_files, self.vanilla_files, self.mod_files
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path_lowercases() {
        assert_eq!(
            normalize_path(Path::new("Textures/Foo/Bar.dds")),
            "textures/foo/bar.dds"
        );
    }

    #[test]
    fn test_normalize_path_backslashes() {
        assert_eq!(
            normalize_path(Path::new("textures\\foo\\bar.dds")),
            "textures/foo/bar.dds"
        );
    }

    #[test]
    fn test_normalize_path_mixed() {
        assert_eq!(
            normalize_path(Path::new("Meshes\\Armor/Iron\\Shield.nif")),
            "meshes/armor/iron/shield.nif"
        );
    }

    #[test]
    fn test_file_source_priority() {
        let vanilla = FileSource::Vanilla(PathBuf::from("/data/test.esp"));
        assert_eq!(vanilla.priority(), 0);

        let modded = FileSource::Mod {
            mod_name: "TestMod".to_string(),
            mod_priority: 5,
            file_path: PathBuf::from("/mods/TestMod/test.esp"),
        };
        assert_eq!(modded.priority(), 5);
    }
}
