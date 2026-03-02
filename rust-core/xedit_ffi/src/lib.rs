//! C FFI bridge for Pascal/Lazarus GUI to call Rust xEdit core.
//!
//! All functions use `extern "C"` calling convention with opaque handles.
//! Panics are caught and converted to error codes.
//! Strings are passed as null-terminated UTF-8 C strings.

use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

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

/// Cross-reference index: maps target FormID -> list of (plugin_idx, group_idx, record_idx)
/// entries that reference that FormID from their subrecord data.
static REFBY_INDEX: Mutex<Option<RefByIndex>> = Mutex::new(None);

/// Async build status: 0 = not started, 1 = building, 2 = done, -1 = error
static REFBY_BUILD_STATUS: AtomicI32 = AtomicI32::new(0);

struct RefByIndex {
    /// target_formid -> Vec<(plugin_idx, group_idx, record_idx)>
    index: HashMap<u32, Vec<(i32, i32, i32)>>,
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
        records[record_idx as usize].subrecords.len() as i32
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
        if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let sig = record.subrecords[sub_idx as usize].signature.as_str();
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
        if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        record.subrecords[sub_idx as usize].data_size() as i32
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
        if sub_idx < 0 || sub_idx as usize >= record.subrecords.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }
        let data = &record.subrecords[sub_idx as usize].raw_data;
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
// Cross-reference (Referenced By) index
// ============================================================================

/// Internal: build the cross-reference index from engine data.
/// Collects all record locations first, then scans subrecord data for FormID references.
/// Returns the built index.
fn build_refby_index_inner(
    plugins: &[xedit_dom::Plugin],
) -> RefByIndex {
    // Phase 1: Collect all known FormIDs and record locations.
    // Pre-collect record locations so we don't call collect_records_from_group twice.
    struct RecordLoc<'a> {
        pi: i32,
        gi: i32,
        ri: i32,
        record: &'a xedit_dom::Record,
    }

    let mut known_formids = HashSet::with_capacity(256_000);
    let mut all_records: Vec<RecordLoc> = Vec::with_capacity(256_000);

    for (pi, plugin) in plugins.iter().enumerate() {
        for (gi, group) in plugin.groups.iter().enumerate() {
            let records = collect_records_from_group(group);
            for (ri, record) in records.iter().enumerate() {
                let fid = record.form_id.0;
                if fid != 0 {
                    known_formids.insert(fid);
                }
                all_records.push(RecordLoc {
                    pi: pi as i32,
                    gi: gi as i32,
                    ri: ri as i32,
                    record,
                });
            }
        }
    }

    if known_formids.is_empty() {
        return RefByIndex {
            index: HashMap::new(),
        };
    }

    // Phase 2: Scan all subrecord data for FormID references.
    // Use a local set per record to deduplicate references within a record.
    let mut index: HashMap<u32, Vec<(i32, i32, i32)>> = HashMap::with_capacity(known_formids.len());

    for rloc in &all_records {
        let source_formid = rloc.record.form_id.0;
        let loc = (rloc.pi, rloc.gi, rloc.ri);
        let mut referenced_in_this_record = HashSet::new();

        for subrecord in &rloc.record.subrecords {
            let sig = subrecord.signature.as_str();
            // Skip text-like subrecords that won't contain FormIDs
            if sig == "EDID" || sig == "FULL" || sig == "DESC" || sig == "MODL"
                || sig == "ICON" || sig == "MICO" || sig == "TX00" || sig == "TX01"
            {
                continue;
            }

            let data = &subrecord.raw_data;
            if data.len() < 4 {
                continue;
            }

            // Scan every 4-byte aligned offset
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
                    && known_formids.contains(&candidate)
                    && referenced_in_this_record.insert(candidate)
                {
                    index.entry(candidate).or_default().push(loc);
                }

                offset += 4;
            }
        }
    }

    RefByIndex { index }
}

/// Build or rebuild the cross-reference index (synchronous).
///
/// Scans all subrecord data in all loaded plugins for 4-byte values that
/// match known FormIDs. Builds a reverse lookup: for each target FormID,
/// stores which records reference it.
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

        let refby = build_refby_index_inner(&engine.plugins);
        drop(lock); // Release engine lock before taking refby lock

        let mut refby_lock = REFBY_INDEX.lock().unwrap();
        *refby_lock = Some(refby);
        REFBY_BUILD_STATUS.store(2, Ordering::SeqCst);

        XEDIT_OK
    })
}

