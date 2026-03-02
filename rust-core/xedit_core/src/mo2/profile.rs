//! MO2 profile parser - handles modlist.txt, loadorder.txt, and archives.txt

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Represents the state of a mod in MO2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModState {
    Enabled,
    Disabled,
}

/// Represents a mod with its metadata.
#[derive(Debug, Clone)]
pub struct Mod {
    pub name: String,
    pub path: PathBuf,
    /// Higher number = higher priority (wins conflicts).
    pub priority: usize,
    pub state: ModState,
}

/// MO2 profile parser - handles modlist.txt, loadorder.txt, and archives.txt.
#[derive(Debug)]
pub struct Profile {
    pub profile_path: PathBuf,
    pub game_data_dir: PathBuf,
    pub mods: Vec<Mod>,
    pub plugin_order: Vec<String>,
    pub additional_archives: Vec<String>,
}

impl Profile {
    /// Load an MO2 profile from the given path.
    ///
    /// # Arguments
    /// * `profile_path` - Path to MO2 profile directory (contains modlist.txt)
    /// * `mods_dir` - Path to MO2 mods directory
    /// * `game_data_dir` - Path to game Data directory
    pub fn load(
        profile_path: impl AsRef<Path>,
        mods_dir: impl AsRef<Path>,
        game_data_dir: impl AsRef<Path>,
    ) -> Result<Self> {
        let profile_path = profile_path.as_ref().to_path_buf();
        let mods_dir = mods_dir.as_ref().to_path_buf();
        let game_data_dir = game_data_dir.as_ref().to_path_buf();

        info!("Loading MO2 profile from: {:?}", profile_path);

        let modlist_path = profile_path.join("modlist.txt");
        let loadorder_path = profile_path.join("loadorder.txt");
        let archives_path = profile_path.join("archives.txt");

        // Parse modlist.txt - this defines mod priority
        let mods = Self::parse_modlist(&modlist_path, &mods_dir)?;
        info!(
            "Loaded {} mods ({} enabled)",
            mods.len(),
            mods.iter().filter(|m| m.state == ModState::Enabled).count()
        );

        // Parse loadorder.txt - plugin load order
        let plugin_order = if loadorder_path.exists() {
            Self::parse_loadorder(&loadorder_path)?
        } else {
            warn!("loadorder.txt not found, skipping plugin parsing");
            Vec::new()
        };

        // Parse archives.txt - additional BSAs to load
        let additional_archives = if archives_path.exists() {
            Self::parse_archives(&archives_path)?
        } else {
            warn!("archives.txt not found, skipping additional archives");
            Vec::new()
        };

        Ok(Self {
            profile_path,
            game_data_dir,
            mods,
            plugin_order,
            additional_archives,
        })
    }

    /// Parse modlist.txt.
    ///
    /// Format:
    /// - `+ModName` - enabled
    /// - `-ModName` - disabled
    /// - `*ModName` - separator, treated as disabled
    ///
    /// Mods at the TOP of the file have LOWEST priority.
    /// Mods at the BOTTOM have HIGHEST priority (win conflicts).
    pub fn parse_modlist(path: &Path, mods_dir: &Path) -> Result<Vec<Mod>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read modlist.txt from {:?}", path))?;

        Ok(parse_modlist_content(&content, mods_dir))
    }

    /// Parse loadorder.txt - list of plugin files (.esp, .esm, .esl).
    pub fn parse_loadorder(path: &Path) -> Result<Vec<String>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read loadorder.txt from {:?}", path))?;

        Ok(parse_loadorder_content(&content))
    }

    /// Parse archives.txt - list of additional BSA files to load.
    pub fn parse_archives(path: &Path) -> Result<Vec<String>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read archives.txt from {:?}", path))?;

        let archives: Vec<String> = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(String::from)
            .collect();

        debug!("Parsed {} additional archives from archives.txt", archives.len());
        Ok(archives)
    }

    /// Get all enabled mods in priority order (lowest to highest).
    pub fn enabled_mods(&self) -> impl Iterator<Item = &Mod> {
        self.mods.iter().filter(|m| m.state == ModState::Enabled)
    }

    /// Get BSA files from plugin load order, searching in mod folders.
    ///
    /// Returns BSAs with their owning mod priority for proper VFS layering.
    /// Each tuple is (bsa_path, source_name, priority).
    pub fn get_plugin_bsas(&self) -> Vec<(PathBuf, String, usize)> {
        let mut bsas = Vec::new();

        // First, check vanilla game Data directory
        for plugin in &self.plugin_order {
            let base_name = strip_plugin_extension(plugin);

            let vanilla_bsa = self.game_data_dir.join(format!("{}.bsa", base_name));
            if vanilla_bsa.exists() {
                debug!("Found vanilla BSA: {:?}", vanilla_bsa);
                bsas.push((vanilla_bsa, "Vanilla".to_string(), 0));
            }

            let vanilla_tex_bsa = self
                .game_data_dir
                .join(format!("{} - Textures.bsa", base_name));
            if vanilla_tex_bsa.exists() {
                debug!("Found vanilla BSA: {:?}", vanilla_tex_bsa);
                bsas.push((vanilla_tex_bsa, "Vanilla".to_string(), 0));
            }
        }

        // Search for BSAs in mod folders based on plugin ownership
        for plugin in &self.plugin_order {
            let base_name = strip_plugin_extension(plugin);

            for mod_entry in self.enabled_mods() {
                let plugin_path = mod_entry.path.join(plugin);
                if plugin_path.exists() {
                    let bsa = mod_entry.path.join(format!("{}.bsa", base_name));
                    if bsa.exists() {
                        debug!("Found BSA in mod {}: {:?}", mod_entry.name, bsa);
                        bsas.push((bsa, mod_entry.name.clone(), mod_entry.priority));
                    }

                    let tex_bsa = mod_entry
                        .path
                        .join(format!("{} - Textures.bsa", base_name));
                    if tex_bsa.exists() {
                        debug!("Found BSA in mod {}: {:?}", mod_entry.name, tex_bsa);
                        bsas.push((tex_bsa, mod_entry.name.clone(), mod_entry.priority));
                    }

                    break; // Found the mod providing this plugin
                }
            }
        }

        // Check archives.txt entries in mod folders
        for archive_name in &self.additional_archives {
            for mod_entry in self.enabled_mods() {
                let archive_path = mod_entry.path.join(archive_name);
                if archive_path.exists() {
                    debug!(
                        "Found archive from archives.txt in mod {}: {:?}",
                        mod_entry.name, archive_path
                    );
                    bsas.push((archive_path, mod_entry.name.clone(), mod_entry.priority));
                    break;
                }
            }
        }

        info!("Found {} BSA files from load order and mods", bsas.len());
        bsas
    }
}

