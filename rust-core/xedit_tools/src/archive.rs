//! BSA/BA2 archive operations: list contents, extract files, create/pack archives.
//!
//! This module wraps the `ba2` crate to provide a unified API for working with
//! Bethesda archive files (BSA for TES4/Skyrim era, BA2 for Fallout 4+).

use std::fs;
use std::path::{Path, PathBuf};

use ba2::prelude::*;
use thiserror::Error;

/// Errors that can occur during archive operations.
#[derive(Error, Debug)]
pub enum ArchiveError {
    /// I/O error reading or writing files.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The archive format could not be determined.
    #[error("unknown archive format for: {0}")]
    UnknownFormat(PathBuf),

    /// Error from the ba2 crate when parsing a TES4-era BSA.
    #[error("BSA read error: {0}")]
    Tes4(#[from] ba2::tes4::Error),

    /// Error from the ba2 crate when parsing a FO4-era BA2.
    #[error("BA2 read error: {0}")]
    Fo4(#[from] ba2::fo4::Error),

    /// The requested file was not found inside the archive.
    #[error("file not found in archive: {0}")]
    FileNotFound(String),

    /// The output directory could not be created.
    #[error("failed to create output directory: {0}")]
    OutputDir(PathBuf),
}

/// Result type for archive operations.
pub type Result<T> = std::result::Result<T, ArchiveError>;

/// An entry in an archive listing.
#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    /// Directory path within the archive (e.g. "textures/armor").
    pub directory: String,
    /// File name within the directory (e.g. "iron_d.dds").
    pub file_name: String,
}

impl ArchiveEntry {
    /// Full path within the archive: "directory/file_name".
    pub fn full_path(&self) -> String {
        if self.directory.is_empty() {
            self.file_name.clone()
        } else {
            format!("{}/{}", self.directory, self.file_name)
        }
    }
}

/// List all file entries inside a BSA archive (TES4/Skyrim era).
///
/// Returns a flat list of `ArchiveEntry` values with directory and file name.
pub fn list_bsa(archive_path: &Path) -> Result<Vec<ArchiveEntry>> {
    let (archive, _meta) =
        ba2::tes4::Archive::read(archive_path)?;

    let mut entries = Vec::new();
    for (dir_key, directory) in &archive {
        let dir_name = dir_key.name().to_string();
        for (file_key, _file) in directory {
            let file_name = file_key.name().to_string();
            entries.push(ArchiveEntry {
                directory: dir_name.clone(),
                file_name,
            });
        }
    }

    Ok(entries)
}

/// Extract a single file from a BSA archive by its full path.
///
/// The `inner_path` should use forward slashes and match the archive layout,
/// e.g. "textures/armor/iron_d.dds". The file is written to `output_path`.
pub fn extract_bsa_file(
    archive_path: &Path,
    inner_path: &str,
    output_path: &Path,
) -> Result<()> {
    let (archive, meta) =
        ba2::tes4::Archive::read(archive_path)?;

    // Split inner_path into directory + file name.
    let (dir_part, file_part) = match inner_path.rfind('/') {
        Some(pos) => (&inner_path[..pos], &inner_path[pos + 1..]),
        None => ("", inner_path),
    };

    let dir_key = ba2::tes4::ArchiveKey::from(dir_part.as_bytes());
    let file_key = ba2::tes4::DirectoryKey::from(file_part.as_bytes());

    let directory = archive
        .get(&dir_key)
        .ok_or_else(|| ArchiveError::FileNotFound(inner_path.to_string()))?;
    let file = directory
        .get(&file_key)
        .ok_or_else(|| ArchiveError::FileNotFound(inner_path.to_string()))?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| ArchiveError::OutputDir(parent.to_path_buf()))?;
    }

    let options: ba2::tes4::FileCompressionOptions = meta.into();
    let mut dst = fs::File::create(output_path)?;
    file.write(&mut dst, &options)?;

    Ok(())
}

/// Create a new BSA archive from a list of (archive_internal_path, disk_path) pairs.
///
/// `version` selects the BSA format version (e.g. `ba2::tes4::Version::SSE`).
pub fn create_bsa(
    entries: &[(&str, &Path)],
    output_path: &Path,
    version: ba2::tes4::Version,
) -> Result<()> {
    use ba2::tes4::{
        Archive, ArchiveKey, ArchiveOptions, ArchiveTypes, Directory, DirectoryKey, File,
    };

    // Group entries by directory.
    let mut dir_map: std::collections::BTreeMap<String, Vec<(String, Vec<u8>)>> =
        std::collections::BTreeMap::new();

    for (inner_path, disk_path) in entries {
        let data = fs::read(disk_path)?;
        let (dir_part, file_part) = match inner_path.rfind('/') {
            Some(pos) => (inner_path[..pos].to_string(), inner_path[pos + 1..].to_string()),
            None => (String::new(), inner_path.to_string()),
        };
        dir_map
            .entry(dir_part)
            .or_default()
            .push((file_part, data));
    }

    let archive: Archive = dir_map
        .into_iter()
        .map(|(dir_name, files)| {
            let directory: Directory = files
                .into_iter()
                .map(|(file_name, data)| {
                    let boxed: Box<[u8]> = data.into_boxed_slice();
                    let file = File::from_decompressed(boxed);
                    (DirectoryKey::from(file_name.as_bytes()), file)
                })
                .collect();
            (ArchiveKey::from(dir_name.as_bytes()), directory)
        })
        .collect();

    let options = ArchiveOptions::builder()
        .types(ArchiveTypes::MISC)
        .version(version)
        .build();

    let mut dst = fs::File::create(output_path)?;
    archive.write(&mut dst, &options)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_create_and_list_bsa() {
        let tmp_dir = std::env::temp_dir().join("xedit_tools_archive_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        // Create a test file on disk.
        let test_file = tmp_dir.join("hello.txt");
        let mut f = fs::File::create(&test_file).unwrap();
        f.write_all(b"Hello, BSA!").unwrap();
        drop(f);

        // Create a BSA containing it.
        let bsa_path = tmp_dir.join("test.bsa");
        create_bsa(
            &[("misc/hello.txt", test_file.as_path())],
            &bsa_path,
            ba2::tes4::Version::SSE,
        )
        .unwrap();

        // List the BSA contents.
        let entries = list_bsa(&bsa_path).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].directory, "misc");
        assert_eq!(entries[0].file_name, "hello.txt");

        // Clean up.
        let _ = fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_extract_bsa_file() {
        let tmp_dir = std::env::temp_dir().join("xedit_tools_extract_test");
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).unwrap();

        // Create a test file on disk.
        let test_file = tmp_dir.join("greeting.txt");
        let mut f = fs::File::create(&test_file).unwrap();
        f.write_all(b"Greetings from the archive!").unwrap();
        drop(f);

        // Create a BSA containing it.
        let bsa_path = tmp_dir.join("extract_test.bsa");
        create_bsa(
            &[("data/greeting.txt", test_file.as_path())],
            &bsa_path,
            ba2::tes4::Version::SSE,
        )
        .unwrap();

        // Extract the file.
        let out_path = tmp_dir.join("extracted.txt");
        extract_bsa_file(&bsa_path, "data/greeting.txt", &out_path).unwrap();

        let content = fs::read_to_string(&out_path).unwrap();
        assert_eq!(content, "Greetings from the archive!");

        // Clean up.
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
