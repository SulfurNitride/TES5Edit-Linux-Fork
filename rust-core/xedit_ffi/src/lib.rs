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
use xedit_dom::{FormId, GameId, Signature};
use xedit_dom::element::IntegerSize;
use xedit_games::definition::{FieldType, FieldDef, EnumDef, FlagDef};
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
pub const XEDIT_ERR_BUFFER_TOO_SMALL: i32 = -7;
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
use xedit_dom::Record;
use xedit_dom::Subrecord;

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

/// Helper: validate plugin index and return a mutable reference to the plugin.
fn get_plugin_mut(engine: &mut XEditEngine, plugin_idx: i32) -> Option<&mut Plugin> {
    if plugin_idx < 0 || plugin_idx as usize >= engine.plugins.len() {
        return None;
    }
    Some(&mut engine.plugins[plugin_idx as usize])
}

/// Helper: validate group index and return a mutable reference to the group.
fn get_group_mut(plugin: &mut Plugin, group_idx: i32) -> Option<&mut Group> {
    if group_idx < 0 || group_idx as usize >= plugin.groups.len() {
        return None;
    }
    Some(&mut plugin.groups[group_idx as usize])
}

/// Collect all direct Record children from a group as mutable references
/// (flattening nested sub-groups). Returns a flat list with indices into
/// the group's children vec.
fn collect_records_from_group_mut(group: &mut Group) -> Vec<&mut Record> {
    let mut records = Vec::new();
    for child in &mut group.children {
        match child {
            GroupChild::Record(r) => records.push(r),
            GroupChild::Group(g) => {
                let nested = collect_records_from_group_mut(g);
                records.extend(nested);
            }
        }
    }
    records
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

/// Get the load order index of a plugin.
/// Returns the load order position (0-based) if valid, -1 if invalid.
/// Currently plugins are loaded sequentially, so plugin_idx IS the load order index.
#[no_mangle]
pub extern "C" fn xedit_plugin_load_order_id(plugin_idx: i32) -> i32 {
    catch_panic(|| {
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return -1,
        };
        match get_plugin(engine, plugin_idx) {
            Some(_) => plugin_idx,
            None => -1,
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

/// Batch-return all FormIDs for a group.
/// Writes form_ids as contiguous u32 values into buf.
/// Returns number of FormIDs written, or negative error.
#[no_mangle]
pub extern "C" fn xedit_group_form_ids(
    plugin_idx: i32,
    group_idx: i32,
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
        let plugin = match get_plugin(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let group = match get_group(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let records = collect_records_from_group(group);
        let max = buf_len as usize;
        let count = records.len().min(max);
        let slice = unsafe { std::slice::from_raw_parts_mut(buf, max) };
        for (i, record) in records.iter().enumerate().take(count) {
            slice[i] = record.form_id.raw();
        }
        count as i32
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
            Some(edid) if !edid.is_empty() => write_to_buf(edid, buf, buf_len),
            _ => {
                // In-memory subrecords may be empty after SQLite offload.
                // Fall back to querying the plugin's SQLite DB for EDID/FULL.
                let db_path = match get_plugin_db_path(plugin_idx) {
                    Some(p) => p,
                    None => return write_to_buf("", buf, buf_len),
                };
                let conn = match Connection::open(&db_path) {
                    Ok(c) => c,
                    Err(_) => return write_to_buf("", buf, buf_len),
                };

                // Query EDID (editor id) — raw_data is a null-terminated UTF-8 string
                let edid: String = conn
                    .query_row(
                        "SELECT raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND signature='EDID' LIMIT 1",
                        params![group_idx, record_idx],
                        |row| {
                            let data: Vec<u8> = row.get(0)?;
                            let s = String::from_utf8_lossy(&data).trim_end_matches('\0').to_string();
                            Ok(s)
                        },
                    )
                    .unwrap_or_default();

                if edid.is_empty() {
                    return write_to_buf("", buf, buf_len);
                }

                // Query FULL (display name) — null-terminated UTF-8, or 4-byte string index if localized
                let full: String = if plugin.is_localized() {
                    // Localized plugins store FULL as a 4-byte string table index, skip it
                    String::new()
                } else {
                    conn.query_row(
                        "SELECT raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND signature='FULL' LIMIT 1",
                        params![group_idx, record_idx],
                        |row| {
                            let data: Vec<u8> = row.get(0)?;
                            let s = String::from_utf8_lossy(&data).trim_end_matches('\0').to_string();
                            Ok(s)
                        },
                    )
                    .unwrap_or_default()
                };

                let result = if full.is_empty() {
                    edid
                } else {
                    format!("{} \"{}\"", edid, full)
                };
                write_to_buf(&result, buf, buf_len)
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

/// Find all plugins that contain a record with the given FormID,
/// returning full coordinates (plugin_idx, group_idx, record_idx) for each.
///
/// Buffer format: [i32 count | (i32 plugin_idx, i32 group_idx, i32 record_idx) ...]
/// Returns XEDIT_OK on success, negative error code on failure.
#[no_mangle]
pub extern "C" fn xedit_find_overrides_full(
    form_id: u32,
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

        // Collect all (plugin_idx, group_idx, record_idx) triples for this FormID
        let mut results: Vec<(i32, i32, i32)> = Vec::new();
        for (pi, plugin) in engine.plugins.iter().enumerate() {
            for (gi, group) in plugin.groups.iter().enumerate() {
                let records = collect_records_from_group(group);
                for (ri, record) in records.iter().enumerate() {
                    if record.form_id.0 == form_id {
                        results.push((pi as i32, gi as i32, ri as i32));
                    }
                }
            }
        }

        let count = results.len() as i32;
        // Need 4 bytes for count + 12 bytes per triple
        let required = 4 + results.len() * 12;
        if (buf_len as usize) < required {
            // Not enough space — write count only if we have room, then return error
            if buf_len >= 4 {
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        &count as *const i32 as *const u8,
                        buf,
                        4,
                    );
                }
            }
            return XEDIT_ERR_BUFFER_TOO_SMALL;
        }

        // Write count
        unsafe {
            std::ptr::copy_nonoverlapping(
                &count as *const i32 as *const u8,
                buf,
                4,
            );
        }

        // Write triples
        for (i, (pi, gi, ri)) in results.iter().enumerate() {
            let offset = 4 + i * 12;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    pi as *const i32 as *const u8,
                    buf.add(offset),
                    4,
                );
                std::ptr::copy_nonoverlapping(
                    gi as *const i32 as *const u8,
                    buf.add(offset + 4),
                    4,
                );
                std::ptr::copy_nonoverlapping(
                    ri as *const i32 as *const u8,
                    buf.add(offset + 8),
                    4,
                );
            }
        }

        XEDIT_OK
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

// ============================================================================
// Subrecord display value
// ============================================================================

/// Decode a subrecord's raw data into a human-readable display string.
/// Returns the length written to buf (not counting null terminator), or negative error code.
#[no_mangle]
pub extern "C" fn xedit_subrecord_display_value(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    sub_idx: i32,
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

        // Get the subrecord (in-memory or from offload DB)
        let (sig_str, raw_data): (String, Vec<u8>) = if !record.subrecords.is_empty() {
            if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
                return XEDIT_ERR_NULL_HANDLE;
            }
            let sub = &record.subrecords[sub_idx as usize];
            let sig = sub.signature.as_str();
            let data = if !sub.raw_data.is_empty() {
                sub.raw_data.clone()
            } else {
                match load_subrecord_from_db(plugin_idx, group_idx, record_idx, sub_idx) {
                    Some(d) => d,
                    None => return XEDIT_ERR_LOAD_FAILED,
                }
            };
            (sig, data)
        } else {
            // All subrecords offloaded — load from DB
            let data = match load_subrecord_from_db(plugin_idx, group_idx, record_idx, sub_idx) {
                Some(d) => d,
                None => return XEDIT_ERR_LOAD_FAILED,
            };
            // Try to get signature from the offload DB
            let sig = match load_subrecord_sig_from_db(plugin_idx, group_idx, record_idx, sub_idx) {
                Some(s) => s,
                None => {
                    // Can't determine type, fall back to hex
                    let display = format_hex_fallback(&data);
                    return write_to_u8_buf(&display, buf, buf_len);
                }
            };
            (sig, data)
        };

        let record_sig = record.signature.as_str();
        let is_localized = plugin.is_localized();
        let display = decode_subrecord_display(engine, &sig_str, &raw_data, &record_sig, is_localized);
        write_to_u8_buf(&display, buf, buf_len)
    })
}

/// Helper: load raw subrecord data from the offload SQLite DB.
fn load_subrecord_from_db(plugin_idx: i32, group_idx: i32, record_idx: i32, sub_idx: i32) -> Option<Vec<u8>> {
    let db_path = get_plugin_db_path(plugin_idx)?;
    let conn = Connection::open(&db_path).ok()?;
    conn.query_row(
        "SELECT raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND sub_idx=?3",
        params![group_idx, record_idx, sub_idx],
        |row| row.get::<_, Vec<u8>>(0),
    ).ok()
}

/// Helper: load subrecord signature from the offload SQLite DB.
fn load_subrecord_sig_from_db(plugin_idx: i32, group_idx: i32, record_idx: i32, sub_idx: i32) -> Option<String> {
    let db_path = get_plugin_db_path(plugin_idx)?;
    let conn = Connection::open(&db_path).ok()?;
    conn.query_row(
        "SELECT signature FROM subrecords WHERE group_idx=?1 AND record_idx=?2 AND sub_idx=?3",
        params![group_idx, record_idx, sub_idx],
        |row| row.get::<_, String>(0),
    ).ok()
}

/// Write a string into a *mut u8 buffer with null termination.
/// Returns bytes written (not counting null), or error code.
fn write_to_u8_buf(s: &str, buf: *mut u8, buf_len: i32) -> i32 {
    if buf.is_null() || buf_len <= 0 {
        return XEDIT_ERR_NULL_HANDLE;
    }
    let bytes = s.as_bytes();
    let max_copy = (buf_len as usize).saturating_sub(1); // leave room for null
    let copy_len = bytes.len().min(max_copy);
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf, copy_len);
        *buf.add(copy_len) = 0; // null terminator
    }
    copy_len as i32
}

