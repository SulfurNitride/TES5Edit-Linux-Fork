//! Load order management and master resolution.

use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;
use xedit_dom::{FormId, GameId, Plugin, Record};

use crate::mo2::{Mo2Config, Profile};

/// Ordered plugin set with helper logic for FormID/master resolution.
#[derive(Debug)]
pub struct LoadOrder {
    /// Active game for this load order.
    pub game_id: GameId,
    /// Loaded plugins in current load order.
    pub plugins: Vec<Plugin>,
    plugin_name_to_index: HashMap<String, usize>,
}

impl LoadOrder {
    /// Create an empty load order for a game.
    pub fn new(game_id: GameId) -> Self {
        Self {
            game_id,
            plugins: Vec::new(),
            plugin_name_to_index: HashMap::new(),
        }
    }

    /// Add a loaded plugin and return its index.
    pub fn add_plugin(&mut self, plugin: Plugin) -> usize {
        let index = self.plugins.len();
        if let Some(name) = plugin_name(&plugin) {
            self.plugin_name_to_index
                .entry(name.to_ascii_lowercase())
                .or_insert(index);
        }
        self.plugins.push(plugin);
        index
    }

    /// Load plugins from an MO2 profile's load order.
    ///
    /// Reads the profile's plugin_order, resolves each plugin file through
    /// the VFS (checks mod folders first, then vanilla Data), loads each
    /// plugin with PluginReader, and builds a LoadOrder with all plugins.
    pub fn load_from_mo2(config: &Mo2Config, profile: &Profile) -> Result<Self> {
        let game_id = config
            .game_id()
            .ok_or_else(|| anyhow::anyhow!("Unknown MO2 game name: {}", config.game_name))?;

        let reader = xedit_io::PluginReader::new(game_id);
        let mut load_order = Self::new(game_id);

        for plugin_name in &profile.plugin_order {
            // Search enabled mods first (higher priority wins)
            let mut found_path = None;
            for mod_entry in profile.enabled_mods() {
                let candidate = mod_entry.path.join(plugin_name);
                if candidate.exists() {
                    found_path = Some(candidate);
                    break;
                }
            }

            // Fall back to vanilla Data directory
            if found_path.is_none() {
                let vanilla = profile.game_data_dir.join(plugin_name);
                if vanilla.exists() {
                    found_path = Some(vanilla);
                }
            }

            if let Some(path) = found_path {
                match reader.read_file(&path) {
                    Ok(plugin) => {
                        load_order.add_plugin(plugin);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load plugin {:?}: {:#}", path, e);
                    }
                }
            } else {
                tracing::warn!("Plugin not found in any mod or Data: {}", plugin_name);
            }
        }

        Ok(load_order)
    }

    /// Sort the load order based on game-specific plugin rules.
    pub fn sort_load_order(&mut self) {
        if self.game_id == GameId::Morrowind {
            // TES3 stays in simple linear order.
            return;
        }

        // Stable sort so relative ordering within each class is preserved.
        self.plugins.sort_by_key(|plugin| !Self::is_master(plugin));
        self.rebuild_name_index();
    }

    /// True when the plugin header has the master (ESM) flag.
    pub fn is_master(plugin: &Plugin) -> bool {
        plugin.header.flags.is_esm()
    }

    /// Resolve a plugin-local FormID into canonical (target plugin index, local FormID).
    pub fn resolve_form_id(
        &self,
        plugin_index: usize,
        raw_form_id: FormId,
    ) -> Option<(usize, FormId)> {
        FormIdContext::new(self, plugin_index)?.resolve(raw_form_id)
    }

    /// Find all records in load order that define/override the same global FormID.
    pub fn find_overrides(&self, form_id: FormId) -> Vec<(usize, &Record)> {
        let Some((target_plugin_index, target_local_id)) = self.resolve_global_form_id(form_id)
        else {
            return Vec::new();
        };

        let mut overrides = Vec::new();
        for (plugin_index, plugin) in self.plugins.iter().enumerate() {
            for record in plugin.all_records() {
                if let Some((resolved_plugin_index, resolved_local_id)) =
                    self.resolve_form_id(plugin_index, record.form_id)
                {
                    if resolved_plugin_index == target_plugin_index
                        && resolved_local_id.raw() == target_local_id.raw()
                    {
                        overrides.push((plugin_index, record));
                    }
                }
            }
        }
        overrides
    }

    /// Return the winning override (last in load order).
    pub fn winning_override(&self, form_id: FormId) -> Option<(usize, &Record)> {
        let mut winner = None;
        for (plugin_index, record) in self.find_overrides(form_id) {
            winner = Some((plugin_index, record));
        }
        winner
    }

    fn resolve_global_form_id(&self, form_id: FormId) -> Option<(usize, FormId)> {
        if form_id.is_null() {
            return None;
        }

        let raw = form_id.raw();
        let top = form_id.master_index();
        if self.supports_esl() && top == 0xFE {
            // ESL range: FE LLL XXX, where LLL is the light index and XXX is local id.
            let light_index = ((raw >> 12) & 0x0000_0FFF) as usize;
            let local_id = FormId::new(raw & 0x0000_0FFF);
            let plugin_index = self.light_plugin_indices().get(light_index).copied()?;
            return Some((plugin_index, local_id));
        }

        let plugin_index = top as usize;
        if plugin_index >= self.plugins.len() {
            return None;
        }

        Some((plugin_index, FormId::new(form_id.local_id())))
    }

    fn supports_esl(&self) -> bool {
        matches!(
            self.game_id,
            GameId::SkyrimSE | GameId::Fallout4 | GameId::Starfield
        )
    }

    fn light_plugin_indices(&self) -> Vec<usize> {
        self.plugins
            .iter()
            .enumerate()
            .filter_map(|(index, plugin)| if plugin.is_light() { Some(index) } else { None })
            .collect()
    }

    fn rebuild_name_index(&mut self) {
        self.plugin_name_to_index.clear();
        for (index, plugin) in self.plugins.iter().enumerate() {
            if let Some(name) = plugin_name(plugin) {
                self.plugin_name_to_index
                    .entry(name.to_ascii_lowercase())
                    .or_insert(index);
            }
        }
    }

    fn find_plugin_index_by_master_name(&self, master_name: &str) -> Option<usize> {
        self.plugin_name_to_index
            .get(&master_name.to_ascii_lowercase())
            .copied()
    }
}

/// Per-plugin context that can resolve plugin-local FormIDs through its master list.
pub struct FormIdContext<'a> {
    load_order: &'a LoadOrder,
    plugin_index: usize,
    masters: &'a [String],
}

