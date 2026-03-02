//! Game-specific record definitions for all Bethesda games.
//!
//! Definitions are transpiled from the original Delphi wbDefinitions*.pas files.
//! Each game module provides record type definitions, subrecord layouts,
//! flag meanings, enum values, and FormID reference types.

pub mod definition;
pub mod registry;

// Game-specific definition modules (populated by transpiler)
pub mod morrowind_defs;
pub mod tes4_defs;
pub mod fo3_defs;
pub mod fnv_defs;
pub mod sse_defs;
pub mod fo4_defs;
pub mod fo76_defs;
pub mod starfield_defs;

pub use definition::{
    RecordDef, SubrecordDef, FieldDef, FieldType, FlagDef, EnumDef,
};
pub use registry::DefinitionRegistry;