/// Start building the cross-reference index in a background thread.
///
/// Returns XEDIT_OK immediately. Use `xedit_refby_build_status` to poll
/// for completion: 0 = not started, 1 = building, 2 = done, -1 = error.
#[no_mangle]
pub extern "C" fn xedit_build_refby_index_async() -> i32 {
    catch_panic(|| {
        // Don't start if already building
        let current = REFBY_BUILD_STATUS.load(Ordering::SeqCst);
        if current == 1 {
            return XEDIT_OK; // Already in progress
        }

        REFBY_BUILD_STATUS.store(1, Ordering::SeqCst);

        // Clone the plugin data we need into the thread.
        // We need to hold the engine lock briefly to extract the data.
        let lock = ENGINE.lock().unwrap();
        let engine = match lock.as_ref() {
            Some(e) => e,
            None => {
                REFBY_BUILD_STATUS.store(-1, Ordering::SeqCst);
                return XEDIT_ERR_NULL_HANDLE;
            }
        };

        // Collect all records' info into owned data so we can release the lock.
        struct OwnedRecordData {
            pi: i32,
            gi: i32,
            ri: i32,
            form_id: u32,
            subrecords: Vec<(String, Vec<u8>)>, // (signature, raw_data)
        }

        let mut all_records: Vec<OwnedRecordData> = Vec::with_capacity(256_000);
        let mut known_formids = HashSet::with_capacity(256_000);

        for (pi, plugin) in engine.plugins.iter().enumerate() {
            for (gi, group) in plugin.groups.iter().enumerate() {
                let records = collect_records_from_group(group);
                for (ri, record) in records.iter().enumerate() {
                    let fid = record.form_id.0;
                    if fid != 0 {
                        known_formids.insert(fid);
                    }
                    all_records.push(OwnedRecordData {
                        pi: pi as i32,
                        gi: gi as i32,
                        ri: ri as i32,
                        form_id: fid,
                        subrecords: record
                            .subrecords
                            .iter()
                            .map(|s| (s.signature.to_string(), s.raw_data.clone()))
                            .collect(),
                    });
                }
            }
        }
        eprintln!("[RefBy] Collected {} records with {} unique FormIDs, starting background scan...",
            all_records.len(), known_formids.len());
        drop(lock); // Release engine lock immediately

        std::thread::spawn(move || {
            use rayon::prelude::*;

            let start = std::time::Instant::now();
            let total_records = all_records.len();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if known_formids.is_empty() {
                    let mut refby_lock = REFBY_INDEX.lock().unwrap();
                    *refby_lock = Some(RefByIndex {
                        index: HashMap::new(),
                    });
                    return;
                }

                // Convert HashSet to sorted Vec for cache-friendly binary search
                let mut sorted_formids: Vec<u32> = known_formids.into_iter().collect();
                sorted_formids.sort_unstable();

                eprintln!("[RefBy] Starting parallel scan of {} records across {} threads...",
                    total_records, rayon::current_num_threads());

                // Parallel scan: each thread builds its own local index
                let chunk_size = (all_records.len() / rayon::current_num_threads()).max(1000);
                let partial_indices: Vec<HashMap<u32, Vec<(i32, i32, i32)>>> = all_records
                    .par_chunks(chunk_size)
                    .map(|chunk| {
                        let mut local_index: HashMap<u32, Vec<(i32, i32, i32)>> = HashMap::new();
                        for rloc in chunk {
                            let loc = (rloc.pi, rloc.gi, rloc.ri);
                            let mut seen = HashSet::new();

                            for (sig, data) in &rloc.subrecords {
                                let s = sig.as_str();
                                if s == "EDID" || s == "FULL" || s == "DESC" || s == "MODL"
                                    || s == "ICON" || s == "MICO" || s == "TX00" || s == "TX01"
                                {
                                    continue;
                                }
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
                                        && candidate != rloc.form_id
                                        && sorted_formids.binary_search(&candidate).is_ok()
                                        && seen.insert(candidate)
                                    {
                                        local_index.entry(candidate).or_default().push(loc);
                                    }
                                    offset += 4;
                                }
                            }
                        }
                        local_index
                    })
                    .collect();

                eprintln!("[RefBy] Merging {} partial indices...", partial_indices.len());

                // Merge partial indices
                let mut index: HashMap<u32, Vec<(i32, i32, i32)>> =
                    HashMap::with_capacity(sorted_formids.len());
                for partial in partial_indices {
                    for (formid, refs) in partial {
                        index.entry(formid).or_default().extend(refs);
                    }
                }

                let mut refby_lock = REFBY_INDEX.lock().unwrap();
                *refby_lock = Some(RefByIndex { index });
            }));

            let elapsed = start.elapsed();
            match result {
                Ok(()) => {
                    eprintln!("[RefBy] Index built in {:.2}s", elapsed.as_secs_f64());
                    REFBY_BUILD_STATUS.store(2, Ordering::SeqCst);
                }
                Err(_) => {
                    eprintln!("[RefBy] Index build FAILED after {:.2}s", elapsed.as_secs_f64());
                    REFBY_BUILD_STATUS.store(-1, Ordering::SeqCst);
                }
            }
        });

        XEDIT_OK
    })
}

/// Check the status of the async refby index build.
/// Returns: 0 = not started, 1 = building, 2 = done, -1 = error.
#[no_mangle]
pub extern "C" fn xedit_refby_build_status() -> i32 {
    REFBY_BUILD_STATUS.load(Ordering::SeqCst)
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
        // Look up the record's FormID first.
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

        // Look up in the cross-reference index.
        let refby_lock = REFBY_INDEX.lock().unwrap();
        let refby = match refby_lock.as_ref() {
            Some(r) => r,
            None => return XEDIT_ERR_NULL_HANDLE, // index not built
        };

        match refby.index.get(&formid) {
            Some(entries) => entries.len() as i32,
            None => 0,
        }
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
/// The cross-reference index must have been built first via
/// `xedit_build_refby_index`.
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

        // Look up the record's FormID first.
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

        // Look up in the cross-reference index.
        let refby_lock = REFBY_INDEX.lock().unwrap();
        let refby = match refby_lock.as_ref() {
            Some(r) => r,
            None => return XEDIT_ERR_NULL_HANDLE, // index not built
        };

        let entries = match refby.index.get(&formid) {
            Some(e) => e,
            None => return XEDIT_ERR_NULL_HANDLE, // no references
        };

        if ref_index < 0 || ref_index as usize >= entries.len() {
            return XEDIT_ERR_NULL_HANDLE;
        }

        let (rp, rg, rr) = entries[ref_index as usize];
        unsafe {
            *out_plugin_idx = rp;
            *out_group_idx = rg;
            *out_record_idx = rr;
        }

        XEDIT_OK
    })
}
