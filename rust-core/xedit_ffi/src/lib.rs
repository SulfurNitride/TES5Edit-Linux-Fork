//! C FFI bridge for Pascal/Lazarus GUI to call Rust xEdit core.
//!
//! All functions use `extern "C"` calling convention with opaque handles.
//! Panics are caught and converted to error codes.
//! Strings are passed as null-terminated UTF-8 C strings.


use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use rayon::prelude::*;
use rusqlite::{params, Connection};

use xedit_core::XEditEngine;
use xedit_core::conflicts::{ConflictDetector, RecordConflict};
use xedit_core::load_order::LoadOrder;
use xedit_core::mo2::{Mo2Config, Profile, VirtualFileSystem};
// Note: Profile is used in xedit_mo2_select_profile
use xedit_dom::{FormId, GameId};
use xedit_tools::asset_scan;
use xedit_tools::cleaner;
use xedit_nif::NiflyLibrary;

// ============================================================================
// Opaque handle types
// ============================================================================

/// Opaque handle to the xEdit engine instance.
pub type EngineHandle = *mut std::ffi::c_void;

/// Opaque handle to a loaded plugin.
pub type PluginHandle = *mut std::ffi::c_void;

// ============================================================================
// Error codes
// ============================================================================

pub const XEDIT_OK: i32 = 0;
pub const XEDIT_ERR_NULL_HANDLE: i32 = -1;
pub const XEDIT_ERR_INVALID_PATH: i32 = -2;
pub const XEDIT_ERR_LOAD_FAILED: i32 = -3;
pub const XEDIT_ERR_SAVE_FAILED: i32 = -4;
pub const XEDIT_ERR_NIFLY_MISSING: i32 = -5;
pub const XEDIT_ERR_INVALID_GAME: i32 = -6;
pub const XEDIT_ERR_PANIC: i32 = -99;

// ============================================================================
// Progress callback type
// ============================================================================

/// Progress callback function pointer.
/// Called from Rust to report status to the Pascal GUI.
///   message: null-terminated UTF-8 string describing current operation
///   progress: 0.0 to 1.0 progress fraction, or -1.0 for indeterminate
pub type ProgressCallback = extern "C" fn(message: *const c_char, progress: f64);

// ============================================================================
// Global state (single engine instance)
// ============================================================================

static ENGINE: Mutex<Option<Box<XEditEngine>>> = Mutex::new(None);
static CONFLICT_REPORT: Mutex<Option<Vec<RecordConflict>>> = Mutex::new(None);
static PROGRESS_CB: Mutex<Option<ProgressCallback>> = Mutex::new(None);
static ASSET_SCAN_RESULTS: Mutex<Option<Vec<String>>> = Mutex::new(None);
static NIFLY: Mutex<Option<Box<NiflyLibrary>>> = Mutex::new(None);
static MO2_CONFIG: Mutex<Option<Mo2Config>> = Mutex::new(None);
static MO2_VFS: Mutex<Option<VirtualFileSystem>> = Mutex::new(None);

/// In-memory referenced-by index: sorted Vec of (target_form_id, src_plugin_idx, src_group_idx, src_record_idx).
/// Sorted by target_form_id for O(log n) binary search lookups.
/// ~270 MB for 16.9M entries — much faster than SQLite for both build and query.
static REFBY_DATA: Mutex<Option<Vec<(u32, i32, i32, i32)>>> = Mutex::new(None);

/// Per-plugin SQLite database connections for offloaded subrecord data.
/// Index matches engine.plugins index. Stored at ~/.cache/xedit/plugins/{filename}.db.
static PLUGIN_DBS: Mutex<Option<Vec<PathBuf>>> = Mutex::new(None);

/// Async build status:
/// 0 = not started, 1 = building refby, 2 = offloading subrecords, 3 = all done, -1 = error
static REFBY_BUILD_STATUS: AtomicI32 = AtomicI32::new(0);

/// Get the xedit cache directory (~/.cache/xedit/).
fn dirs_path() -> PathBuf {
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(cache).join("xedit")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".cache").join("xedit")
    } else {
        PathBuf::from("/tmp/xedit-cache")
    }
}

/// Read current RSS memory usage in MB.
/// Linux: reads /proc/self/statm. Windows: uses GetProcessMemoryInfo.
fn rss_mb_from_statm() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let mut parts = statm.split_whitespace();
        let _total_pages = parts.next()?;
        let resident_pages: u64 = parts.next()?.parse().ok()?;

        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page_size <= 0 {
            return None;
        }

        let rss_bytes = resident_pages.saturating_mul(page_size as u64);
        Some(rss_bytes as f64 / (1024.0 * 1024.0))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None // RSS tracking not implemented on this platform
    }
}


/// Cached texture paths from the last NIF scan, keyed by the NIF path.
/// This avoids re-scanning the same NIF for block_count / texture_count / texture_path calls.
static NIF_TEXTURE_CACHE: Mutex<Option<NifTextureCache>> = Mutex::new(None);

struct NifTextureCache {
    nif_path: String,
    textures: Vec<String>,
    block_count: u32,
}


/// Helper to catch panics and return error codes.
fn catch_panic<F: FnOnce() -> i32 + std::panic::UnwindSafe>(f: F) -> i32 {
    match std::panic::catch_unwind(f) {
        Ok(code) => code,
        Err(_) => XEDIT_ERR_PANIC,
    }
}

// ============================================================================
// Lifecycle operations
// ============================================================================

/// Initialize the xEdit engine for a specific game.
///
/// game_name: one of "SSE", "FO4", "Starfield", "FO76", "FNV", "FO3", "TES4", "TES3"
/// data_path: path to the game's Data directory (null-terminated UTF-8)
///
/// Returns XEDIT_OK on success, error code on failure.
#[no_mangle]
pub extern "C" fn xedit_init(
    game_name: *const c_char,
    data_path: *const c_char,
    _progress: Option<ProgressCallback>,
) -> i32 {
    catch_panic(|| {
        let game_str = unsafe { CStr::from_ptr(game_name) }
            .to_str()
            .unwrap_or("");
        let path_str = unsafe { CStr::from_ptr(data_path) }
            .to_str()
            .unwrap_or("");

        let game_id = match game_str {
            "SSE" | "TES5" => GameId::SkyrimSE,
            "FO4" => GameId::Fallout4,
            "Starfield" | "SF1" => GameId::Starfield,
            "FO76" => GameId::Fallout76,
            "FNV" => GameId::FalloutNV,
            "FO3" => GameId::Fallout3,
            "TES4" => GameId::Oblivion,
            "TES3" => GameId::Morrowind,
            _ => return XEDIT_ERR_INVALID_GAME,
        };

        // Initialize logging
        let _ = tracing_subscriber::fmt()
            .with_env_filter("xedit=info")
            .try_init();

        match XEditEngine::new(game_id, PathBuf::from(path_str)) {
            Ok(engine) => {
                let mut lock = ENGINE.lock().unwrap();
                *lock = Some(Box::new(engine));
                XEDIT_OK
            }
            Err(e) => {
                tracing::error!("Engine init failed: {:#}", e);
                XEDIT_ERR_NIFLY_MISSING
            }
        }
    })
}

/// Shut down the engine and free all resources.
#[no_mangle]
pub extern "C" fn xedit_shutdown() -> i32 {
    catch_panic(|| {
        let mut lock = ENGINE.lock().unwrap();
        *lock = None;
        XEDIT_OK
    })
}

// ============================================================================
// File operations
// ============================================================================

/// Load a plugin file. Returns the plugin index (>= 0) or an error code (< 0).
#[no_mangle]
pub extern "C" fn xedit_load_plugin(file_path: *const c_char) -> i32 {
    catch_panic(|| {
        if file_path.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let path_str = unsafe { CStr::from_ptr(file_path) }
            .to_str()
            .unwrap_or("");

        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        match engine.load_plugin(std::path::Path::new(path_str)) {
            Ok(idx) => idx as i32,
            Err(e) => {
                tracing::error!("Load plugin failed: {:#}", e);
                XEDIT_ERR_LOAD_FAILED
            }
        }
    })
}

/// Save a plugin to disk. Returns XEDIT_OK or error code.
#[no_mangle]
pub extern "C" fn xedit_save_plugin(plugin_index: i32, file_path: *const c_char) -> i32 {
    catch_panic(|| {
        if file_path.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let path_str = unsafe { CStr::from_ptr(file_path) }
            .to_str()
            .unwrap_or("");

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        match engine.save_plugin(plugin_index as usize, std::path::Path::new(path_str)) {
            Ok(()) => XEDIT_OK,
            Err(e) => {
                tracing::error!("Save plugin failed: {:#}", e);
                XEDIT_ERR_SAVE_FAILED
            }
        }
    })
}

/// Get the number of loaded plugins.
#[no_mangle]
pub extern "C" fn xedit_plugin_count() -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        match lock.as_ref() {
            Some(e) => e.plugin_count() as i32,
            None => 0,
        }
    })
}

// ============================================================================
// Record query operations
// ============================================================================

/// Get the number of top-level records/groups in a plugin.
#[no_mangle]
pub extern "C" fn xedit_plugin_record_count(plugin_index: i32) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        if plugin_index < 0 || plugin_index as usize >= engine.plugins.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        engine.plugins[plugin_index as usize].top_level_count() as i32
    })
}

