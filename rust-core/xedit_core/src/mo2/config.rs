//! MO2 configuration parser for auto-detecting paths from ModOrganizer.ini

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use xedit_dom::GameId;

/// Parsed MO2 configuration from ModOrganizer.ini.
#[derive(Debug, Clone, Default)]
pub struct Mo2Config {
    /// Path to the MO2 installation folder.
    pub mo2_folder: PathBuf,
    /// Game name as it appears in the INI (e.g., "Skyrim Special Edition").
    pub game_name: String,
    /// Path to the game installation directory.
    pub game_path: PathBuf,
    /// Currently selected profile name.
    pub selected_profile: String,
    /// Available profiles (folder names in profiles/).
    pub available_profiles: Vec<String>,
    /// Derived: mods folder path (mo2_folder/mods).
    pub mods_path: PathBuf,
    /// Derived: profile folder path for the selected profile.
    pub profile_path: PathBuf,
    /// Derived: game Data folder path.
    pub data_path: PathBuf,
}

impl Mo2Config {
    /// Load MO2 configuration from a folder containing ModOrganizer.ini.
    pub fn load(mo2_folder: &Path) -> Result<Self> {
        let ini_path = mo2_folder.join("ModOrganizer.ini");

        if !ini_path.exists() {
            anyhow::bail!("ModOrganizer.ini not found in {:?}", mo2_folder);
        }

        let content = fs::read_to_string(&ini_path)
            .with_context(|| format!("Failed to read {:?}", ini_path))?;

        let mut config = Self {
            mo2_folder: mo2_folder.to_path_buf(),
            ..Default::default()
        };

        // Parse INI file
        let values = parse_ini(&content);

        // Extract game name
        if let Some(name) = values.get("gameName") {
            config.game_name = name.clone();
        }

        // Extract and convert game path
        if let Some(path) = values.get("gamePath") {
            let cleaned = parse_byte_array(path);
            config.game_path = convert_wine_path(&cleaned);
        }

        // Extract selected profile
        if let Some(profile) = values.get("selected_profile") {
            config.selected_profile = parse_byte_array(profile);
        }

        // Scan for available profiles
        let profiles_dir = mo2_folder.join("profiles");
        if profiles_dir.exists() {
            config.available_profiles = list_profiles(&profiles_dir)?;
        }

        // Derive standard paths
        config.mods_path = mo2_folder.join("mods");

        // Profile path (use selected or first available)
        let profile_name = if !config.selected_profile.is_empty() {
            config.selected_profile.clone()
        } else if !config.available_profiles.is_empty() {
            config.available_profiles[0].clone()
        } else {
            String::new()
        };

        if !profile_name.is_empty() {
            config.profile_path = profiles_dir.join(&profile_name);
        }

        // Data path is game_path + "Data" (or just game_path if it ends with Data)
        if config.game_path.ends_with("Data") {
            config.data_path = config.game_path.clone();
        } else {
            config.data_path = config.game_path.join("Data");
        }

        Ok(config)
    }

    /// Check if the configuration appears valid (paths exist on disk).
    pub fn is_valid(&self) -> bool {
        self.mods_path.exists()
            && self.profile_path.exists()
            && (self.data_path.exists() || self.game_path.exists())
    }

    /// Get validation errors describing which paths are missing.
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if !self.mods_path.exists() {
            errors.push(format!("Mods folder not found: {:?}", self.mods_path));
        }
        if !self.profile_path.exists() && !self.selected_profile.is_empty() {
            errors.push(format!(
                "Profile folder not found: {:?}",
                self.profile_path
            ));
        }
        if !self.data_path.exists() && !self.game_path.exists() {
            errors.push(format!(
                "Game Data folder not found: {:?}",
                self.data_path
            ));
        }

        errors
    }

    /// Set the active profile and update profile_path accordingly.
    pub fn set_profile(&mut self, profile_name: &str) {
        self.selected_profile = profile_name.to_string();
        self.profile_path = self.mo2_folder.join("profiles").join(profile_name);
    }

    /// Map the MO2 gameName string to our GameId enum.
    ///
    /// MO2 uses display names like "Skyrim Special Edition", "Fallout 4", etc.
    pub fn game_id(&self) -> Option<GameId> {
        game_name_to_id(&self.game_name)
    }
}

/// Map an MO2 gameName string to a GameId.
pub fn game_name_to_id(game_name: &str) -> Option<GameId> {
    match game_name {
        "Skyrim Special Edition" | "Skyrim VR" => Some(GameId::SkyrimSE),
        "Fallout 4" | "Fallout 4 VR" => Some(GameId::Fallout4),
        "Starfield" => Some(GameId::Starfield),
        "Fallout 76" => Some(GameId::Fallout76),
        "TTW" | "Fallout 3" => Some(GameId::Fallout3),
        "New Vegas" | "Fallout New Vegas" => Some(GameId::FalloutNV),
        "Oblivion" => Some(GameId::Oblivion),
        "Morrowind" => Some(GameId::Morrowind),
        _ => None,
    }
}

