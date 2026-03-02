//! MO2 (Mod Organizer 2) integration module.
//!
//! Provides configuration parsing, profile loading, and virtual file system
//! construction for loading plugins from MO2-managed mod setups.

pub mod config;
pub mod profile;
pub mod vfs;

pub use config::Mo2Config;
pub use profile::Profile;
pub use vfs::{FileSource, VirtualFileSystem};