/// Get the filename of a plugin by index. Writes to caller-provided buffer.
/// Returns bytes written (excl. null) on success, or error code.
#[no_mangle]
pub extern "C" fn xedit_plugin_filename(
    plugin_index: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let pi = plugin_index as usize;
        if pi >= engine.plugins.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let name = engine.plugins[pi]
            .file_path
            .as_deref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("(unknown)");

        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min((buf_len - 1) as usize);
        unsafe {
            ptr::copy_nonoverlapping(name_bytes.as_ptr(), buf as *mut u8, copy_len);
            *buf.add(copy_len) = 0;
        }
        copy_len as i32
    })
}

/// Get the master count for a plugin.
#[no_mangle]
pub extern "C" fn xedit_plugin_master_count(plugin_index: i32) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        if plugin_index < 0 || plugin_index as usize >= engine.plugins.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        engine.plugins[plugin_index as usize].masters.len() as i32
    })
}

/// Get a master name by index. Writes to caller-provided buffer.
/// Returns the number of bytes written (excluding null terminator), or error code.
#[no_mangle]
pub extern "C" fn xedit_plugin_master_name(
    plugin_index: i32,
    master_index: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let pi = plugin_index as usize;
        let mi = master_index as usize;

        if pi >= engine.plugins.len() || mi >= engine.plugins[pi].masters.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let name = &engine.plugins[pi].masters[mi];
        let c_name = match CString::new(name.as_str()) {
            Ok(s) => s,
            Err(_) => return XEDIT_ERR_INVALID_PATH,
        };

        let bytes = c_name.as_bytes_with_nul();
        let copy_len = bytes.len().min(buf_len as usize);
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
        }

        (copy_len - 1) as i32 // exclude null terminator from count
    })
}

// ============================================================================
// Version info
// ============================================================================

/// Get the library version string. Writes to caller-provided buffer.
#[no_mangle]
pub extern "C" fn xedit_version(buf: *mut c_char, buf_len: i32) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let version = env!("CARGO_PKG_VERSION");
        let c_version = CString::new(version).unwrap();
        let bytes = c_version.as_bytes_with_nul();
        let copy_len = bytes.len().min(buf_len as usize);
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
        }

        (copy_len - 1) as i32
    })
}

// ============================================================================
// Internal helpers
// ============================================================================

use xedit_dom::group::{Group, GroupChild, GroupType};
use xedit_dom::Plugin;

/// Helper: write a Rust string into a caller-provided C buffer.
/// Returns number of bytes written (excluding null terminator), or -1 on error.
fn write_to_buf(s: &str, buf: *mut c_char, buf_len: i32) -> i32 {
    if buf.is_null() || buf_len <= 0 {
        return XEDIT_ERR_NULL_HANDLE;
    }
    let c_str = match CString::new(s) {
        Ok(c) => c,
        Err(_) => return XEDIT_ERR_INVALID_PATH,
    };
    let bytes = c_str.as_bytes_with_nul();
    let copy_len = bytes.len().min(buf_len as usize);
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, copy_len);
    }
    // Ensure null termination even if truncated
    if copy_len > 0 {
        unsafe {
            *((buf as *mut u8).add(copy_len - 1)) = 0;
        }
    }
    (copy_len.saturating_sub(1)) as i32
}

/// Helper: validate plugin index and return a reference to the plugin.
fn get_plugin(engine: &XEditEngine, plugin_idx: i32) -> Option<&Plugin> {
    if plugin_idx < 0 || plugin_idx as usize >= engine.plugins.len() {
        return None;
    }
    Some(&engine.plugins[plugin_idx as usize])
}

/// Helper: validate group index and return a reference to the group.
fn get_group(plugin: &Plugin, group_idx: i32) -> Option<&Group> {
    if group_idx < 0 || group_idx as usize >= plugin.groups.len() {
        return None;
    }
    Some(&plugin.groups[group_idx as usize])
}

/// Collect all direct Record children from a group (flattening nested sub-groups).
/// This gives a flat list of records that belong to a top-level group.
fn collect_records_from_group(group: &Group) -> Vec<&xedit_dom::Record> {
    let mut records = Vec::new();
    for child in &group.children {
        match child {
            GroupChild::Record(r) => records.push(r),
            GroupChild::Group(g) => {
                // Recurse into nested groups (cell sub-blocks, etc.)
                let nested = collect_records_from_group(g);
                records.extend(nested);
            }
        }
    }
    records
}

/// Get the per-plugin offload DB path if available.
fn get_plugin_db_path(plugin_idx: i32) -> Option<PathBuf> {
    if plugin_idx < 0 {
        return None;
    }
    let db_lock = PLUGIN_DBS.lock().unwrap();
    match db_lock.as_ref() {
        Some(db_paths) if (plugin_idx as usize) < db_paths.len() => {
            Some(db_paths[plugin_idx as usize].clone())
        }
        _ => None,
    }
}

/// Get a human-readable name for a group type.
fn group_type_name(gt: &GroupType) -> String {
    match gt {
        GroupType::Top(sig) => format!("{}", sig.as_str()),
        GroupType::WorldChildren(id) => format!("World Children [{}]", id),
        GroupType::InteriorCellBlock(n) => format!("Interior Cell Block {}", n),
        GroupType::InteriorCellSubBlock(n) => format!("Interior Cell Sub-Block {}", n),
        GroupType::ExteriorCellBlock { x, y } => {
            format!("Exterior Cell Block ({}, {})", x, y)
        }
        GroupType::ExteriorCellSubBlock { x, y } => {
            format!("Exterior Cell Sub-Block ({}, {})", x, y)
        }
        GroupType::CellChildren(id) => format!("Cell Children [{}]", id),
        GroupType::TopicChildren(id) => format!("Topic Children [{}]", id),
        GroupType::CellPersistentChildren(id) => {
            format!("Cell Persistent Children [{}]", id)
        }
        GroupType::CellTemporaryChildren(id) => {
            format!("Cell Temporary Children [{}]", id)
        }
        GroupType::CellVisibleDistantChildren(id) => {
            format!("Cell Visible Distant Children [{}]", id)
        }
    }
}

/// Get the signature string for a group type.
fn group_type_signature(gt: &GroupType) -> String {
    match gt {
        GroupType::Top(sig) => sig.as_str(),
        _ => "GRUP".to_string(),
    }
}

// ============================================================================
// Tree navigation - Group level
// ============================================================================

/// Get the number of top-level groups in a plugin.
#[no_mangle]
pub extern "C" fn xedit_plugin_group_count(plugin_idx: i32) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        match get_plugin(engine, plugin_idx) {
            Some(p) => p.groups.len() as i32,
            None => XEDIT_ERR_NULL_HANDLE,
        }
    })
}

/// Get the type signature of a top-level group (e.g. "WEAP", "NPC_").
/// Writes to caller-provided buffer.
/// Returns bytes written (excl. null) on success, or error code.
#[no_mangle]
pub extern "C" fn xedit_group_signature(
    plugin_idx: i32,
    group_idx: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let sig = group_type_signature(&group.group_type);
        write_to_buf(&sig, buf, buf_len)
    })
}

/// Get a human-readable name for a top-level group.
/// Writes to caller-provided buffer.
/// Returns bytes written (excl. null) on success, or error code.
#[no_mangle]
pub extern "C" fn xedit_group_name(
    plugin_idx: i32,
    group_idx: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let name = group_type_name(&group.group_type);
        write_to_buf(&name, buf, buf_len)
    })
}

/// Get the number of records in a top-level group (recursively flattened).
#[no_mangle]
pub extern "C" fn xedit_group_record_count(plugin_idx: i32, group_idx: i32) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        collect_records_from_group(group).len() as i32
    })
}

// ============================================================================
// Tree navigation - Record level
// ============================================================================

/// Get the Editor ID (EDID) of a record within a group.
/// Writes to caller-provided buffer.
/// Returns bytes written (excl. null) on success, 0 if no EDID, or error code.
#[no_mangle]
pub extern "C" fn xedit_record_editor_id(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let record = records[record_idx as usize];
        match record.editor_id() {
            Some(edid) => write_to_buf(edid, buf, buf_len),
            None => {
                // Write empty string
                write_to_buf("", buf, buf_len)
            }
        }
    })
}

/// Get the FormID of a record within a group.
/// Returns the FormID as u32, or 0 on error.
#[no_mangle]
pub extern "C" fn xedit_record_form_id(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
) -> u32 {
    match std::panic::catch_unwind(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return 0u32,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return 0,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return 0,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return 0;
        }
        records[record_idx as usize].form_id.0
    }) {
        Ok(v) => v,
        Err(_) => 0,
    }
}

/// Get the type signature of a record within a group.
/// Writes to caller-provided buffer.
/// Returns bytes written (excl. null) on success, or error code.
#[no_mangle]
pub extern "C" fn xedit_record_signature(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let sig = records[record_idx as usize].signature.as_str();
        write_to_buf(&sig, buf, buf_len)
    })
}

// ============================================================================
// Record detail view - Subrecord access
// ============================================================================

/// Get the number of subrecords in a record.
#[no_mangle]
pub extern "C" fn xedit_record_subrecord_count(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let record = records[record_idx as usize];
        if !record.subrecords.is_empty() {
            return record.subrecords.len() as i32;
        }

        // After offload, subrecords vec can be empty; count via plugin DB.
        let db_path = match get_plugin_db_path(plugin_idx) {
            Some(p) => p,
            None => return 0,
        };
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(_) => return XEDIT_ERR_LOAD_FAILED,
        };
        match conn.query_row(
            "SELECT COUNT(*) FROM subrecords WHERE group_idx=?1 AND record_idx=?2",
            params![group_idx, record_idx],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(count) => count as i32,
            Err(_) => XEDIT_ERR_LOAD_FAILED,
        }
    })
}

