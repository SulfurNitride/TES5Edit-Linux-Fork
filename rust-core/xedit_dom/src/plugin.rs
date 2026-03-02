use crate::{GameId, Group, Record, Signature};

/// A loaded Bethesda plugin file (ESP/ESM/ESL).
///
/// This is the top-level container for a plugin's data.
/// For TES4+ games, it contains a file header record and top-level GRUPs.
/// For TES3, it contains a flat list of records.
#[derive(Debug, Clone)]
pub struct Plugin {
    /// Which game this plugin belongs to
    pub game_id: GameId,
    /// Original file path
    pub file_path: Option<std::path::PathBuf>,
    /// The file header record (TES4 or TES3 signature)
    pub header: Record,
    /// Top-level groups (TES4+ only)
    pub groups: Vec<Group>,
    /// Flat record list (TES3 only - Morrowind doesn't use GRUPs)
    pub tes3_records: Vec<Record>,
    /// Master file list extracted from header
    pub masters: Vec<String>,
    /// Plugin description from header
    pub description: Option<String>,
    /// Author from header
    pub author: Option<String>,
    /// Whether any record in this plugin has been modified
    pub modified: bool,
}

impl Plugin {
    /// Get the plugin's file header signature.
    pub fn header_signature(&self) -> Signature {
        self.header.signature
    }

    /// Check if this is a master file (ESM flag set).
    pub fn is_master(&self) -> bool {
        self.header.flags.is_esm()
    }

    /// Check if this is a light master (ESL flag set).
    pub fn is_light(&self) -> bool {
        self.header.flags.is_esl()
    }

    /// Check if this plugin uses localized strings.
    pub fn is_localized(&self) -> bool {
        self.header.flags.is_localized()
    }

    /// Iterate all records in the plugin (flattening groups for TES4+).
    pub fn all_records(&self) -> Vec<&Record> {
        match self.game_id.dialect_family() {
            crate::dialect::DialectFamily::TES3 => self.tes3_records.iter().collect(),
            crate::dialect::DialectFamily::TES4Plus => {
                let mut records = Vec::new();
                for group in &self.groups {
                    collect_records_from_group(group, &mut records);
                }
                records
            }
        }
    }

    /// Get the number of top-level groups (TES4+) or records (TES3).
    pub fn top_level_count(&self) -> usize {
        match self.game_id.dialect_family() {
            crate::dialect::DialectFamily::TES3 => self.tes3_records.len(),
            crate::dialect::DialectFamily::TES4Plus => self.groups.len(),
        }
    }
}

fn collect_records_from_group<'a>(group: &'a Group, out: &mut Vec<&'a Record>) {
    for child in &group.children {
        match child {
            crate::group::GroupChild::Record(r) => out.push(r),
            crate::group::GroupChild::Group(g) => collect_records_from_group(g, out),
        }
    }
}