/// Parse a simple INI file and extract key-value pairs from the [General] section.
pub fn parse_ini(content: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    let mut in_general = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for section headers
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_general = trimmed == "[General]";
            continue;
        }

        // Only parse [General] section
        if !in_general {
            continue;
        }

        // Parse key=value
        if let Some(pos) = trimmed.find('=') {
            let key = trimmed[..pos].trim().to_string();
            let value = trimmed[pos + 1..].trim().to_string();
            values.insert(key, value);
        }
    }

    values
}

/// Parse @ByteArray(...) wrapper and return the inner string.
///
/// MO2 wraps certain INI values in @ByteArray() for encoding safety.
pub fn parse_byte_array(value: &str) -> String {
    if value.starts_with("@ByteArray(") && value.ends_with(')') {
        let inner = &value[11..value.len() - 1];
        inner.to_string()
    } else {
        value.to_string()
    }
}

/// Convert paths from MO2 INI to native platform paths.
///
/// On Linux: Wine paths like `Z:\home\luke\...` become `/home/luke/...`.
/// On Windows: Paths are already native, just normalize slashes.
pub fn convert_wine_path(path: &str) -> PathBuf {
    let mut result = path.to_string();

    // Replace double backslashes with single
    result = result.replace("\\\\", "\\");

    #[cfg(target_os = "windows")]
    {
        result = result.replace('/', "\\");
        return PathBuf::from(result);
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Linux/macOS, convert Wine-style paths
        result = result.replace('\\', "/");

        // Remove Wine drive letter prefixes (Z:, C:, etc.)
        if result.len() >= 2 && result.chars().nth(1) == Some(':') {
            // Any drive letter maps through - Z: is Linux root, others too
            result = result[2..].to_string();
        }

        // Ensure it starts with /
        if !result.starts_with('/')
            && result.contains('/')
            && (result.starts_with("home/") || result.starts_with("mnt/"))
        {
            result = format!("/{}", result);
        }

        PathBuf::from(result)
    }
}

/// List available profiles in the profiles directory.
pub fn list_profiles(profiles_dir: &Path) -> Result<Vec<String>> {
    let mut profiles = Vec::new();

    if let Ok(entries) = fs::read_dir(profiles_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if !name.starts_with('.') {
                        profiles.push(name.to_string());
                    }
                }
            }
        }
    }

    profiles.sort();
    Ok(profiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ini_extracts_general_section() {
        let ini = "\
[General]
gameName=Skyrim Special Edition
gamePath=@ByteArray(Z:\\\\home\\\\luke\\\\Games\\\\SSE)
selected_profile=@ByteArray(Default)
[Settings]
other_key=other_value
";
        let values = parse_ini(ini);
        assert_eq!(values.get("gameName").unwrap(), "Skyrim Special Edition");
        assert_eq!(
            values.get("gamePath").unwrap(),
            "@ByteArray(Z:\\\\home\\\\luke\\\\Games\\\\SSE)"
        );
        assert_eq!(
            values.get("selected_profile").unwrap(),
            "@ByteArray(Default)"
        );
        // Settings section should not be parsed
        assert!(values.get("other_key").is_none());
    }

    #[test]
    fn test_parse_byte_array_extracts_inner() {
        assert_eq!(
            parse_byte_array("@ByteArray(Masterstroke (Creature Profile))"),
            "Masterstroke (Creature Profile)"
        );
        assert_eq!(parse_byte_array("plain value"), "plain value");
        assert_eq!(parse_byte_array("@ByteArray()"), "");
        assert_eq!(
            parse_byte_array("@ByteArray(Default)"),
            "Default"
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_convert_wine_path_z_drive() {
        assert_eq!(
            convert_wine_path("Z:\\\\home\\\\luke\\\\Games"),
            PathBuf::from("/home/luke/Games")
        );
        assert_eq!(
            convert_wine_path("Z:/home/luke/Games"),
            PathBuf::from("/home/luke/Games")
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_convert_wine_path_native() {
        assert_eq!(
            convert_wine_path("/home/luke/Games"),
            PathBuf::from("/home/luke/Games")
        );
    }

    #[test]
    fn test_game_name_to_id() {
        assert_eq!(
            game_name_to_id("Skyrim Special Edition"),
            Some(GameId::SkyrimSE)
        );
        assert_eq!(game_name_to_id("Fallout 4"), Some(GameId::Fallout4));
        assert_eq!(game_name_to_id("Starfield"), Some(GameId::Starfield));
        assert_eq!(game_name_to_id("Oblivion"), Some(GameId::Oblivion));
        assert_eq!(game_name_to_id("Unknown Game"), None);
    }
}
