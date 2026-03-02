//! The main xEdit engine that manages loaded plugins and operations.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;

use xedit_dom::{FormId, GameId, Plugin};
use xedit_games::DefinitionRegistry;
use xedit_nif::NiflyLibrary;

/// The central xEdit engine instance.
///
/// Manages the loaded plugin set, definitions, and provides
/// all operations the GUI needs.
pub struct XEditEngine {
    /// Which game we're editing for
    pub game_id: GameId,
    /// Game data directory
    pub data_path: PathBuf,
    /// Loaded plugins in load order
    pub plugins: Vec<Plugin>,
    /// Record definition registry
    pub definitions: DefinitionRegistry,
    /// nifly library handle (mandatory)
    pub nifly: NiflyLibrary,
    /// FormID -> plugin index + record index for fast lookup
    pub form_id_index: HashMap<FormId, Vec<(usize, usize)>>,
}

impl XEditEngine {
    /// Initialize the engine for a specific game.
    ///
    /// This loads the nifly library (mandatory) and game definitions.
    /// Fails fast if nifly is not available.
    pub fn new(game_id: GameId, data_path: PathBuf) -> Result<Self> {
        // Load nifly - mandatory, fail fast
        let nifly = NiflyLibrary::load().map_err(|e| {
            anyhow::anyhow!(
                "nifly is required but could not be loaded: {}. \
                 Place nifly_wrapper.dll/.so next to the executable.",
                e
            )
        })?;

        let definitions = DefinitionRegistry::new();
        // TODO: Load game-specific definitions based on game_id

        Ok(Self {
            game_id,
            data_path,
            plugins: Vec::new(),
            definitions,
            nifly,
            form_id_index: HashMap::new(),
        })
    }

    /// Load a plugin file and add it to the load order.
    pub fn load_plugin(&mut self, path: &std::path::Path) -> Result<usize> {
        let reader = xedit_io::PluginReader::new(self.game_id);
        let plugin = reader.read_file(path)?;

        let index = self.plugins.len();
        self.plugins.push(plugin);

        // TODO: Build FormID index for the new plugin

        Ok(index)
    }

    /// Save a plugin to disk.
    pub fn save_plugin(&self, plugin_index: usize, path: &std::path::Path) -> Result<()> {
        let plugin = &self.plugins[plugin_index];
        xedit_io::PluginWriter::write_file(plugin, path)
    }

    /// Get the number of loaded plugins.
    pub fn plugin_count(&self) -> usize {
        self.plugins.len()
    }
}