/// Format raw bytes as hex fallback: "0A FF 12 34", truncated at 64 bytes.
fn format_hex_fallback(data: &[u8]) -> String {
    let truncated = data.len() > 64;
    let slice = if truncated { &data[..64] } else { data };
    let mut hex = String::with_capacity(slice.len() * 3);
    for (i, b) in slice.iter().enumerate() {
        if i > 0 {
            hex.push(' ');
        }
        hex.push_str(&format!("{:02X}", b));
    }
    if truncated {
        hex.push_str("...");
    }
    hex
}

/// Decode a subrecord's raw data into a human-readable display string.
///
/// First attempts definition-based decoding using the game's DefinitionRegistry.
/// Falls back to the heuristic decoder if no definition is found.
fn decode_subrecord_display(engine: &XEditEngine, sig: &str, data: &[u8], record_sig: &str, is_localized: bool) -> String {
    // Try definition-based decoding first
    if sig.len() == 4 && record_sig.len() >= 4 {
        let mut rec_bytes = [b' '; 4];
        for (i, b) in record_sig.bytes().take(4).enumerate() {
            rec_bytes[i] = b;
        }
        let rec_signature = Signature(rec_bytes);

        let mut sub_bytes = [b' '; 4];
        for (i, b) in sig.bytes().take(4).enumerate() {
            sub_bytes[i] = b;
        }
        let sub_signature = Signature(sub_bytes);

        if let Some(record_def) = engine.definitions.get(engine.game_id, rec_signature) {
            // Find the matching subrecord definition
            if let Some(subrec_def) = record_def.members.iter().find(|m| m.signature == sub_signature) {
                if !subrec_def.fields.is_empty() {
                    let result = decode_fields_from_def(&subrec_def.fields, data, is_localized);
                    if !result.is_empty() {
                        return result;
                    }
                }
            }
        }
    }

    // Fall back to heuristic decoder
    decode_subrecord_heuristic(sig, data, record_sig, is_localized)
}

/// Decode a slice of field definitions against raw data bytes.
/// Returns the decoded display string, or empty string on failure.
fn decode_fields_from_def(fields: &[FieldDef], data: &[u8], is_localized: bool) -> String {
    let mut offset: usize = 0;
    let mut parts: Vec<String> = Vec::new();

    for field in fields {
        if offset >= data.len() {
            break;
        }
        let remaining = &data[offset..];
        match decode_field_type(&field.field_type, remaining, is_localized) {
            Some((display, consumed)) => {
                if !field.name.is_empty() {
                    parts.push(format!("{}: {}", field.name, display));
                } else {
                    parts.push(display);
                }
                offset += consumed;
            }
            None => {
                // Could not decode this field — return what we have so far
                // plus hex for the remainder
                if !remaining.is_empty() {
                    parts.push(format_hex_fallback(remaining));
                }
                break;
            }
        }
    }

    if parts.is_empty() {
        String::new()
    } else if parts.len() == 1 {
        // Single field: return its value directly (without field name prefix for cleaner display)
        // Re-decode to get just the value
        if fields.len() == 1 {
            if let Some((display, _)) = decode_field_type(&fields[0].field_type, data, is_localized) {
                return display;
            }
        }
        parts.remove(0)
    } else {
        parts.join(" | ")
    }
}

/// Decode a single field type from raw data at the current position.
/// Returns (display_string, bytes_consumed) or None on failure.
fn decode_field_type(field_type: &FieldType, data: &[u8], is_localized: bool) -> Option<(String, usize)> {
    match field_type {
        FieldType::String => {
            // Localized plugins store string references as 4-byte u32 indices
            // into external .STRINGS/.DLSTRINGS/.ILSTRINGS files instead of
            // inline text. Decoding these 4 bytes as UTF-8 produces garbled text.
            if is_localized && data.len() == 4 {
                let index = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                return Some((format!("String Index: 0x{:08X}", index), 4));
            }
            let s = decode_null_terminated_string(data);
            // Consume up to and including the null terminator, or all data
            let consumed = match data.iter().position(|&b| b == 0) {
                Some(pos) => pos + 1,
                None => data.len(),
            };
            Some((s, consumed))
        }
        FieldType::LenString => {
            // Length-prefixed string: first 2 bytes are u16 length
            if data.len() < 2 {
                return None;
            }
            let len = u16::from_le_bytes([data[0], data[1]]) as usize;
            let consumed = 2 + len;
            if consumed > data.len() {
                return None;
            }
            let s = match std::str::from_utf8(&data[2..2 + len]) {
                Ok(s) => s.trim_end_matches('\0').to_string(),
                Err(_) => String::from_utf8_lossy(&data[2..2 + len]).into_owned(),
            };
            Some((s, consumed))
        }
        FieldType::Integer { size, enum_def, flags_def } => {
            let byte_size = size.byte_size();
            if data.len() < byte_size {
                return None;
            }
            let (display, raw_val) = decode_integer(size, &data[..byte_size]);

            // Check enum first, then flags
            let display = if let Some(enum_def) = enum_def {
                lookup_enum(raw_val, enum_def).unwrap_or(display)
            } else if let Some(flags_def) = flags_def {
                format_flags(raw_val as u64, flags_def)
            } else {
                display
            };

            Some((display, byte_size))
        }
        FieldType::Float => {
            if data.len() < 4 {
                return None;
            }
            let val = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some((format!("{:.6}", val), 4))
        }
        FieldType::HalfFloat => {
            if data.len() < 2 {
                return None;
            }
            let bits = u16::from_le_bytes([data[0], data[1]]);
            let val = half_to_f32(bits);
            Some((format!("{:.6}", val), 2))
        }
        FieldType::FormId { .. } => {
            if data.len() < 4 {
                return None;
            }
            let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            Some((format!("[{:08X}]", val), 4))
        }
        FieldType::Struct { fields, .. } => {
            let mut offset: usize = 0;
            let mut parts: Vec<String> = Vec::new();
            for field in fields {
                if offset >= data.len() {
                    break;
                }
                match decode_field_type(&field.field_type, &data[offset..], is_localized) {
                    Some((display, consumed)) => {
                        if !field.name.is_empty() {
                            parts.push(format!("{}: {}", field.name, display));
                        } else {
                            parts.push(display);
                        }
                        offset += consumed;
                    }
                    None => break,
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some((parts.join(" | "), offset))
            }
        }
        FieldType::Array { element, count, .. } => {
            let mut offset: usize = 0;
            let mut parts: Vec<String> = Vec::new();
            let max_count = if *count > 0 { *count } else { usize::MAX };
            let mut i = 0;
            while offset < data.len() && i < max_count {
                match decode_field_type(&element.field_type, &data[offset..], is_localized) {
                    Some((display, consumed)) => {
                        if consumed == 0 {
                            break; // prevent infinite loop
                        }
                        parts.push(display);
                        offset += consumed;
                        i += 1;
                    }
                    None => break,
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some((parts.join(", "), offset))
            }
        }
        FieldType::ByteArray { size } => {
            let byte_count = if *size > 0 { (*size).min(data.len()) } else { data.len() };
            let hex = format_hex_fallback(&data[..byte_count]);
            Some((hex, byte_count))
        }
        FieldType::Flags(flag_def) => {
            // Flags are typically stored as u32
            if data.len() < 4 {
                if data.len() >= 2 {
                    let val = u16::from_le_bytes([data[0], data[1]]) as u64;
                    return Some((format_flags(val, flag_def), 2));
                } else if data.len() >= 1 {
                    return Some((format_flags(data[0] as u64, flag_def), 1));
                }
                return None;
            }
            let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as u64;
            Some((format_flags(val, flag_def), 4))
        }
        FieldType::Enum(enum_def) => {
            // Enums are typically stored as u32
            if data.len() < 4 {
                if data.len() >= 2 {
                    let val = u16::from_le_bytes([data[0], data[1]]) as i64;
                    return Some((lookup_enum(val, enum_def).unwrap_or_else(|| format!("{}", val)), 2));
                } else if data.len() >= 1 {
                    let val = data[0] as i64;
                    return Some((lookup_enum(val, enum_def).unwrap_or_else(|| format!("{}", val)), 1));
                }
                return None;
            }
            let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as i64;
            Some((lookup_enum(val, enum_def).unwrap_or_else(|| format!("{}", val)), 4))
        }
        FieldType::Union { members } => {
            // Try each member type until one succeeds
            for member in members {
                if let Some(result) = decode_field_type(&member.field_type, data, is_localized) {
                    return Some(result);
                }
            }
            None
        }
        FieldType::Unknown => {
            // Fall through — caller should use heuristic
            None
        }
    }
}

/// Decode an integer of the given size from raw bytes.
/// Returns (display_string, raw_signed_value).
fn decode_integer(size: &IntegerSize, data: &[u8]) -> (String, i64) {
    match size {
        IntegerSize::U8 => {
            let v = data[0] as u8;
            (format!("{}", v), v as i64)
        }
        IntegerSize::S8 => {
            let v = data[0] as i8;
            (format!("{}", v), v as i64)
        }
        IntegerSize::U16 => {
            let v = u16::from_le_bytes([data[0], data[1]]);
            (format!("{}", v), v as i64)
        }
        IntegerSize::S16 => {
            let v = i16::from_le_bytes([data[0], data[1]]);
            (format!("{}", v), v as i64)
        }
        IntegerSize::U32 => {
            let v = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            (format!("{}", v), v as i64)
        }
        IntegerSize::S32 => {
            let v = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            (format!("{}", v), v as i64)
        }
        IntegerSize::U64 => {
            let v = u64::from_le_bytes([
                data[0], data[1], data[2], data[3],
                data[4], data[5], data[6], data[7],
            ]);
            (format!("{}", v), v as i64)
        }
        IntegerSize::S64 => {
            let v = i64::from_le_bytes([
                data[0], data[1], data[2], data[3],
                data[4], data[5], data[6], data[7],
            ]);
            (format!("{}", v), v)
        }
    }
}

/// Look up an integer value in an enum definition.
fn lookup_enum(val: i64, enum_def: &EnumDef) -> Option<String> {
    enum_def.values.iter()
        .find(|(k, _)| *k == val)
        .map(|(_, name)| name.clone())
}

/// Format a bitmask value using flag definitions.
fn format_flags(val: u64, flag_def: &FlagDef) -> String {
    if val == 0 {
        return "0x00000000".to_string();
    }
    let mut set_flags: Vec<&str> = Vec::new();
    for (bit, name) in &flag_def.flags {
        if val & bit != 0 {
            set_flags.push(name);
        }
    }
    if set_flags.is_empty() {
        format!("0x{:08X}", val)
    } else {
        set_flags.join(", ")
    }
}

/// Convert IEEE 754 half-precision (16-bit) float to f32.
fn half_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) & 1) as u32;
    let exponent = ((bits >> 10) & 0x1F) as u32;
    let mantissa = (bits & 0x3FF) as u32;

    if exponent == 0 {
        if mantissa == 0 {
            // Zero
            f32::from_bits(sign << 31)
        } else {
            // Subnormal
            let val = (mantissa as f32) / 1024.0 * 2.0f32.powi(-14);
            if sign == 1 { -val } else { val }
        }
    } else if exponent == 31 {
        if mantissa == 0 {
            f32::from_bits((sign << 31) | 0x7F800000) // Infinity
        } else {
            f32::NAN
        }
    } else {
        // Normal
        let f32_exp = (exponent as i32 - 15 + 127) as u32;
        let f32_mantissa = mantissa << 13;
        f32::from_bits((sign << 31) | (f32_exp << 23) | f32_mantissa)
    }
}

