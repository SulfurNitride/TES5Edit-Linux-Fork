//! NIF mesh combiner — calls LODGen binary to produce per-cell LOD blocks.
//!
//! LODGen.exe has been ported to .NET 10 for native Linux support.
//! It reads a LODGen.txt export file and produces combined NIF meshes.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use tracing::{info, warn};

/// Find the LODGen binary.
///
/// Search order:
/// 1. lodgen-dotnet/bin/ in the project directory
/// 2. Next to the current executable
/// 3. In PATH
pub fn find_lodgen_binary(project_dir: Option<&Path>) -> Option<PathBuf> {
    // Check project directory
    if let Some(dir) = project_dir {
        let candidate = dir.join("lodgen-dotnet/bin/Debug/net10.0/LODGenx64.dll");
        if candidate.exists() {
            return Some(candidate);
        }
        let candidate = dir.join("lodgen-dotnet/bin/Release/net10.0/LODGenx64.dll");
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // Check next to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("LODGenx64.dll");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Run LODGen to combine NIF meshes.
///
/// # Arguments
/// * `lodgen_dll` - Path to the LODGenx64.dll
/// * `export_file` - Path to the LODGen.txt export file
/// * `log_callback` - Optional callback for log lines
pub fn run_lodgen(
    lodgen_dll: &Path,
    export_file: &Path,
    log_callback: Option<&dyn Fn(&str)>,
) -> Result<()> {
    info!("Running LODGen: {:?} with export {:?}", lodgen_dll, export_file);

    let mut cmd = Command::new("dotnet");
    cmd.arg(lodgen_dll.to_string_lossy().as_ref());
    cmd.arg(export_file.to_string_lossy().as_ref());

    // Capture output
    let output = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to run dotnet LODGenx64")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Forward log lines
    if let Some(cb) = log_callback {
        for line in stdout.lines() {
            cb(line);
        }
        for line in stderr.lines() {
            cb(line);
        }
    }

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        warn!("LODGen exited with code {}", code);
        if !stderr.is_empty() {
            warn!("LODGen stderr: {}", stderr.trim());
        }
        anyhow::bail!("LODGen failed with exit code {}: {}", code, stderr.trim());
    }

    info!("LODGen completed successfully");
    Ok(())
}