impl<'a> FormIdContext<'a> {
    pub fn new(load_order: &'a LoadOrder, plugin_index: usize) -> Option<Self> {
        let plugin = load_order.plugins.get(plugin_index)?;
        Some(Self {
            load_order,
            plugin_index,
            masters: &plugin.masters,
        })
    }

    /// Resolve plugin-local FormID to canonical (plugin index, local FormID).
    pub fn resolve(&self, raw_form_id: FormId) -> Option<(usize, FormId)> {
        if raw_form_id.is_null() {
            return None;
        }

        if self.load_order.game_id == GameId::Morrowind {
            // TES3 has simple linear index semantics in this placeholder model.
            let plugin_index = raw_form_id.master_index() as usize;
            if plugin_index >= self.load_order.plugins.len() {
                return None;
            }
            return Some((plugin_index, FormId::new(raw_form_id.local_id())));
        }

        if self.load_order.supports_esl() && raw_form_id.master_index() == 0xFE {
            return self.load_order.resolve_global_form_id(raw_form_id);
        }

        let master_slot = raw_form_id.master_index() as usize;
        let local = FormId::new(raw_form_id.local_id());
        if master_slot < self.masters.len() {
            let master_name = &self.masters[master_slot];
            let plugin_index = self
                .load_order
                .find_plugin_index_by_master_name(master_name)?;
            return Some((plugin_index, local));
        }

        if master_slot == self.masters.len() {
            return Some((self.plugin_index, local));
        }

        None
    }
}

/// Helper for resolving FormIDs across plugin masters and fetching concrete records.
pub struct MasterResolver<'a> {
    load_order: &'a LoadOrder,
}

impl<'a> MasterResolver<'a> {
    pub fn new(load_order: &'a LoadOrder) -> Self {
        Self { load_order }
    }

    pub fn resolve_form_id(
        &self,
        plugin_index: usize,
        raw_form_id: FormId,
    ) -> Option<(usize, FormId)> {
        self.load_order.resolve_form_id(plugin_index, raw_form_id)
    }

    pub fn resolve_record(
        &self,
        plugin_index: usize,
        raw_form_id: FormId,
    ) -> Option<(usize, &'a Record)> {
        let (target_plugin_index, target_local_form_id) =
            self.resolve_form_id(plugin_index, raw_form_id)?;

        let plugin = self.load_order.plugins.get(target_plugin_index)?;
        for record in plugin.all_records() {
            if record.form_id.local_id() == target_local_form_id.raw() {
                return Some((target_plugin_index, record));
            }
        }
        None
    }
}

/// Ordered override chain for a single global FormID.
pub struct OverrideChain<'a> {
    pub entries: Vec<(usize, &'a Record)>,
}

impl<'a> OverrideChain<'a> {
    pub fn new(load_order: &'a LoadOrder, form_id: FormId) -> Self {
        Self {
            entries: load_order.find_overrides(form_id),
        }
    }

    pub fn winning(&self) -> Option<(usize, &'a Record)> {
        self.entries.last().copied()
    }
}

