//! Resource resolution from BSA/BA2 archives and loose files.
//!
//! For MO2 setups, detects the game's actual Data directory from ModOrganizer.ini
//! and scans both the game dir and enabled mod directories for BSA/BA2 archives.
//!
//! Builds an in-memory index of all BSA file paths on construction for O(1)
//! existence checks and direct BSA lookups on resolve().

use anyhow::Result;
use ba2::prelude::*;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolves game resource paths to file data, checking loose files first
/// then searching BSA/BA2 archives in load order.
pub struct ResourceLoader {
    /// Directories to check for loose files (first match wins)
    loose_dirs: Vec<PathBuf>,
    bsa_paths: Vec<PathBuf>,
    /// Maps lowercase backslash-separated path → index into bsa_paths.
    /// Last-write-wins: later BSAs (higher index) override earlier ones.
    bsa_file_map: HashMap<String, usize>,
}

impl ResourceLoader {
    /// Create a new resource loader.
    /// Builds an in-memory file index from all BSAs using parallel scanning.
    pub fn new(data_path: &Path) -> Result<Self> {
        let mut loose_dirs = vec![data_path.to_path_buf()];
        let mut bsa_paths = Vec::new();

        // Check if this is an MO2 directory
        let mo2_ini = data_path.join("ModOrganizer.ini");
        if mo2_ini.exists() {
            tracing::info!("ResourceLoader: detected MO2 directory, scanning for game path and mods");

            if let Ok(ini_content) = std::fs::read_to_string(&mo2_ini) {
                if let Some(game_data) = parse_mo2_game_data_path(&ini_content) {
                    tracing::info!("ResourceLoader: game Data path = {:?}", game_data);
                    scan_dir_for_bsas(&game_data, &mut bsa_paths);
                    loose_dirs.push(game_data);
                }
            }

            let mods_dir = data_path.join("mods");
            if mods_dir.is_dir() {
                let enabled_mods = read_mo2_enabled_mods(data_path);
                if !enabled_mods.is_empty() {
                    for mod_name in &enabled_mods {
                        let mod_path = mods_dir.join(mod_name);
                        if mod_path.is_dir() {
                            scan_dir_for_bsas(&mod_path, &mut bsa_paths);
                            loose_dirs.push(mod_path);
                        }
                    }
                } else {
                    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
                        let mut mod_dirs: Vec<_> = entries
                            .flatten()
                            .filter(|e| e.path().is_dir())
                            .collect();
                        mod_dirs.sort_by_key(|e| e.file_name());
                        for entry in mod_dirs {
                            scan_dir_for_bsas(&entry.path(), &mut bsa_paths);
                            loose_dirs.push(entry.path());
                        }
                    }
                }
            }
        } else {
            scan_dir_for_bsas(data_path, &mut bsa_paths);
        }

        bsa_paths.sort();

        // Build file index from all BSAs in parallel.
        // Each element: Vec<(lowercase_path, bsa_index)>
        let index_results: Vec<Vec<(String, usize)>> = bsa_paths
            .par_iter()
            .enumerate()
            .map(|(bsa_idx, bsa_path)| {
                match ba2::tes4::Archive::read(bsa_path.as_path()) {
                    Ok((archive, _)) => {
                        let mut entries = Vec::new();
                        for (dir_key, folder) in &archive {
                            let dir_name = dir_key.name().to_string().to_lowercase();
                            for (file_key, _) in folder {
                                let file_name = file_key.name().to_string().to_lowercase();
                                let full = if dir_name.is_empty() || dir_name == "." {
                                    file_name
                                } else {
                                    format!("{}\\{}", dir_name, file_name)
                                };
                                entries.push((full, bsa_idx));
                            }
                        }
                        entries
                    }
                    Err(e) => {
                        tracing::debug!("Failed to read BSA {:?}: {}", bsa_path, e);
                        Vec::new()
                    }
                }
            })
            .collect();

        let mut bsa_file_map = HashMap::new();
        for entries in index_results {
            for (path, idx) in entries {
                bsa_file_map.insert(path, idx);
            }
        }

        tracing::info!(
            "ResourceLoader: {} loose dirs, {} BSA/BA2 archives, {} indexed files",
            loose_dirs.len(),
            bsa_paths.len(),
            bsa_file_map.len()
        );