/// Get the type signature of a subrecord.
/// Writes to caller-provided buffer.
/// Returns bytes written (excl. null) on success, or error code.
#[no_mangle]
pub extern "C" fn xedit_subrecord_signature(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    sub_idx: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let record = records[record_idx as usize];
        if !record.subrecords.is_empty() {
            if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
                return XEDIT_ERR_NULL_HANDLE;
            }
            let sig = record.subrecords[sub_idx as usize].signature.as_str();
            return write_to_buf(&sig, buf, buf_len);
        }

        let db_path = match get_plugin_db_path(plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(_) => return XEDIT_ERR_LOAD_FAILED,
        };
        let sig: String = match conn.query_row(
            "SELECT signature FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND sub_idx=?3",
            params![group_idx, record_idx, sub_idx],
            |row| row.get(0),
        ) {
            Ok(s) => s,
            Err(_) => return XEDIT_ERR_LOAD_FAILED,
        };
        write_to_buf(&sig, buf, buf_len)
    })
}

/// Get the data size of a subrecord in bytes.
#[no_mangle]
pub extern "C" fn xedit_subrecord_size(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    sub_idx: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let record = records[record_idx as usize];
        if !record.subrecords.is_empty() {
            if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
                return XEDIT_ERR_NULL_HANDLE;
            }
            return record.subrecords[sub_idx as usize].size as i32;
        }

        let db_path = match get_plugin_db_path(plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(_) => return XEDIT_ERR_LOAD_FAILED,
        };
        match conn.query_row(
            "SELECT length(raw_data) FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND sub_idx=?3",
            params![group_idx, record_idx, sub_idx],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(size) => size as i32,
            Err(_) => XEDIT_ERR_LOAD_FAILED,
        }
    })
}

/// Get raw subrecord data as a hex string.
/// Each byte becomes two hex characters (e.g. "0A FF 12").
/// Writes to caller-provided buffer.
/// Returns bytes written (excl. null) on success, or error code.
#[no_mangle]
pub extern "C" fn xedit_subrecord_data(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    sub_idx: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let record = records[record_idx as usize];
        let data: Vec<u8> = if !record.subrecords.is_empty() {
            if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
                return XEDIT_ERR_NULL_HANDLE;
            }
            let in_memory = &record.subrecords[sub_idx as usize].raw_data;
            if !in_memory.is_empty() {
                in_memory.clone()
            } else {
                let db_path = match get_plugin_db_path(plugin_idx) {
                    Some(p) => p,
                    None => return XEDIT_ERR_NULL_HANDLE,
                };
                let conn = match Connection::open(&db_path) {
                    Ok(c) => c,
                    Err(_) => return XEDIT_ERR_LOAD_FAILED,
                };
                match conn.query_row(
                    "SELECT raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND sub_idx=?3",
                    params![group_idx, record_idx, sub_idx],
                    |row| row.get::<_, Vec<u8>>(0),
                ) {
                    Ok(blob) => blob,
                    Err(_) => return XEDIT_ERR_LOAD_FAILED,
                }
            }
        } else {
            let db_path = match get_plugin_db_path(plugin_idx) {
                Some(p) => p,
                None => return XEDIT_ERR_NULL_HANDLE,
            };
            let conn = match Connection::open(&db_path) {
                Ok(c) => c,
                Err(_) => return XEDIT_ERR_LOAD_FAILED,
            };
            match conn.query_row(
                "SELECT raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND sub_idx=?3",
                params![group_idx, record_idx, sub_idx],
                |row| row.get::<_, Vec<u8>>(0),
            ) {
                Ok(blob) => blob,
                Err(_) => return XEDIT_ERR_LOAD_FAILED,
            }
        };
        // Format as space-separated hex
        let hex: String = data
            .iter()
            .enumerate()
            .fold(String::new(), |mut acc, (i, b)| {
                if i > 0 {
                    acc.push(' ');
                }
                acc.push_str(&format!("{:02X}", b));
                acc
            });
        write_to_buf(&hex, buf, buf_len)
    })
}

/// Get all subrecords for a record in one packed binary buffer.
///
/// Buffer layout:
/// - i32: subrecord count
/// - For each subrecord:
///   - 4 bytes: signature raw bytes
///   - i32: data_size
///   - data_size bytes: raw_data
///
/// Returns:
/// - >= 0: total bytes written
/// - < 0: error code, or `-needed_size` when buffer is too small
#[no_mangle]
pub extern "C" fn xedit_record_subrecords_batch(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    buf: *mut u8,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let record = records[record_idx as usize];

        // Fast path: still in memory, no DB access.
        if !record.subrecords.is_empty() {
            if record.subrecords.len() > i32::MAX as usize {
                return XEDIT_ERR_LOAD_FAILED;
            }

            let mut needed: usize = 4;
            for sub in &record.subrecords {
                if sub.raw_data.len() > i32::MAX as usize {
                    return XEDIT_ERR_LOAD_FAILED;
                }
                needed = needed.saturating_add(8 + sub.raw_data.len());
            }
            if needed > i32::MAX as usize {
                return XEDIT_ERR_LOAD_FAILED;
            }

            let needed_i32 = needed as i32;
            if buf_len < needed_i32 {
                return -needed_i32;
            }

            let out = unsafe { std::slice::from_raw_parts_mut(buf, buf_len as usize) };
            let mut offset = 0usize;
            out[offset..offset + 4]
                .copy_from_slice(&(record.subrecords.len() as i32).to_le_bytes());
            offset += 4;

            for sub in &record.subrecords {
                let mut sig_bytes = [0u8; 4];
                let sig = sub.signature.as_str();
                let sig_src = sig.as_bytes();
                let sig_copy = sig_src.len().min(4);
                sig_bytes[..sig_copy].copy_from_slice(&sig_src[..sig_copy]);

                out[offset..offset + 4].copy_from_slice(&sig_bytes);
                offset += 4;

                let data_len_i32 = sub.raw_data.len() as i32;
                out[offset..offset + 4].copy_from_slice(&data_len_i32.to_le_bytes());
                offset += 4;

                let data_len = sub.raw_data.len();
                out[offset..offset + data_len].copy_from_slice(&sub.raw_data);
                offset += data_len;
            }

            return needed_i32;
        }

        // Offloaded path: open plugin DB once and fetch all rows for this record.
        let db_path = match get_plugin_db_path(plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(_) => return XEDIT_ERR_LOAD_FAILED,
        };
        let mut stmt = match conn.prepare(
            "SELECT signature, raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 ORDER BY sub_idx",
        ) {
            Ok(s) => s,
            Err(_) => return XEDIT_ERR_LOAD_FAILED,
        };
        let rows = match stmt.query_map(params![group_idx, record_idx], |row| {
            let signature: String = row.get(0)?;
            let raw_data: Vec<u8> = row.get(1)?;
            Ok((signature, raw_data))
        }) {
            Ok(r) => r,
            Err(_) => return XEDIT_ERR_LOAD_FAILED,
        };

        let mut subrecords = Vec::<([u8; 4], Vec<u8>)>::new();
        let mut needed: usize = 4;

        for row in rows {
            let (signature, raw_data) = match row {
                Ok(v) => v,
                Err(_) => return XEDIT_ERR_LOAD_FAILED,
            };
            if raw_data.len() > i32::MAX as usize {
                return XEDIT_ERR_LOAD_FAILED;
            }

            let mut sig_bytes = [0u8; 4];
            let sig_src = signature.as_bytes();
            let sig_copy = sig_src.len().min(4);
            sig_bytes[..sig_copy].copy_from_slice(&sig_src[..sig_copy]);

            needed = needed.saturating_add(8 + raw_data.len());
            subrecords.push((sig_bytes, raw_data));
        }

        if subrecords.len() > i32::MAX as usize || needed > i32::MAX as usize {
            return XEDIT_ERR_LOAD_FAILED;
        }

        let needed_i32 = needed as i32;
        if buf_len < needed_i32 {
            return -needed_i32;
        }

        let out = unsafe { std::slice::from_raw_parts_mut(buf, buf_len as usize) };
        let mut offset = 0usize;
        out[offset..offset + 4].copy_from_slice(&(subrecords.len() as i32).to_le_bytes());
        offset += 4;

        for (sig_bytes, raw_data) in &subrecords {
            out[offset..offset + 4].copy_from_slice(sig_bytes);
            offset += 4;

            let data_len_i32 = raw_data.len() as i32;
            out[offset..offset + 4].copy_from_slice(&data_len_i32.to_le_bytes());
            offset += 4;

            out[offset..offset + raw_data.len()].copy_from_slice(raw_data);
            offset += raw_data.len();
        }

        needed_i32
    })
}

// ============================================================================
// Subrecord data offload to SQLite
// ============================================================================

fn clear_group_in_memory_data(group: &mut Group) -> u64 {
    let mut bytes_freed: u64 = 0;
    for child in &mut group.children {
        match child {
            GroupChild::Record(record) => {
                let old_subs = std::mem::take(&mut record.subrecords);
                bytes_freed += old_subs
                    .iter()
                    .map(|s| s.raw_data.len() + std::mem::size_of::<xedit_dom::Subrecord>())
                    .sum::<usize>() as u64;
                drop(old_subs);

                if let Some(hdr) = std::mem::take(&mut record.raw_header) {
                    bytes_freed += hdr.len() as u64;
                    drop(hdr);
                }
                if let Some(rd) = std::mem::take(&mut record.raw_data) {
                    bytes_freed += rd.len() as u64;
                    drop(rd);
                }
                if let Some(rcd) = std::mem::take(&mut record.raw_compressed_data) {
                    bytes_freed += rcd.len() as u64;
                    drop(rcd);
                }
            }
            GroupChild::Group(g) => {
                bytes_freed += clear_group_in_memory_data(g);
            }
        }
    }

    if let Some(hdr) = std::mem::take(&mut group.raw_header) {
        bytes_freed += hdr.len() as u64;
        drop(hdr);
    }

    bytes_freed
}