/// Heuristic subrecord decoder — fallback when no definition is available.
/// Decodes based on the 4-char signature type using hardcoded patterns.
fn decode_subrecord_heuristic(sig: &str, data: &[u8], record_sig: &str, is_localized: bool) -> String {
    // Localized string subrecords: FULL, DESC, and other localizable sigs
    // store a u32 string table index instead of inline text when the plugin
    // has the LOCALIZED flag set.
    const LOCALIZABLE_SIGS: &[&str] = &[
        "FULL", "DESC", "NNAM", "SHRT", "TNAM", "ITXT", "RNAM",
    ];
    if is_localized && data.len() == 4 && LOCALIZABLE_SIGS.contains(&sig) {
        let index = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        return format!("String Index: 0x{:08X}", index);
    }

    // String subrecords (null-terminated UTF-8 or Windows-1252)
    match sig {
        "EDID" | "FULL" | "DESC" | "NNAM" | "TNAM" | "ANAM" | "MNAM" | "ONAM"
        | "TX00" | "TX01" | "TX02" | "TX03" | "TX04" | "TX05" | "TX06" | "TX07"
        | "MODL" | "MOD2" | "MOD3" | "MOD4" | "MOD5" | "ICON" | "MICO" | "ICO2"
        | "MIC2" | "NIFN" | "XATO" | "SHRT" => {
            return decode_null_terminated_string(data);
        }
        // These are strings when the data length suggests it (> 4 bytes or has null terminator)
        "DNAM" | "RNAM" => {
            if data.len() > 4 || (!data.is_empty() && data.last() == Some(&0)) {
                if let Some(s) = try_null_terminated_string(data) {
                    if !s.is_empty() {
                        return s;
                    }
                }
            }
            // Otherwise fall through to other decoders
        }
        // FormID-like when exactly 4 bytes, otherwise try as string
        "INAM" | "CNAM" | "SNAM" | "YNAM" | "ZNAM" | "BNAM" => {
            if data.len() == 4 {
                let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                return format!("[{:08X}]", val);
            }
            return decode_null_terminated_string(data);
        }
        // Always FormID references (4 bytes)
        "SCRI" | "EITM" | "BIDS" | "BAMT" | "ETYP" | "TPLT" | "WNAM" => {
            if data.len() == 4 {
                let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                return format!("[{:08X}]", val);
            }
        }
        // Float subrecords
        "XSCL" | "FLTV" => {
            if data.len() == 4 {
                let val = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                return format!("{:.6}", val);
            }
        }
        // Integer subrecords
        "INTV" => {
            if data.len() == 4 {
                let val = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                return format!("{}", val);
            } else if data.len() == 2 {
                let val = i16::from_le_bytes([data[0], data[1]]);
                return format!("{}", val);
            }
        }
        // DATA: context-dependent
        "DATA" => {
            if data.len() == 4 {
                // Record types where DATA is known to be a struct, not a simple value
                let struct_data_records = [
                    "NPC_", "WEAP", "ARMO", "AMMO", "BOOK", "INGR", "ALCH",
                    "MISC", "LIGH", "CELL", "REFR", "ACHR", "FACT", "RACE",
                    "MGEF", "ENCH", "SPEL", "DIAL", "QUST",
                ];
                if !struct_data_records.contains(&record_sig) {
                    // Try as f32 first
                    let fval = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                    if fval.is_finite() && !fval.is_subnormal() && fval != 0.0 {
                        return format!("{:.6}", fval);
                    }
                    // Otherwise as u32
                    let uval = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                    return format!("{}", uval);
                }
            }
        }
        _ => {}
    }

    // Fallback: hex dump
    format_hex_fallback(data)
}

/// Decode null-terminated string data. Try UTF-8 first, fall back to lossy.
fn decode_null_terminated_string(data: &[u8]) -> String {
    // Strip trailing null(s)
    let trimmed = match data.iter().position(|&b| b == 0) {
        Some(pos) => &data[..pos],
        None => data,
    };
    if trimmed.is_empty() {
        return String::new();
    }
    match std::str::from_utf8(trimmed) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(trimmed).into_owned(),
    }
}

/// Try to decode as null-terminated string; returns None if data doesn't look like a string.
fn try_null_terminated_string(data: &[u8]) -> Option<String> {
    let trimmed = match data.iter().position(|&b| b == 0) {
        Some(pos) => &data[..pos],
        None => data,
    };
    if trimmed.is_empty() {
        return Some(String::new());
    }
    // Check if it looks like printable text (at least mostly ASCII/printable)
    let printable_count = trimmed.iter().filter(|&&b| b >= 0x20 && b < 0x7F).count();
    if printable_count * 2 >= trimmed.len() {
        // More than half printable — likely a string
        match std::str::from_utf8(trimmed) {
            Ok(s) => Some(s.to_string()),
            Err(_) => Some(String::from_utf8_lossy(trimmed).into_owned()),
        }
    } else {
        None
    }
}

// ============================================================================
// Per-record and per-subrecord conflict status FFI
// ============================================================================

/// Collect all subrecord raw data concatenated by signature for a record.
/// Returns a vec of (Signature, concatenated_bytes) in order of first appearance.
fn collect_subrecord_data_ordered(record: &xedit_dom::Record) -> Vec<(xedit_dom::Signature, Vec<u8>)> {
    let mut result: Vec<(xedit_dom::Signature, Vec<u8>)> = Vec::new();
    for sr in &record.subrecords {
        if let Some(entry) = result.iter_mut().find(|(sig, _)| *sig == sr.signature) {
            entry.1.extend_from_slice(&sr.raw_data);
        } else {
            result.push((sr.signature, sr.raw_data.clone()));
        }
    }
    result
}