        Ok(Self { loose_dirs, bsa_paths, bsa_file_map })
    }

    /// Resolve a game path to bytes.
    /// Checks loose files first, then uses the BSA index for direct lookup.
    pub fn resolve(&self, game_path: &str) -> Result<Vec<u8>> {
        // 1. Check loose files
        let relative = game_path.replace('\\', "/");
        for dir in &self.loose_dirs {
            let loose = dir.join(&relative);
            if loose.exists() {
                return Ok(std::fs::read(&loose)?);
            }
        }

        // 2. Direct BSA lookup via index
        let normalized = game_path.replace('/', "\\").to_lowercase();
        let (dir_name, file_name) = if let Some(idx) = normalized.rfind('\\') {
            (&normalized[..idx], &normalized[idx + 1..])
        } else {
            ("", normalized.as_str())
        };

        if let Some(&bsa_idx) = self.bsa_file_map.get(&normalized) {
            let bsa_path = &self.bsa_paths[bsa_idx];
            if let Ok((archive, meta)) = ba2::tes4::Archive::read(bsa_path.as_path()) {
                let compression_options: ba2::tes4::FileCompressionOptions = meta.into();
                for (dir_key, folder) in &archive {
                    let current_dir = dir_key.name().to_string().to_lowercase();
                    if current_dir == dir_name {
                        for (file_key, file) in folder {
                            let current_file = file_key.name().to_string().to_lowercase();
                            if current_file == file_name {
                                let data = if file.is_decompressed() {
                                    file.as_bytes().to_vec()
                                } else {
                                    file.decompress(&compression_options)?.as_bytes().to_vec()
                                };
                                return Ok(data);
                            }
                        }
                    }
                }
            }
        }

        anyhow::bail!("Resource not found: {}", game_path)
    }

    /// Get the list of discovered BSA/BA2 archive paths.
    pub fn bsa_paths(&self) -> &[PathBuf] {
        &self.bsa_paths
    }

    /// Check if a resource exists (O(1) via index).
    pub fn exists(&self, game_path: &str) -> bool {
        // Check loose files
        let relative = game_path.replace('\\', "/");
        for dir in &self.loose_dirs {
            if dir.join(&relative).exists() {
                return true;
            }
        }
        // Check BSA index
        let normalized = game_path.replace('/', "\\").to_lowercase();
        self.bsa_file_map.contains_key(&normalized)
    }

    /// Get all indexed file paths matching a directory prefix.
    pub fn list_files_in_dir(&self, dir_prefix: &str) -> Vec<String> {
        let normalized = dir_prefix.replace('/', "\\").to_lowercase();
        let prefix = if normalized.ends_with('\\') {
            normalized
        } else {
            format!("{}\\", normalized)
        };
        self.bsa_file_map
            .keys()
            .filter(|p| p.starts_with(&prefix))
            .cloned()
            .collect()
    }
}

/// Scan a directory for BSA/BA2 files.
fn scan_dir_for_bsas(dir: &Path, bsa_paths: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if ext_lower == "bsa" || ext_lower == "ba2" {
                    bsa_paths.push(path);
                }
            }
        }
    }
}

/// Parse the game Data path from ModOrganizer.ini content.
fn parse_mo2_game_data_path(ini_content: &str) -> Option<PathBuf> {
    for line in ini_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("gamePath=") {
            let value = trimmed.strip_prefix("gamePath=")?;
            let path_str = if value.starts_with("@ByteArray(") && value.ends_with(')') {
                &value[11..value.len() - 1]
            } else {
                value
            };
            let linux_path = if path_str.len() >= 3 && &path_str[1..3] == ":\\" {
                let without_drive = &path_str[2..];
                without_drive.replace('\\', "/")
            } else {
                path_str.replace('\\', "/")
            };
            let linux_path = linux_path.replace("//", "/");
            let game_dir = PathBuf::from(&linux_path);
            let data_dir = game_dir.join("Data");
            if data_dir.is_dir() {
                return Some(data_dir);
            }
            if game_dir.is_dir() {
                return Some(game_dir);
            }
            tracing::warn!("ResourceLoader: gamePath not found: {:?}", game_dir);
            return None;
        }
    }
    None
}

/// Read enabled mods from MO2 profile's modlist.txt.
fn read_mo2_enabled_mods(mo2_dir: &Path) -> Vec<String> {
    let profiles_dir = mo2_dir.join("profiles");
    if !profiles_dir.is_dir() {
        return Vec::new();
    }

    let ini_path = mo2_dir.join("ModOrganizer.ini");
    let selected_profile = if let Ok(content) = std::fs::read_to_string(&ini_path) {
        content.lines()
            .find(|l| l.trim().starts_with("selected_profile="))
            .and_then(|l| l.trim().strip_prefix("selected_profile="))
            .map(|s| {
                if s.starts_with("@ByteArray(") && s.ends_with(')') {
                    s[11..s.len() - 1].to_string()
                } else {
                    s.to_string()
                }
            })
    } else {
        None
    };

    let modlist_path = if let Some(ref profile) = selected_profile {
        profiles_dir.join(profile).join("modlist.txt")
    } else {
        if let Ok(mut entries) = std::fs::read_dir(&profiles_dir) {
            if let Some(first) = entries.find_map(|e| e.ok()) {
                first.path().join("modlist.txt")
            } else {
                return Vec::new();
            }
        } else {
            return Vec::new();
        }
    };

    if let Ok(content) = std::fs::read_to_string(&modlist_path) {
        content.lines()
            .filter(|l| l.starts_with('+'))
            .map(|l| l[1..].to_string())
            .collect()
    } else {
        Vec::new()
    }
}

/// List all files in a BSA archive (returns backslash-separated paths).
pub fn list_bsa_files(bsa_path: &Path) -> Result<Vec<String>> {
    let (archive, _) = ba2::tes4::Archive::read(bsa_path)?;
    let mut files = Vec::new();
    for (dir_key, folder) in &archive {
        let dir_name = dir_key.name().to_string();
        for (file_key, _) in folder {
            let file_name = file_key.name().to_string();
            let full_path = if dir_name.is_empty() || dir_name == "." {
                file_name
            } else {
                format!("{}\\{}", dir_name, file_name)
            };
            files.push(full_path);
        }
    }
    Ok(files)
}