/// Sanitize plugin filenames for per-plugin SQLite DB files.
/// Keeps ASCII alphanumerics plus '.', '-' and '_'; maps everything else to '_'.
fn sanitize_plugin_db_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

/// Internal implementation: offload all plugin subrecord data to per-plugin SQLite DBs.
/// Returns DB paths for later on-demand reads.
fn offload_subrecords_internal(engine: &mut XEditEngine) -> Result<Vec<PathBuf>, i32> {
    let plugins_dir = dirs_path().join("plugins");
    if let Err(e) = std::fs::create_dir_all(&plugins_dir) {
        tracing::error!("Failed to create plugins cache dir: {}", e);
        return Err(XEDIT_ERR_SAVE_FAILED);
    }

    let num_plugins = engine.plugins.len();
    let mut total_bytes_freed: u64 = 0;
    let ram_before_mb = rss_mb_from_statm();

    // Phase 1: parallel offload to per-plugin SQLite DBs.
    let offload_results: Vec<Result<PathBuf, i32>> = engine
        .plugins
        .par_iter()
        .enumerate()
        .map(|(pi, plugin)| {
            let raw_name = plugin
                .file_path
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("plugin_{}", pi));
            let filename = sanitize_plugin_db_filename(&raw_name);
            let db_path = plugins_dir.join(format!("{}.db", filename));

            let _ = std::fs::remove_file(&db_path);

            let conn = match Connection::open(&db_path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Failed to open plugin DB {}: {}", db_path.display(), e);
                    return Err(XEDIT_ERR_SAVE_FAILED);
                }
            };

            conn.execute_batch(
                "PRAGMA journal_mode=OFF;
                    PRAGMA synchronous=OFF;
                    PRAGMA cache_size=-65536;
                    PRAGMA temp_store=MEMORY;",
            )
            .ok();

            if let Err(e) = conn.execute_batch(
                "CREATE TABLE subrecords (
                        group_idx INTEGER NOT NULL,
                        record_idx INTEGER NOT NULL,
                        sub_idx INTEGER NOT NULL,
                        signature TEXT NOT NULL DEFAULT '',
                        raw_data BLOB NOT NULL,
                        PRIMARY KEY (group_idx, record_idx, sub_idx)
                    );
                    CREATE TABLE record_data (
                        group_idx INTEGER NOT NULL,
                        record_idx INTEGER NOT NULL,
                        raw_data BLOB,
                        raw_compressed_data BLOB,
                        PRIMARY KEY (group_idx, record_idx)
                    );",
            ) {
                tracing::error!("Failed to create schema for {}: {}", db_path.display(), e);
                return Err(XEDIT_ERR_SAVE_FAILED);
            }

            if let Err(e) = conn.execute_batch("BEGIN TRANSACTION") {
                tracing::error!("Failed to begin transaction: {}", e);
                return Err(XEDIT_ERR_SAVE_FAILED);
            }

            // Use prepared statements — compiled once, rebound per row.
            // 5-10x faster than re-parsing SQL for each INSERT.
            {
                let mut sub_stmt = match conn.prepare(
                    "INSERT INTO subrecords (group_idx, record_idx, sub_idx, signature, raw_data) VALUES (?1,?2,?3,?4,?5)"
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to prepare subrecord stmt: {}", e);
                        return Err(XEDIT_ERR_SAVE_FAILED);
                    }
                };
                let mut rec_stmt = match conn.prepare(
                    "INSERT INTO record_data (group_idx, record_idx, raw_data, raw_compressed_data) VALUES (?1,?2,?3,?4)"
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("Failed to prepare record_data stmt: {}", e);
                        return Err(XEDIT_ERR_SAVE_FAILED);
                    }
                };

                for (gi, group) in plugin.groups.iter().enumerate() {
                    let records = collect_records_from_group(group);
                    for (ri, record) in records.iter().enumerate() {
                        for (si, sub) in record.subrecords.iter().enumerate() {
                            if let Err(e) = sub_stmt.execute(params![
                                gi as i32,
                                ri as i32,
                                si as i32,
                                sub.signature.as_str(),
                                sub.raw_data
                            ]) {
                                tracing::error!("Failed to insert subrecord [{},{},{}]: {}", gi, ri, si, e);
                                let _ = conn.execute_batch("ROLLBACK");
                                return Err(XEDIT_ERR_SAVE_FAILED);
                            }
                        }

                        if record.raw_data.is_some() || record.raw_compressed_data.is_some() {
                            if let Err(e) = rec_stmt.execute(params![
                                gi as i32,
                                ri as i32,
                                record.raw_data.as_deref(),
                                record.raw_compressed_data.as_deref(),
                            ]) {
                                tracing::error!("Failed to insert record_data [{},{}]: {}", gi, ri, e);
                                let _ = conn.execute_batch("ROLLBACK");
                                return Err(XEDIT_ERR_SAVE_FAILED);
                            }
                        }
                    }
                }
            }

            if let Err(e) = conn.execute_batch("COMMIT") {
                tracing::error!("Failed to commit plugin DB: {}", e);
                return Err(XEDIT_ERR_SAVE_FAILED);
            }

            drop(conn);
            tracing::info!(
                "Offloaded plugin {} ({}) to {}",
                pi,
                filename,
                db_path.display()
            );
            Ok(db_path)
        })
        .collect();

    let mut db_paths: Vec<PathBuf> = Vec::with_capacity(num_plugins);
    for result in offload_results {
        db_paths.push(result?);
    }

    // Phase 2: clear in-memory data (requires mutable access, done sequentially).
    for pi in 0..num_plugins {
        let num_groups = engine.plugins[pi].groups.len();
        let plugin_mut = &mut engine.plugins[pi];
        for gi in 0..num_groups {
            let group = &mut plugin_mut.groups[gi];
            total_bytes_freed += clear_group_in_memory_data(group);
        }
    }

    // Force allocator to return freed pages to the OS
    #[cfg(target_os = "linux")]
    let trim_result = unsafe { libc::malloc_trim(0) };
    #[cfg(not(target_os = "linux"))]
    let trim_result = 0i32;

    let ram_after_mb = rss_mb_from_statm();

    let mb_freed = total_bytes_freed as f64 / (1024.0 * 1024.0);
    tracing::info!(
        "Subrecord offload complete: {:.1} MB freed from {} plugins",
        mb_freed,
        num_plugins
    );
    match (ram_before_mb, ram_after_mb) {
        (Some(before), Some(after)) => tracing::info!(
            "Subrecord offload RSS: before {:.1} MB, after {:.1} MB (malloc_trim={})",
            before,
            after,
            trim_result
        ),
        _ => tracing::info!(
            "Subrecord offload RSS info unavailable (malloc_trim={})",
            trim_result
        ),
    }

    Ok(db_paths)
}

/// Offload all subrecord raw data and record raw data to per-plugin SQLite
/// databases on disk, then clear the in-memory copies to free RAM.
/// Call this after plugins are loaded and the refby index is built.
/// Returns XEDIT_OK on success or an error code.
#[no_mangle]
pub extern "C" fn xedit_offload_subrecords() -> i32 {
    catch_panic(|| {
        let mut engine_lock = ENGINE.lock().unwrap();
        let engine = match engine_lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        match offload_subrecords_internal(engine) {
            Ok(db_paths) => {
                // Store DB paths for later lookups
                let mut db_lock = PLUGIN_DBS.lock().unwrap();
                *db_lock = Some(db_paths);
                XEDIT_OK
            }
            Err(code) => code,
        }
    })
}

// ============================================================================
// Search operations
// ============================================================================

/// Search for records by Editor ID substring match.
/// Writes matching (group_idx, record_idx) pairs into results_buf as
/// consecutive i32 pairs: [group0, record0, group1, record1, ...].
/// max_results is the maximum number of result pairs (buffer must hold 2*max_results i32s).
/// Returns the number of matches found (up to max_results), or error code.
#[no_mangle]
pub extern "C" fn xedit_search_editor_id(
    plugin_idx: i32,
    query: *const c_char,
    results_buf: *mut i32,
    max_results: i32,
) -> i32 {
    catch_panic(|| {
        if query.is_null() || results_buf.is_null() || max_results <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let query_str = unsafe { CStr::from_ptr(query) }
            .to_str()
            .unwrap_or("");
        let query_lower = query_str.to_lowercase();

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let mut count: i32 = 0;
        for (gi, group) in plugin.groups.iter().enumerate() {
            let records = collect_records_from_group(group);
            for (ri, record) in records.iter().enumerate() {
                if count >= max_results {
                    return count;
                }
                if let Some(edid) = record.editor_id() {
                    if edid.to_lowercase().contains(&query_lower) {
                        unsafe {
                            *results_buf.offset(count as isize * 2) = gi as i32;
                            *results_buf.offset(count as isize * 2 + 1) = ri as i32;
                        }
                        count += 1;
                    }
                }
            }
        }
        count
    })
}