fn plugin_name(plugin: &Plugin) -> Option<&str> {
    plugin
        .file_path
        .as_deref()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use xedit_dom::group::{GroupChild, GroupType};
    use xedit_dom::record::RecordFlags;
    use xedit_dom::{FormId, GameId, Group, Plugin, Record, Signature};

    use crate::load_order::LoadOrder;

    fn make_record(signature: Signature, form_id: u32, flags: u32, edid: Option<&str>) -> Record {
        let mut subrecords = Vec::new();
        if let Some(edid) = edid {
            let mut data = edid.as_bytes().to_vec();
            data.push(0);
            subrecords.push(xedit_dom::Subrecord::new(Signature::EDID, data));
        }

        Record {
            signature,
            flags: RecordFlags(flags),
            form_id: FormId::new(form_id),
            vc_info: 0,
            version: 0,
            unknown: 0,
            subrecords,
            raw_header: None,
            raw_compressed_data: None,
            raw_data: None,
            source_offset: None,
            modified: false,
        }
    }

    fn make_plugin(
        game_id: GameId,
        name: &str,
        header_flags: u32,
        masters: Vec<&str>,
        records: Vec<Record>,
    ) -> Plugin {
        let group = Group {
            group_type: GroupType::Top(Signature::WEAP),
            stamp: 0,
            unknown: 0,
            children: records.into_iter().map(GroupChild::Record).collect(),
            raw_header: None,
            source_offset: None,
        };

        Plugin {
            game_id,
            file_path: Some(PathBuf::from(name)),
            header: make_record(Signature::TES4, 0, header_flags, None),
            groups: vec![group],
            tes3_records: Vec::new(),
            masters: masters.into_iter().map(str::to_string).collect(),
            description: None,
            author: None,
            modified: false,
        }
    }

    fn plugin_name(plugin: &Plugin) -> &str {
        plugin
            .file_path
            .as_deref()
            .and_then(Path::file_name)
            .and_then(|p| p.to_str())
            .expect("plugin name")
    }

    use std::path::Path;

    #[test]
    fn esm_sorts_before_esp() {
        let mut load_order = LoadOrder::new(GameId::SkyrimSE);

        load_order.add_plugin(make_plugin(GameId::SkyrimSE, "Mod.esp", 0, vec![], vec![]));
        load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "Base.esm",
            RecordFlags::ESM,
            vec![],
            vec![],
        ));

        load_order.sort_load_order();

        assert_eq!(plugin_name(&load_order.plugins[0]), "Base.esm");
        assert_eq!(plugin_name(&load_order.plugins[1]), "Mod.esp");
    }

    #[test]
    fn resolves_form_ids_through_master_lists() {
        let mut load_order = LoadOrder::new(GameId::SkyrimSE);

        let master_index = load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "Master.esm",
            RecordFlags::ESM,
            vec![],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("MASTER_WEAP"),
            )],
        ));

        let child_index = load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "Child.esp",
            0,
            vec!["Master.esm"],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("CHILD_WEAP"),
            )],
        ));

        load_order.sort_load_order();

        let resolved = load_order
            .resolve_form_id(child_index, FormId::new(0x0000_1234))
            .expect("resolved form id");
        assert_eq!(resolved.0, master_index);
        assert_eq!(resolved.1.raw(), 0x0000_1234);
    }

    #[test]
    fn override_chain_is_in_load_order() {
        let mut load_order = LoadOrder::new(GameId::SkyrimSE);
        load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "Master.esm",
            RecordFlags::ESM,
            vec![],
            vec![make_record(Signature::WEAP, 0x0000_1234, 0, Some("MASTER"))],
        ));
        load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "Patch.esp",
            0,
            vec!["Master.esm"],
            vec![make_record(Signature::WEAP, 0x0000_1234, 0, Some("PATCH"))],
        ));
        load_order.sort_load_order();

        let overrides = load_order.find_overrides(FormId::new(0x0000_1234));

        assert_eq!(overrides.len(), 2);
        assert_eq!(overrides[0].1.editor_id(), Some("MASTER"));
        assert_eq!(overrides[1].1.editor_id(), Some("PATCH"));
    }

    #[test]
    fn winning_override_is_last_plugin_version() {
        let mut load_order = LoadOrder::new(GameId::SkyrimSE);
        load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "Master.esm",
            RecordFlags::ESM,
            vec![],
            vec![make_record(Signature::WEAP, 0x0000_1234, 0, Some("MASTER"))],
        ));
        load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "PatchA.esp",
            0,
            vec!["Master.esm"],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("PATCH_A"),
            )],
        ));
        load_order.add_plugin(make_plugin(
            GameId::SkyrimSE,
            "PatchB.esp",
            0,
            vec!["Master.esm"],
            vec![make_record(
                Signature::WEAP,
                0x0000_1234,
                0,
                Some("PATCH_B"),
            )],
        ));
        load_order.sort_load_order();

        let winner = load_order
            .winning_override(FormId::new(0x0000_1234))
            .expect("winning override");
        assert_eq!(winner.1.editor_id(), Some("PATCH_B"));
    }
}