/// Helper: find all records matching a FormID across all plugins, returning
/// (plugin_index, &Record) pairs in load order.
fn find_records_for_form_id(engine: &XEditEngine, form_id: u32) -> Vec<(usize, &xedit_dom::Record)> {
    let mut results = Vec::new();
    for (pi, plugin) in engine.plugins.iter().enumerate() {
        for group in &plugin.groups {
            let records = collect_records_from_group(group);
            for record in records {
                if record.form_id.0 == form_id {
                    results.push((pi, record));
                }
            }
        }
    }
    results
}

/// Get conflict status for a record across all overriding plugins.
///
/// Buffer format: [i32 conflict_all | i32 plugin_count | i32 conflict_this_0 | i32 conflict_this_1 | ...]
///
/// conflict_all values: 0=Unknown, 1=OnlyOne, 2=NoConflict, 3=ConflictBenign, 4=Override, 5=Conflict, 6=ConflictCritical
/// conflict_this values: 0=Unknown, 1=Ignored, 2=NotDefined, 3=IdenticalToMaster, 4=OnlyOne,
///                       5=HiddenByModGroup, 6=Master, 7=ConflictBenign, 8=Override,
///                       9=IdenticalToMasterWinsConflict, 10=ConflictWins, 11=ConflictLoses
#[no_mangle]
pub extern "C" fn xedit_record_conflict_status(
    form_id: u32,
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

        let entries = find_records_for_form_id(engine, form_id);
        let plugin_count = entries.len() as i32;

        // Need: 4 (conflict_all) + 4 (plugin_count) + 4 * plugin_count (conflict_this per plugin)
        let required = 8 + entries.len() * 4;
        if (buf_len as usize) < required {
            return XEDIT_ERR_BUFFER_TOO_SMALL;
        }

        if entries.is_empty() {
            // No records found — write Unknown(0) and count 0
            let zero: i32 = 0;
            unsafe {
                std::ptr::copy_nonoverlapping(&zero as *const i32 as *const u8, buf, 4);
                std::ptr::copy_nonoverlapping(&zero as *const i32 as *const u8, buf.add(4), 4);
            }
            return XEDIT_OK;
        }

        // Collect concatenated subrecord bytes per plugin for whole-record comparison
        let record_bytes: Vec<Vec<u8>> = entries.iter().map(|(_, record)| {
            let mut bytes = Vec::new();
            for sr in &record.subrecords {
                bytes.extend_from_slice(&sr.signature.0);
                bytes.extend_from_slice(&sr.raw_data);
            }
            bytes
        }).collect();

        // Determine conflict_all based on number of unique byte representations
        let mut unique_sets: Vec<&Vec<u8>> = Vec::new();
        for rb in &record_bytes {
            if !unique_sets.iter().any(|u| *u == rb) {
                unique_sets.push(rb);
            }
        }

        let conflict_all: i32 = if entries.len() == 1 {
            1 // OnlyOne
        } else {
            match unique_sets.len() {
                1 => 2, // NoConflict — all identical
                2 => 4, // Override — two distinct versions
                _ => 5, // Conflict — 3+ distinct versions
            }
        };

        // Determine conflict_this for each plugin
        let mut conflict_this: Vec<i32> = Vec::with_capacity(entries.len());

        if entries.len() == 1 {
            conflict_this.push(4); // OnlyOne
        } else {
            let master_bytes = &record_bytes[0];
            let winner_bytes = &record_bytes[record_bytes.len() - 1];

            for (i, rb) in record_bytes.iter().enumerate() {
                if i == 0 {
                    conflict_this.push(6); // Master
                } else if rb == master_bytes {
                    conflict_this.push(3); // IdenticalToMaster
                } else if rb == winner_bytes {
                    conflict_this.push(10); // ConflictWins
                } else {
                    conflict_this.push(11); // ConflictLoses
                }
            }
        }

        // Write buffer: conflict_all, plugin_count, then conflict_this values
        unsafe {
            std::ptr::copy_nonoverlapping(
                &conflict_all as *const i32 as *const u8,
                buf,
                4,
            );
            std::ptr::copy_nonoverlapping(
                &plugin_count as *const i32 as *const u8,
                buf.add(4),
                4,
            );
            for (i, ct) in conflict_this.iter().enumerate() {
                std::ptr::copy_nonoverlapping(
                    ct as *const i32 as *const u8,
                    buf.add(8 + i * 4),
                    4,
                );
            }
        }

        XEDIT_OK
    })
}

/// Get per-subrecord conflict status across all overriding plugins.
///
/// Buffer format: [i32 sub_count | for each sub: [4-byte sig | i32 conflict_all | i32 n_plugins | i32 ct_0 | ct_1 | ...]]
///
/// conflict_all values: 0=Unknown, 2=NoConflict, 5=Conflict
/// conflict_this values: 0=Unknown, 2=NotDefined, 3=IdenticalToMaster, 6=Master, 10=ConflictWins, 11=ConflictLoses
#[no_mangle]
pub extern "C" fn xedit_subrecord_conflict_status(
    form_id: u32,
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

        let entries = find_records_for_form_id(engine, form_id);
        let n_plugins = entries.len();

        if entries.is_empty() {
            // Write sub_count = 0
            if buf_len < 4 {
                return XEDIT_ERR_BUFFER_TOO_SMALL;
            }
            let zero: i32 = 0;
            unsafe {
                std::ptr::copy_nonoverlapping(&zero as *const i32 as *const u8, buf, 4);
            }
            return XEDIT_OK;
        }

        // Collect subrecord data per plugin: Vec<Vec<(Signature, data)>>
        let plugin_subs: Vec<Vec<(xedit_dom::Signature, Vec<u8>)>> = entries
            .iter()
            .map(|(_, record)| collect_subrecord_data_ordered(record))
            .collect();

        // Gather all unique signatures in order of first appearance across all plugins
        let mut all_sigs: Vec<xedit_dom::Signature> = Vec::new();
        let mut seen_sigs: HashSet<[u8; 4]> = HashSet::new();
        for subs in &plugin_subs {
            for (sig, _) in subs {
                if seen_sigs.insert(sig.0) {
                    all_sigs.push(*sig);
                }
            }
        }

        let sub_count = all_sigs.len() as i32;

        // Calculate required buffer size:
        // 4 (sub_count) + for each sig: 4 (sig) + 4 (conflict_all) + 4 (n_plugins) + 4 * n_plugins (conflict_this)
        let per_sub_size = 4 + 4 + 4 + n_plugins * 4; // sig + conflict_all + n_plugins + ct values
        let required = 4 + all_sigs.len() * per_sub_size;
        if (buf_len as usize) < required {
            // Write sub_count so caller knows how much space is needed
            if buf_len >= 4 {
                unsafe {
                    std::ptr::copy_nonoverlapping(&sub_count as *const i32 as *const u8, buf, 4);
                }
            }
            return XEDIT_ERR_BUFFER_TOO_SMALL;
        }

        // Write sub_count
        let mut offset = 0usize;
        unsafe {
            std::ptr::copy_nonoverlapping(&sub_count as *const i32 as *const u8, buf, 4);
        }
        offset += 4;

        let n_plugins_i32 = n_plugins as i32;

        for sig in &all_sigs {
            // For each plugin, find the data for this signature (or None if missing)
            let per_plugin_data: Vec<Option<&Vec<u8>>> = plugin_subs
                .iter()
                .map(|subs| {
                    subs.iter()
                        .find(|(s, _)| s == sig)
                        .map(|(_, data)| data)
                })
                .collect();

            // Determine conflict_all: compare all present data
            let present_data: Vec<&Vec<u8>> = per_plugin_data.iter().filter_map(|d| *d).collect();

            let all_same = if present_data.len() <= 1 {
                true
            } else {
                present_data[1..].iter().all(|d| *d == present_data[0])
            };

            // Also check if any plugin is missing this subrecord
            let any_missing = per_plugin_data.iter().any(|d| d.is_none());

            let conflict_all: i32 = if all_same && !any_missing {
                2 // NoConflict
            } else {
                5 // Conflict
            };

            // Determine conflict_this for each plugin
            let master_data = per_plugin_data[0];
            let winner_data = per_plugin_data[n_plugins - 1];

            let mut conflict_this: Vec<i32> = Vec::with_capacity(n_plugins);
            for (i, data) in per_plugin_data.iter().enumerate() {
                match data {
                    None => {
                        conflict_this.push(2); // NotDefined
                    }
                    Some(d) => {
                        if n_plugins == 1 {
                            conflict_this.push(6); // Master (only one)
                        } else if i == 0 {
                            conflict_this.push(6); // Master
                        } else if master_data.is_some() && *d == master_data.unwrap() {
                            conflict_this.push(3); // IdenticalToMaster
                        } else if winner_data.is_some() && *d == winner_data.unwrap() {
                            conflict_this.push(10); // ConflictWins
                        } else {
                            conflict_this.push(11); // ConflictLoses
                        }
                    }
                }
            }

            // Write: 4-byte sig, i32 conflict_all, i32 n_plugins, then conflict_this values
            unsafe {
                // Signature bytes
                std::ptr::copy_nonoverlapping(sig.0.as_ptr(), buf.add(offset), 4);
                offset += 4;
                // conflict_all
                std::ptr::copy_nonoverlapping(
                    &conflict_all as *const i32 as *const u8,
                    buf.add(offset),
                    4,
                );
                offset += 4;
                // n_plugins
                std::ptr::copy_nonoverlapping(
                    &n_plugins_i32 as *const i32 as *const u8,
                    buf.add(offset),
                    4,
                );
                offset += 4;
                // conflict_this per plugin
                for ct in &conflict_this {
                    std::ptr::copy_nonoverlapping(
                        ct as *const i32 as *const u8,
                        buf.add(offset),
                        4,
                    );
                    offset += 4;
                }
            }
        }

        XEDIT_OK
    })
}