/// Search for a record by FormID.
/// Writes the group_idx and record_idx of the first match into the return value:
///   - Returns group_idx * 0x10000 + record_idx on success (packed pair).
///   - Returns -1 if not found or on error.
///
/// For convenience from Pascal, also provides out parameters if non-null:
///   out_group_idx and out_record_idx.
#[no_mangle]
pub extern "C" fn xedit_search_form_id(
    plugin_idx: i32,
    form_id: u32,
    out_group_idx: *mut i32,
    out_record_idx: *mut i32,
) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        for (gi, group) in plugin.groups.iter().enumerate() {
            let records = collect_records_from_group(group);
            for (ri, record) in records.iter().enumerate() {
                if record.form_id.0 == form_id {
                    if !out_group_idx.is_null() {
                        unsafe { *out_group_idx = gi as i32; }
                    }
                    if !out_record_idx.is_null() {
                        unsafe { *out_record_idx = ri as i32; }
                    }
                    return XEDIT_OK;
                }
            }
        }
        XEDIT_ERR_NULL_HANDLE // not found
    })
}

// ============================================================================
// Conflict detection FFI
// ============================================================================

/// Run conflict detection on all loaded plugins.
///
/// Populates the internal conflict report and returns the number of
/// conflicts found, or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_detect_conflicts(_handle: *mut std::ffi::c_void) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let mut load_order = LoadOrder::new(engine.game_id);
        for plugin in &engine.plugins {
            load_order.add_plugin(plugin.clone());
        }
        load_order.sort_load_order();

        let detector = ConflictDetector::new(&load_order);
        let conflicts = detector.detect_all_conflicts();
        let count = conflicts.len() as i32;

        let mut cr_lock = CONFLICT_REPORT.lock().unwrap();
        *cr_lock = Some(conflicts);

        count
    })
}

/// Get the FormID of a conflict by index.
///
/// Returns the raw FormID (u32), or 0 on error.
#[no_mangle]
pub extern "C" fn xedit_conflict_form_id(
    _handle: *mut std::ffi::c_void,
    conflict_index: i32,
) -> u32 {
    match std::panic::catch_unwind(|| {
        let lock = CONFLICT_REPORT.lock().unwrap();
        let conflicts = match lock.as_ref() {
            Some(c) => c,
            None => return 0u32,
        };
        if conflict_index < 0 || conflict_index as usize >= conflicts.len() {
            return 0;
        }
        conflicts[conflict_index as usize].form_id.raw()
    }) {
        Ok(v) => v,
        Err(_) => 0,
    }
}

/// Get the severity of a conflict by index.
///
/// Returns 0=benign, 1=override, 2=critical, 3=itm, or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_conflict_severity(
    _handle: *mut std::ffi::c_void,
    conflict_index: i32,
) -> i32 {
    catch_panic(|| {
        let lock = CONFLICT_REPORT.lock().unwrap();
        let conflicts = match lock.as_ref() {
            Some(c) => c,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        if conflict_index < 0 || conflict_index as usize >= conflicts.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        conflicts[conflict_index as usize].severity as i32
    })
}

/// Get the number of plugins involved in a conflict.
///
/// Returns the count, or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_conflict_plugin_count(
    _handle: *mut std::ffi::c_void,
    conflict_index: i32,
) -> i32 {
    catch_panic(|| {
        let lock = CONFLICT_REPORT.lock().unwrap();
        let conflicts = match lock.as_ref() {
            Some(c) => c,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        if conflict_index < 0 || conflict_index as usize >= conflicts.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        conflicts[conflict_index as usize].entries.len() as i32
    })
}

/// Detect Identical To Master (ITM) records in a specific plugin.
///
/// Writes the raw FormIDs of ITM records into `buf` (as consecutive u32 values).
/// `buf_len` is the maximum number of u32 entries the buffer can hold.
/// Returns the number of ITM records found (may exceed buf_len; only buf_len
/// entries are written), or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_detect_itm(
    _handle: *mut std::ffi::c_void,
    plugin_index: i32,
    buf: *mut u32,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        if plugin_index < 0 || plugin_index as usize >= engine.plugins.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let mut load_order = LoadOrder::new(engine.game_id);
        for plugin in &engine.plugins {
            load_order.add_plugin(plugin.clone());
        }
        load_order.sort_load_order();

        let detector = ConflictDetector::new(&load_order);
        let itms = detector.detect_itm(plugin_index as usize);
        let write_count = itms.len().min(buf_len as usize);
        for (i, fid) in itms.iter().enumerate().take(write_count) {
            unsafe {
                *buf.add(i) = fid.raw();
            }
        }

        itms.len() as i32
    })
}

// ============================================================================
// Load order FFI
// ============================================================================

/// Sort the loaded plugins by ESM/ESP rules.
///
/// Builds an internal LoadOrder from the engine's plugins, sorts it,
/// and writes the sorted order back into the engine. Returns XEDIT_OK
/// or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_sort_load_order(_handle: *mut std::ffi::c_void) -> i32 {
    catch_panic(|| {
        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let mut load_order = LoadOrder::new(engine.game_id);
        // Move plugins into the load order for sorting.
        let plugins = std::mem::take(&mut engine.plugins);
        for plugin in plugins {
            load_order.add_plugin(plugin);
        }
        load_order.sort_load_order();

        // Move sorted plugins back into the engine.
        engine.plugins = load_order.plugins;

        // Store the load order for later use by resolve/find operations.
        // Rebuild a fresh LoadOrder referencing engine plugins by cloning
        // the name index state. Since we consumed the plugins above, we
        // rebuild from the engine's now-sorted list.
        let mut lo = LoadOrder::new(engine.game_id);
        let sorted = std::mem::take(&mut engine.plugins);
        for plugin in sorted {
            lo.add_plugin(plugin);
        }
        engine.plugins = lo.plugins;

        // We cannot easily keep lo alive because it owns the plugins.
        // Instead, store a fresh LoadOrder in LOAD_ORDER that mirrors
        // the engine state. We clone the plugins for the LoadOrder.
        // NOTE: For a production build you would want a shared reference
        // architecture. For the FFI bridge, we rebuild on demand.

        XEDIT_OK
    })
}

/// Resolve a plugin-local FormID through the master list.
///
/// Given a plugin index and a raw FormID as it appears in that plugin,
/// resolves it to the canonical FormID (with correct master index byte).
/// Returns the resolved raw FormID, or 0 on error.
#[no_mangle]
pub extern "C" fn xedit_resolve_form_id(
    _handle: *mut std::ffi::c_void,
    plugin_index: i32,
    raw_form_id: u32,
) -> u32 {
    match std::panic::catch_unwind(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return 0u32,
        };

        if plugin_index < 0 || plugin_index as usize >= engine.plugins.len() {
            return 0;
        }

        // Build a temporary LoadOrder to perform resolution.
        let mut load_order = LoadOrder::new(engine.game_id);
        for plugin in &engine.plugins {
            load_order.add_plugin(plugin.clone());
        }

        match load_order.resolve_form_id(plugin_index as usize, FormId::new(raw_form_id)) {
            Some((target_plugin, local_id)) => {
                // Reconstruct the canonical FormID: plugin index in upper byte,
                // local ID in lower 3 bytes.
                ((target_plugin as u32) << 24) | local_id.raw()
            }
            None => 0,
        }
    }) {
        Ok(v) => v,
        Err(_) => 0,
    }
}

/// Find all plugins that override a given FormID.
///
/// Writes plugin indices into `buf` (as consecutive i32 values).
/// `buf_len` is the maximum number of i32 entries the buffer can hold.
/// Returns the number of overriding plugins found (may exceed buf_len;
/// only buf_len entries are written), or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_find_overrides(
    _handle: *mut std::ffi::c_void,
    form_id: u32,
    buf: *mut i32,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let mut load_order = LoadOrder::new(engine.game_id);
        for plugin in &engine.plugins {
            load_order.add_plugin(plugin.clone());
        }

        let overrides = load_order.find_overrides(FormId::new(form_id));
        let write_count = overrides.len().min(buf_len as usize);
        for (i, (plugin_idx, _record)) in overrides.iter().enumerate().take(write_count) {
            unsafe {
                *buf.add(i) = *plugin_idx as i32;
            }
        }

        overrides.len() as i32
    })
}

// ============================================================================
// Progress callback FFI
// ============================================================================

/// Register a progress callback that will be called during long operations.
///
/// The callback receives a null-terminated UTF-8 message string and a
/// progress fraction (0.0 to 1.0, or -1.0 for indeterminate).
/// Pass null to unregister the callback. Returns XEDIT_OK or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_set_progress_callback(
    _handle: *mut std::ffi::c_void,
    callback: Option<ProgressCallback>,
) -> i32 {
    catch_panic(|| {
        let mut lock = PROGRESS_CB.lock().unwrap();
        *lock = callback;
        XEDIT_OK
    })
}

// ============================================================================
// NIF operations
// ============================================================================

/// Ensure the nifly library is loaded and cached.
/// Returns XEDIT_OK on success, XEDIT_ERR_NIFLY_MISSING on failure.
fn ensure_nifly() -> i32 {
    let mut lock = NIFLY.lock().unwrap();
    if lock.is_some() {
        return XEDIT_OK;
    }
    match NiflyLibrary::load() {
        Ok(lib) => {
            *lock = Some(Box::new(lib));
            XEDIT_OK
        }
        Err(e) => {
            tracing::error!("Failed to load nifly: {}", e);
            XEDIT_ERR_NIFLY_MISSING
        }
    }
}

