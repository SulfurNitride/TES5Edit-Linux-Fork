//! Plugin file I/O and surgical patch writer for xEdit.
//!
//! Handles reading and writing Bethesda plugin files (ESP/ESM/ESL)
//! with lossless binary preservation for unmodified records.

pub mod reader;
pub mod writer;
#[cfg(test)]
mod tests;

pub use reader::PluginReader;
pub use writer::PluginWriter;