// ============================================================================
// Record/Subrecord mutation operations
// ============================================================================

/// Replace a subrecord's raw data with new data.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn xedit_set_subrecord_data(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
    sub_idx: i32,
    data: *const u8,
    data_len: i32,
) -> i32 {
    catch_panic(|| {
        if data.is_null() || data_len < 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let new_data = unsafe { std::slice::from_raw_parts(data, data_len as usize) }.to_vec();

        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin_mut(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        plugin.modified = true;
        let group = match get_group_mut(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let mut records = collect_records_from_group_mut(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let record = &mut records[record_idx as usize];
        if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
            // If subrecords are offloaded (vec empty), update the DB directly
            let db_path = match get_plugin_db_path(plugin_idx) {
                Some(p) => p,
                None => return XEDIT_ERR_NULL_HANDLE,
            };
            let conn = match Connection::open(&db_path) {
                Ok(c) => c,
                Err(_) => return XEDIT_ERR_LOAD_FAILED,
            };
            let updated = match conn.execute(
                "UPDATE subrecords SET raw_data=?1 WHERE group_idx=?2 AND record_idx=?3 AND sub_idx=?4",
                params![new_data, group_idx, record_idx, sub_idx],
            ) {
                Ok(n) => n,
                Err(_) => return XEDIT_ERR_SAVE_FAILED,
            };
            if updated == 0 {
                return XEDIT_ERR_NULL_HANDLE;
            }
            record.modified = true;
            return XEDIT_OK;
        }
        let subrecord = &mut record.subrecords[sub_idx as usize];
        subrecord.size = new_data.len() as u32;
        subrecord.raw_data = new_data.clone();
        subrecord.modified = true;
        record.modified = true;

        // If subrecord data has been offloaded to SQLite, also update the DB entry
        if let Some(db_path) = get_plugin_db_path(plugin_idx) {
            if let Ok(conn) = Connection::open(&db_path) {
                let _ = conn.execute(
                    "UPDATE subrecords SET raw_data=?1 WHERE group_idx=?2 AND record_idx=?3 AND sub_idx=?4",
                    params![new_data, group_idx, record_idx, sub_idx],
                );
            }
        }

        XEDIT_OK
    })
}

/// Mark a record as deleted (sets the deleted flag in the record header).
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn xedit_delete_record(
    plugin_idx: i32,
    group_idx: i32,
    record_idx: i32,
) -> i32 {
    catch_panic(|| {
        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin_mut(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        plugin.modified = true;
        let group = match get_group_mut(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let mut records = collect_records_from_group_mut(group);
        if record_idx < 0 || record_idx as usize >= records.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let record = &mut records[record_idx as usize];
        use xedit_dom::record::RecordFlags;
        record.flags = RecordFlags(record.flags.0 | RecordFlags::DELETED);
        record.modified = true;
        XEDIT_OK
    })
}

/// Copy a record from one plugin to another.
/// Returns the new record index in the destination plugin's group, or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_copy_record(
    src_plugin_idx: i32,
    src_group_idx: i32,
    src_record_idx: i32,
    dst_plugin_idx: i32,
) -> i32 {
    catch_panic(|| {
        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        // First, clone the source record (requires immutable borrow of source plugin)
        let (cloned_record, group_sig) = {
            let src_plugin = match get_plugin(engine, src_plugin_idx) {
                Some(p) => p,
                None => return XEDIT_ERR_NULL_HANDLE,
            };
            let src_group = match get_group(src_plugin, src_group_idx) {
                Some(g) => g,
                None => return XEDIT_ERR_NULL_HANDLE,
            };
            let src_records = collect_records_from_group(src_group);
            if src_record_idx < 0 || src_record_idx as usize >= src_records.len() {
                return XEDIT_ERR_NULL_HANDLE;
            }
            let src_record = src_records[src_record_idx as usize];

            // If subrecords were offloaded, reload them from the DB before cloning
            let mut cloned = src_record.clone();
            if cloned.subrecords.is_empty() {
                if let Some(db_path) = get_plugin_db_path(src_plugin_idx) {
                    if let Ok(conn) = Connection::open(&db_path) {
                        if let Ok(mut stmt) = conn.prepare(
                            "SELECT sub_idx, signature, raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 ORDER BY sub_idx"
                        ) {
                            if let Ok(rows) = stmt.query_map(
                                params![src_group_idx, src_record_idx],
                                |row| {
                                    let sig_str: String = row.get(1)?;
                                    let raw_data: Vec<u8> = row.get(2)?;
                                    Ok((sig_str, raw_data))
                                },
                            ) {
                                for row_result in rows {
                                    if let Ok((sig_str, raw_data)) = row_result {
                                        let sig_bytes = sig_str.as_bytes();
                                        let mut sig_arr = [0u8; 4];
                                        let copy_len = sig_bytes.len().min(4);
                                        sig_arr[..copy_len].copy_from_slice(&sig_bytes[..copy_len]);
                                        cloned.subrecords.push(Subrecord::new(
                                            Signature::from_bytes(&sig_arr),
                                            raw_data,
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            cloned.modified = true;
            // Clear source-specific fields since this is a new copy
            cloned.source_offset = None;
            cloned.raw_header = None;
            cloned.raw_compressed_data = None;
            cloned.raw_data = None;

            let group_sig = match src_group.group_type {
                GroupType::Top(sig) => sig,
                _ => cloned.signature,
            };

            (cloned, group_sig)
        };

        // Now mutably access the destination plugin
        let dst_plugin = match get_plugin_mut(engine, dst_plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        dst_plugin.modified = true;

        // Find or create the matching top-level group in the destination
        let dst_group_pos = dst_plugin.groups.iter().position(|g| {
            matches!(g.group_type, GroupType::Top(sig) if sig == group_sig)
        });

        let dst_group = if let Some(idx) = dst_group_pos {
            &mut dst_plugin.groups[idx]
        } else {
            // Create a new top-level group
            dst_plugin.groups.push(Group {
                group_type: GroupType::Top(group_sig),
                stamp: 0,
                unknown: 0,
                children: Vec::new(),
                raw_header: None,
                source_offset: None,
            });
            dst_plugin.groups.last_mut().unwrap()
        };

        // Count existing records to determine the new index
        let new_idx = collect_records_from_group(dst_group).len();
        dst_group.children.push(GroupChild::Record(cloned_record));
        new_idx as i32
    })
}

/// Create a new empty record in a plugin's group.
/// Returns the new record index, or -1 on error.
#[no_mangle]
pub extern "C" fn xedit_add_record(
    plugin_idx: i32,
    group_idx: i32,
    form_id: u32,
    signature: *const u8,
) -> i32 {
    catch_panic(|| {
        if signature.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let sig_bytes = unsafe { std::slice::from_raw_parts(signature, 4) };
        let mut sig_arr = [0u8; 4];
        sig_arr.copy_from_slice(sig_bytes);
        let sig = Signature::from_bytes(&sig_arr);

        let mut lock = ENGINE.lock().unwrap();
        let engine = match lock.as_mut() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        let plugin = match get_plugin_mut(engine, plugin_idx) {
            Some(p) => p,
            None => return XEDIT_ERR_NULL_HANDLE,
        };
        plugin.modified = true;
        let group = match get_group_mut(plugin, group_idx) {
            Some(g) => g,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        // Count existing records to determine the new index
        let new_idx = collect_records_from_group(group).len();

        let new_record = Record {
            signature: sig,
            flags: xedit_dom::record::RecordFlags::NONE,
            form_id: FormId(form_id),
            vc_info: 0,
            version: 0,
            unknown: 0,
            subrecords: Vec::new(),
            raw_header: None,
            raw_compressed_data: None,
            raw_data: None,
            source_offset: None,
            modified: true,
        };

        group.children.push(GroupChild::Record(new_record));
        new_idx as i32
    })
}

// ============================================================================
// LOD Generation
// ============================================================================

/// Global LOD generation job state.
/// None = no job, Some = job in progress or completed.
static LOD_JOB: Mutex<Option<LodJobState>> = Mutex::new(None);
static LOD_CANCEL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct LodJobState {
    status: i32,    // 0=running, 1=done, -1=error
    error: String,
}

/// Scan loaded plugins for LOD-relevant STAT/TREE bases and REFR placements.
/// Reads subrecord data from SQLite offload DBs (in-memory subrecords are cleared after offload).
fn scan_lod_references_from_dbs(
    engine: &XEditEngine,
    db_paths: Option<&Vec<PathBuf>>,
) -> anyhow::Result<(Vec<xedit_lod::LodReference>, std::collections::HashMap<u32, xedit_lod::LodBase>, std::collections::HashMap<String, u32>)> {
    use xedit_lod::{LodBase, LodReference};

    let mut all_bases: std::collections::HashMap<u32, LodBase> = std::collections::HashMap::new();
    let mut all_refs: Vec<LodReference> = Vec::new();

    // Record flag constants (used in collect_wrld_refs_recursive)
    let _ = 0x0000_0020u32; // FLAG_DELETED — used in inner function
    let _ = 0x0000_0800u32; // FLAG_INITIALLY_DISABLED
    let _ = 0x0000_8000u32; // FLAG_VWD
    let _ = 0x0000_0040u32; // FLAG_HAS_TREE_LOD

    let game_id = engine.game_id;
    let is_fnv = matches!(game_id, xedit_dom::GameId::Fallout3 | xedit_dom::GameId::FalloutNV);

    // Signatures for additional base types (FO3/FNV)
    let scol_sig = xedit_dom::Signature::from_bytes(b"SCOL");
    let acti_sig = xedit_dom::Signature::from_bytes(b"ACTI");
    let mstt_sig = xedit_dom::Signature::from_bytes(b"MSTT");
    let tree_sig = xedit_dom::Signature::from_bytes(b"TREE");
    let _xesp_sig = xedit_dom::Signature::from_bytes(b"XESP");

    // Pass 1: Collect all LOD-eligible bases across all plugins
    // FO3/FNV: STAT, SCOL, ACTI, MSTT (all statics, no VWD requirement)
    // Skyrim: STAT, TREE
    for (pi, plugin) in engine.plugins.iter().enumerate() {
        let db_path = db_paths.and_then(|paths| paths.get(pi));
        let conn = db_path.and_then(|p| Connection::open(p).ok());

        for (gi, group) in plugin.groups.iter().enumerate() {
            let group_sig = match group.group_type {
                xedit_dom::group::GroupType::Top(sig) => sig,
                _ => continue,
            };

            let is_lod_group = if is_fnv {
                group_sig == xedit_dom::Signature::STAT
                    || group_sig == scol_sig
                    || group_sig == acti_sig
                    || group_sig == mstt_sig
                    || group_sig == tree_sig
            } else {
                group_sig == xedit_dom::Signature::STAT
                    || group_sig == tree_sig
            };

            if !is_lod_group {
                continue;
            }

            let records = collect_records_from_group(group);
            for (ri, record) in records.iter().enumerate() {
                let form_id = record.form_id.raw();
                let mut lod_models = vec![None; 4];
                let mut full_model = None;
                let mut editor_id = String::new();

                // Try in-memory subrecords first, fall back to SQLite
                if !record.subrecords.is_empty() {
                    for sr in &record.subrecords {
                        parse_base_subrecord(
                            &sr.signature, &sr.raw_data,
                            &mut editor_id, &mut full_model, &mut lod_models,
                        );
                    }
                } else if let Some(ref c) = conn {
                    query_subrecords_and_parse(c, gi as i32, ri as i32, |sig, data| {
                        parse_base_subrecord(sig, data, &mut editor_id, &mut full_model, &mut lod_models);
                    });
                }

                // For FO3/FNV: any base with a MODL is LOD-eligible
                // (LOD mesh = MODL + _lod.nif suffix)
                // For Skyrim: need MNAM LOD models or be a TREE with full_model
                let has_lod = if is_fnv {
                    full_model.is_some()
                } else {
                    lod_models.iter().any(|m| m.is_some()) || full_model.is_some()
                };

                if has_lod {
                    all_bases.insert(form_id, LodBase {
                        form_id,
                        signature: record.signature,
                        editor_id,
                        lod_models,
                        full_model,
                    });
                }
            }
        }
    }

    tracing::info!("Pass 1 complete: {} LOD bases found", all_bases.len());

    // Pass 2: Walk WRLD groups to collect:
    //   - WRLD record editor_id → form_id mappings
    //   - REFR references tagged with their parent worldspace form_id
    let mut worldspace_map: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

    for (pi, plugin) in engine.plugins.iter().enumerate() {
        let db_path = db_paths.and_then(|paths| paths.get(pi));
        let conn = db_path.and_then(|p| Connection::open(p).ok());

        for (gi, group) in plugin.groups.iter().enumerate() {
            let group_sig = match group.group_type {
                xedit_dom::group::GroupType::Top(sig) => sig,
                _ => continue,
            };

            if group_sig != xedit_dom::Signature::WRLD {
                continue;
            }

            // Get the flat record list for SQLite record_idx mapping
            let flat_records = collect_records_from_group(group);

            // First, collect WRLD records' editor IDs
            for (ri, record) in flat_records.iter().enumerate() {
                if record.signature != xedit_dom::Signature::WRLD {
                    continue;
                }
                let mut editor_id = String::new();

                if !record.subrecords.is_empty() {
                    for sr in &record.subrecords {
                        if sr.signature == xedit_dom::Signature::EDID {
                            let len = sr.raw_data.iter().position(|&b| b == 0).unwrap_or(sr.raw_data.len());
                            editor_id = String::from_utf8_lossy(&sr.raw_data[..len]).to_string();
                        }
                    }
                } else if let Some(ref c) = conn {
                    query_subrecords_and_parse(c, gi as i32, ri as i32, |sig, data| {
                        if *sig == xedit_dom::Signature::EDID {
                            let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                            editor_id = String::from_utf8_lossy(&data[..len]).to_string();
                        }
                    });
                }

                if !editor_id.is_empty() {
                    worldspace_map.insert(editor_id.to_lowercase(), record.form_id.raw());
                }
            }

            // Use recursive walker that tracks worldspace_form_id through the hierarchy
            let mut flat_idx: usize = 0;
            collect_wrld_refs_recursive(
                group, conn.as_ref(), gi as i32, pi,
                0, // no worldspace at top WRLD group level
                &mut flat_idx, &all_bases, &mut all_refs,
            );
        }
    }

    tracing::info!(
        "Pass 2 complete: {} worldspace mappings, {} REFR references",
        worldspace_map.len(),
        all_refs.len()
    );

    Ok((all_refs, all_bases, worldspace_map))
}

/// Query subrecords from SQLite and call a callback for each one.
fn query_subrecords_and_parse(
    conn: &Connection,
    group_idx: i32,
    record_idx: i32,
    mut callback: impl FnMut(&xedit_dom::Signature, &[u8]),
) {
    if let Ok(mut stmt) = conn.prepare_cached(
        "SELECT signature, raw_data FROM subrecords WHERE group_idx=?1 AND record_idx=?2 ORDER BY sub_idx"
    ) {
        if let Ok(rows) = stmt.query_map(params![group_idx, record_idx], |row| {
            let sig: String = row.get(0)?;
            let data: Vec<u8> = row.get(1)?;
            Ok((sig, data))
        }) {
            for row in rows.flatten() {
                let sig_bytes: &[u8; 4] = row.0.as_bytes()
                    .get(..4)
                    .and_then(|s| s.try_into().ok())
                    .unwrap_or(b"    ");
                let sig = xedit_dom::Signature::from_bytes(sig_bytes);
                callback(&sig, &row.1);
            }
        }
    }
}

/// Parse a single REFR subrecord for placement data (NAME, DATA, XSCL).
fn parse_refr_subrecord(
    sig: &xedit_dom::Signature,
    data: &[u8],
    base_form_id: &mut u32,
    position: &mut [f32; 3],
    rotation: &mut [f32; 3],
    scale: &mut f32,
    has_xesp: &mut bool,
) {
    if *sig == xedit_dom::Signature::NAME && data.len() >= 4 {
        *base_form_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    } else if *sig == xedit_dom::Signature::DATA && data.len() >= 24 {
        let f = |o: usize| f32::from_le_bytes([data[o], data[o+1], data[o+2], data[o+3]]);
        *position = [f(0), f(4), f(8)];
        *rotation = [f(12), f(16), f(20)];
    } else if *sig == xedit_dom::Signature::from_bytes(b"XSCL") && data.len() >= 4 {
        *scale = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    } else if *sig == xedit_dom::Signature::from_bytes(b"XESP") {
        *has_xesp = true;
    }
}

/// Parse a single subrecord for STAT/TREE base data (EDID, MODL, MNAM).
fn parse_base_subrecord(
    sig: &xedit_dom::Signature,
    data: &[u8],
    editor_id: &mut String,
    full_model: &mut Option<String>,
    lod_models: &mut Vec<Option<String>>,
) {
    if *sig == xedit_dom::Signature::EDID {
        let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        *editor_id = String::from_utf8_lossy(&data[..len]).to_string();
    } else if *sig == xedit_dom::Signature::MODL {
        let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        *full_model = Some(String::from_utf8_lossy(&data[..len]).to_string());
    } else if *sig == xedit_dom::Signature::from_bytes(b"MNAM") {
        // MNAM contains up to 4 LOD model paths as 260-byte fixed-size entries
        // (matching Delphi: wbString(True, 'Level N', 260))
        // Total: 1040 bytes for 4 levels. Each is null-terminated within 260 bytes.
        if data.len() >= 1040 {
            // Fixed 260-byte entries (Skyrim/SSE/FO4)
            for level in 0..4 {
                let start = level * 260;
                let end = start + 260;
                if end <= data.len() {
                    let entry = &data[start..end];
                    let null_pos = entry.iter().position(|&b| b == 0).unwrap_or(260);
                    if null_pos > 0 {
                        let path = String::from_utf8_lossy(&entry[..null_pos]).to_string();
                        if !path.is_empty() {
                            lod_models[level] = Some(path);
                        }
                    }
                }
            }
        } else {
            // Fallback for smaller MNAM data: parse as null-separated strings
            let mut offset = 0;
            let mut level = 0;
            while offset < data.len() && level < 4 {
                let end = data[offset..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|pos| offset + pos)
                    .unwrap_or(data.len());
                if end > offset {
                    let path = String::from_utf8_lossy(&data[offset..end]).to_string();
                    if !path.is_empty() {
                        lod_models[level] = Some(path);
                    }
                }
                offset = end + 1;
                level += 1;
            }
        }
    }
}

/// Recursively walk WRLD group hierarchy, tracking the current worldspace form_id
/// from WorldChildren groups and collecting REFR records with their worldspace tag.
/// `flat_idx` tracks the flat record index for SQLite record_idx queries.
fn collect_wrld_refs_recursive(
    group: &xedit_dom::Group,
    conn: Option<&Connection>,
    group_idx: i32,
    plugin_index: usize,
    worldspace_form_id: u32,
    flat_idx: &mut usize,
    bases: &std::collections::HashMap<u32, xedit_lod::LodBase>,
    refs_out: &mut Vec<xedit_lod::LodReference>,
) {
    use xedit_dom::group::{GroupChild, GroupType};

    // Flag constants for REFR filtering (matching Delphi xEdit)
    const FLAG_DELETED: u32 = 0x0000_0020;
    const FLAG_INITIALLY_DISABLED: u32 = 0x0000_0800;
    const FLAG_VWD: u32 = 0x0000_8000;

    for child in &group.children {
        match child {
            GroupChild::Record(record) => {
                let ri = *flat_idx;
                *flat_idx += 1;

                // Only collect REFR records when we're inside a WorldChildren hierarchy
                if worldspace_form_id == 0 || record.signature != xedit_dom::Signature::REFR {
                    continue;
                }

                let ref_flags = record.flags.0;

                // Skip deleted references (matching Delphi: Flags.IsDeleted)
                if ref_flags & FLAG_DELETED != 0 {
                    continue;
                }

                // Skip initially disabled references (unless VWD)
                if ref_flags & FLAG_INITIALLY_DISABLED != 0
                    && ref_flags & FLAG_VWD == 0
                {
                    continue;
                }

                let form_id = record.form_id.raw();
                let mut base_form_id = 0u32;
                let mut position = [0.0f32; 3];
                let mut rotation = [0.0f32; 3];
                let mut scale = 1.0f32;
                let mut has_xesp = false;

                if !record.subrecords.is_empty() {
                    for sr in &record.subrecords {
                        parse_refr_subrecord(
                            &sr.signature, &sr.raw_data,
                            &mut base_form_id, &mut position, &mut rotation, &mut scale,
                            &mut has_xesp,
                        );
                    }
                } else if let Some(c) = conn {
                    query_subrecords_and_parse(c, group_idx, ri as i32, |sig, data| {
                        parse_refr_subrecord(sig, data, &mut base_form_id, &mut position, &mut rotation, &mut scale, &mut has_xesp);
                    });
                }

                // Skip references with enable parent (XESP) unless VWD flag set
                // Matching Delphi: skip if has XESP and NOT VWD
                if has_xesp && ref_flags & FLAG_VWD == 0 {
                    continue;
                }

                if base_form_id != 0 {
                    if let Some(base) = bases.get(&base_form_id) {
                        refs_out.push(xedit_lod::LodReference {
                            form_id,
                            base_form_id,
                            base_signature: base.signature,
                            position,
                            rotation,
                            scale,
                            plugin_index,
                            worldspace_form_id,
                        });
                    }
                }
            }
            GroupChild::Group(sub_group) => {
                let ws_id = match sub_group.group_type {
                    GroupType::WorldChildren(id) => id,
                    _ => worldspace_form_id,
                };

                collect_wrld_refs_recursive(
                    sub_group, conn, group_idx, plugin_index,
                    ws_id, flat_idx, bases, refs_out,
                );
            }
        }
    }
}

/// List worldspaces that have LOD settings files.
/// Scans WRLD records from loaded plugins, then filters to those with matching
/// lodsettings files (loose or in BSA/BA2 archives).
/// Returns newline-separated worldspace editor IDs in buf.
#[no_mangle]
pub extern "C" fn xedit_lod_list_worldspaces(buf: *mut u8, buf_len: i32) -> i32 {
    catch_panic(|| {
        if buf.is_null() || buf_len <= 0 {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE,
        };

        let db_lock = PLUGIN_DBS.lock().unwrap();
        let db_paths = db_lock.as_ref();

        if matches!(engine.game_id, xedit_dom::GameId::Oblivion | xedit_dom::GameId::Morrowind) {
            // No lodsettings for Oblivion/Morrowind
            unsafe { *buf = 0; }
            return 0;
        }

        tracing::info!("LOD list: game_id = {:?}, data_path = {:?}", engine.game_id, engine.data_path);

        // Lodsettings extensions to try — FO3/FNV use .dlodsettings, others use .lod.
        // TTW (Tale of Two Wastelands) runs as FNV but may have either format,
        // so we try both extensions for robustness.
        let extensions: &[&str] = match engine.game_id {
            xedit_dom::GameId::Fallout3 | xedit_dom::GameId::FalloutNV => &["dlodsettings", "lod"],
            _ => &["lod", "dlodsettings"],
        };

        // Create resource loader for checking file existence
        let loader = match xedit_lod::ResourceLoader::new(&engine.data_path) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to create resource loader: {:#}", e);
                return XEDIT_ERR_LOAD_FAILED;
            }
        };

        // Use the pre-built BSA index to quickly list lodsettings files
        let lodsettings_files = loader.list_files_in_dir("lodsettings");
        tracing::info!(
            "LOD list: {} BSA archives, {} lodsettings files in index",
            loader.bsa_paths().len(),
            lodsettings_files.len()
        );
        for f in lodsettings_files.iter().take(10) {
            tracing::info!("  {}", f);
        }

        // Collect all WRLD editor IDs from loaded plugins, filtered by lodsettings existence
        let mut worldspace_edids: Vec<String> = Vec::new();
        let mut all_wrld_edids: Vec<String> = Vec::new(); // debug: track all found
        let mut seen_form_ids = std::collections::HashSet::new();

        for (pi, plugin) in engine.plugins.iter().enumerate() {
            let db_path = db_paths.and_then(|paths| paths.get(pi));
            let conn = db_path.and_then(|p| Connection::open(p).ok());

            for (gi, group) in plugin.groups.iter().enumerate() {
                let group_sig = match group.group_type {
                    xedit_dom::group::GroupType::Top(sig) => sig,
                    _ => continue,
                };
                if group_sig != xedit_dom::Signature::WRLD {
                    continue;
                }

                // Use flattened record list (matches SQLite offload indices)
                let records = collect_records_from_group(group);
                for (ri, record) in records.iter().enumerate() {
                    if record.signature != xedit_dom::Signature::WRLD {
                        continue;
                    }

                    // Deduplicate by form_id (later plugins override earlier ones)
                    let fid = record.form_id.raw();
                    if !seen_form_ids.insert(fid) {
                        continue;
                    }

                    let mut editor_id = String::new();
                    if !record.subrecords.is_empty() {
                        for sr in &record.subrecords {
                            if sr.signature == xedit_dom::Signature::EDID {
                                let len = sr.raw_data.iter().position(|&b| b == 0).unwrap_or(sr.raw_data.len());
                                editor_id = String::from_utf8_lossy(&sr.raw_data[..len]).to_string();
                            }
                        }
                    } else if let Some(ref c) = conn {
                        query_subrecords_and_parse(c, gi as i32, ri as i32, |sig, data| {
                            if *sig == xedit_dom::Signature::EDID {
                                let len = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                                editor_id = String::from_utf8_lossy(&data[..len]).to_string();
                            }
                        });
                    }

                    if editor_id.is_empty() {
                        continue;
                    }

                    all_wrld_edids.push(editor_id.clone());

                    // Check if lodsettings file exists with any supported extension
                    let has_settings = extensions.iter().any(|ext| {
                        loader.exists(&format!("lodsettings\\{}.{}", editor_id, ext))
                            || loader.exists(&format!("lodsettings/{}.{}", editor_id, ext))
                    });
                    if has_settings {
                        worldspace_edids.push(editor_id);
                    }
                }
            }
        }

        tracing::info!(
            "LOD list: found {} WRLD editor IDs total: {:?}",
            all_wrld_edids.len(),
            &all_wrld_edids[..std::cmp::min(20, all_wrld_edids.len())]
        );

        worldspace_edids.sort_unstable_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        worldspace_edids.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

        tracing::info!(
            "Found {} worldspaces with LOD settings",
            worldspace_edids.len(),
        );

        let result = worldspace_edids.join("\n");
        let result_bytes = result.as_bytes();
        let copy_len = std::cmp::min(result_bytes.len(), (buf_len - 1) as usize);
        unsafe {
            std::ptr::copy_nonoverlapping(result_bytes.as_ptr(), buf, copy_len);
            *buf.add(copy_len) = 0;
        }

        copy_len as i32
    })
}

/// Start async LOD generation with JSON options.
/// options_json: null-terminated UTF-8 JSON string with LodOptions
/// progress_cb: optional progress callback (message, progress_fraction)
/// Returns 0 on success (job started), negative on error.
#[no_mangle]
pub extern "C" fn xedit_lod_generate(
    options_json: *const c_char,
    progress_cb: Option<ProgressCallback>,
) -> i32 {
    catch_panic(|| {
        if options_json.is_null() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let json_str = unsafe { CStr::from_ptr(options_json) }
            .to_str()
            .unwrap_or("");

        let mut options: xedit_lod::LodOptions = match serde_json::from_str(json_str) {
            Ok(o) => o,
            Err(e) => {
                tracing::error!("Failed to parse LOD options JSON: {:#}", e);
                let mut lock = LOD_JOB.lock().unwrap();
                *lock = Some(LodJobState {
                    status: -1,
                    error: format!("Invalid options JSON: {}", e),
                });
                return XEDIT_ERR_LOAD_FAILED;
            }
        };

        // Grab data path and game_id from engine (quick, non-blocking)
        let (data_path, game_id) = {
            let lock = ENGINE.lock().unwrap();
            let engine = match lock.as_ref() {
                Some(e) => e,
                None => return XEDIT_ERR_NULL_HANDLE,
            };
            (engine.data_path.clone(), engine.game_id)
        };

        // Set game-specific lodsettings extension and game ID
        options.lod_extension = match game_id {
            xedit_dom::GameId::Fallout3 | xedit_dom::GameId::FalloutNV => "dlodsettings".to_string(),
            _ => "lod".to_string(),
        };
        options.game_id = match game_id {
            xedit_dom::GameId::Morrowind => "Morrowind".to_string(),
            xedit_dom::GameId::Oblivion => "Oblivion".to_string(),
            xedit_dom::GameId::Fallout3 => "Fallout3".to_string(),
            xedit_dom::GameId::FalloutNV => "FalloutNV".to_string(),
            xedit_dom::GameId::SkyrimSE => "SkyrimSE".to_string(),
            xedit_dom::GameId::Fallout4 => "Fallout4".to_string(),
            xedit_dom::GameId::Starfield => "Starfield".to_string(),
            xedit_dom::GameId::Fallout76 => "Fallout76".to_string(),
        };

        // Reset cancel flag
        LOD_CANCEL.store(false, Ordering::SeqCst);

        // Set job as running
        {
            let mut lock = LOD_JOB.lock().unwrap();
            *lock = Some(LodJobState {
                status: 0,
                error: String::new(),
            });
        }

        // Spawn worker thread — scanning + generation all happens off the UI thread
        std::thread::spawn(move || {
            // Ensure output directory exists
            let _ = std::fs::create_dir_all(&options.output_dir);

            // Single log file next to the executable — captures ALL tracing output
            let log_path = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("LODGen_log.txt")))
                .unwrap_or_else(|| std::path::PathBuf::from("LODGen_log.txt"));

            // Install a tracing subscriber that writes to the log file
            // This captures all tracing::info!/warn!/error! from the LOD pipeline
            use tracing_subscriber::fmt;
            use tracing_subscriber::prelude::*;

            let log_file = std::fs::File::create(&log_path).ok();
            let _guard = if let Some(file) = log_file {
                let file_layer = fmt::layer()
                    .with_writer(std::sync::Mutex::new(file))
                    .with_ansi(false)
                    .with_target(false)
                    .with_level(true)
                    .with_timer(fmt::time::uptime());

                let subscriber = tracing_subscriber::registry().with(file_layer);
                // Set as thread-local default (doesn't affect other threads)
                Some(tracing::subscriber::set_default(subscriber))
            } else {
                None
            };

            tracing::info!("LOD Generation started — output: {}", options.output_dir);
            tracing::info!("Game: {}, Worldspaces: {}", options.game_id, options.worldspaces.join(", "));

            let progress = xedit_lod::progress::Progress::new(
                progress_cb.map(|cb| cb as xedit_lod::progress::ProgressFn),
                &LOD_CANCEL,
            );

            progress.report("Scanning plugins for LOD references...");

            // Scan references from loaded plugins + SQLite DBs
            let (references, bases, worldspace_map) = {
                let lock = ENGINE.lock().unwrap();
                let engine = match lock.as_ref() {
                    Some(e) => e,
                    None => {
                        let mut job = LOD_JOB.lock().unwrap();
                        *job = Some(LodJobState { status: -1, error: "Engine not initialized".into() });
                        return;
                    }
                };

                let db_lock = PLUGIN_DBS.lock().unwrap();
                let db_paths = db_lock.as_ref();

                tracing::info!("Scanning {} plugins for LOD references", engine.plugins.len());

                match scan_lod_references_from_dbs(engine, db_paths) {
                    Ok(result) => result,
                    Err(e) => {
                        let msg = format!("Failed to scan references: {}", e);
                        tracing::error!("{}", msg);
                        let mut job = LOD_JOB.lock().unwrap();
                        *job = Some(LodJobState { status: -1, error: msg });
                        return;
                    }
                }
            };

            tracing::info!(
                "Found {} LOD references, {} LOD bases, {} worldspaces",
                references.len(), bases.len(), worldspace_map.len()
            );

            progress.report(&format!(
                "Found {} references, {} bases. Starting LOD generation...",
                references.len(), bases.len()
            ));

            // Wrap generation in catch_unwind to survive C++ crashes (nifly)
            let gen_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                xedit_lod::generate_lod(&options, &data_path, &references, &bases, &worldspace_map, &progress)
            }));

            let mut lock = LOD_JOB.lock().unwrap();
            match gen_result {
                Ok(Ok(())) => {
                    tracing::info!("LOD generation completed successfully");
                    *lock = Some(LodJobState {
                        status: 1,
                        error: String::new(),
                    });
                }
                Ok(Err(e)) => {
                    let msg = format!("LOD generation failed: {:#}", e);
                    tracing::error!("{}", msg);
                    *lock = Some(LodJobState {
                        status: -1,
                        error: format!("{:#}", e),
                    });
                }
                Err(_panic) => {
                    let msg = "LOD generation crashed (panic in native code). Check LODGen_log.txt for details.";
                    tracing::error!("{}", msg);
                    *lock = Some(LodJobState {
                        status: -1,
                        error: msg.to_string(),
                    });
                }
            }
        });

        XEDIT_OK
    })
}

/// Check LOD generation status.
/// Returns: 0 = running, 1 = done successfully, -1 = error, -2 = no job.
#[no_mangle]
pub extern "C" fn xedit_lod_status() -> i32 {
    let lock = LOD_JOB.lock().unwrap();
    match lock.as_ref() {
        Some(job) => job.status,
        None => -2,
    }
}

/// Get LOD generation error message.
/// Returns length of error string, or 0 if no error.
#[no_mangle]
pub extern "C" fn xedit_lod_error(buf: *mut u8, buf_len: i32) -> i32 {
    if buf.is_null() || buf_len <= 0 {
        return 0;
    }

    let lock = LOD_JOB.lock().unwrap();
    let error = match lock.as_ref() {
        Some(job) if !job.error.is_empty() => &job.error,
        _ => {
            unsafe { *buf = 0; }
            return 0;
        }
    };

    let error_bytes = error.as_bytes();
    let copy_len = std::cmp::min(error_bytes.len(), (buf_len - 1) as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(error_bytes.as_ptr(), buf, copy_len);
        *buf.add(copy_len) = 0;
    }
    copy_len as i32
}

/// Cancel the in-progress LOD generation job.
/// Returns 0 if cancellation was requested, -2 if no job is running.
#[no_mangle]
pub extern "C" fn xedit_lod_cancel() -> i32 {
    let lock = LOD_JOB.lock().unwrap();
    match lock.as_ref() {
        Some(job) if job.status == 0 => {
            LOD_CANCEL.store(true, Ordering::SeqCst);
            XEDIT_OK
        }
        _ => -2,
    }
}