/// Load (or re-use cached) NIF metadata for the given path.
/// Populates the NIF_TEXTURE_CACHE with block count and texture list.
fn load_nif_cache(nif_path_str: &str) -> i32 {
    // Check if already cached
    {
        let cache_lock = NIF_TEXTURE_CACHE.lock().unwrap();
        if let Some(ref cache) = *cache_lock {
            if cache.nif_path == nif_path_str {
                return XEDIT_OK;
            }
        }
    }

    let nifly_lock = NIFLY.lock().unwrap();
    let nifly = match nifly_lock.as_ref() {
        Some(n) => n,
        None => return XEDIT_ERR_NIFLY_MISSING,
    };

    let path = std::path::Path::new(nif_path_str);
    let meta = match xedit_nif::extract_metadata(nifly, path) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("NIF metadata extraction failed: {}", e);
            return XEDIT_ERR_LOAD_FAILED;
        }
    };

    let mut cache_lock = NIF_TEXTURE_CACHE.lock().unwrap();
    *cache_lock = Some(NifTextureCache {
        nif_path: nif_path_str.to_string(),
        textures: meta.texture_paths,
        block_count: meta.block_count,
    });

    XEDIT_OK
}

/// Get the block count of a NIF file.
///
/// `handle` is reserved (pass null). `path` is a null-terminated UTF-8 path to the NIF.
/// Returns the block count (>= 0) on success, or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_nif_block_count(
    _handle: *mut std::ffi::c_void,
    path: *const c_char,
) -> i32 {
    catch_panic(|| {
        if path.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let path_str = unsafe { CStr::from_ptr(path) }
            .to_str()
            .unwrap_or("");

        let rc = ensure_nifly();
        if rc != XEDIT_OK {
            return rc;
        }
        let rc = load_nif_cache(path_str);
        if rc != XEDIT_OK {
            return rc;
        }

        let cache_lock = NIF_TEXTURE_CACHE.lock().unwrap();
        match cache_lock.as_ref() {
            Some(cache) => cache.block_count as i32,
            None => XEDIT_ERR_LOAD_FAILED,
        }
    })
}

/// Get the number of unique texture paths in a NIF file.
///
/// `handle` is reserved (pass null). `path` is a null-terminated UTF-8 path to the NIF.
/// Returns the texture count (>= 0) on success, or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_nif_texture_count(
    _handle: *mut std::ffi::c_void,
    path: *const c_char,
) -> i32 {
    catch_panic(|| {
        if path.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let path_str = unsafe { CStr::from_ptr(path) }
            .to_str()
            .unwrap_or("");

        let rc = ensure_nifly();
        if rc != XEDIT_OK {
            return rc;
        }
        let rc = load_nif_cache(path_str);
        if rc != XEDIT_OK {
            return rc;
        }

        let cache_lock = NIF_TEXTURE_CACHE.lock().unwrap();
        match cache_lock.as_ref() {
            Some(cache) => cache.textures.len() as i32,
            None => XEDIT_ERR_LOAD_FAILED,
        }
    })
}

/// Get a texture path by index from a NIF file.
///
/// `handle` is reserved (pass null). `path` is a null-terminated UTF-8 path to the NIF.
/// `index` is the zero-based texture index (from 0 to texture_count - 1).
/// The texture path is written into `buf` (null-terminated, up to `buf_len` bytes).
/// Returns the number of bytes written (excluding null terminator) on success,
/// or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_nif_texture_path(
    _handle: *mut std::ffi::c_void,
    path: *const c_char,
    index: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if path.is_null() || buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let path_str = unsafe { CStr::from_ptr(path) }
            .to_str()
            .unwrap_or("");

        let rc = ensure_nifly();
        if rc != XEDIT_OK {
            return rc;
        }
        let rc = load_nif_cache(path_str);
        if rc != XEDIT_OK {
            return rc;
        }

        let cache_lock = NIF_TEXTURE_CACHE.lock().unwrap();
        let cache = match cache_lock.as_ref() {
            Some(c) => c,
            None => return XEDIT_ERR_LOAD_FAILED,
        };

        if index < 0 || index as usize >= cache.textures.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        write_to_buf(&cache.textures[index as usize], buf, buf_len)
    })
}

// ============================================================================
// Unified tools FFI
// ============================================================================

/// Scan a plugin for asset path references (textures, meshes, sounds).
///
/// Scans all records in the specified plugin and writes the unique asset
/// paths (newline-separated) into `buf`. Each path is separated by a newline
/// character. The buffer is null-terminated.
///
/// Returns the total number of unique asset paths found, or a negative
/// error code. If `buf` is too small, as many complete paths as fit are
/// written, but the return value still reflects the total count.
#[no_mangle]
pub extern "C" fn xedit_scan_assets(
    _handle: *mut std::ffi::c_void,
    plugin_index: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        if plugin_index < 0 || plugin_index as usize >= engine.plugins.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let plugin = &engine.plugins[plugin_index as usize];
        let paths = asset_scan::scan_unique_asset_paths(plugin);
        let count = paths.len() as i32;

        // Store the paths for potential later retrieval.
        {
            let mut scan_lock = ASSET_SCAN_RESULTS.lock().unwrap();
            *scan_lock = Some(paths.clone());
        }

        // Write newline-separated paths into the buffer.
        let joined = paths.join("\n");
        write_to_buf(&joined, buf, buf_len);

        count
    })
}

/// Clean ITM (Identical to Master) records from a plugin.
///
/// Uses conflict detection to identify records in the specified plugin that
/// are byte-identical to their master version, then removes them.
///
/// Returns the number of ITM records removed, or a negative error code.
/// The plugin is modified in place and must be saved separately.
#[no_mangle]
pub extern "C" fn xedit_clean_itm(
    _handle: *mut std::ffi::c_void,
    plugin_index: i32,
) -> i32 {
    catch_panic(|| {
        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        if plugin_index < 0 || plugin_index as usize >= engine.plugins.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        // Build a load order to detect ITMs.
        let mut load_order = LoadOrder::new(engine.game_id);
        for plugin in &engine.plugins {
            load_order.add_plugin(plugin.clone());
        }
        load_order.sort_load_order();

        let detector = ConflictDetector::new(&load_order);
        let itm_form_ids = detector.detect_itm(plugin_index as usize);

        if itm_form_ids.is_empty() {
            return 0;
        }

        // Remove the ITM records from the plugin.
        let removed = cleaner::remove_itm_records(
            &mut engine.plugins[plugin_index as usize],
            &itm_form_ids,
        );

        removed as i32
    })
}

/// Clean deleted references in a plugin by undeleting them.
///
/// Finds records with the DELETED flag in the specified plugin and
/// transforms them into initially-disabled references positioned
/// underground to prevent crashes.
///
/// Returns the number of deleted references cleaned, or a negative error code.
/// The plugin is modified in place and must be saved separately.
#[no_mangle]
pub extern "C" fn xedit_clean_deleted(
    _handle: *mut std::ffi::c_void,
    plugin_index: i32,
) -> i32 {
    catch_panic(|| {
        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        if plugin_index < 0 || plugin_index as usize >= engine.plugins.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        // Build a load order to detect deleted references.
        let mut load_order = LoadOrder::new(engine.game_id);
        for plugin in &engine.plugins {
            load_order.add_plugin(plugin.clone());
        }
        load_order.sort_load_order();

        let detector = ConflictDetector::new(&load_order);
        let deleted_form_ids = detector.detect_deleted_references(plugin_index as usize);

        if deleted_form_ids.is_empty() {
            return 0;
        }

        // Undelete the references.
        let cleaned = cleaner::undelete_references(
            &mut engine.plugins[plugin_index as usize],
            &deleted_form_ids,
        );

        cleaned as i32
    })
}

// ============================================================================
// MO2 integration FFI
// ============================================================================

/// Detect and load MO2 configuration from the given folder path.
///
/// `_handle` is reserved (pass null).
/// `mo2_folder_path` is a null-terminated UTF-8 path to the MO2 installation folder.
///
/// Returns XEDIT_OK on success, or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_load_mo2(
    _handle: *mut std::ffi::c_void,
    mo2_folder_path: *const c_char,
) -> i32 {
    catch_panic(|| {
        if mo2_folder_path.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let path_str = unsafe { CStr::from_ptr(mo2_folder_path) }
            .to_str()
            .unwrap_or("");
        if path_str.is_empty() {
            return XEDIT_ERR_INVALID_PATH;
        }

        let mo2_folder = PathBuf::from(path_str);
        match Mo2Config::load(&mo2_folder) {
            Ok(config) => {
                let mut lock = MO2_CONFIG.lock().unwrap();
                *lock = Some(config);
                XEDIT_OK
            }
            Err(e) => {
                tracing::error!("Failed to load MO2 config: {:#}", e);
                XEDIT_ERR_LOAD_FAILED
            }
        }
    })
}

/// Get the number of available MO2 profiles.
///
/// Returns the profile count (>= 0) on success, or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_mo2_profile_count(_handle: *mut std::ffi::c_void) -> i32 {
    catch_panic(|| {
        let lock = MO2_CONFIG.lock().unwrap();
        match lock.as_ref() {
            Some(config) => config.available_profiles.len() as i32,
            None => XEDIT_ERR_NULL_HANDLE,
        }
    })
}

/// Get the name of an MO2 profile by index.
///
/// Writes the profile name into `buf` (null-terminated, up to `buf_len` bytes).
/// Returns the number of bytes written (excluding null terminator) on success,
/// or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_mo2_profile_name(
    _handle: *mut std::ffi::c_void,
    index: i32,
    buf: *mut c_char,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = MO2_CONFIG.lock().unwrap();
        let config = match lock.as_ref() {
            Some(c) => c,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        if index < 0 || index as usize >= config.available_profiles.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        write_to_buf(&config.available_profiles[index as usize], buf, buf_len)
    })
}