/// Strip the plugin extension (.esp, .esm, .esl) from a filename.
fn strip_plugin_extension(plugin: &str) -> &str {
    plugin
        .strip_suffix(".esp")
        .or_else(|| plugin.strip_suffix(".esm"))
        .or_else(|| plugin.strip_suffix(".esl"))
        .unwrap_or(plugin)
}

/// Parse modlist.txt content into mod entries.
///
/// Exposed for testing without filesystem access.
pub fn parse_modlist_content(content: &str, mods_dir: &Path) -> Vec<Mod> {
    let mut mods = Vec::new();
    let mut priority = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (state, name) = match line.chars().next() {
            Some('+') => (ModState::Enabled, &line[1..]),
            Some('-') | Some('*') => (ModState::Disabled, &line[1..]),
            _ => {
                // Invalid line format, skip
                continue;
            }
        };

        let mod_path = mods_dir.join(name);

        mods.push(Mod {
            name: name.to_string(),
            path: mod_path,
            priority,
            state,
        });

        priority += 1;
    }

    debug!("Parsed {} mods from modlist content", mods.len());
    mods
}

/// Parse loadorder.txt content into plugin names.
///
/// Exposed for testing without filesystem access.
pub fn parse_loadorder_content(content: &str) -> Vec<String> {
    let plugins: Vec<String> = content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            // Handle both "Plugin.esp" and "*Plugin.esp" format
            if let Some(stripped) = line.strip_prefix('*') {
                stripped.to_string()
            } else {
                line.to_string()
            }
        })
        .collect();

    debug!("Parsed {} plugins from loadorder content", plugins.len());
    plugins
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_modlist_enabled_disabled() {
        let content = "+EnabledMod\n-DisabledMod\n*Separator\n+AnotherEnabled\n";
        let mods = parse_modlist_content(content, Path::new("/mods"));

        assert_eq!(mods.len(), 4);
        assert_eq!(mods[0].name, "EnabledMod");
        assert_eq!(mods[0].state, ModState::Enabled);
        assert_eq!(mods[0].priority, 0);

        assert_eq!(mods[1].name, "DisabledMod");
        assert_eq!(mods[1].state, ModState::Disabled);
        assert_eq!(mods[1].priority, 1);

        assert_eq!(mods[2].name, "Separator");
        assert_eq!(mods[2].state, ModState::Disabled);
        assert_eq!(mods[2].priority, 2);

        assert_eq!(mods[3].name, "AnotherEnabled");
        assert_eq!(mods[3].state, ModState::Enabled);
        assert_eq!(mods[3].priority, 3);
    }

    #[test]
    fn test_parse_modlist_paths() {
        let content = "+MyMod\n";
        let mods = parse_modlist_content(content, Path::new("/home/user/MO2/mods"));

        assert_eq!(mods[0].path, PathBuf::from("/home/user/MO2/mods/MyMod"));
    }

    #[test]
    fn test_parse_modlist_empty_lines() {
        let content = "\n+Mod1\n\n+Mod2\n\n";
        let mods = parse_modlist_content(content, Path::new("/mods"));
        assert_eq!(mods.len(), 2);
    }

    #[test]
    fn test_parse_loadorder_with_comments_and_stars() {
        let content = "# This file was auto-generated\n*Skyrim.esm\nUpdate.esm\n*Dawnguard.esm\nMyMod.esp\n";
        let plugins = parse_loadorder_content(content);

        assert_eq!(plugins.len(), 4);
        assert_eq!(plugins[0], "Skyrim.esm");
        assert_eq!(plugins[1], "Update.esm");
        assert_eq!(plugins[2], "Dawnguard.esm");
        assert_eq!(plugins[3], "MyMod.esp");
    }

    #[test]
    fn test_parse_loadorder_empty() {
        let content = "# comment only\n\n";
        let plugins = parse_loadorder_content(content);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_strip_plugin_extension() {
        assert_eq!(strip_plugin_extension("Skyrim.esm"), "Skyrim");
        assert_eq!(strip_plugin_extension("MyMod.esp"), "MyMod");
        assert_eq!(strip_plugin_extension("Light.esl"), "Light");
        assert_eq!(strip_plugin_extension("NoExtension"), "NoExtension");
    }
}