/// Select an MO2 profile by name and load it.
///
/// This parses modlist.txt, loadorder.txt, and archives.txt from the profile,
/// and builds the virtual file system. Returns XEDIT_OK on success.
#[no_mangle]
pub extern "C" fn xedit_mo2_select_profile(
    _handle: *mut std::ffi::c_void,
    profile_name: *const c_char,
) -> i32 {
    catch_panic(|| {
        if profile_name.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let name_str = unsafe { CStr::from_ptr(profile_name) }
            .to_str()
            .unwrap_or("");
        if name_str.is_empty() {
            return XEDIT_ERR_INVALID_PATH;
        }

        // Update config with selected profile
        {
            let mut lock = MO2_CONFIG.lock().unwrap();
            let config = match lock.as_mut() {
                Some(c) => c,
                None => return XEDIT_ERR_NULL_HANDLE,
            };
            config.set_profile(name_str);
        }

        // Load the profile
        let lock = MO2_CONFIG.lock().unwrap();
        let config = lock.as_ref().unwrap();

        match Profile::load(&config.profile_path, &config.mods_path, &config.data_path) {
            Ok(profile) => {
                // Build VFS from profile
                match VirtualFileSystem::new(profile) {
                    Ok(vfs) => {
                        // Store profile reference from VFS (VFS owns the profile)
                        let mut vfs_lock = MO2_VFS.lock().unwrap();
                        *vfs_lock = Some(vfs);
                        XEDIT_OK
                    }
                    Err(e) => {
                        tracing::error!("Failed to build MO2 VFS: {:#}", e);
                        XEDIT_ERR_LOAD_FAILED
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to load MO2 profile: {:#}", e);
                XEDIT_ERR_LOAD_FAILED
            }
        }
    })
}

/// Load all plugins from the MO2 profile's load order into the engine.
///
/// Resolves each plugin file through the VFS (check mod folders first,
/// then vanilla Data), loads them with PluginReader, and adds them to the
/// engine's plugin list.
///
/// Returns the number of plugins loaded on success, or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_mo2_load_order(_handle: *mut std::ffi::c_void) -> i32 {
    catch_panic(|| {
        use rayon::prelude::*;

        let vfs_lock = MO2_VFS.lock().unwrap();
        let vfs = match vfs_lock.as_ref() {
            Some(v) => v,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let plugin_paths = vfs.list_plugins();
        drop(vfs_lock);

        let mut engine_lock = ENGINE.lock().unwrap();
        let engine = match engine_lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let total = plugin_paths.len();
        eprintln!("[Load] Parsing {} plugins in parallel across {} threads...",
            total, rayon::current_num_threads());
        let start = std::time::Instant::now();

        // Phase 1: Parse all plugins in parallel (IO + parsing is the bottleneck)
        let reader = xedit_io::PluginReader::new(engine.game_id);
        let parsed: Vec<Option<xedit_dom::Plugin>> = plugin_paths
            .par_iter()
            .map(|path| {
                match reader.read_file(path) {
                    Ok(plugin) => Some(plugin),
                    Err(e) => {
                        eprintln!("[Load] Failed to load {:?}: {:#}",
                            path.file_name().unwrap_or_default(), e);
                        None
                    }
                }
            })
            .collect();

        let parse_elapsed = start.elapsed();
        eprintln!("[Load] Parallel parse done in {:.2}s", parse_elapsed.as_secs_f64());

        // Phase 2: Push into engine in load order (sequential, but very fast)
        let mut loaded = 0i32;
        for plugin in parsed {
            if let Some(p) = plugin {
                engine.plugins.push(p);
                loaded += 1;
            }
        }

        let total_elapsed = start.elapsed();
        eprintln!("[Load] {} plugins loaded in {:.2}s", loaded, total_elapsed.as_secs_f64());

        loaded
    })
}

// ============================================================================
// Cross-reference (Referenced By) index — SQLite backed
// ============================================================================

/// Internal: build the refby index as a sorted in-memory Vec.
/// No SQLite — just parallel scan + sort. Returns sorted Vec of
/// (target_form_id, src_plugin_idx, src_group_idx, src_record_idx).
fn build_refby_index(plugins: &[xedit_dom::Plugin]) -> Vec<(u32, i32, i32, i32)> {
    // Flatten all records so par_iter works at record granularity (3.9M work units)
    // instead of plugin granularity (1566 uneven units where Skyrim.esm dominates).
    struct RecordRef<'a> {
        plugin_idx: i32,
        group_idx: i32,
        record_idx: i32,
        record: &'a xedit_dom::Record,
    }

    let flatten_start = std::time::Instant::now();
    let all_records: Vec<RecordRef> = plugins
        .iter()
        .enumerate()
        .flat_map(|(pi, plugin)| {
            let pi_i32 = pi as i32;
            plugin.groups.iter().enumerate().flat_map(move |(gi, group)| {
                let gi_i32 = gi as i32;
                collect_records_from_group(group)
                    .into_iter()
                    .enumerate()
                    .map(move |(ri, record)| RecordRef {
                        plugin_idx: pi_i32,
                        group_idx: gi_i32,
                        record_idx: ri as i32,
                        record,
                    })
            })
        })
        .collect();

    eprintln!("[RefBy] Flattened {} records across {} plugins in {:.2}s",
        all_records.len(), plugins.len(), flatten_start.elapsed().as_secs_f64());

    // Collect known FormIDs (parallel over flat records)
    let formid_start = std::time::Instant::now();
    let known_formids: HashSet<u32> = all_records
        .par_iter()
        .filter_map(|r| {
            let fid = r.record.form_id.0;
            if fid != 0 { Some(fid) } else { None }
        })
        .collect();

    let mut sorted_formids: Vec<u32> = known_formids.into_iter().collect();
    sorted_formids.sort_unstable();

    eprintln!("[RefBy] {} unique FormIDs collected in {:.2}s",
        sorted_formids.len(), formid_start.elapsed().as_secs_f64());

    if sorted_formids.is_empty() {
        return Vec::new();
    }

    // Scan all subrecord data for FormID references in parallel.
    eprintln!("[RefBy] Scanning {} records for cross-references across {} threads...",
        all_records.len(), rayon::current_num_threads());
    let scan_start = std::time::Instant::now();

    let mut ref_tuples: Vec<(u32, i32, i32, i32)> = all_records
        .par_iter()
        .flat_map_iter(|r| {
            let source_formid = r.record.form_id.0;
            let mut seen = HashSet::new();
            let mut local_refs = Vec::new();

            for subrecord in &r.record.subrecords {
                let sig = subrecord.signature.as_str();
                if sig == "EDID" || sig == "FULL" || sig == "DESC" || sig == "MODL"
                    || sig == "ICON" || sig == "MICO" || sig == "TX00" || sig == "TX01"
                {
                    continue;
                }

                let data = &subrecord.raw_data;
                if data.len() < 4 {
                    continue;
                }

                let mut offset = 0;
                while offset + 4 <= data.len() {
                    let candidate = u32::from_le_bytes([
                        data[offset],
                        data[offset + 1],
                        data[offset + 2],
                        data[offset + 3],
                    ]);

                    if candidate != 0
                        && candidate != source_formid
                        && sorted_formids.binary_search(&candidate).is_ok()
                        && seen.insert(candidate)
                    {
                        local_refs.push((candidate, r.plugin_idx, r.group_idx, r.record_idx));
                    }

                    offset += 4;
                }
            }

            local_refs.into_iter()
        })
        .collect();

    eprintln!("[RefBy] Scan done in {:.2}s, found {} cross-references",
        scan_start.elapsed().as_secs_f64(), ref_tuples.len());

    // Sort by target_form_id for binary search lookups.
    let sort_start = std::time::Instant::now();
    ref_tuples.par_sort_unstable_by_key(|entry| entry.0);
    eprintln!("[RefBy] Sorted {} entries in {:.2}s",
        ref_tuples.len(), sort_start.elapsed().as_secs_f64());

    let total_mb = (ref_tuples.len() * std::mem::size_of::<(u32, i32, i32, i32)>()) as f64 / (1024.0 * 1024.0);
    eprintln!("[RefBy] Index size: {:.1} MB in memory", total_mb);

    ref_tuples
}

/// Build or rebuild the cross-reference index (synchronous).
///
/// Scans all subrecord data in all loaded plugins for 4-byte values that
/// match known FormIDs. Builds a sorted in-memory index.
///
/// Must be called after all plugins are loaded. Returns XEDIT_OK on
/// success, or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_build_refby_index() -> i32 {
    catch_panic(|| {
        REFBY_BUILD_STATUS.store(1, Ordering::SeqCst);

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => {
                REFBY_BUILD_STATUS.store(-1, Ordering::SeqCst);
                return XEDIT_ERR_NULL_HANDLE;
            }
        };

        let index = build_refby_index(&engine.plugins);
        drop(lock);

        let mut data_lock = REFBY_DATA.lock().unwrap();
        *data_lock = Some(index);
        REFBY_BUILD_STATUS.store(3, Ordering::SeqCst);
        XEDIT_OK
    })
}

/// Start building the cross-reference index in a background thread.
///
/// Holds the engine lock for the duration of the scan (no data cloning).
/// Writes results to SQLite on disk at ~/.cache/xedit/refby.db.
///
/// Returns XEDIT_OK immediately. Use `xedit_refby_build_status` to poll
/// for completion:
/// 0 = not started, 1 = building refby, 2 = offloading subrecords, 3 = all done, -1 = error.
#[no_mangle]
pub extern "C" fn xedit_build_refby_index_async() -> i32 {
    catch_panic(|| {
        // Don't start if already building
        let current = REFBY_BUILD_STATUS.load(Ordering::SeqCst);
        if current == 1 || current == 2 {
            return XEDIT_OK; // Already in progress
        }

        REFBY_BUILD_STATUS.store(1, Ordering::SeqCst);

        std::thread::spawn(|| {
            let start = std::time::Instant::now();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Hold the engine lock for the scan — no data cloning needed.
                let lock = ENGINE.lock().unwrap();
                let engine = match lock.as_ref() {
                    Some(e) => e,
                    None => {
                        REFBY_BUILD_STATUS.store(-1, Ordering::SeqCst);
                        return;
                    }
                };

                let index = build_refby_index(&engine.plugins);
                drop(lock); // Release engine lock

                let mut data_lock = REFBY_DATA.lock().unwrap();
                *data_lock = Some(index);
                drop(data_lock);

                REFBY_BUILD_STATUS.store(2, Ordering::SeqCst);

                // Re-acquire ENGINE lock mutably for subrecord offload.
                let mut engine_lock = ENGINE.lock().unwrap();
                let engine_mut = match engine_lock.as_mut() {
                    Some(e) => e,
                    None => {
                        REFBY_BUILD_STATUS.store(-1, Ordering::SeqCst);
                        return;
                    }
                };

                match offload_subrecords_internal(engine_mut) {
                    Ok(db_paths) => {
                        let mut plugin_db_lock = PLUGIN_DBS.lock().unwrap();
                        *plugin_db_lock = Some(db_paths);
                    }
                    Err(_) => {
                        REFBY_BUILD_STATUS.store(-1, Ordering::SeqCst);
                    }
                }
            }));

            let elapsed = start.elapsed();
            match result {
                Ok(()) => {
                    // Only set to done if not already marked as error
                    if REFBY_BUILD_STATUS.load(Ordering::SeqCst) != -1 {
                        eprintln!("[RefBy+Offload] Build/offload completed in {:.2}s", elapsed.as_secs_f64());
                        REFBY_BUILD_STATUS.store(3, Ordering::SeqCst);
                    }
                }
                Err(_) => {
                    eprintln!("[RefBy] Database build PANICKED after {:.2}s", elapsed.as_secs_f64());
                    REFBY_BUILD_STATUS.store(-1, Ordering::SeqCst);
                }
            }
        });

        XEDIT_OK
    })
}

/// Check the status of the async refby index build.
/// Returns:
/// 0 = not started, 1 = building refby, 2 = offloading subrecords, 3 = all done, -1 = error.
#[no_mangle]
pub extern "C" fn xedit_refby_build_status() -> i32 {
    REFBY_BUILD_STATUS.load(Ordering::SeqCst)
}

/// Find the range of entries in the sorted refby index matching a given FormID.
/// Returns (start, end) indices into the sorted Vec, where end is exclusive.
fn refby_range_for_formid(data: &[(u32, i32, i32, i32)], formid: u32) -> (usize, usize) {
    // Binary search for first occurrence
    let start = data.partition_point(|e| e.0 < formid);
    if start >= data.len() || data[start].0 != formid {
        return (0, 0);
    }
    // Find end of range
    let end = data[start..].partition_point(|e| e.0 == formid) + start;
    (start, end)
}

/// Get how many records reference a given record (by its FormID).
///
/// The record is identified by (plugin_idx, group_idx, record_idx).
/// Returns the reference count (>= 0), or a negative error code.
/// The cross-reference index must have been built first via
/// `xedit_build_refby_index`.
#[no_mangle]
pub extern "C" fn xedit_record_refby_count(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
) -> i32 {
    catch_panic(|| {
        let engine_lock = ENGINE.lock().unwrap();
        let engine = match engine_lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let formid = records[record_idx as usize].form_id.0;
        drop(engine_lock);

        let data_lock = REFBY_DATA.lock().unwrap();
        let data = match data_lock.as_ref() {
            Some(d) => d,
            None => return 0,
        };

        let (start, end) = refby_range_for_formid(data, formid);
        (end - start) as i32
    })
}

/// Get the Nth referencing record for a given record.
///
/// The target record is identified by (plugin_idx, group_idx, record_idx).
/// `ref_index` selects which referencing record to return (0-based).
/// The referencing record's location is written to `out_plugin_idx`,
/// `out_group_idx`, and `out_record_idx`.
///
/// Returns XEDIT_OK on success, or a negative error code.
#[no_mangle]
pub extern "C" fn xedit_record_refby_entry(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    ref_index: i32,
    out_plugin_idx: *mut i32,
    out_group_idx: *mut i32,
    out_record_idx: *mut i32,
) -> i32 {
    catch_panic(|| {
        if out_plugin_idx.is_null() || out_group_idx.is_null() || out_record_idx.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let engine_lock = ENGINE.lock().unwrap();
        let engine = match engine_lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let formid = records[record_idx as usize].form_id.0;
        drop(engine_lock);

        let data_lock = REFBY_DATA.lock().unwrap();
        let data = match data_lock.as_ref() {
            Some(d) => d,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let (start, end) = refby_range_for_formid(data, formid);
        let idx = start + ref_index as usize;
        if idx >= end {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let entry = &data[idx];
        unsafe {
            *out_plugin_idx = entry.1;
            *out_group_idx = entry.2;
            *out_record_idx = entry.3;
        }
        XEDIT_OK
    })
}

/// Batch-fetch all referenced-by entries for a record, including metadata.
///
/// Uses binary search on sorted in-memory index + engine lookup for metadata.
///
/// Results are written as packed entries into `buf`:
///   - plugin_idx: i32 (4 bytes, little-endian)
///   - group_idx:  i32 (4 bytes, little-endian)
///   - record_idx: i32 (4 bytes, little-endian)
///   - form_id:    u32 (4 bytes, little-endian)
///   - sig_len:    u16 (2 bytes, little-endian)
///   - signature:  [u8; sig_len]
///   - edid_len:   u16 (2 bytes, little-endian)
///   - editor_id:  [u8; edid_len]
///   - fname_len:  u16 (2 bytes, little-endian)
///   - filename:   [u8; fname_len]
///
/// Returns: number of entries written (>= 0), or negative error code.
#[no_mangle]
pub extern "C" fn xedit_record_refby_batch(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    buf: *mut u8,
    buf_len: i32,
) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        // Look up the target record's FormID.
        let engine_lock = ENGINE.lock().unwrap();
        let engine = match engine_lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let records = collect_records_from_group(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let formid = records[record_idx as usize].form_id.0;

        // Binary search the in-memory index
        let data_lock = REFBY_DATA.lock().unwrap();
        let data = match data_lock.as_ref() {
            Some(d) => d,
            None => return 0,
        };

        let (start, end) = refby_range_for_formid(data, formid);

        let buf_size = buf_len as usize;
        let mut offset: usize = 0;
        let mut count: i32 = 0;

        for entry in &data[start..end] {
            let (_, rp, rg, rr) = *entry;

            // Look up metadata from engine memory
            let (ref_form_id, sig, edid, fname) = match get_plugin(engine, rp) {
                Some(src_plugin) => {
                    let fname = src_plugin
                        .file_path
                        .as_deref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("(unknown)");

                    match get_group(src_plugin, rg) {
                        Some(src_group) => {
                            let src_records = collect_records_from_group(src_group);
                            if rr >= 0 && (rr as usize) < src_records.len() {
                                let rec = src_records[rr as usize];
                                (
                                    rec.form_id.0,
                                    rec.signature.as_str().to_string(),
                                    rec.editor_id().unwrap_or("").to_string(),
                                    fname.to_string(),
                                )
                            } else {
                                (0u32, String::new(), String::new(), fname.to_string())
                            }
                        }
                        None => (0u32, String::new(), String::new(), fname.to_string()),
                    }
                }
                None => continue,
            };

            let sig_bytes = sig.as_bytes();
            let edid_bytes = edid.as_bytes();
            let fname_bytes = fname.as_bytes();

            let entry_size = 4 + 4 + 4 + 4
                + 2 + sig_bytes.len()
                + 2 + edid_bytes.len()
                + 2 + fname_bytes.len();

            if offset + entry_size > buf_size {
                break;
            }

            unsafe {
                let dst = buf.add(offset);

                ptr::copy_nonoverlapping(rp.to_le_bytes().as_ptr(), dst, 4);
                ptr::copy_nonoverlapping(rg.to_le_bytes().as_ptr(), dst.add(4), 4);
                ptr::copy_nonoverlapping(rr.to_le_bytes().as_ptr(), dst.add(8), 4);
                ptr::copy_nonoverlapping(ref_form_id.to_le_bytes().as_ptr(), dst.add(12), 4);

                let mut pos = 16;

                ptr::copy_nonoverlapping((sig_bytes.len() as u16).to_le_bytes().as_ptr(), dst.add(pos), 2);
                pos += 2;
                ptr::copy_nonoverlapping(sig_bytes.as_ptr(), dst.add(pos), sig_bytes.len());
                pos += sig_bytes.len();

                ptr::copy_nonoverlapping((edid_bytes.len() as u16).to_le_bytes().as_ptr(), dst.add(pos), 2);
                pos += 2;
                ptr::copy_nonoverlapping(edid_bytes.as_ptr(), dst.add(pos), edid_bytes.len());
                pos += edid_bytes.len();

                ptr::copy_nonoverlapping((fname_bytes.len() as u16).to_le_bytes().as_ptr(), dst.add(pos), 2);
                pos += 2;
                ptr::copy_nonoverlapping(fname_bytes.as_ptr(), dst.add(pos), fname_bytes.len());
            }

            offset += entry_size;
            count += 1;
        }

        count
    })
}
